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
import { resolvePlayerChampionId } from "../types/gameInfo";
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
  const playerData = ref<Record<string | number, PlayerData>>({});

  // 选人阶段我方与敌方队员数据与英雄快照（用于 Loading 阶段继承）
  const champSelectTeamSnapshot = ref<PremadePlayerLike[]>([]);
  const champSelectTheirTeamSnapshot = ref<PremadePlayerLike[]>([]);

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
      if (store.champSelectSession?.myTeam && store.champSelectSession.myTeam.length > 0) {
        return store.champSelectSession.myTeam;
      }
      if (champSelectTeamSnapshot.value.length > 0) {
        return champSelectTeamSnapshot.value;
      }
    }
    return gameflowMyTeam.value;
  });

  const theirTeam = computed(() => {
    if (isGameActive.value) {
      if (gameflowTheirTeam.value.length > 0) return gameflowTheirTeam.value;
      if (store.champSelectSession?.theirTeam && store.champSelectSession.theirTeam.length > 0) {
        return store.champSelectSession.theirTeam;
      }
      if (champSelectTheirTeamSnapshot.value.length > 0) {
        return champSelectTheirTeamSnapshot.value;
      }
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
        const t1 = teamOne || [];
        const t2 = teamTwo || [];
        if (t1.length > 0 || t2.length > 0) {
          const isTeamOne = t1.some(
            (p) =>
              (currentSummonerId.value && p.summonerId === currentSummonerId.value) ||
              (currentSummonerPuuid.value && p.puuid === currentSummonerPuuid.value),
          );
          const ally = isTeamOne || t2.length === 0 ? t1 : t2;
          const enemy = isTeamOne || t2.length === 0 ? t2 : t1;
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
          (p: ChampSelectPlayer) =>
            p.teamParticipantId !== undefined || p.partyId !== undefined,
        )
      ) {
        premadeColorsMy.value = computePremadeColors(
          store.champSelectSession.myTeam,
        );
      }
      if (
        Object.keys(premadeColorsTheir.value).length === 0 &&
        store.champSelectSession?.theirTeam?.some(
          (p: ChampSelectPlayer) =>
            p.teamParticipantId !== undefined || p.partyId !== undefined,
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
      if (phaseResp.success && phaseResp.data) {
        store.setGamePhase(phaseResp.data);
        if (phaseResp.data === "InProgress" || phaseResp.data === "GameStart") {
          loadFromGameflowSession();
        }
      }
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
    fallbackPlayer?: PremadePlayerLike,
  ) {
    if (!summonerId && !playerPuuid && !fallbackPlayer) return;

    const existing =
      playerData.value[cellId] ||
      (summonerId ? playerData.value[summonerId] : undefined) ||
      (playerPuuid ? playerData.value[playerPuuid] : undefined);
    if (existing?.info && !existing.loading) {
      // 防止选人阶段敌方占位数据（空 puuid / cellId 兜底 summonerId）在进入对局后
      // 污染真实玩家信息：若当前持有真实 puuid 且与已有数据的 puuid 不一致，强制重新加载
      const puuidMismatch =
        Boolean(playerPuuid && playerPuuid.trim() !== "") &&
        Boolean(existing.info.puuid && existing.info.puuid.trim() !== "") &&
        existing.info.puuid !== playerPuuid;
      if (!puuidMismatch) {
        playerData.value[cellId] = existing;
        if (summonerId) playerData.value[summonerId] = existing;
        if (playerPuuid) playerData.value[playerPuuid] = existing;
        return;
      }
      // puuid 不匹配，继续往下加载真实数据
    }

    // 机器人/电脑玩家本地极速识别，无需请求 LCU API，避免 404 和延迟
    const isBotPlayer =
      Boolean(fallbackPlayer?.bot) ||
      Boolean(fallbackPlayer?.isBot) ||
      Boolean(fallbackPlayer?.botChampionId) ||
      Boolean(fallbackPlayer?.displayName?.includes("电脑")) ||
      Boolean(fallbackPlayer?.summonerName?.includes("电脑")) ||
      (!summonerId && !playerPuuid && Boolean(fallbackPlayer?.displayName || fallbackPlayer?.summonerName));

    if (isBotPlayer && fallbackPlayer) {
      const botName =
        fallbackPlayer.displayName ||
        fallbackPlayer.summonerName ||
        fallbackPlayer.botName ||
        fallbackPlayer.gameName ||
        `电脑${cellId + 1}`;
      const iconId = fallbackPlayer.profileIconId ?? 29;
      const botInfo: SummonerDisplay = {
        accountId: 0,
        summonerId: summonerId || 0,
        puuid: playerPuuid || "",
        displayName: botName,
        gameName: botName,
        tagLine: "",
        profileIconId: iconId,
        profileIconUrl: `/lol-game-data/assets/v1/profile-icons/${iconId}.jpg`,
        summonerLevel: 0,
        percentCompleteForNextLevel: 0,
        xpSinceLastLevel: 0,
        xpUntilNextLevel: 0,
      };
      const botDataObj: PlayerData = {
        info: botInfo,
        matches: [],
        ranked: { solo: null, flex: null },
        loading: false,
        matchHistoryHidden: true,
        championId: fallbackPlayer.championId || fallbackPlayer.botChampionId || 0,
      };
      playerData.value[cellId] = botDataObj;
      if (summonerId && summonerId !== cellId) playerData.value[summonerId] = botDataObj;
      if (playerPuuid) playerData.value[playerPuuid] = botDataObj;
      debouncedSavePlayerData();
      return;
    }

    playerData.value[cellId] = {
      info: null,
      matches: [],
      ranked: { solo: null, flex: null },
      loading: true,
    };

    try {
      let info: SummonerDisplay | null = null;
      if (playerPuuid) {
        const resp = await lcuRequest<SummonerDisplay>(
          "GET",
          `/lol-summoner/v2/summoners/puuid/${playerPuuid}`,
        );
        if (resp.success && resp.data) {
          info = resp.data;
        }
      }
      if (!info && summonerId) {
        const resp = await lcuRequest<SummonerDisplay>(
          "GET",
          `/lol-summoner/v1/summoners/${summonerId}`,
        );
        if (resp.success && resp.data) {
          info = resp.data;
        }
      }
      if (!info && (fallbackPlayer?.displayName || fallbackPlayer?.gameName)) {
        const queryName = fallbackPlayer.gameName || fallbackPlayer.displayName;
        if (queryName) {
          try {
            const resp = await lcuRequest<SummonerDisplay>(
              "GET",
              `/lol-summoner/v1/summoners?name=${encodeURIComponent(queryName)}`,
            );
            if (resp.success && resp.data) {
              info = resp.data;
            }
          } catch {
            /* ignore */
          }
        }
      }

      let matchHistoryHidden = false;

      if (!info) {
        // 在选人阶段，只有当玩家明确为敌方队伍且尚未公开身份时，才属于敌方等待
        const isEnemy = store.champSelectSession?.theirTeam?.some(
          (t) => (t.cellId !== undefined && t.cellId === cellId) || (t.summonerId && t.summonerId === summonerId),
        );
        const isChampSelectEnemyWaiting =
          store.gamePhase === "ChampSelect" &&
          isEnemy &&
          !playerPuuid &&
          !summonerId &&
          !fallbackPlayer?.puuid &&
          !fallbackPlayer?.summonerId;

        if (fallbackPlayer && !isChampSelectEnemyWaiting) {
          const fallbackDisplayName =
            fallbackPlayer.displayName ||
            (fallbackPlayer.gameName
              ? fallbackPlayer.tagLine
                ? `${fallbackPlayer.gameName}#${fallbackPlayer.tagLine}`
                : fallbackPlayer.gameName
              : fallbackPlayer.summonerName) ||
            `玩家${cellId + 1}`;
          const iconId = fallbackPlayer.profileIconId ?? 29;
          info = {
            accountId: 0,
            summonerId: summonerId || 0,
            puuid: playerPuuid || fallbackPlayer.puuid || "",
            displayName: fallbackDisplayName,
            gameName: fallbackPlayer.gameName || fallbackDisplayName,
            tagLine: fallbackPlayer.tagLine || "",
            profileIconId: iconId,
            profileIconUrl: `/lol-game-data/assets/v1/profile-icons/${iconId}.jpg`,
            summonerLevel: 0,
            percentCompleteForNextLevel: 0,
            xpSinceLastLevel: 0,
            xpUntilNextLevel: 0,
          };
          matchHistoryHidden = true;
        } else {
          playerData.value[cellId] = {
            info: null,
            matches: [],
            ranked: { solo: null, flex: null },
            loading: false,
          };
          return;
        }
      }

      const safeInfo = info;
      if (!safeInfo.profileIconUrl && (safeInfo.profileIconId !== undefined || safeInfo.profileIconId !== null)) {
        safeInfo.profileIconUrl = `/lol-game-data/assets/v1/profile-icons/${safeInfo.profileIconId ?? 29}.jpg`;
      }

      const filterEnabled = appConfig.value?.Functions?.GameInfoFilter ?? false;
      const maxMatches = filterEnabled ? 50 : 10;

      const [rawMatches, rankedResp] = await Promise.all([
        safeInfo.puuid
          ? fetchMatchHistory(safeInfo.puuid, 0, maxMatches)
              .then((res) => {
                if (!res || res.length === 0) {
                  // 如果接口返回空列表（未抛错），对局中隐藏战绩常返回空
                  matchHistoryHidden = true;
                }
                return res || [];
              })
              .catch((e) => {
                matchHistoryHidden = true;
                console.debug(`[GameInfo] 战绩拉取失败/已隐藏 (puuid: ${safeInfo.puuid}):`, e);
                return [] as MatchDisplay[];
              })
          : Promise.resolve([] as MatchDisplay[]),
        safeInfo.puuid
          ? (() => {
              const cached = getRankFromCache(safeInfo.puuid);
              if (cached) {
                return Promise.resolve({ success: true, data: cached });
              }
              return lcuRequest<RankedStats>(
                "GET",
                `/lol-ranked/v1/ranked-stats/${safeInfo.puuid}`,
              )
                .then((rResp) => {
                  if (rResp.success && rResp.data) {
                    setRankToCache(safeInfo.puuid, rResp.data);
                  }
                  return rResp;
                })
                .catch(() => ({ success: false as const }));
            })()
          : Promise.resolve({ success: false as const }),
      ]);

      const isCurrentPlayer =
        summonerId === currentSummonerId.value ||
        (!!safeInfo.puuid && safeInfo.puuid === currentSummonerPuuid.value);

      let matches: MatchDisplay[] = rawMatches;
      if (safeInfo.puuid && isCurrentPlayer) {
        matches = mergeMatchesWithCache(safeInfo.puuid, rawMatches);
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

      let avgKda: number | undefined = undefined;
      let winRate: number | undefined = undefined;
      let winCount: number | undefined = undefined;
      let lossesCount: number | undefined = undefined;

      if (matches && matches.length > 0) {
        let totalKills = 0;
        let totalDeaths = 0;
        let totalAssists = 0;
        let remakeCount = 0;
        let currentWinCount = 0;
        let currentLossesCount = 0;

        matches.forEach((m: MatchDisplay) => {
          if (m.remake) {
            remakeCount++;
          } else {
            totalKills += m.kills ?? 0;
            totalDeaths += m.deaths ?? 0;
            totalAssists += m.assists ?? 0;
            if (m.win) {
              currentWinCount++;
            } else {
              currentLossesCount++;
            }
          }
        });

        winCount = currentWinCount;
        lossesCount = currentLossesCount;
        const validMatches = matches.length - remakeCount;
        winRate =
          validMatches > 0 ? Math.round((currentWinCount / validMatches) * 100) : 0;
        const deathsForCalc = totalDeaths === 0 ? 1 : totalDeaths;
        avgKda = (totalKills + totalAssists) / deathsForCalc;
      }

      let fateFlag: "ally" | "enemy" | null = null;
      let recentlyChampionName = "";
      if (currentSummonerId.value && matches.length > 0 && !isCurrentPlayer && safeInfo.puuid) {
        try {
          const lastGameId = matches[0].gameId;
          const fateInfo = await fetchPlayerFateInfo(
            lastGameId,
            safeInfo.puuid,
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

      const dataObj: PlayerData = {
        info: safeInfo,
        matches,
        ranked: { solo, flex },
        loading: false,
        matchHistoryHidden,
        championId: fallbackPlayer?.championId || fallbackPlayer?.botChampionId || 0,
        avgKda,
        winRate,
        winCount,
        lossesCount,
        fateFlag,
        recentlyChampionName,
      };
      playerData.value[cellId] = dataObj;
      if (summonerId && summonerId !== cellId) {
        playerData.value[summonerId] = dataObj;
      }
      if (safeInfo.puuid) {
        playerData.value[safeInfo.puuid] = dataObj;
      }
      debouncedSavePlayerData();
    } catch {
      const existingInfo =
        playerData.value[cellId]?.info ||
        (summonerId ? playerData.value[summonerId]?.info : undefined) ||
        (playerPuuid ? playerData.value[playerPuuid]?.info : undefined);
      const dataObj: PlayerData = {
        info: existingInfo || null,
        matches: [],
        ranked: { solo: null, flex: null },
        loading: false,
        matchHistoryHidden: true,
        championId: fallbackPlayer?.championId || fallbackPlayer?.botChampionId || 0,
      };
      playerData.value[cellId] = dataObj;
      if (summonerId && summonerId !== cellId) {
        playerData.value[summonerId] = dataObj;
      }
      if (playerPuuid) {
        playerData.value[playerPuuid] = dataObj;
      }
    }
  }

  async function loadAllPlayers() {
    const my = myTeam.value;
    const their = theirTeam.value;
    if (my.length === 0 && their.length === 0) return;
    await updateCurrentQueueId();

    const filterValidPlayers = (team: PremadePlayerLike[], isEnemy: boolean) => {
      if (store.gamePhase === "ChampSelect" && isEnemy) {
        // 选人阶段敌方队伍若无有效身份且非人机，不请求
        return team.filter(
          (p) =>
            Boolean(
              p.puuid ||
              p.summonerId ||
              p.bot ||
              p.isBot ||
              p.botChampionId ||
              p.displayName ||
              p.summonerName,
            ),
        );
      }
      return team;
    };

    const isMyVisible = activeTab.value === "my";
    // 先加载当前可见队伍，再后台加载另一队，避免请求风暴
    const visible = filterValidPlayers(isMyVisible ? my : their, !isMyVisible);
    const background = filterValidPlayers(isMyVisible ? their : my, isMyVisible);

    await runWithConcurrency(visible, 3, (p) => {
      const cid = p.cellId ?? p.summonerId;
      const sid = p.summonerId ?? p.cellId;
      if (cid !== undefined && (sid !== undefined || p.puuid)) {
        return loadPlayerData(cid, sid ?? 0, p.puuid, p);
      }
      return Promise.resolve();
    });
    void runWithConcurrency(background, 3, (p) => {
      const cid = p.cellId ?? p.summonerId;
      const sid = p.summonerId ?? p.cellId;
      if (cid !== undefined && (sid !== undefined || p.puuid)) {
        return loadPlayerData(cid, sid ?? 0, p.puuid, p);
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

    const checkMatches = (p: GameflowParticipant) => {
      const matchLocal =
        (currentSummonerId.value && p.summonerId === currentSummonerId.value) ||
        (currentSummonerPuuid.value && p.puuid === currentSummonerPuuid.value);
      if (matchLocal) return true;
      // 结合选人阶段我方队员快照进行队伍识别
      return champSelectTeamSnapshot.value.some(
        (cs) =>
          (p.puuid && cs.puuid && cs.puuid === p.puuid) ||
          (p.summonerId && cs.summonerId && cs.summonerId === p.summonerId) ||
          (p.summonerName && cs.displayName && cs.displayName === p.summonerName) ||
          (p.gameName && cs.gameName && cs.gameName === p.gameName),
      );
    };

    const isTeamOne = teamOne.some(checkMatches);
    const isTeamTwo = teamTwo.some(checkMatches);

    let allyTeam: GameflowParticipant[];
    let enemyTeam: GameflowParticipant[];

    if (isTeamOne) {
      allyTeam = teamOne;
      enemyTeam = teamTwo;
    } else if (isTeamTwo) {
      allyTeam = teamTwo;
      enemyTeam = teamOne;
    } else {
      // 兜底：若均未匹配到当前玩家（例如自定义人机且 summonerId 延迟），非空队伍优先作为 allyTeam
      allyTeam = teamOne.length > 0 ? teamOne : teamTwo;
      enemyTeam = teamOne.length > 0 ? teamTwo : teamOne;
    }

    if (gameflowMyTeam.value && gameflowMyTeam.value.length > 0) {
      for (const p of gameflowMyTeam.value) {
        const sourceData =
          (p.cellId !== undefined ? playerData.value[p.cellId] : undefined) ||
          (p.summonerId ? playerData.value[p.summonerId] : undefined) ||
          (p.puuid ? playerData.value[p.puuid] : undefined);
        if (sourceData) {
          if (p.cellId !== undefined) playerData.value[p.cellId] = sourceData;
          if (p.summonerId) playerData.value[p.summonerId] = sourceData;
          if (p.puuid) playerData.value[p.puuid] = sourceData;
        }
      }
    }

    const mapParticipant = (
      p: GameflowParticipant,
      idx: number,
      offset: number,
      isEnemy: boolean,
    ): PremadePlayerLike => {
      const resolvedName =
        p.gameName
          ? p.tagLine
            ? `${p.gameName}#${p.tagLine}`
            : p.gameName
          : p.summonerName ||
            p.displayName ||
            p.botName ||
            (p.bot || p.isBot ? "电脑" : "");
      // 为敌我双方生成互不碰撞的稳定 cellId：我方 0..4，敌方 5..9
      const stableCellId = offset + idx;

      // 尝试从选人快照中继承已选英雄和已有数据
      let resolvedChampId = p.championId ?? p.botChampionId ?? 0;
      let profileIconId = p.profileIconId;
      const targetSnapshot = isEnemy
        ? champSelectTheirTeamSnapshot.value
        : champSelectTeamSnapshot.value;
      const snap = targetSnapshot.find(
        (cs) =>
          (p.puuid && cs.puuid && cs.puuid === p.puuid) ||
          (p.summonerId && cs.summonerId && cs.summonerId === p.summonerId) ||
          (p.summonerName && cs.displayName && cs.displayName === p.summonerName) ||
          (p.gameName && cs.gameName && cs.gameName === p.gameName) ||
          (p.displayName && cs.displayName && cs.displayName === p.displayName) ||
          (cs.cellId !== undefined && cs.cellId === idx) ||
          (cs.cellId !== undefined && cs.cellId === stableCellId),
      ) || targetSnapshot[idx];
      if (snap) {
        if (!resolvedChampId || resolvedChampId <= 0) {
          resolvedChampId = snap.championId ?? snap.championPickIntent ?? 0;
        }
        if (!profileIconId && snap.profileIconId) {
          profileIconId = snap.profileIconId;
        }
      }
      // 若仍未解析到英雄 ID，尝试直接用下标对齐的快照兜底
      if ((!resolvedChampId || resolvedChampId <= 0) && targetSnapshot[idx]) {
        resolvedChampId =
          targetSnapshot[idx].championId ??
          targetSnapshot[idx].championPickIntent ??
          0;
      }
      // 最终兜底：从当前已渲染的队伍数据中找同一玩家的 championId
      // 这解决 InProgress 阶段 gameflow session 不再携带 championId 导致覆盖 GameStart 时有效值的问题
      if (!resolvedChampId || resolvedChampId <= 0) {
        const prevTeam = isEnemy ? gameflowTheirTeam.value : gameflowMyTeam.value;
        const prevPlayer = prevTeam.find(
          (pp) =>
            (p.puuid && pp.puuid && pp.puuid === p.puuid) ||
            (p.summonerId && pp.summonerId && pp.summonerId === p.summonerId) ||
            pp.cellId === stableCellId,
        );
        if (prevPlayer?.championId && prevPlayer.championId > 0) {
          resolvedChampId = prevPlayer.championId;
        }
      }
      // 同样从 playerData 里找已有的 championId
      if (!resolvedChampId || resolvedChampId <= 0) {
        const pd =
          (p.puuid ? playerData.value[p.puuid] : undefined) ||
          (p.summonerId ? playerData.value[p.summonerId] : undefined) ||
          playerData.value[stableCellId];
        if (pd?.championId && pd.championId > 0) {
          resolvedChampId = pd.championId;
        }
      }

      return {
        ...p,
        cellId: stableCellId,
        championId: resolvedChampId,
        summonerId: p.summonerId,
        puuid: p.puuid,
        gameName: p.gameName,
        tagLine: p.tagLine,
        profileIconId,
        displayName: resolvedName,
        bot: Boolean(p.bot || p.isBot),
        isBot: Boolean(p.bot || p.isBot),
      };
    };

    gameflowMyTeam.value = allyTeam.map((p, idx) =>
      mapParticipant(p, idx, 0, false),
    );
    gameflowTheirTeam.value = enemyTeam.map((p, idx) =>
      mapParticipant(p, idx, 5, true),
    );

    // 为双方队员预填充基础占位，避免后台异步请求完成前界面出现空白
    const seedInitialPlayerData = (players: PremadePlayerLike[]) => {
      for (const p of players) {
        const key = p.cellId ?? 0;
        const isBot = Boolean(p.bot || p.isBot || p.botChampionId);
        // 如果已有选人阶段完整加载的数据，保留并继承
        const existing =
          playerData.value[key] ||
          (p.summonerId ? playerData.value[p.summonerId] : undefined) ||
          (p.puuid ? playerData.value[p.puuid] : undefined);
        if (existing) {
          // 若 gameflow 带来了新的 championId（加载中阶段可获取），始终更新
          if (p.championId && p.championId > 0) {
            existing.championId = p.championId;
          }
          playerData.value[key] = existing;
          if (p.summonerId) playerData.value[p.summonerId] = existing;
          if (p.puuid) playerData.value[p.puuid] = existing;
          continue;
        }
        if (!playerData.value[key]) {
          const fallbackName =
            p.displayName ||
            p.gameName ||
            p.summonerName ||
            (isBot ? "电脑" : `玩家${key + 1}`);
          const iconId = p.profileIconId ?? 29;
          const placeholder: PlayerData = {
            info: {
              accountId: 0,
              summonerId: p.summonerId ?? 0,
              puuid: p.puuid ?? "",
              displayName: fallbackName,
              gameName: p.gameName || fallbackName,
              tagLine: p.tagLine || "",
              profileIconId: iconId,
              profileIconUrl: `/lol-game-data/assets/v1/profile-icons/${iconId}.jpg`,
              summonerLevel: 0,
              percentCompleteForNextLevel: 0,
              xpSinceLastLevel: 0,
              xpUntilNextLevel: 0,
            },
            matches: [],
            ranked: { solo: null, flex: null },
            loading: !isBot,
            matchHistoryHidden: isBot,
            championId: p.championId,
          };
          playerData.value[key] = placeholder;
        } else if (p.championId && (!playerData.value[key].championId || playerData.value[key].championId! <= 0)) {
          playerData.value[key].championId = p.championId;
        }
        if (p.summonerId && p.summonerId !== key && !playerData.value[p.summonerId]) {
          // 防止选人阶段占位数据（cellId 充当 summonerId）污染真实 summonerId 键
          const existingSid = playerData.value[key]?.info?.summonerId;
          if (!existingSid || existingSid === p.summonerId || existingSid === key) {
            playerData.value[p.summonerId] = playerData.value[key];
          }
        }
        if (p.puuid && !playerData.value[p.puuid]) {
          // 防止选人阶段空 puuid 占位数据污染真实 puuid 键
          const existingPuuid = playerData.value[key]?.info?.puuid;
          if (!existingPuuid || existingPuuid === p.puuid) {
            playerData.value[p.puuid] = playerData.value[key];
          }
        }
      }
    };
    seedInitialPlayerData(gameflowMyTeam.value);
    seedInitialPlayerData(gameflowTheirTeam.value);

    premadeColorsMy.value = computePremadeColors(gameflowMyTeam.value);
    premadeColorsTheir.value = computePremadeColors(gameflowTheirTeam.value);

    // 无论当前 activeTab 是哪一队，均并发启动双方队伍加载（当前可见队伍优先启动）
    // 避免 10 列视图时敌方被推迟到 background 甚至因异步延迟导致渲染空白
    const visible =
      activeTab.value === "my" ? gameflowMyTeam.value : gameflowTheirTeam.value;
    const background =
      activeTab.value === "my" ? gameflowTheirTeam.value : gameflowMyTeam.value;

    const loadTeam = (team: PremadePlayerLike[]) =>
      runWithConcurrency(team, 3, (p) =>
        loadPlayerData(p.cellId ?? 0, p.summonerId ?? 0, p.puuid, p),
      );

    // 优先启动可见队，紧随其后启动后台队，全部完成后写盘落盘
    loadTeam(visible)
      .catch((e) => console.debug("[GameInfo] 队伍数据加载异常:", e));

    loadTeam(background)
      .then(() => {
        // 双方 10 人信息加载完毕，立即保存完整对局
        writeReserveData();
      })
      .catch((err) => {
        console.debug("[GameInfo] 队伍数据预加载失败:", err);
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
      const t1 = teamOne || [];
      const t2 = teamTwo || [];
      if (t1.length === 0 && t2.length === 0) {
        let retried = 0;
        const maxRetries = 10;
        while (retried < maxRetries) {
          await new Promise((r) => setTimeout(r, 1000));
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
          if (
            rt &&
            ((rt.teamOne && rt.teamOne.length > 0) ||
              (rt.teamTwo && rt.teamTwo.length > 0))
          ) {
            return processTeamData(rt.teamOne || [], rt.teamTwo || []);
          }
          retried++;
        }
        loading.value = false;
        return;
      }

      if (t1.length === 0 || t2.length === 0) {
        // 若其中一队为空，给它短暂重试机会（最多 3 次，每次 1 秒），若依然只有单边（如自定义单边练习），直接处理已有队伍，不阻塞
        let retried = 0;
        let currentT1 = t1;
        let currentT2 = t2;
        while (
          retried < 3 &&
          (currentT1.length === 0 || currentT2.length === 0)
        ) {
          await new Promise((r) => setTimeout(r, 1000));
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
          if (rt?.teamOne?.length && rt?.teamTwo?.length) {
            currentT1 = rt.teamOne;
            currentT2 = rt.teamTwo;
            break;
          }
          if (rt?.teamOne && rt.teamOne.length > 0) currentT1 = rt.teamOne;
          if (rt?.teamTwo && rt.teamTwo.length > 0) currentT2 = rt.teamTwo;
          retried++;
        }
        if (reqId !== currentGameflowSessionRequestId) return;
        await processTeamData(currentT1, currentT2);
        loading.value = false;
        return;
      }

      if (reqId !== currentGameflowSessionRequestId) return;
      await processTeamData(t1, t2);
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
      if (phase === "InProgress" || phase === "GameStart") {
        // 从选人阶段进入载入或游戏时，清空 session 缓存
        cachedSession = null;
        loadFromGameflowSession();
      }
    },
    { immediate: true },
  );

  // 监听 WebSocket 推送的 gameflowSession 变化，对局就绪即刻自动解析
  watch(
    () => store.gameflowSession,
    (session) => {
      if (!session?.gameData) return;
      if (store.gamePhase !== "InProgress" && store.gamePhase !== "GameStart") return;
      const { teamOne, teamTwo } = session.gameData;
      if ((teamOne && teamOne.length > 0) || (teamTwo && teamTwo.length > 0)) {
        // 如果当前尚未加载或者队伍为空，立即加载
        if (gameflowMyTeam.value.length === 0 || gameflowTheirTeam.value.length === 0) {
          processTeamData(teamOne || [], teamTwo || []);
        }
      }
    },
  );

  // 团队内容签名：成员 cellId + 英雄 ID（包含锁定、预选及 actions 挑选）。session 高频事件中仅倒计时变化时签名不变，跳过无效重载
  let lastSessionTeamSig = "";
  const teamSig = (team: ChampSelectPlayer[], session?: any) =>
    (team || [])
      .map((p) => {
        const champId = resolvePlayerChampionId(p, session);
        return `${p.cellId}:${champId}:${p.puuid || ""}`;
      })
      .join(",");

  watch(
    () => store.champSelectSession,
    (session) => {
      if (session && store.gamePhase === "ChampSelect") {
        const myTeam = session.myTeam || [];
        const theirTeam = session.theirTeam || [];
        const sig = teamSig(myTeam, session) + "|" + teamSig(theirTeam, session);
        if (sig === lastSessionTeamSig) return;
        lastSessionTeamSig = sig;

        // 增量更新选人阶段双方队员与已锁定英雄快照（若之前已有有效 championId，避免被过渡帧冲为 0）
        const mergeSnapshot = (
          newPlayers: ChampSelectPlayer[],
          prevSnapshot: PremadePlayerLike[],
        ): PremadePlayerLike[] => {
          return newPlayers.map((p) => {
            const detectedChampId = resolvePlayerChampionId(p, session);
            const prev = prevSnapshot.find(
              (old) =>
                (p.puuid && old.puuid === p.puuid) ||
                (p.summonerId && old.summonerId === p.summonerId) ||
                old.cellId === p.cellId,
            );
            const finalChampId =
              detectedChampId > 0
                ? detectedChampId
                : prev?.championId && prev.championId > 0
                  ? prev.championId
                  : 0;
            return {
              ...p,
              championId: finalChampId,
            };
          });
        };

        champSelectTeamSnapshot.value = mergeSnapshot(
          myTeam,
          champSelectTeamSnapshot.value,
        );
        champSelectTheirTeamSnapshot.value = mergeSnapshot(
          theirTeam,
          champSelectTheirTeamSnapshot.value,
        );

        loading.value = false;
        error.value = "";
        gameflowMyTeam.value = champSelectTeamSnapshot.value;
        gameflowTheirTeam.value = champSelectTheirTeamSnapshot.value;
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
