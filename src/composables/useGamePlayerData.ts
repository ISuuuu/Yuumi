import { ref, computed, watch, onMounted, type Ref } from "vue";
import { useLcuStore, type ChampSelectPlayer } from "../store/lcuStore";
import {
  getGameflowPhase,
  getChampSelectSession,
  fetchMatchHistory,
  fetchCurrentSummoner,
  lcuRequest,
  fetchConfig,
  fetchPlayerFateInfo,
  type MatchDisplay,
  type AppConfig,
} from "../api/lcu";
import type {
  PlayerData,
  PremadePlayerLike,
} from "../types/gameInfo";
import type {
  GameflowParticipant,
  GameflowSession,
  RankedQueueEntry,
  RankedStats,
} from "../types/lcu";
import type { SummonerDisplay } from "../api/lcu";
import { computePremadeColors } from "./usePremadeGroup";
import { lazySetItem } from "../utils/lazyStorage";
import { runWithConcurrency } from "../utils/runWithConcurrency";

// ── 排位数据缓存（puuid → { data, timestamp }），带 LRU / 容量上限保护，避免内存泄露
const rankCache = new Map<string, { data: RankedStats; timestamp: number }>();
const RANK_CACHE_TTL = 5 * 60 * 1000; // 5 分钟
const RANK_CACHE_MAX_SIZE = 100; // 最多缓存 100 个玩家的排位信息

function getRankFromCache(puuid: string) {
  const cached = rankCache.get(puuid);
  if (!cached) return null;
  if (Date.now() - cached.timestamp >= RANK_CACHE_TTL) {
    rankCache.delete(puuid);
    return null;
  }
  return cached.data;
}

function setRankToCache(puuid: string, data: RankedStats) {
  if (rankCache.size >= RANK_CACHE_MAX_SIZE) {
    const firstKey = rankCache.keys().next().value;
    if (firstKey) rankCache.delete(firstKey);
  }
  rankCache.set(puuid, { data, timestamp: Date.now() });
}

// ── 保留对局数据 localStorage：持续保留上一局数据，直到新对局开始（ChampSelect 时清理）
const RESERVE_TEAM_KEYS = [
  "yuumi_last_gameflow_my_team",
  "yuumi_last_gameflow_their_team",
  "yuumi_last_game_player_data",
  "yuumi_last_game_loaded_count",
  "yuumi_last_premade_colors_my",
  "yuumi_last_premade_colors_their",
];

function clearReserveDataFromStorage() {
  try {
    for (const k of RESERVE_TEAM_KEYS) {
      localStorage.removeItem(k);
    }
  } catch {
    /* ignore */
  }
}

// ── gameflow session 短期缓存，避免同一流程中多次请求同一端点
let cachedSession: { data: GameflowSession; timestamp: number } | null = null;
const SESSION_CACHE_TTL = 30 * 1000; // 30 秒
let currentGameflowSessionRequestId = 0; // 用于防并发竞态的请求标识计数器

async function fetchSessionCached(): Promise<GameflowSession | null> {
  const now = Date.now();
  if (cachedSession && now - cachedSession.timestamp < SESSION_CACHE_TTL) {
    return cachedSession.data;
  }
  try {
    const resp = await lcuRequest<GameflowSession>("GET", "/lol-gameflow/v1/session");
    if (resp.success && resp.data) {
      cachedSession = { data: resp.data, timestamp: now };
      return resp.data;
    }
  } catch {
    /* ignore */
  }
  return null;
}

const MATCHES_CACHE_KEY = (puuid: string) => `yuumi_gf_matches_cache_${puuid}`;
function mergeMatchesWithCache(
  puuid: string,
  fresh: MatchDisplay[],
): MatchDisplay[] {
  let cached: MatchDisplay[] = [];
  try {
    const raw = localStorage.getItem(MATCHES_CACHE_KEY(puuid));
    if (raw) cached = JSON.parse(raw);
  } catch {
    /* ignore */
  }

  const merged = [...fresh, ...cached]
    .filter(
      (m, idx, arr) => arr.findIndex((x) => x.gameId === m.gameId) === idx,
    )
    .sort((a, b) => b.timeStamp - a.timeStamp);

  lazySetItem(MATCHES_CACHE_KEY(puuid), merged);

  return merged;
}

