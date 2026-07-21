import { ref, computed, watch, onMounted, type Ref } from "vue";
import { useLcuStore } from "../store/lcuStore";
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
import type { PlayerData } from "../types/gameInfo";
import { computePremadeColors } from "./usePremadeGroup";

// ── 排位数据缓存（puuid → { data, timestamp }），带 LRU / 容量上限保护，避免内存泄露
const rankCache = new Map<string, { data: any; timestamp: number }>();
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

function setRankToCache(puuid: string, data: any) {
  if (rankCache.size >= RANK_CACHE_MAX_SIZE) {
    const firstKey = rankCache.keys().next().value;
    if (firstKey) rankCache.delete(firstKey);
  }
  rankCache.set(puuid, { data, timestamp: Date.now() });
}

// ── gameflow session 短期缓存，避免同一流程中多次请求同一端点
let cachedSession: { data: any; timestamp: number } | null = null;
const SESSION_CACHE_TTL = 30 * 1000; // 30 秒
let currentGameflowSessionRequestId = 0; // 用于防并发竞态的请求标识计数器

async function fetchSessionCached(): Promise<any | null> {
  const now = Date.now();
  if (cachedSession && now - cachedSession.timestamp < SESSION_CACHE_TTL) {
    return cachedSession.data;
  }
  try {
    const resp = await lcuRequest<any>("GET", "/lol-gameflow/v1/session");
    if (resp.success && resp.data) {
      cachedSession = { data: resp.data, timestamp: now };
      return resp.data;
    }
  } catch {
    /* ignore */
  }
  return null;
}

const MATCHES_CACHE_KEY = (puuid: string) => `yuumi_matches_cache_${puuid}`;
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

  try {
    localStorage.setItem(MATCHES_CACHE_KEY(puuid), JSON.stringify(merged));
  } catch {
    /* ignore */
  }

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

  const sessionAllyTeam = ref<any[]>([]);
  const sessionEnemyTeam = ref<any[]>([]);

  const gameflowMyTeam = ref<any[]>([]);
  const gameflowTheirTeam = ref<any[]>([]);

  const currentQueueId = ref<number | null>(null);
  const isTftMode = ref(false);

  // ── localStorage 写入防抖
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  function debouncedSavePlayerData() {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      try {
        localStorage.setItem(
          "yuumi_last_game_player_data",
          JSON.stringify(playerData.value),
        );
        localStorage.setItem(
          "yuumi_last_gameflow_my_team",
          JSON.stringify(myTeam.value),
        );
        localStorage.setItem(
          "yuumi_last_gameflow_their_team",
          JSON.stringify(theirTeam.value),
        );
      } catch {
        /* ignore */
      }
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
      const data = await fetchSessionCached();
      if (!data?.gameData) return;
      const { teamOne, teamTwo } = data.gameData;
      if (!teamOne || !teamTwo || teamOne.length === 0) return;

      const isTeamOne = teamOne.some(
        (p: any) => p.summonerId === currentSummonerId.value,
      );
      const ally = isTeamOne ? teamOne : teamTwo;
      const enemy = isTeamOne ? teamTwo : teamOne;
      sessionAllyTeam.value = ally;
      sessionEnemyTeam.value = enemy;
      premadeColorsMy.value = computePremadeColors(ally);
      premadeColorsTheir.value = computePremadeColors(enemy);
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

  async function loadPlayerData(cellId: number, summonerId: number) {
    if (!summonerId) return;

    const existing = playerData.value[cellId];
    const existingIsCurrentPlayer =
      summonerId === currentSummonerId.value ||
      (!!existing?.info?.puuid &&
        existing.info.puuid === currentSummonerPuuid.value);
    if (
      existing?.info &&
      (!existingIsCurrentPlayer || existing.matches.length >= 10)
    )
      return;

    playerData.value[cellId] = {
      info: null,
      matches: [],
      ranked: { solo: null, flex: null },
      loading: true,
    };

    try {
      const resp = await lcuRequest<any>(
        "GET",
        `/lol-summoner/v1/summoners/${summonerId}`,
      );
      if (!resp.success || !resp.data) {
        playerData.value[cellId] = {
          info: null,
          matches: [],
          ranked: { solo: null, flex: null },
          loading: false,
        };
        return;
      }
      const info = resp.data;

      const filterEnabled = appConfig.value?.Functions?.GameInfoFilter ?? false;
      const maxMatches = filterEnabled ? 50 : 10;

      const [rawMatches, rankedResp] = await Promise.all([
        info.puuid
          ? fetchMatchHistory(info.puuid, 0, maxMatches)
          : Promise.resolve([]),
        info.puuid
          ? (() => {
              const cached = getRankFromCache(info.puuid);
              if (cached) {
                return Promise.resolve({ success: true, data: cached });
              }
              return lcuRequest<any>(
                "GET",
                `/lol-ranked/v1/ranked-stats/${info.puuid}`,
              ).then((rResp) => {
                if (rResp.success && rResp.data) {
                  setRankToCache(info.puuid, rResp.data);
                }
                return rResp;
              });
            })()
          : Promise.resolve({ success: false } as any),
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

      let solo = null,
        flex = null;
      if (rankedResp.success && rankedResp.data?.queues) {
        solo =
          rankedResp.data.queues.find(
            (q: any) => q.queueType === "RANKED_SOLO_5x5",
          ) || null;
        flex =
          rankedResp.data.queues.find(
            (q: any) => q.queueType === "RANKED_FLEX_SR",
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
    const team = currentTeam.value;
    if (!team || team.length === 0) return;
    await updateCurrentQueueId();
    await Promise.all(
      team.map((p: any) => loadPlayerData(p.cellId, p.summonerId)),
    );
  }

  async function processTeamData(teamOne: any[], teamTwo: any[]) {
    const isTeamOne = teamOne.some(
      (p: any) => p.summonerId === currentSummonerId.value,
    );
    const allyTeam = isTeamOne ? teamOne : teamTwo;
    const enemyTeam = isTeamOne ? teamTwo : teamOne;

    if (gameflowMyTeam.value && gameflowMyTeam.value.length > 0) {
      for (const p of gameflowMyTeam.value) {
        if (
          p.summonerId &&
          p.cellId !== undefined &&
          playerData.value[p.cellId]
        ) {
          playerData.value[p.summonerId] = playerData.value[p.cellId];
        }
      }
    }

    gameflowMyTeam.value = allyTeam.map((p: any) => ({
      ...p,
      cellId: p.summonerId,
      displayName: p.summonerName || p.displayName,
    }));
    gameflowTheirTeam.value = enemyTeam.map((p: any) => ({
      ...p,
      cellId: p.summonerId,
      displayName: p.summonerName || p.displayName,
    }));

    premadeColorsMy.value = computePremadeColors(allyTeam);
    premadeColorsTheir.value = computePremadeColors(enemyTeam);

    try {
      localStorage.setItem(
        "yuumi_last_gameflow_my_team",
        JSON.stringify(gameflowMyTeam.value),
      );
      localStorage.setItem(
        "yuumi_last_gameflow_their_team",
        JSON.stringify(gameflowTheirTeam.value),
      );
    } catch {
      /* ignore */
    }

    await Promise.all([
      ...allyTeam.map((p: any) => loadPlayerData(p.summonerId, p.summonerId)),
      ...enemyTeam.map((p: any) => loadPlayerData(p.summonerId, p.summonerId)),
    ]);
  }

  async function loadFromGameflowSession() {
    loading.value = true;
    error.value = "";

    const reqId = ++currentGameflowSessionRequestId;

    cachedSession = null;
    await updateCurrentQueueId();
    if (isTftMode.value) {
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
        while (retried < 5) {
          await new Promise((r) => setTimeout(r, 3000));

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
          if (rt?.teamOne?.length > 0 && rt?.teamTwo?.length > 0) {
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
      try {
        const savedMyTeam = localStorage.getItem("yuumi_last_gameflow_my_team");
        const savedTheirTeam = localStorage.getItem(
          "yuumi_last_gameflow_their_team",
        );
        const savedPlayerData = localStorage.getItem(
          "yuumi_last_game_player_data",
        );
        if (savedMyTeam) {
          const parsed = JSON.parse(savedMyTeam);
          if (Array.isArray(parsed) && parsed.length > 0)
            gameflowMyTeam.value = parsed;
        }
        if (savedTheirTeam) {
          const parsed = JSON.parse(savedTheirTeam);
          if (Array.isArray(parsed) && parsed.length > 0)
            gameflowTheirTeam.value = parsed;
        }
        if (savedPlayerData) {
          const parsed = JSON.parse(savedPlayerData);
          if (parsed && Object.keys(parsed).length > 0) playerData.value = parsed;
        }
      } catch {
        /* ignore */
      }
    } else {
      gameflowMyTeam.value = [];
      gameflowTheirTeam.value = [];
      playerData.value = {};
    }
  });

  watch(
    () => store.gamePhase,
    (phase: string) => {
      if (phase === "None" || phase === "Lobby") {
        isTftMode.value = false;
      }
      if (phase === "ChampSelect") {
        isTftMode.value = false;
        gameflowMyTeam.value = [];
        gameflowTheirTeam.value = [];
        playerData.value = {};
        premadeColorsMy.value = {};
        premadeColorsTheir.value = {};
        try {
          localStorage.removeItem("yuumi_last_gameflow_my_team");
          localStorage.removeItem("yuumi_last_gameflow_their_team");
          localStorage.removeItem("yuumi_last_game_player_data");
        } catch {
          /* ignore */
        }
        refreshState();
      }
      if (phase === "InProgress" || phase === "GameStart")
        loadFromGameflowSession();
    },
  );

  watch(
    () => store.champSelectSession,
    (session: any) => {
      if (session && store.gamePhase === "ChampSelect") {
        loading.value = false;
        error.value = "";
        gameflowMyTeam.value = session.myTeam || [];
        loadAllPlayers();
        fetchPremadeColors();

        try {
          if (session.myTeam && session.myTeam.length > 0) {
            localStorage.setItem(
              "yuumi_last_gameflow_my_team",
              JSON.stringify(session.myTeam),
            );
          }
          if (session.theirTeam && session.theirTeam.length > 0) {
            localStorage.setItem(
              "yuumi_last_gameflow_their_team",
              JSON.stringify(session.theirTeam),
            );
          }
        } catch {
          /* ignore */
        }
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
      try {
        const savedMyTeam = localStorage.getItem("yuumi_last_gameflow_my_team");
        const savedTheirTeam = localStorage.getItem(
          "yuumi_last_gameflow_their_team",
        );
        const savedPlayerData = localStorage.getItem("yuumi_last_game_player_data");
        if (savedMyTeam) {
          const parsed = JSON.parse(savedMyTeam);
          if (Array.isArray(parsed) && parsed.length > 0)
            gameflowMyTeam.value = parsed;
        }
        if (savedTheirTeam) {
          const parsed = JSON.parse(savedTheirTeam);
          if (Array.isArray(parsed) && parsed.length > 0)
            gameflowTheirTeam.value = parsed;
        }
        if (savedPlayerData) {
          const parsed = JSON.parse(savedPlayerData);
          if (parsed && Object.keys(parsed).length > 0) playerData.value = parsed;
        }

        if (gameflowMyTeam.value.length > 0)
          premadeColorsMy.value = computePremadeColors(gameflowMyTeam.value);
        if (gameflowTheirTeam.value.length > 0)
          premadeColorsTheir.value = computePremadeColors(gameflowTheirTeam.value);
      } catch {
        /* ignore */
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