export function useGamePlayerData(
  appConfig: Ref<AppConfig | null>,
  premadeColorsMy: Ref<Record<number, number>>,
  premadeColorsTheir: Ref<Record<number, number>>,
  activeTab: Ref<"my" | "their">,
) {
  const store = useLcuStore();

  const loading = ref(false);
  const error = ref("");
  const currentSummonerId = ref<number>(0);
  const currentSummonerPuuid = ref<string>("");
  const playerData = ref<Record<number, PlayerData>>({});

  const sessionAllyTeam = ref<PremadePlayerLike[]>([]);
  const sessionEnemyTeam = ref<PremadePlayerLike[]>([]);

  const gameflowMyTeam = ref<PremadePlayerLike[]>([]);
  const gameflowTheirTeam = ref<PremadePlayerLike[]>([]);

  const currentQueueId = ref<number | null>(null);
  const isTftMode = ref(false);

  // ── 保留对局数据写入：只要有队伍数据且包含玩家数据就落盘保存对局快照
  function writeReserveData() {
    const loadedCount = Object.values(playerData.value).filter(
      (d) => d.info !== null,
    ).length;
    if (gameflowMyTeam.value.length === 0 || loadedCount === 0) {
      return;
    }
    // 防止选人阶段部分加载的数据覆盖上一局更完整的快照：
    // 真实对局阶段始终写入保证时效性；其余阶段仅在新快照数据规模不缩水时覆盖
    let savedLoadedCount = 0;
    try {
      savedLoadedCount =
        Number(localStorage.getItem("yuumi_last_game_loaded_count")) || 0;
    } catch {
      /* ignore */
    }
    const inRealGame =
      store.gamePhase === "GameStart" || store.gamePhase === "InProgress";
    if (!inRealGame && loadedCount < savedLoadedCount) {
      return;
    }
    lazySetItem("yuumi_last_game_player_data", playerData.value);
    lazySetItem("yuumi_last_gameflow_my_team", gameflowMyTeam.value);
    lazySetItem("yuumi_last_gameflow_their_team", gameflowTheirTeam.value);
    lazySetItem("yuumi_last_premade_colors_my", premadeColorsMy.value);
    lazySetItem("yuumi_last_premade_colors_their", premadeColorsTheir.value);
    lazySetItem("yuumi_last_game_loaded_count", loadedCount);
  }

  // ── 从 localStorage 恢复保留数据（有数据即恢复，直到新对局开始）
  function restoreReserveDataFromLocalStorage(): boolean {
    try {
      const savedMyTeam = localStorage.getItem("yuumi_last_gameflow_my_team");
      const savedTheirTeam = localStorage.getItem(
        "yuumi_last_gameflow_their_team",
      );
      const savedPlayerData = localStorage.getItem("yuumi_last_game_player_data");
      const savedPremadeMy = localStorage.getItem("yuumi_last_premade_colors_my");
      const savedPremadeTheir = localStorage.getItem("yuumi_last_premade_colors_their");
      let hasRestored = false;
      if (savedMyTeam) {
        const parsed = JSON.parse(savedMyTeam);
        if (Array.isArray(parsed) && parsed.length > 0) {
          gameflowMyTeam.value = parsed;
          hasRestored = true;
        }
      }
      if (savedTheirTeam) {
        const parsed = JSON.parse(savedTheirTeam);
        if (Array.isArray(parsed) && parsed.length > 0) {
          gameflowTheirTeam.value = parsed;
          hasRestored = true;
        }
      }
      if (savedPlayerData) {
        const parsed = JSON.parse(savedPlayerData);
        if (parsed && Object.keys(parsed).length > 0) {
          playerData.value = parsed;
          hasRestored = true;
        }
      }
      if (savedPremadeMy) {
        try {
          premadeColorsMy.value = JSON.parse(savedPremadeMy);
        } catch {
          if (gameflowMyTeam.value.length > 0)
            premadeColorsMy.value = computePremadeColors(gameflowMyTeam.value);
        }
      } else if (gameflowMyTeam.value.length > 0) {
        premadeColorsMy.value = computePremadeColors(gameflowMyTeam.value);
      }
      if (savedPremadeTheir) {
        try {
          premadeColorsTheir.value = JSON.parse(savedPremadeTheir);
        } catch {
          if (gameflowTheirTeam.value.length > 0)
            premadeColorsTheir.value = computePremadeColors(gameflowTheirTeam.value);
        }
      } else if (gameflowTheirTeam.value.length > 0) {
        premadeColorsTheir.value = computePremadeColors(gameflowTheirTeam.value);
      }
      return hasRestored;
    } catch {
      return false;
    }
  }

  // ── localStorage 写入防抖
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  function debouncedSavePlayerData() {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      writeReserveData();
    }, 500);
  }

  const myTeam = computed(() => {
    if (isGameActive.value) {
      if (gameflowMyTeam.value.length > 0) return gameflowMyTeam.value;
      return store.champSelectSession?.myTeam ?? [];
    }
    return gameflowMyTeam.value;
  });

  const theirTeam = computed(() => {
    if (isGameActive.value) {
      if (gameflowTheirTeam.value.length > 0) return gameflowTheirTeam.value;
      return store.champSelectSession?.theirTeam ?? [];
    }
    return gameflowTheirTeam.value;
  });

  const currentTeam = computed(() =>
    activeTab.value === "my" ? myTeam.value : theirTeam.value,
  );

  const isGameActive = computed(
    () =>
      store.gamePhase === "ChampSelect" ||
      store.gamePhase === "GameStart" ||
      store.gamePhase === "InProgress",
  );

  const shouldShowContent = computed(() => {
    if (isTftMode.value) return false;
    if (isGameActive.value) return true;
    if (appConfig.value?.Functions?.EnableReserveGameinfo) {
      return Object.keys(playerData.value).length > 0;
    }
    return false;
  });

  async function updateCurrentQueueId() {
    try {
      const data = await fetchSessionCached();
      if (data?.gameData?.queue?.id !== undefined) {
        currentQueueId.value = data.gameData.queue.id;
        const qId = currentQueueId.value;
        const gameMode = data.gameData.queue.gameMode;
        if (gameMode === "TFT" || (qId !== null && qId >= 1090 && qId <= 1200)) {
          isTftMode.value = true;
        } else {
          isTftMode.value = false;
        }
      } else {
        currentQueueId.value = null;
        isTftMode.value = false;
      }
    } catch {
      currentQueueId.value = null;
      isTftMode.value = false;
    }
  }

  async function fetchPremadeColors() {
    try {
      if (!currentSummonerId.value && !currentSummonerPuuid.value) {
        const s = await fetchCurrentSummoner();
        if (s?.summonerId) currentSummonerId.value = s.summonerId;
        if (s?.puuid) currentSummonerPuuid.value = s.puuid;
      }
      const data = await fetchSessionCached();
      if (data?.gameData) {
        const { teamOne, teamTwo } = data.gameData;
        if (teamOne && teamTwo && teamOne.length > 0 && teamTwo.length > 0) {
          const isTeamOne = teamOne.some(
            (p) =>
              (currentSummonerId.value && p.summonerId === currentSummonerId.value) ||
              (currentSummonerPuuid.value && p.puuid === currentSummonerPuuid.value),
          );
          const ally = isTeamOne ? teamOne : teamTwo;
          const enemy = isTeamOne ? teamTwo : teamOne;
          sessionAllyTeam.value = ally;
          sessionEnemyTeam.value = enemy;
          premadeColorsMy.value = computePremadeColors(ally);
          premadeColorsTheir.value = computePremadeColors(enemy);
          return;
        }
      }
      // 备选降级：若 gameflow 暂无组队数据，但选人 session 已有 teamParticipantId
      if (
        Object.keys(premadeColorsMy.value).length === 0 &&
        store.champSelectSession?.myTeam?.some(
          (p) => p.teamParticipantId !== undefined || p.partyId !== undefined,
        )
      ) {
        premadeColorsMy.value = computePremadeColors(
          store.champSelectSession.myTeam,
        );
      }
      if (
        Object.keys(premadeColorsTheir.value).length === 0 &&
        store.champSelectSession?.theirTeam?.some(
          (p) => p.teamParticipantId !== undefined || p.partyId !== undefined,
        )
      ) {
        premadeColorsTheir.value = computePremadeColors(
          store.champSelectSession.theirTeam,
        );
      }
    } catch {
      /* ignore */
    }
  }

  async function refreshState() {
    loading.value = true;
    try {
      const phaseResp = await getGameflowPhase();
      if (phaseResp.success && phaseResp.data) store.setGamePhase(phaseResp.data);
    } catch {
      /* ignore */
    }
    try {
      const sessionResp = await getChampSelectSession();
      if (sessionResp.success && sessionResp.data)
        store.setChampSelectSession(sessionResp.data);
    } catch {
      /* ignore */
    }
    loading.value = false;
  }

  async function loadPlayerData(
    cellId: number,
    summonerId: number,
    playerPuuid?: string,
  ) {
    if (!summonerId && !playerPuuid) return;

    const existing = playerData.value[cellId];
    if (existing?.info) return;

    playerData.value[cellId] = {
      info: null,
      matches: [],
      ranked: { solo: null, flex: null },
      loading: true,
    };

    try {
      let info: SummonerDisplay | null = null;
      if (summonerId) {
        const resp = await lcuRequest<SummonerDisplay>(
          "GET",
          `/lol-summoner/v1/summoners/${summonerId}`,
        );
        if (resp.success && resp.data) {
          info = resp.data;
        }
      }
      if (!info && playerPuuid) {
        const resp = await lcuRequest<SummonerDisplay>(
          "GET",
          `/lol-summoner/v2/summoners/puuid/${playerPuuid}`,
        );
        if (resp.success && resp.data) {
          info = resp.data;
        }
      }
      if (!info) {
        playerData.value[cellId] = {
          info: null,
          matches: [],
          ranked: { solo: null, flex: null },
          loading: false,
        };
        return;
      }

      const filterEnabled = appConfig.value?.Functions?.GameInfoFilter ?? false;
      const maxMatches = filterEnabled ? 50 : 10;

      const [rawMatches, rankedResp] = await Promise.all([
        info.puuid
          ? fetchMatchHistory(info.puuid, 0, maxMatches)
          : Promise.resolve([]),
        info.puuid
          ? (() => {
              const cached = getRankFromCache(info.puuid!);
              if (cached) {
                return Promise.resolve({ success: true, data: cached });
              }
              return lcuRequest<RankedStats>(
                "GET",
                `/lol-ranked/v1/ranked-stats/${info.puuid}`,
              ).then((rResp) => {
                if (rResp.success && rResp.data) {
                  setRankToCache(info.puuid!, rResp.data);
                }
                return rResp;
              });
            })()
          : Promise.resolve({ success: false as const }),
      ]);

      const isCurrentPlayer =
        summonerId === currentSummonerId.value ||
        (!!info.puuid && info.puuid === currentSummonerPuuid.value);

      let matches: MatchDisplay[] = rawMatches;
      if (info.puuid && isCurrentPlayer) {
        matches = mergeMatchesWithCache(info.puuid, rawMatches);
      }

      if (filterEnabled && currentQueueId.value !== null) {
        matches = matches.filter(
          (m: MatchDisplay) => m.queueId === currentQueueId.value,
        );
      }
      matches = matches.slice(0, 10);

      let solo: RankedQueueEntry | null = null,
        flex: RankedQueueEntry | null = null;
      if (rankedResp.success && rankedResp.data?.queues) {
        solo =
          rankedResp.data.queues.find(
            (q) => q.queueType === "RANKED_SOLO_5x5",
          ) || null;
        flex =
          rankedResp.data.queues.find(
            (q) => q.queueType === "RANKED_FLEX_SR",
          ) || null;
      }

      let avgKda = 0;
      let winRate = 0;
      let winCount = 0;
      let lossesCount = 0;

      if (matches && matches.length > 0) {
        let totalKills = 0;
        let totalDeaths = 0;
        let totalAssists = 0;
        let remakeCount = 0;

        matches.forEach((m: MatchDisplay) => {
          if (m.remake) {
            remakeCount++;
          } else {
            totalKills += m.kills ?? 0;
            totalDeaths += m.deaths ?? 0;
            totalAssists += m.assists ?? 0;
            if (m.win) {
              winCount++;
            } else {
              lossesCount++;
            }
          }
        });

        const validMatches = matches.length - remakeCount;
        winRate =
          validMatches > 0 ? Math.round((winCount / validMatches) * 100) : 0;
        const deathsForCalc = totalDeaths === 0 ? 1 : totalDeaths;
        avgKda = (totalKills + totalAssists) / deathsForCalc;
      }

      let fateFlag: "ally" | "enemy" | null = null;
      let recentlyChampionName = "";
      if (currentSummonerId.value && matches.length > 0 && !isCurrentPlayer) {
        try {
          const lastGameId = matches[0].gameId;
          const fateInfo = await fetchPlayerFateInfo(
            lastGameId,
            info.puuid,
            currentSummonerId.value,
          );
          if (fateInfo) {
            fateFlag = fateInfo.fateFlag;
            recentlyChampionName = fateInfo.recentlyChampionName || "";
          }
        } catch (e) {
          console.error("宿命检测失败:", e);
        }
      }

      playerData.value[cellId] = {
        info,
        matches,
        ranked: { solo, flex },
        loading: false,
        avgKda,
        winRate,
        winCount,
        lossesCount,
        fateFlag,
        recentlyChampionName,
      };
      debouncedSavePlayerData();
    } catch {
      playerData.value[cellId] = {
        info: null,
        matches: [],
        ranked: { solo: null, flex: null },
        loading: false,
      };
    }
  }

  async function loadAllPlayers() {
    const my = myTeam.value;
    const their = theirTeam.value;
    if (my.length === 0 && their.length === 0) return;
    await updateCurrentQueueId();

    // 先加载当前可见队伍，再后台加载另一队，避免请求风暴
    const visible = activeTab.value === "my" ? my : their;
    const background = activeTab.value === "my" ? their : my;

    await runWithConcurrency(visible, 3, (p) => {
      const cid = p.cellId ?? p.summonerId;
      const sid = p.summonerId ?? p.cellId;
      if (cid !== undefined && (sid !== undefined || p.puuid)) {
        return loadPlayerData(cid, sid ?? 0, p.puuid);
      }
      return Promise.resolve();
    });
    void runWithConcurrency(background, 3, (p) => {
      const cid = p.cellId ?? p.summonerId;
      const sid = p.summonerId ?? p.cellId;
      if (cid !== undefined && (sid !== undefined || p.puuid)) {
        return loadPlayerData(cid, sid ?? 0, p.puuid);
      }
      return Promise.resolve();
    })
      .then(() => {
        writeReserveData();
      })
      .catch((err) => {
        console.debug("[GameInfo] 后台队伍数据预加载失败:", err);
        writeReserveData();
      });
  }

  async function processTeamData(
    teamOne: GameflowParticipant[],
    teamTwo: GameflowParticipant[],
  ) {
    if (!currentSummonerId.value && !currentSummonerPuuid.value) {
      try {
        const s = await fetchCurrentSummoner();
        if (s?.summonerId) currentSummonerId.value = s.summonerId;
        if (s?.puuid) currentSummonerPuuid.value = s.puuid;
      } catch {
        /* ignore */
      }
    }

    const isTeamOne = teamOne.some(
      (p) =>
        (currentSummonerId.value && p.summonerId === currentSummonerId.value) ||
        (currentSummonerPuuid.value && p.puuid === currentSummonerPuuid.value),
    );
    const allyTeam = isTeamOne ? teamOne : teamTwo;
    const enemyTeam = isTeamOne ? teamTwo : teamOne;

    if (gameflowMyTeam.value && gameflowMyTeam.value.length > 0) {
      for (const p of gameflowMyTeam.value) {
        if (
          p.summonerId &&
          p.cellId !== undefined &&
          p.cellId !== p.summonerId &&
          playerData.value[p.cellId]
        ) {
          playerData.value[p.summonerId] = playerData.value[p.cellId];
          delete playerData.value[p.cellId];
        }
      }
    }

    gameflowMyTeam.value = allyTeam.map(
      (p): PremadePlayerLike => ({
        ...p,
        cellId: p.summonerId,
        displayName: p.summonerName || p.displayName,
      }),
    );
    gameflowTheirTeam.value = enemyTeam.map(
      (p): PremadePlayerLike => ({
        ...p,
        cellId: p.summonerId,
        displayName: p.summonerName || p.displayName,
      }),
    );

    premadeColorsMy.value = computePremadeColors(allyTeam);
    premadeColorsTheir.value = computePremadeColors(enemyTeam);

    // 先加载当前可见队伍，再后台加载另一队，避免一次性并发请求过多
    const visible = activeTab.value === "my" ? allyTeam : enemyTeam;
    const background = activeTab.value === "my" ? enemyTeam : allyTeam;

    await runWithConcurrency(visible, 3, (p) =>
      p.summonerId ? loadPlayerData(p.summonerId, p.summonerId) : Promise.resolve(),
    );
    void runWithConcurrency(background, 3, (p) =>
      p.summonerId ? loadPlayerData(p.summonerId, p.summonerId) : Promise.resolve(),
    )
      .then(() => {
        // 双方 10 人信息加载完毕，立即保存完整对局
        writeReserveData();
      })
      .catch((err) => {
        console.debug("[GameInfo] 后台队伍数据预加载失败:", err);
        // 异常兜底，只要队伍齐备也保存
        writeReserveData();
      });
  }

  async function loadFromGameflowSession() {
    loading.value = true;
    error.value = "";

    const reqId = ++currentGameflowSessionRequestId;

    cachedSession = null;
    await updateCurrentQueueId();
    if (isTftMode.value) {
      gameflowMyTeam.value = [];
      gameflowTheirTeam.value = [];
      playerData.value = {};
      premadeColorsMy.value = {};
      premadeColorsTheir.value = {};
      clearReserveDataFromStorage();
      loading.value = false;
      return;
    }

    if (!currentSummonerId.value) {
      try {
        const s = await fetchCurrentSummoner();
        if (s?.summonerId) currentSummonerId.value = s.summonerId;
        if (s?.puuid) currentSummonerPuuid.value = s.puuid;
      } catch {
        /* ignore */
      }
    }

    try {
      const data = await fetchSessionCached();
      if (reqId !== currentGameflowSessionRequestId) return;

      if (!data?.gameData) {
        error.value = "无法获取对局 Session";
        loading.value = false;
        return;
      }

      const { teamOne, teamTwo } = data.gameData;
      if (!teamOne || !teamTwo || teamOne.length === 0 || teamTwo.length === 0) {
        let retried = 0;
        const maxRetries = 30; // 30 次重试，每次间隔 2 秒（覆盖进游戏后 60 秒），确保对局中随时能补齐 10 人
        while (retried < maxRetries) {
          await new Promise((r) => setTimeout(r, 2000));

          if (reqId !== currentGameflowSessionRequestId) return;
          if (
            store.gamePhase !== "InProgress" &&
            store.gamePhase !== "GameStart"
          ) {
            loading.value = false;
            return;
          }

          cachedSession = null;
          const retryData = await fetchSessionCached();
          if (reqId !== currentGameflowSessionRequestId) return;

          const rt = retryData?.gameData;
          if (rt?.teamOne?.length && rt.teamTwo?.length) {
            return processTeamData(rt.teamOne, rt.teamTwo);
          }
          retried++;
        }
        loading.value = false;
        return;
      }

      if (reqId !== currentGameflowSessionRequestId) return;
      await processTeamData(teamOne, teamTwo);
    } catch (e) {
      if (reqId !== currentGameflowSessionRequestId) return;
      console.error("加载 gameflow session 失败:", e);
      error.value = "加载对局数据失败";
    }
    loading.value = false;
  }

  // 监听 Watchers
  watch(isGameActive, (active) => {
    if (!active) {
      // 离开游戏活跃状态（回到 Lobby / EndOfGame 等）：
      const hasPlayerData =
        gameflowMyTeam.value.length > 0 &&
        Object.keys(playerData.value).length > 0;
      if (hasPlayerData) {
        // 内存中已有刚打完的对局，确保落盘
        writeReserveData();
      } else {
        // 否则（如中途启动或选人秒退离开），尝试从 localStorage 恢复上一次对局快照
        restoreReserveDataFromLocalStorage();
      }
    } else {
      // 刚进入选人阶段时，清空当前内存视图以展示当前选人
      if (store.gamePhase === "ChampSelect") {
        gameflowMyTeam.value = [];
        gameflowTheirTeam.value = [];
        playerData.value = {};
      }
    }
  });

  watch(
    () => store.gamePhase,
    (phase: string) => {
      if (phase !== "InProgress" && phase !== "GameStart") {
        isTftMode.value = false;
      }
      if (phase === "ChampSelect") {
        gameflowMyTeam.value = [];
        gameflowTheirTeam.value = [];
        playerData.value = {};
        premadeColorsMy.value = {};
        premadeColorsTheir.value = {};
        // 选人阶段不立即清空 localStorage，避免秒退导致已有完整对局丢失
        refreshState();
      }
      if (phase === "InProgress" || phase === "GameStart")
        loadFromGameflowSession();
    },
  );

  // 团队内容签名：成员 cellId + 英雄 ID。session 高频事件中仅倒计时变化时签名不变，跳过无效重载
  let lastSessionTeamSig = "";
  const teamSig = (team: ChampSelectPlayer[]) =>
    (team || [])
      .map((p) => `${p.cellId}:${p.championId || p.championPickIntent || 0}`)
      .join(",");

  watch(
    () => store.champSelectSession,
    (session) => {
      if (session && store.gamePhase === "ChampSelect") {
        const myTeam = session.myTeam || [];
        const theirTeam = session.theirTeam || [];
        const sig = teamSig(myTeam) + "|" + teamSig(theirTeam);
        if (sig === lastSessionTeamSig) return;
        lastSessionTeamSig = sig;

        loading.value = false;
        error.value = "";
        gameflowMyTeam.value = myTeam;
        loadAllPlayers();
        fetchPremadeColors();
      }
    },
  );

  watch(activeTab, () => loadAllPlayers());

  watch(
    () => store.currentPage,
    (newPage) => {
      if (newPage === "gameinfo") {
        if (store.gamePhase === "InProgress" || store.gamePhase === "GameStart") {
          loadFromGameflowSession();
        } else if (store.gamePhase === "ChampSelect") {
          loadAllPlayers();
        } else {
          refreshState();
        }
      }
    },
  );

  onMounted(async () => {
    if (!appConfig.value) {
      try {
        appConfig.value = await fetchConfig();
      } catch {
        /* ignore */
      }
    }

    if (!isGameActive.value && appConfig.value?.Functions?.EnableReserveGameinfo) {
      if (restoreReserveDataFromLocalStorage()) {
        if (Object.keys(premadeColorsMy.value).length === 0 && gameflowMyTeam.value.length > 0)
          premadeColorsMy.value = computePremadeColors(gameflowMyTeam.value);
        if (Object.keys(premadeColorsTheir.value).length === 0 && gameflowTheirTeam.value.length > 0)
          premadeColorsTheir.value = computePremadeColors(gameflowTheirTeam.value);
      }
    }

    try {
      const s = await fetchCurrentSummoner();
      if (s?.summonerId) currentSummonerId.value = s.summonerId;
      if (s?.puuid) currentSummonerPuuid.value = s.puuid;
    } catch {
      /* ignore */
    }

    refreshState();
  });

  return {
    loading,
    error,
    currentSummonerId,
    currentSummonerPuuid,
    playerData,
    sessionAllyTeam,
    sessionEnemyTeam,
    gameflowMyTeam,
    gameflowTheirTeam,
    currentQueueId,
    isTftMode,
    myTeam,
    theirTeam,
    currentTeam,
    isGameActive,
    shouldShowContent,
    refreshState,
    loadAllPlayers,
    loadFromGameflowSession,
  };
}
