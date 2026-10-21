import {
  ref,
  computed,
  watch,
  onMounted,
  onUnmounted,
  toRaw,
  type Ref,
} from "vue";
import { useLcuStore, type ChampSelectPlayer } from "../store/lcuStore";
import {
  getGameflowPhase,
  getChampSelectSession,
  fetchMatchHistory,
  fetchMatchHistorySgp,
  fetchMatchHistorySmart,
  fetchCurrentSummoner,
  lcuRequest,
  fetchConfig,
  fetchPlayerFateInfo,
  getLiveClientPlayerList,
  getLiveClientActivePlayerName,
  getMapSide,
  writeFrontendLog,
  type MatchDisplay,
  type AppConfig,
  type SavedPlayerMarker,
} from "../api/lcu";
import type {
  PlayerData,
  PremadePlayerLike,
  StreakInfo,
} from "../types/gameInfo";
import { resolvePlayerChampionId } from "../types/gameInfo";
import type { GameflowParticipant, RankedQueueEntry } from "../types/lcu";
import type { SummonerDisplay } from "../api/lcu";
import { computePremadeColors, hasPremadeGroup } from "./usePremadeGroup";
import { runWithConcurrency } from "../utils/runWithConcurrency";
import type {
  AggregatedPositions,
  AggregatedChampion,
} from "../utils/matchAnalysis";
import {
  fetchChampions,
  findChampionIdByLiveClientName,
  isRealSummonerName,
} from "../utils/championCache";
import {
  fetchPlayerMastery,
  querySavedPlayersMapDedup,
  fetchRankedStatsCached,
} from "../utils/playerDataCache";
import {
  isIdentityCompatible,
  inheritPlaceholderChampion,
  getValidTpid,
} from "../utils/playerIdentity";
import { computeMatchStats } from "../utils/matchStats";
import {
  createPlaceholderPlayerData,
  createSummonerDisplay,
} from "../utils/playerPlaceholder";
import {
  fetchSessionCached,
  clearSessionCache,
  seedSessionCache,
} from "../utils/gameflowSession";
import {
  resolveTeamSide,
  buildGameflowTeams,
  rekeyExistingPlayerData,
  seedInitialPlayerData,
  resolvePremadeColors,
  type TeamBuildContext,
} from "../utils/teamBuilder";

// 对外 API 兼容：这些工具已迁至 utils 模块，仍从本文件转发供视图层导入
export { isIdentityCompatible, NEW_PLAYER_MAX_LEVEL } from "../utils/playerIdentity";
export {
  querySavedPlayersMapDedup,
  fetchPlayerMastery,
} from "../utils/playerDataCache";


// 用于防止 loadFromGameflowSession 并发竞态的请求标识计数器
let currentGameflowSessionRequestId = 0;

// ── 保留对局数据 localStorage：持续保留上一局数据，直到新对局开始（ChampSelect 时清理）
const RESERVE_TEAM_KEYS = [
  "yuumi_last_gameflow_my_team",
  "yuumi_last_gameflow_their_team",
  "yuumi_last_game_player_data",
  "yuumi_last_game_loaded_count",
  "yuumi_last_premade_colors_my",
  "yuumi_last_premade_colors_their",
  "yuumi_last_game_id",
  "yuumi_last_game_team_count",
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

export function useGamePlayerData(
  appConfig: Ref<AppConfig | null>,
  premadeColorsMy: Ref<Record<string | number, number>>,
  premadeColorsTheir: Ref<Record<string | number, number>>,
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
  // 当前对局 gameId：InProgress 精简 session 恢复整局时校验落盘快照归属，防串到上一局
  const currentGameId = ref<number | null>(null);

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
    // 同一局内禁止用更瘦的快照覆盖落盘：InProgress 阶段 LCU 会把 gameflow 队伍精简到仅剩自己，
    // 刷新后残缺 session 回写不得毒化 GameStart 时落盘的完整快照
    const currentTotal =
      gameflowMyTeam.value.length + gameflowTheirTeam.value.length;
    if (currentGameId.value) {
      let savedGameId = 0;
      let savedTotal = 0;
      try {
        savedGameId = Number(localStorage.getItem("yuumi_last_game_id")) || 0;
        savedTotal =
          Number(localStorage.getItem("yuumi_last_game_team_count")) || 0;
      } catch {
        /* ignore */
      }
      if (
        savedGameId === currentGameId.value &&
        savedTotal > 0 &&
        currentTotal < savedTotal
      ) {
        return;
      }
    }
    // 仅在当前已加载人数/队伍总人数不低于已有快照时才更新持久化，避免被过渡状态的不完整快照覆盖
    try {
      // loading 占位不入快照：恢复后没有重拉逻辑，会永久停留在转圈态
      const persistableData: Record<string | number, PlayerData> = {};
      for (const key of Object.keys(playerData.value)) {
        const entry = playerData.value[key];
        if (!entry || entry.loading) continue;
        persistableData[key] = entry;
      }
      localStorage.setItem(
        "yuumi_last_game_player_data",
        JSON.stringify(persistableData),
      );
      localStorage.setItem(
        "yuumi_last_gameflow_my_team",
        JSON.stringify(gameflowMyTeam.value),
      );
      localStorage.setItem(
        "yuumi_last_gameflow_their_team",
        JSON.stringify(gameflowTheirTeam.value),
      );
      localStorage.setItem(
        "yuumi_last_premade_colors_my",
        JSON.stringify(premadeColorsMy.value),
      );
      localStorage.setItem(
        "yuumi_last_premade_colors_their",
        JSON.stringify(premadeColorsTheir.value),
      );
      localStorage.setItem("yuumi_last_game_loaded_count", String(loadedCount));
      if (currentGameId.value) {
        localStorage.setItem("yuumi_last_game_id", String(currentGameId.value));
        localStorage.setItem(
          "yuumi_last_game_team_count",
          String(currentTotal),
        );
      }
    } catch {
      /* ignore */
    }
  }

  // ── 从 localStorage 恢复保留数据（有数据即恢复，直到新对局开始）
  let isBackfillingMasteries = false;
  function backfillMissingMasteries() {
    if (isBackfillingMasteries) return;
    const entries = Object.values(playerData.value);
    const needBackfill = entries.filter(
      (e) =>
        e &&
        e.info &&
        !e.loading &&
        (!e.masteries || e.masteries.length === 0) &&
        (e.info.puuid || e.info.summonerId),
    );
    if (needBackfill.length === 0) return;

    isBackfillingMasteries = true;
    // 异步排队后台拉取，每人间隔 100ms，避免突发并发
    Promise.allSettled(
      needBackfill.map(
        (p, idx) =>
          new Promise<void>((resolve) => {
            setTimeout(async () => {
              try {
                const puuid = p.info?.puuid;
                const sid = p.info?.summonerId;
                const isMe =
                  sid === currentSummonerId.value ||
                  (!!puuid && puuid === currentSummonerPuuid.value);
                const m = await fetchPlayerMastery(puuid, sid, isMe);
                if (m && m.length > 0) {
                  p.masteries = m;
                }
              } catch {
                /* ignore */
              } finally {
                resolve();
              }
            }, idx * 100);
          }),
      ),
    ).finally(() => {
      isBackfillingMasteries = false;
      debouncedSavePlayerData();
    });
  }

  let isSyncingCurrentPlayerMatches = false;
  let lastCurrentPlayerSyncTime = 0;

  async function syncCurrentPlayerMatches(forceNetwork = false) {
    if (!store.isConnected) return;
    if (isSyncingCurrentPlayerMatches) return;
    if (!forceNetwork && Date.now() - lastCurrentPlayerSyncTime < 10_000) {
      return;
    }

    let puuid =
      currentSummonerPuuid.value || store.currentSummoner?.puuid || "";
    let sid =
      currentSummonerId.value || store.currentSummoner?.summonerId || 0;

    if (!puuid || !sid) {
      try {
        const s = await fetchCurrentSummoner();
        if (s?.puuid) {
          puuid = s.puuid;
          currentSummonerPuuid.value = s.puuid;
        }
        if (s?.summonerId) {
          sid = s.summonerId;
          currentSummonerId.value = s.summonerId;
        }
      } catch {
        /* ignore */
      }
    }
    if (!puuid) return;

    const myTeamMember = gameflowMyTeam.value.find(
      (p) =>
        (puuid && p.puuid === puuid) || (sid && p.summonerId === sid),
    );
    const myCellKey =
      myTeamMember?.cellId !== undefined ? String(myTeamMember.cellId) : null;

    const targetEntries: PlayerData[] = [];
    for (const key of Object.keys(playerData.value)) {
      const entry = playerData.value[key];
      if (!entry || entry.loading) continue;
      if (
        (entry.info?.puuid && entry.info.puuid === puuid) ||
        (entry.info?.summonerId && sid && entry.info.summonerId === sid) ||
        key === puuid ||
        (sid && key === String(sid)) ||
        (myCellKey !== null && key === myCellKey)
      ) {
        if (!targetEntries.includes(entry)) {
          targetEntries.push(entry);
        }
      }
    }
    if (targetEntries.length === 0) return;

    const filterEnabled = appConfig.value?.Functions?.GameInfoFilter ?? false;
    isSyncingCurrentPlayerMatches = true;
    lastCurrentPlayerSyncTime = Date.now();
    try {
      const maxMatches = 50;
      const rawMatches = await fetchMatchHistorySmart(puuid, 0, maxMatches - 1, {
        forceSgp: true,
      });
      if (rawMatches && rawMatches.length > 0) {
        let filtered = rawMatches;
        if (filterEnabled && currentQueueId.value !== null) {
          filtered = filtered.filter((m) => m.queueId === currentQueueId.value);
        }
        if (filtered.length > 0) {
          const prevFirstId = targetEntries[0].matches?.[0]?.gameId;
          const prevLen = targetEntries[0].matches?.length ?? 0;
          const stats = computeMatchStats(filtered);
          for (const entry of targetEntries) {
            entry.matches = filtered.slice(0, 10);
            entry.matchHistoryHidden = false;
            entry.winCount = stats.winCount;
            entry.lossesCount = stats.lossesCount;
            entry.winRate = stats.winRate;
            entry.avgKda = stats.avgKda;
            entry.streak = stats.streak;
            entry.positions = stats.positions;
            entry.topChampions = stats.topChampions;
          }
          if (filtered[0]?.gameId !== prevFirstId || filtered.length !== prevLen) {
            debouncedSavePlayerData();
          }
        }
      }
    } catch (e) {
      console.debug("[GamePlayerData] 同步当前玩家最新战绩失败:", e);
    } finally {
      isSyncingCurrentPlayerMatches = false;
    }
  }

  function restoreReserveDataFromLocalStorage(): boolean {
    // 严格收紧 localStorage 恢复：
    // 1. 若处于活跃对局阶段（ChampSelect/GameStart/InProgress）：
    //    仅在同局精简修复场景（liveGameId 与 savedId 一致且有效）时允许调用方恢复；跨局或不同 gameId 一律禁止恢复！
    const isGameActivePhase =
      store.gamePhase === "ChampSelect" ||
      store.gamePhase === "GameStart" ||
      store.gamePhase === "InProgress";
    if (isGameActivePhase) {
      const savedId = Number(localStorage.getItem("yuumi_last_game_id")) || 0;
      const curId = currentGameId.value || 0;
      if (!curId || !savedId || curId !== savedId) {
        return false;
      }
    } else {
      // 2. 若处于非活跃阶段（None/Lobby/EndOfGame等）：
      //    必须严格校验配置项 EnableReserveGameinfo 为 true，未开启则严禁恢复
      if (!appConfig.value?.Functions?.EnableReserveGameinfo) {
        return false;
      }
    }

    try {
      const savedMyTeam = localStorage.getItem("yuumi_last_gameflow_my_team");
      const savedTheirTeam = localStorage.getItem(
        "yuumi_last_gameflow_their_team",
      );
      const savedPlayerData = localStorage.getItem(
        "yuumi_last_game_player_data",
      );
      const savedPremadeMy = localStorage.getItem(
        "yuumi_last_premade_colors_my",
      );
      const savedPremadeTheir = localStorage.getItem(
        "yuumi_last_premade_colors_their",
      );
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
        if (parsed && typeof parsed === "object") {
          // 兼容旧快照：剔除 loading 占位，避免恢复后无重拉逻辑导致该列永久转圈
          const restored: Record<string | number, PlayerData> = {};
          for (const key of Object.keys(parsed)) {
            const entry = parsed[key];
            if (entry && !entry.loading) restored[key] = entry;
          }
          if (Object.keys(restored).length > 0) {
            playerData.value = restored;
            hasRestored = true;
            // 异步检查并补齐缺少熟练度的玩家（例如旧版本保留的数据）
            backfillMissingMasteries();
            // 同步当前玩家最新战绩（优先读本地缓存秒级刷新，若在线则拉取最新）
            syncCurrentPlayerMatches();
          }
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
            premadeColorsTheir.value = computePremadeColors(
              gameflowTheirTeam.value,
            );
        }
      } else if (gameflowTheirTeam.value.length > 0) {
        premadeColorsTheir.value = computePremadeColors(
          gameflowTheirTeam.value,
        );
      }
      return hasRestored;
    } catch {
      return false;
    }
  }

  // ── localStorage 写入防抖（1000ms 避免高频序列化卡顿）
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  function debouncedSavePlayerData() {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      saveTimer = null;
      writeReserveData();
    }, 1000);
  }

  /// 立即执行挂起中的防抖写入：卸载/即将丢失上下文时调用，避免落在 1s 窗口内的最后一次快照被丢弃
  function flushPendingSave() {
    if (!saveTimer) return;
    clearTimeout(saveTimer);
    saveTimer = null;
    writeReserveData();
  }

  // 窗口关闭 / 页面转入后台时兜底落盘：Tauri 直接关窗不一定会触发组件的 onUnmounted
  const handleFlushOnUnload = () => flushPendingSave();
  const handleFlushOnHidden = () => {
    if (document.visibilityState === "hidden") flushPendingSave();
  };

  // ── 低优先级静默扩充样本队列（单任务顺序执行，排在首屏之后，避免争抢信号量通道）
  const extraMatchesQueue: Array<() => Promise<void>> = [];
  let isProcessingExtraMatches = false;

  function enqueueExtraMatches(task: () => Promise<void>) {
    extraMatchesQueue.push(task);
  }

  async function processExtraMatchesQueue() {
    if (isProcessingExtraMatches) return;
    isProcessingExtraMatches = true;
    try {
      while (extraMatchesQueue.length > 0) {
        const task = extraMatchesQueue.shift();
        if (task) {
          try {
            await task();
          } catch {
            /* ignore */
          }
        }
      }
    } finally {
      isProcessingExtraMatches = false;
    }
  }

  const myTeam = computed(() => {
    if (isGameActive.value) {
      if (gameflowMyTeam.value.length > 0) return gameflowMyTeam.value;
      if (
        store.champSelectSession?.myTeam &&
        store.champSelectSession.myTeam.length > 0
      ) {
        // LCU 原生归属：myTeam（含自定义人机）即我方，直接返回（人机由下游按 bot 处理）
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
      if (store.champSelectSession?.theirTeam) {
        // LCU 原生归属：theirTeam 即敌方；空占位（无任何身份）丢弃，避免幽灵列
        const real = (store.champSelectSession.theirTeam || []).filter(
          (p) => p.isHumanoid || p.puuid || p.summonerId || p.championId,
        );
        if (real.length > 0) return real;
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
        if (
          gameMode === "TFT" ||
          (qId !== null && qId >= 1090 && qId <= 1200)
        ) {
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
              (currentSummonerId.value &&
                p.summonerId === currentSummonerId.value) ||
              (currentSummonerPuuid.value &&
                p.puuid === currentSummonerPuuid.value) ||
              champSelectTeamSnapshot.value.some(
                (cs) =>
                  (p.puuid && cs.puuid && cs.puuid === p.puuid) ||
                  (p.summonerId &&
                    cs.summonerId &&
                    cs.summonerId === p.summonerId) ||
                  (p.summonerName &&
                    cs.displayName &&
                    cs.displayName === p.summonerName) ||
                  (p.gameName && cs.gameName && cs.gameName === p.gameName),
              ),
          );
          const ally = isTeamOne || t2.length === 0 ? t1 : t2;
          const enemy = isTeamOne || t2.length === 0 ? t2 : t1;
          sessionAllyTeam.value = ally;
          sessionEnemyTeam.value = enemy;

          // 将 gameflow 中的组队标识同步回填至选人快照，供加载阶段继承及 cellId 映射使用
          for (const cs of champSelectTeamSnapshot.value) {
            if (!cs.teamParticipantId && !cs.partyId) {
              const matchedAlly = ally.find(
                (ap) =>
                  (cs.puuid && ap.puuid && cs.puuid === ap.puuid) ||
                  (cs.summonerId &&
                    ap.summonerId &&
                    cs.summonerId === ap.summonerId) ||
                  (cs.displayName &&
                    ap.summonerName &&
                    cs.displayName === ap.summonerName) ||
                  (cs.gameName && ap.gameName && cs.gameName === ap.gameName),
              );
              const tpid =
                matchedAlly?.teamParticipantId ?? matchedAlly?.partyId;
              if (tpid !== undefined && tpid !== null && tpid !== "" && tpid !== 0 && tpid !== "0") {
                cs.teamParticipantId = tpid;
                cs.partyId = tpid;
              }
            }
          }

          const allyColors = computePremadeColors(ally);
          const enemyColors = computePremadeColors(enemy);
          if (hasPremadeGroup(allyColors)) {
            // 若选人快照已回填组队 ID，合并快照计算出的 cellId 映射
            const snapColors = computePremadeColors(champSelectTeamSnapshot.value);
            premadeColorsMy.value = { ...allyColors, ...snapColors };
          }
          if (hasPremadeGroup(enemyColors)) {
            premadeColorsTheir.value = enemyColors;
          }
        }
      }
      // 备选降级：若 gameflow 暂无有效组队数据，但选人 session 或快照已有 teamParticipantId
      if (!hasPremadeGroup(premadeColorsMy.value)) {
        const csMySource =
          champSelectTeamSnapshot.value.length > 0
            ? champSelectTeamSnapshot.value
            : store.champSelectSession?.myTeam || [];
        const csMyColors = computePremadeColors(csMySource);
        if (hasPremadeGroup(csMyColors)) {
          premadeColorsMy.value = csMyColors;
        }
      }
      if (!hasPremadeGroup(premadeColorsTheir.value)) {
        const csTheirSource =
          champSelectTheirTeamSnapshot.value.length > 0
            ? champSelectTheirTeamSnapshot.value
            : store.champSelectSession?.theirTeam || [];
        const csTheirColors = computePremadeColors(csTheirSource);
        if (hasPremadeGroup(csTheirColors)) {
          premadeColorsTheir.value = csTheirColors;
        }
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
        } else if (phaseResp.data !== "ChampSelect") {
          // 非活跃阶段刷新时，主动强制同步当前玩家最新战绩
          syncCurrentPlayerMatches(true);
        }
      } else if (
        store.gamePhase !== "InProgress" &&
        store.gamePhase !== "GameStart" &&
        store.gamePhase !== "ChampSelect"
      ) {
        syncCurrentPlayerMatches(true);
      }
    } catch {
      /* ignore */
    }
    try {
      const sessionResp = await getChampSelectSession();
      if (sessionResp.success && sessionResp.data) {
        store.setChampSelectSession(sessionResp.data);
      }
    } catch {
      /* ignore */
    }
    loading.value = false;
  }

  // 拉取窗口 / 拉取失败 / 异常兜底三处的英雄保留：
  // 1. 选人阶段从选人会话 session / actions 动态反查优先 (保证挑选悬停、锁定、ARAM换英雄实时响应)
  // 2. fallback 自带英雄
  // 3. 继承占位
  const resolveCarryChampionId = (
    fallbackPlayer: PremadePlayerLike | undefined,
    cellId: number,
  ): number => {
    if (store.gamePhase === "ChampSelect" && store.champSelectSession) {
      const fromResolver = resolvePlayerChampionId(
        fallbackPlayer,
        store.champSelectSession,
      );
      if (fromResolver > 0) return fromResolver;
    }
    if (fallbackPlayer?.championId && fallbackPlayer.championId > 0)
      return fallbackPlayer.championId;
    if (fallbackPlayer?.botChampionId && fallbackPlayer.botChampionId > 0)
      return fallbackPlayer.botChampionId;
    // 非 ChampSelect 阶段策略上不使用选人残留 session（同 usePlayerChampionId），此处补预选意图兜底（等价 resolver 第 2 步）
    if (
      fallbackPlayer?.championPickIntent &&
      fallbackPlayer.championPickIntent > 0
    )
      return fallbackPlayer.championPickIntent;
    const existing =
      (fallbackPlayer?.puuid
        ? playerData.value[fallbackPlayer.puuid]
        : undefined) ||
      (fallbackPlayer?.summonerId
        ? playerData.value[fallbackPlayer.summonerId]
        : undefined) ||
      playerData.value[cellId];
    return inheritPlaceholderChampion(existing, cellId, fallbackPlayer) || 0;
  };

  // 路人集标记映射：与 GameInfo 视图共享同一份去重查询（同批 10 人并发加载只发一次）
  const getSavedPlayersMapCached = () =>
    querySavedPlayersMapDedup(currentSummonerPuuid.value);

  /**
   * 尝试复用已加载的同身份条目，避免重复请求。
   * @returns true 表示已命中并完成挂载，调用方应直接返回
   */
  function tryReuseLoadedPlayer(
    cellId: number,
    summonerId: number,
    realSummonerId: number,
    playerPuuid: string | undefined,
    fallbackPlayer: PremadePlayerLike | undefined,
  ): boolean {
    // 身份优先查找：puuid → summonerId → cellId。cell 槽在选人/对局切换或顺序变化时
    // 可能残留异队旧数据，身份键优先才能命中同一玩家的已加载项，避免误杀重拉
    const candidates = [
      playerPuuid ? playerData.value[playerPuuid] : undefined,
      summonerId ? playerData.value[summonerId] : undefined,
      playerData.value[cellId],
    ];
    const incoming = { puuid: playerPuuid, summonerId: realSummonerId, cellId };
    const reusable = candidates.find((e) => {
      // 仅已加载项可复用（loading 占位不可复用，必须走真实拉取）
      if (!e?.info || e.loading) return false;
      // 占位（空 puuid / cellId 兜底 sid）→真实身份必须强制重载
      const ePuuid = (e.info.puuid || "").trim();
      const eSid = e.info.summonerId || 0;
      if (incoming.puuid && !ePuuid) return false;
      if (incoming.summonerId && (!eSid || eSid === cellId)) return false;
      return isIdentityCompatible(e, incoming);
    });
    if (!reusable) return false;
    // 若处于选人阶段或有新的英雄变更，及时同步复用条目的 championId
    const newChampId = resolveCarryChampionId(fallbackPlayer, cellId);
    if (newChampId > 0 && reusable.championId !== newChampId) {
      reusable.championId = newChampId;
    }
    playerData.value[cellId] = reusable;
    if (realSummonerId) playerData.value[realSummonerId] = reusable;
    if (playerPuuid) playerData.value[playerPuuid] = reusable;

    // 如果复用的条目缺少熟练度（如旧版缓存或部分加载异常），后台静默异步补填
    if (
      (!reusable.masteries || reusable.masteries.length === 0) &&
      (playerPuuid || realSummonerId)
    ) {
      const targetPuuid = playerPuuid || reusable.info?.puuid;
      const targetSid = realSummonerId || reusable.info?.summonerId;
      const isMe =
        targetSid === currentSummonerId.value ||
        (!!targetPuuid && targetPuuid === currentSummonerPuuid.value);
      fetchPlayerMastery(targetPuuid, targetSid, isMe)
        .then((m) => {
          if (m && m.length > 0) {
            reusable.masteries = m;
            debouncedSavePlayerData();
          }
        })
        .catch(() => {
          /* ignore */
        });
    }
    // 若复用的是当前玩家，后台异步核对最新战绩并同步（仅在非游戏加载/进行阶段执行）
    const isMe =
      realSummonerId === currentSummonerId.value ||
      (!!playerPuuid && playerPuuid === currentSummonerPuuid.value);
    if (
      isMe &&
      store.gamePhase !== "GameStart" &&
      store.gamePhase !== "InProgress"
    ) {
      syncCurrentPlayerMatches();
    }
    return true;
  }

  /** 电脑玩家：按 LCU 官方标记构造本地占位，不发起任何网络请求 */
  function loadBotPlayer(
    cellId: number,
    realSummonerId: number,
    playerPuuid: string | undefined,
    fallbackPlayer: PremadePlayerLike,
  ): void {
    const botName =
      fallbackPlayer.displayName ||
      fallbackPlayer.summonerName ||
      fallbackPlayer.botName ||
      fallbackPlayer.gameName ||
      `电脑${cellId + 1}`;
    const botDataObj = createPlaceholderPlayerData({
      info: createSummonerDisplay({
        summonerId: realSummonerId,
        puuid: playerPuuid,
        displayName: botName,
        profileIconId: fallbackPlayer.profileIconId,
      }),
      loading: false,
      matchHistoryHidden: true,
      championId:
        fallbackPlayer.championId || fallbackPlayer.botChampionId || 0,
    });
    playerData.value[cellId] = botDataObj;
    if (realSummonerId) playerData.value[realSummonerId] = botDataObj;
    if (playerPuuid) playerData.value[playerPuuid] = botDataObj;
    debouncedSavePlayerData();
  }

  /**
   * 加载单个玩家的完整数据：先尝试复用，电脑玩家走本地占位，其余进入网络拉取。
   */
  async function loadPlayerData(
    cellId: number,
    summonerId: number,
    playerPuuid?: string,
    fallbackPlayer?: PremadePlayerLike,
  ) {
    if (!summonerId && !playerPuuid && !fallbackPlayer) return;

    // 防止 cellId 误传为 summonerId（如 0..9 的 cellId）
    const realSummonerId = summonerId && summonerId !== cellId ? summonerId : 0;

    if (
      tryReuseLoadedPlayer(
        cellId,
        summonerId,
        realSummonerId,
        playerPuuid,
        fallbackPlayer,
      )
    ) {
      return;
    }

    // 机器人/电脑玩家识别：严格依赖 LCU 官方标记（bot/isBot/isHumanoid/botChampionId）
    // 严禁通过名字模糊匹配（避免真实玩家 ID 包含“电脑”被误判）
    const isBotPlayer =
      Boolean(fallbackPlayer?.bot) ||
      Boolean(fallbackPlayer?.isBot) ||
      Boolean(
        (fallbackPlayer as PremadePlayerLike & { isHumanoid?: boolean })
          ?.isHumanoid,
      ) ||
      Boolean(fallbackPlayer?.botChampionId);

    if (isBotPlayer && fallbackPlayer) {
      loadBotPlayer(cellId, realSummonerId, playerPuuid, fallbackPlayer);
      return;
    }

    await fetchRealPlayerData(
      cellId,
      summonerId,
      realSummonerId,
      playerPuuid,
      fallbackPlayer,
    );
  }

  /** 真人玩家：拉取召唤师信息、战绩、排位与熟练度并写入 playerData */
  async function fetchRealPlayerData(
    cellId: number,
    summonerId: number,
    realSummonerId: number,
    playerPuuid: string | undefined,
    fallbackPlayer: PremadePlayerLike | undefined,
  ): Promise<void> {
    playerData.value[cellId] = {
      info: null,
      matches: [],
      ranked: { solo: null, flex: null },
      loading: true,
      // 拉取窗口内保留英雄，避免加载中头像空白（仅继承无身份占位的）
      championId: resolveCarryChampionId(fallbackPlayer, cellId),
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
      if (!info && realSummonerId) {
        const resp = await lcuRequest<SummonerDisplay>(
          "GET",
          `/lol-summoner/v1/summoners/${realSummonerId}`,
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
          (t) =>
            (t.cellId !== undefined && t.cellId === cellId) ||
            (t.summonerId && t.summonerId === realSummonerId),
        );
        const isChampSelectEnemyWaiting =
          store.gamePhase === "ChampSelect" &&
          isEnemy &&
          !playerPuuid &&
          !realSummonerId &&
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
            summonerId: realSummonerId || 0,
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
          // 只有真实召唤师通过 puuid/sid 拉取战绩被拒或返回空（且不是低级新号）时才属于战绩隐藏。
          // 脱敏占位仅代表召唤师信息尚未揭晓，绝不能预先标记为 matchHistoryHidden
          matchHistoryHidden = false;
        } else {
          const fallbackCid = resolveCarryChampionId(fallbackPlayer, cellId);
          console.warn(
            `[GamePlayerData] 无法获取召唤师信息: cellId=${cellId}, isChampSelectEnemyWaiting=${isChampSelectEnemyWaiting}, fallbackChampId=${fallbackCid}`,
          );
          playerData.value[cellId] = {
            info: null,
            matches: [],
            ranked: { solo: null, flex: null },
            loading: false,
            // 拉取失败也保留所选英雄，只展示头像与隐藏标识，不留空白列
            championId: fallbackCid,
          };
          return;
        }
      }

      const safeInfo = info;
      if (
        !safeInfo.profileIconUrl &&
        (safeInfo.profileIconId !== undefined ||
          safeInfo.profileIconId !== null)
      ) {
        safeInfo.profileIconUrl = `/lol-game-data/assets/v1/profile-icons/${safeInfo.profileIconId ?? 29}.jpg`;
      }

      const filterEnabled = appConfig.value?.Functions?.GameInfoFilter ?? false;
      const initialMatchesCount = 10;

      const [rawMatches, rankedResp, masteryData] = await Promise.all([
        safeInfo.puuid
          ? fetchMatchHistorySmart(safeInfo.puuid, 0, initialMatchesCount - 1)
              .then((res) => {
                if (!res || res.length === 0) {
                  // 对局中只要战绩返回空，即代表隐藏战绩（或被隐私设置拦截）
                  matchHistoryHidden = true;
                  writeFrontendLog(
                    "info",
                    "GameInfo:HiddenPlayer",
                    `[战绩隐藏] 玩家: name="${safeInfo.gameName || safeInfo.displayName}", puuid="${safeInfo.puuid}", sid=${safeInfo.summonerId}, lvl=${safeInfo.summonerLevel ?? 0}, cellId=${cellId}, champId=${fallbackPlayer?.championId || 0} - 战绩列表返回为空`,
                  );
                }
                return res || [];
              })
              .catch((e) => {
                matchHistoryHidden = true;
                writeFrontendLog(
                  "warn",
                  "GameInfo:HiddenPlayer",
                  `[战绩隐藏/拉取拒绝] 玩家: name="${safeInfo.gameName || safeInfo.displayName}", puuid="${safeInfo.puuid}", sid=${safeInfo.summonerId}, cellId=${cellId} - 请求异常: ${String(e)}`,
                );
                console.debug(
                  `[GameInfo] 战绩拉取失败/已隐藏 (puuid: ${safeInfo.puuid}):`,
                  e,
                );
                return [] as MatchDisplay[];
              })
          : Promise.resolve([] as MatchDisplay[]),
        safeInfo.puuid
          ? fetchRankedStatsCached(safeInfo.puuid)
          : Promise.resolve({ success: false as const }),
        fetchPlayerMastery(
          safeInfo.puuid,
          summonerId,
          summonerId === currentSummonerId.value ||
            (!!safeInfo.puuid && safeInfo.puuid === currentSummonerPuuid.value),
        ),
      ]);

      const isCurrentPlayer =
        summonerId === currentSummonerId.value ||
        (!!safeInfo.puuid && safeInfo.puuid === currentSummonerPuuid.value);

      if (isCurrentPlayer) {
        lastCurrentPlayerSyncTime = Date.now();
      }

      let matches: MatchDisplay[] = rawMatches;

      if (filterEnabled && currentQueueId.value !== null) {
        matches = matches.filter(
          (m: MatchDisplay) => m.queueId === currentQueueId.value,
        );
      }

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
      let streak: StreakInfo | null = null;
      let positions: AggregatedPositions | null = null;
      let topChampions: AggregatedChampion[] = [];

      if (matches && matches.length > 0) {
        const stats = computeMatchStats(matches);
        winCount = stats.winCount;
        lossesCount = stats.lossesCount;
        winRate = stats.winRate;
        avgKda = stats.avgKda;
        streak = stats.streak;
        positions = stats.positions;
        topChampions = stats.topChampions;
      }

      const targetChampId = resolveCarryChampionId(fallbackPlayer, cellId);
      // ：只要 privacy 存在且不是 PUBLIC（或者为 PRIVATE），即为私密
      const isProfilePrivate = Boolean(
        safeInfo.privacy && safeInfo.privacy.toUpperCase() !== "PUBLIC",
      );
      if (isProfilePrivate) {
        matchHistoryHidden = true;
        writeFrontendLog(
          "info",
          "GameInfo:PrivateProfile",
          `[生涯私密] 玩家: name="${safeInfo.gameName || safeInfo.displayName}", puuid="${safeInfo.puuid}", sid=${safeInfo.summonerId}, cellId=${cellId} - 客户端设置为生涯不公开`,
        );
      }

      const dataObj: PlayerData = {
        info: safeInfo,
        matches,
        ranked: { solo, flex },
        loading: false,
        matchHistoryHidden,
        isProfilePrivate,
        championId: targetChampId,
        avgKda,
        winRate,
        winCount,
        lossesCount,
        masteries: masteryData,
        masteryFetched: Boolean(masteryData && masteryData.length > 0),
        streak,
        positions,
        topChampions,
      };
      playerData.value[cellId] = dataObj;
      if (summonerId && summonerId !== cellId) {
        playerData.value[summonerId] = dataObj;
      }
      if (safeInfo.puuid) {
        playerData.value[safeInfo.puuid] = dataObj;
      }
      debouncedSavePlayerData();

      // 宿命检测异步补填：不阻塞玩家主数据上屏（列先渲染，宿命徽标就绪后响应式点亮）
      if (currentSummonerId.value && !isCurrentPlayer && safeInfo.puuid) {
        void backfillFateInfo(dataObj, safeInfo, matches);
      }

      // 第二阶段：后台静默拉取第 11~50 局（40 场），单任务排队执行，保证样本池与官方 teamPosition 权威性
      if (safeInfo.puuid && !matchHistoryHidden && !isProfilePrivate) {
        enqueueExtraMatches(async () => {
          try {
            const currentEntry = playerData.value[safeInfo.puuid];
            if (!currentEntry || toRaw(currentEntry) !== dataObj) return;

            // 优先从 LCU 本地缓存极速拉取；若本地战绩不足 20 场或失败，降级通过 SGP 补齐权威样本池
            let extraMatches = await fetchMatchHistory(safeInfo.puuid, 10, 49).catch(
              () => [] as MatchDisplay[],
            );
            if (!extraMatches || extraMatches.length < 20) {
              try {
                const sgpMatches = await fetchMatchHistorySgp(safeInfo.puuid, 10, 49);
                if (sgpMatches && sgpMatches.length > 0) {
                  // 将 SGP 结果与 LCU 结果取并集（SGP 覆盖重合对局以提供权威 teamPosition，并保留 LCU 其余对局）
                  const mergedExtraMap = new Map<number, MatchDisplay>();
                  for (const m of extraMatches || []) {
                    if (m.gameId) mergedExtraMap.set(m.gameId, m);
                  }
                  for (const m of sgpMatches) {
                    if (m.gameId) mergedExtraMap.set(m.gameId, m);
                  }
                  extraMatches = Array.from(mergedExtraMap.values());
                }
              } catch {
                /* ignore SGP fallback failure */
              }
            }
            if (!extraMatches || extraMatches.length === 0) return;

            let validExtra = extraMatches;
            if (filterEnabled && currentQueueId.value !== null) {
              validExtra = validExtra.filter(
                (m: MatchDisplay) => m.queueId === currentQueueId.value,
              );
            }
            // 将前 10 局与后 40 局合并去重构成完整样本池
            const combinedMap = new Map<number, MatchDisplay>();
            for (const m of matches) combinedMap.set(m.gameId, m);
            for (const m of validExtra) combinedMap.set(m.gameId, m);
            const allMatches = Array.from(combinedMap.values()).sort(
              (a, b) => b.timeStamp - a.timeStamp,
            );

            if (allMatches.length > matches.length) {
              const fullStats = computeMatchStats(allMatches);
              currentEntry.winCount = fullStats.winCount;
              currentEntry.lossesCount = fullStats.lossesCount;
              currentEntry.winRate = fullStats.winRate;
              currentEntry.avgKda = fullStats.avgKda;
              currentEntry.positions = fullStats.positions;
              currentEntry.topChampions = fullStats.topChampions;
              debouncedSavePlayerData();
            }
          } catch {
            /* ignore background extra fetch failure */
          }
        });
      }
    } catch (err) {
      const existingInfo =
        playerData.value[cellId]?.info ||
        (summonerId ? playerData.value[summonerId]?.info : undefined) ||
        (playerPuuid ? playerData.value[playerPuuid]?.info : undefined);
      const errCid = resolveCarryChampionId(fallbackPlayer, cellId);
      console.warn(
        `[GamePlayerData] loadPlayerData 发生异常兜底: cellId=${cellId}, name=${fallbackPlayer?.displayName || fallbackPlayer?.gameName}, champId=${errCid}, err=`,
        err,
      );
      const dataObj: PlayerData = {
        info: existingInfo || null,
        matches: [],
        ranked: { solo: null, flex: null },
        loading: false,
        matchHistoryHidden: true,
        // 异常兜底也保留所选英雄，不留空白列
        championId: errCid,
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

  // 宿命信息异步补填：交集快速剪枝，彻底避免无意义的重型对局详情网络 I/O 阻塞信号量
  async function backfillFateInfo(
    dataObj: PlayerData,
    safeInfo: SummonerDisplay,
    matches: MatchDisplay[],
  ) {
    try {
      const targetGids: number[] = [];
      for (const m of matches.slice(0, 5)) {
        if (m.gameId && !targetGids.includes(m.gameId)) {
          targetGids.push(m.gameId);
        }
      }

      let targetSaved: SavedPlayerMarker | undefined;
      const savedEncounteredIds = new Set<number>();
      if (currentSummonerPuuid.value) {
        try {
          const savedMap = await getSavedPlayersMapCached();
          targetSaved = savedMap[safeInfo.puuid];
          if (targetSaved?.encounteredGameIds && targetSaved.encounteredGameIds.length > 0) {
            for (const gid of targetSaved.encounteredGameIds) {
              savedEncounteredIds.add(gid);
            }
          } else if (targetSaved?.lastEncounteredGameId) {
            savedEncounteredIds.add(targetSaved.lastEncounteredGameId);
          }
        } catch {
          // ignore saved map lookup failure
        }
      }

      // 候选对局 ID 列表：目标玩家近 5 场对局与本地路人集相遇记录合并去重
      // 注：不可仅依据当前玩家首屏（仅 10 局且受模式过滤）做交集剪枝，
      // 避免当共同对局在当前玩家 10 场之前或跨模式时被静默丢弃导致漏检
      const candidateIds: number[] = [];
      for (const gid of targetGids) {
        if (!candidateIds.includes(gid)) candidateIds.push(gid);
      }
      for (const gid of savedEncounteredIds) {
        if (!candidateIds.includes(gid)) candidateIds.push(gid);
      }

      if (candidateIds.length === 0) return;

      const fateInfo = await fetchPlayerFateInfo(
        candidateIds,
        safeInfo.puuid,
        currentSummonerId.value,
      );
      if (fateInfo && fateInfo.fateFlag) {
        // dataObj 已入 ref 但 ref 不包装写入值：必须经 playerData 读出的代理写入才能触发响应式；
        // toRaw 归属校验防止补填期间条目被重载/重置后误写旧数据
        const entry = playerData.value[safeInfo.puuid];
        if (!entry || toRaw(entry) !== dataObj) return;
        entry.fateFlag = fateInfo.fateFlag;
        entry.recentlyChampionName = fateInfo.recentlyChampionName || "";
        entry.fateAllyCount = fateInfo.allyCount ?? 0;
        entry.fateEnemyCount = fateInfo.enemyCount ?? 0;
        entry.fateGameCreation = fateInfo.gameCreation ?? undefined;
        // 如果命中的对局不是 matches[0]，或者是好几局之前的，则标记为非最上一局
        if (
          matches[0] &&
          fateInfo.gameId &&
          fateInfo.gameId !== matches[0].gameId
        ) {
          entry.fateIsLastGame = false;
        }
        if (!entry.fateGameCreation && fateInfo.gameId) {
          const matched = matches.find((m) => m.gameId === fateInfo.gameId);
          if (matched?.timeStamp) {
            entry.fateGameCreation = matched.timeStamp;
          }
        }
        if (!entry.fateGameCreation && targetSaved?.lastMetAt) {
          entry.fateGameCreation = targetSaved.lastMetAt;
        }
        debouncedSavePlayerData();
      }
    } catch (e) {
      console.error("宿命检测失败:", e);
    }
  }

  // 辅助函数：将当前玩家排在有序队列首位，使自身战绩优先就绪，保证宿命与基础信息优先展示
  const sortWithCurrentFirst = (players: PremadePlayerLike[]) => {
    const myPuuid = currentSummonerPuuid.value;
    const mySid = currentSummonerId.value;
    return [...players].sort((a, b) => {
      const aIsMe =
        (myPuuid && a.puuid === myPuuid) || (mySid && a.summonerId === mySid);
      const bIsMe =
        (myPuuid && b.puuid === myPuuid) || (mySid && b.summonerId === mySid);
      if (aIsMe) return -1;
      if (bIsMe) return 1;
      return 0;
    });
  };

  const MIN_API_CONCURRENCY = 1;
  const MAX_API_CONCURRENCY = 10;
  const DEFAULT_API_CONCURRENCY = 5;

  // 辅助函数：根据用户配置动态获取并发限制（1~10 之间，默认 5，与设置项及后端信号量保持一致）
  const getConcurrencyLimit = () => {
    const cfg = appConfig.value?.Functions?.ApiConcurrencyNumber;
    if (typeof cfg === "number" && !isNaN(cfg)) {
      return Math.min(MAX_API_CONCURRENCY, Math.max(MIN_API_CONCURRENCY, cfg));
    }
    return DEFAULT_API_CONCURRENCY;
  };

  async function loadAllPlayers() {
    const my = myTeam.value;
    const their = theirTeam.value;
    if (my.length === 0 && their.length === 0) return;
    await updateCurrentQueueId();

    const filterValidPlayers = (
      team: PremadePlayerLike[],
      isEnemy: boolean,
    ) => {
      if (store.gamePhase === "ChampSelect" && isEnemy) {
        // 选人阶段敌方队伍若无有效身份且非人机，不请求
        //（敌方人机含原生 isHumanoid 标记，同样放行走 bot 本地占位）
        return team.filter((p) =>
          Boolean(
            p.puuid ||
            p.summonerId ||
            p.bot ||
            p.isBot ||
            p.botChampionId ||
            (p as PremadePlayerLike & { isHumanoid?: boolean }).isHumanoid ||
            p.displayName ||
            p.summonerName,
          ),
        );
      }
      return team;
    };

    const isMyVisible = activeTab.value === "my";
    // 优先可见队，后台队排在随后；对整体应用 sortWithCurrentFirst 保证无论在哪个 Tab 当前玩家始终最优先
    const visible = filterValidPlayers(isMyVisible ? my : their, !isMyVisible);
    const background = filterValidPlayers(
      isMyVisible ? their : my,
      isMyVisible,
    );

    const ordered = sortWithCurrentFirst([...visible, ...background]);
    const limit = getConcurrencyLimit();

    await runWithConcurrency(ordered, limit, (p) => {
      const cid = p.cellId ?? p.summonerId ?? 0;
      const sid = p.summonerId ?? 0;
      if (
        cid !== undefined &&
        (sid || p.puuid || p.displayName || p.summonerName)
      ) {
        return loadPlayerData(cid, sid, p.puuid, p);
      }
      return Promise.resolve();
    });
    writeReserveData();
    void processExtraMatchesQueue();
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

    const buildCtx = (): TeamBuildContext => ({
      currentSummonerId: currentSummonerId.value,
      currentSummonerPuuid: currentSummonerPuuid.value,
      mapSide: store.mapSide,
      champSelectTeamSnapshot: champSelectTeamSnapshot.value,
      champSelectTheirTeamSnapshot: champSelectTheirTeamSnapshot.value,
      sessionAllyTeam: sessionAllyTeam.value,
      sessionEnemyTeam: sessionEnemyTeam.value,
      gameflowMyTeam: gameflowMyTeam.value,
      gameflowTheirTeam: gameflowTheirTeam.value,
      playerData: playerData.value,
    });

    const { allyTeam, enemyTeam, side } = await resolveTeamSide(
      teamOne,
      teamTwo,
      buildCtx(),
    );
    if (side) store.setMapSide(side);

    if (gameflowMyTeam.value.length > 0) {
      rekeyExistingPlayerData(gameflowMyTeam.value, playerData.value);
    }

    const { mappedMy, mappedTheir } = buildGameflowTeams(
      allyTeam,
      enemyTeam,
      store.mapSide,
      buildCtx(),
    );
    // LCU 会话偶发残缺（如加载瞬间有人尚未连入）：新队伍更短时保留已有更全的，
    // 不让整列消失；人数补齐后由 gameflowSession 监听触发重处理
    if (mappedMy.length >= gameflowMyTeam.value.length) {
      gameflowMyTeam.value = mappedMy;
    } else {
      console.debug(
        `[GameInfo] 忽略残缺队伍快照: 我方新 ${mappedMy.length} 人 / 已有 ${gameflowMyTeam.value.length} 人`,
      );
    }
    if (mappedTheir.length >= gameflowTheirTeam.value.length) {
      gameflowTheirTeam.value = mappedTheir;
    } else {
      console.debug(
        `[GameInfo] 忽略残缺队伍快照: 敌方新 ${mappedTheir.length} 人 / 已有 ${gameflowTheirTeam.value.length} 人`,
      );
    }

    // 为双方队员预填充基础占位，避免后台异步请求完成前界面出现空白
    seedInitialPlayerData(gameflowMyTeam.value, playerData.value);
    seedInitialPlayerData(gameflowTheirTeam.value, playerData.value);

    premadeColorsMy.value = resolvePremadeColors(
      gameflowMyTeam.value,
      premadeColorsMy.value,
    );
    premadeColorsTheir.value = resolvePremadeColors(
      gameflowTheirTeam.value,
      premadeColorsTheir.value,
    );

    // 无论当前 activeTab 是哪一队，按“可见队优先、后台队随后”排列进入全局受控并发队列
    // 并将当前玩家置顶优先加载，并发度联动配置项，彻底杜绝双队抢占 LCU 信号量
    const visible =
      activeTab.value === "my" ? gameflowMyTeam.value : gameflowTheirTeam.value;
    const background =
      activeTab.value === "my" ? gameflowTheirTeam.value : gameflowMyTeam.value;

    const orderedPlayers = sortWithCurrentFirst([...visible, ...background]);
    const limit = getConcurrencyLimit();
    runWithConcurrency(orderedPlayers, limit, (p) =>
      loadPlayerData(p.cellId ?? 0, p.summonerId ?? 0, p.puuid, p),
    )
      .then(() => {
        // 双方 10 人信息加载完毕，立即保存完整对局
        writeReserveData();
        // 在游戏加载/进行阶段，尝试通过 LiveClientData 补充可能缺失的英雄头像
        if (
          store.gamePhase === "GameStart" ||
          store.gamePhase === "InProgress"
        ) {
          pollLiveClientChampions();
        }
      })
      .catch((err) => {
        console.debug("[GameInfo] 队伍数据加载异常:", err);
        // 异常兜底，只要队伍齐备也保存
        writeReserveData();
        if (
          store.gamePhase === "GameStart" ||
          store.gamePhase === "InProgress"
        ) {
          pollLiveClientChampions();
        }
      })
      .finally(() => {
        void processExtraMatchesQueue();
      });
  }


  async function loadFromGameflowSession() {
    loading.value = true;
    error.value = "";

    const reqId = ++currentGameflowSessionRequestId;

    clearSessionCache();
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
      const liveGameId = data.gameData.gameId ?? null;
      const isCustomGame = data.gameData.queue?.isCustom === true;
      if (liveGameId) {
        const savedId = Number(localStorage.getItem("yuumi_last_game_id")) || 0;
        // 检测到新真实 gameId（与之前保存的 savedId 或已记录的 currentGameId 不一致）：
        // 若经历过本局选人阶段（champSelectTeamSnapshot 有队员），我方 5 人数据已在选人阶段完整就绪，严禁清空！
        // 仅在未经历选人阶段（直连进游戏、大乱斗或游戏中途中断重连）且 gameId 变化时，才清空上一局残留
        const hasChampSelectSnapshot = champSelectTeamSnapshot.value.length > 0;
        const isNewGame =
          !hasChampSelectSnapshot &&
          ((savedId > 0 && savedId !== liveGameId) ||
            (currentGameId.value !== null && currentGameId.value !== liveGameId));
        if (isNewGame) {
          console.debug(
            `[GamePlayerData] 检测到全新对局 gameId=${liveGameId} (上一局 savedId=${savedId}, curId=${currentGameId.value})，彻底清空上一局数据`,
          );
          playerData.value = {};
          gameflowMyTeam.value = [];
          gameflowTheirTeam.value = [];
          clearReserveDataFromStorage();
        }
        currentGameId.value = liveGameId;
      }
      // InProgress 阶段 LCU 会把 gameflow 队伍精简（甚至只剩我方或单边）：
      // 当落盘有同 gameId 的完整快照，且当前队伍为空或当前队伍少于落盘人数或当前 session 少于落盘人数时，
      // 先从快照恢复整局数据，后续逻辑中的长度守卫与 identity 合并会保留更全的数据，防止敌方丢失
      const sessionTotal = t1.length + t2.length;
      if (liveGameId && !isCustomGame) {
        try {
          const savedId =
            Number(localStorage.getItem("yuumi_last_game_id")) || 0;
          const savedTotal =
            Number(localStorage.getItem("yuumi_last_game_team_count")) || 0;
          const currentTotal =
            gameflowMyTeam.value.length + gameflowTheirTeam.value.length;
          if (
            savedId > 0 &&
            liveGameId > 0 &&
            savedId === liveGameId &&
            (currentTotal === 0 ||
              gameflowTheirTeam.value.length === 0 ||
              (savedTotal > 0 &&
                (sessionTotal < savedTotal || currentTotal < savedTotal)))
          ) {
            restoreReserveDataFromLocalStorage();
          }
        } catch {
          /* ignore */
        }
      }
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
          clearSessionCache();
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
        // 自定义对局（人机游戏）teamTwo 可能永远为空，无需重试，直接处理。
        // 人机位于我方（myTeam 快照），敌方快照的人机位同样计入；queue.isCustom 缺失时以此兜底
        const snapshotHasBots =
          champSelectTheirTeamSnapshot.value.some((p) => p.bot || p.isBot) ||
          champSelectTeamSnapshot.value.some((p) => p.bot || p.isBot);
        if (isCustomGame || snapshotHasBots) {
          if (reqId !== currentGameflowSessionRequestId) return;
          await processTeamData(t1, t2);
          loading.value = false;
          return;
        }
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
          clearSessionCache();
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

  // ── 通过 Live Client Data (2999端口) 精准补全英雄头像（特别针对敌方隐藏战绩玩家）──
  let liveClientPollTimer: ReturnType<typeof setTimeout> | null = null;
  let isPollingLiveClient = false;
  const queriedLiveClientNames = new Set<string>();

  async function pollLiveClientChampions(retryCount = 0) {
    if (isPollingLiveClient && retryCount === 0) return;
    if (store.gamePhase !== "GameStart" && store.gamePhase !== "InProgress")
      return;

    // 检查是否还有任何玩家（尤其隐藏战绩或敌方）英雄缺失
    const allPlayers = [...gameflowMyTeam.value, ...gameflowTheirTeam.value];
    const missingHeroPlayers = allPlayers.filter((p) => {
      const cid = p.championId || p.botChampionId || 0;
      const key = p.cellId ?? 0;
      const pd =
        (p.puuid ? playerData.value[p.puuid] : undefined) ||
        (p.summonerId ? playerData.value[p.summonerId] : undefined) ||
        playerData.value[key];
      const pdCid = pd?.championId || 0;
      return cid <= 0 || pdCid <= 0;
    });

    // 如果都有英雄了且已经轮询过，就不再轮询
    if (
      allPlayers.length > 0 &&
      missingHeroPlayers.length === 0 &&
      retryCount > 0
    ) {
      return;
    }

    isPollingLiveClient = true;
    try {
      const clientList = await getLiveClientPlayerList();
      if (Array.isArray(clientList) && clientList.length > 0) {
        console.debug(
          `[GamePlayerData] 成功拉取 LiveClientData 玩家列表: ${clientList.length} 人`,
        );
        const champList = await fetchChampions();

        let hasUpdated = false;

        // 规范化名字比较辅助工具：去掉 #tag、空格及不可见字符，转小写
        const normalizeName = (name?: string) => {
          if (!name) return "";
          return name.split("#")[0].replace(/\s+/g, "").toLowerCase();
        };

        // 1. 判断我方在 LiveClientData 中的 team 阵营（"ORDER" 为蓝方，"CHAOS" 为红方）
        let myLiveClientTeam: string | null = null;

        // 1.1 优先级 1（最可信）：若 store.mapSide 已经锁定，直接采用
        if (store.mapSide === "blue") {
          myLiveClientTeam = "ORDER";
        } else if (store.mapSide === "red") {
          myLiveClientTeam = "CHAOS";
        }

        // 1.2 优先级 2：通过我方已知【实名】队员（严格排除纯英雄名、称号、占位名）在 clientList 中匹配
        if (!myLiveClientTeam) {
          const allyRealPlayers = gameflowMyTeam.value.filter((p) => {
            const rawName = p.gameName || p.displayName || p.summonerName;
            const hasIdentity = Boolean(
              p.puuid || p.tagLine || (p.summonerId && p.summonerId !== p.cellId),
            );
            const isBot = Boolean(p.bot || p.isBot);
            return isRealSummonerName(rawName, champList, { hasIdentity, isBot });
          });

          for (const lp of clientList) {
            const lpNames = [
              normalizeName(lp.riotIdGameName),
              normalizeName(lp.summonerName),
              normalizeName(lp.riotId),
            ].filter(Boolean);
            const isMyAlly = allyRealPlayers.some((p) => {
              const pNames = [
                normalizeName(p.gameName),
                normalizeName(p.displayName),
                normalizeName(p.summonerName),
              ].filter(Boolean);
              return pNames.some((pn) => lpNames.some((ln) => pn === ln));
            });
            if (isMyAlly && lp.team) {
              myLiveClientTeam = lp.team;
              console.debug(
                `[GamePlayerData] 通过我方实名队友匹配锁定阵营: ${myLiveClientTeam}`,
              );
              break;
            }
          }
        }

        // 1.3 优先级 3：尝试通过 2999 端口 activeplayername 获取当前本机操作的玩家名字
        // 【关键防御】：严禁当 activeplayername 为纯英雄中文名/称号/别名或占位名时推导！
        // 且匹配到的 LiveClient 玩家阵营必须唯一（无同名跨阵营歧义，如盲选双提莫）
        if (!myLiveClientTeam) {
          try {
            const activeName = await getLiveClientActivePlayerName();
            const hasIdentity = Boolean(store.currentSummoner?.puuid);
            if (activeName && isRealSummonerName(activeName, champList, { hasIdentity })) {
              const activeNorm = normalizeName(activeName);
              const matchedPlayers = clientList.filter((lp) => {
                const lpNames = [
                  normalizeName(lp.riotIdGameName),
                  normalizeName(lp.summonerName),
                  normalizeName(lp.riotId),
                ].filter(Boolean);
                return lpNames.some((ln) => ln === activeNorm);
              });
              const uniqueTeams = new Set(
                matchedPlayers.map((p) => p.team).filter(Boolean),
              );
              if (uniqueTeams.size === 1 && matchedPlayers[0]?.team) {
                myLiveClientTeam = matchedPlayers[0].team;
                console.debug(
                  `[GamePlayerData] 通过实名 activeplayername (${activeName}) 锁定我方阵营: ${myLiveClientTeam}`,
                );
              } else if (uniqueTeams.size > 1) {
                console.warn(
                  `[GamePlayerData] activeplayername (${activeName}) 命中多个不同阵营玩家，跳过推导`,
                );
              }
            } else if (activeName) {
              console.debug(
                `[GamePlayerData] activeplayername (${activeName}) 为脱敏英雄名/占位名，跳过阵营推导`,
              );
            }
          } catch {
            /* ignore */
          }
        }

        // 1.4 优先级 4：兜底通过 LCU /liveclientdata 阵营识别接口
        if (!myLiveClientTeam) {
          try {
            const side = await getMapSide();
            if (side === "blue" || side === "red") {
              myLiveClientTeam = side === "blue" ? "ORDER" : "CHAOS";
              store.setMapSide(side);
            }
          } catch {
            /* ignore */
          }
        }

        console.debug(
          `[GamePlayerData] 推导 LiveClient 我方阵营: ${myLiveClientTeam || "未推导成功"}`,
        );

        // 1.1 若确定了我方真实阵营且 store.mapSide 尚未设定，同步至 store
        if (myLiveClientTeam) {
          const realSide =
            myLiveClientTeam.toUpperCase() === "ORDER" ? "blue" : "red";
          if (!store.mapSide) {
            store.setMapSide(realSide);
          }
        }

        // 2. 将 LiveClient 玩家根据阵营划分为我方列表和敌方列表
        const allyLiveClients = clientList.filter((lp) =>
          myLiveClientTeam ? lp.team === myLiveClientTeam : false,
        );
        const enemyLiveClients = clientList.filter((lp) =>
          myLiveClientTeam ? lp.team !== myLiveClientTeam : false,
        );

        console.debug(
          `[GamePlayerData] LiveClient 我方: ${allyLiveClients.length} 人, 敌方: ${enemyLiveClients.length} 人`,
        );

        // 已被匹配绑定的槽位集合（使用 cellId 或 puuid 作为 key），避免多个 LiveClient 玩家覆盖同一槽位
        const matchedTargetKeys = new Set<string | number>();

        // 3. 处理每个 LiveClient 玩家
        for (const lp of clientList) {
          if (!lp.championName && !lp.rawChampionName) continue;

          const champId = findChampionIdByLiveClientName(
            champList,
            lp.championName,
            lp.rawChampionName,
          );

          if (champId <= 0) {
            console.debug(
              `[GamePlayerData] LiveClient 英雄未能匹配到 ID: champName=${lp.championName}, raw=${lp.rawChampionName}`,
            );
            continue;
          }

          const lpCleanNames = [
            normalizeName(lp.riotIdGameName),
            normalizeName(lp.summonerName),
            normalizeName(lp.riotId),
          ].filter(Boolean);

          // 严格按 LiveClient 阵营隔离：敌方仅在 gameflowTheirTeam 匹配/回填，我方仅在 gameflowMyTeam 匹配/回填
          const isEnemyLp = myLiveClientTeam
            ? lp.team !== myLiveClientTeam
            : false;
          const candidateTeam = isEnemyLp
            ? gameflowTheirTeam.value
            : gameflowMyTeam.value;

          // 1) 优先进行严格全等名字匹配（双方均须为真实召唤师名称，排除脱敏英雄名与占位名误撞）
          let targetPlayer = candidateTeam.find((p) => {
            const rawPName = p.gameName || p.displayName || p.summonerName;
            const hasIdentity = Boolean(
              p.puuid || p.tagLine || (p.summonerId && p.summonerId !== p.cellId),
            );
            const isBot = Boolean(p.bot || p.isBot);
            if (!isRealSummonerName(rawPName, champList, { hasIdentity, isBot })) return false;
            const pCleanNames = [
              normalizeName(p.gameName),
              normalizeName(p.displayName),
              normalizeName(p.summonerName),
            ].filter(Boolean);
            return pCleanNames.some((pn) =>
              lpCleanNames.some((ln) => pn === ln),
            );
          });

          // 2) 若名字未匹配，且已有确定英雄：尝试通过 championId 匹配已有该英雄的槽位
          if (!targetPlayer) {
            targetPlayer = candidateTeam.find((p) => {
              const targetKey = p.puuid || p.cellId;
              if (targetKey !== undefined && matchedTargetKeys.has(targetKey))
                return false;
              return p.championId === champId;
            });
          }

          // 3) 仅对【敌方队伍】（在排位等模式通常完全脱敏为“玩家1”~“玩家5”），且名字和英雄都未匹配时，
          // 且该敌方槽位尚未绑定其他英雄或玩家时，才考虑按未绑定的槽位有序回填
          const realName = lp.riotIdGameName || lp.summonerName || lp.riotId;
          const lpHasIdentity = Boolean(
            lp.riotIdTagLine || (realName && realName.includes("#")),
          );
          const lpIsBot = Boolean(lp.isBot);
          const isRealName = isRealSummonerName(realName, champList, {
            hasIdentity: lpHasIdentity,
            isBot: lpIsBot,
          });
          if (!targetPlayer && isEnemyLp) {
            const enemyLiveGroup = enemyLiveClients;
            const lpIndexInGroup = enemyLiveGroup.indexOf(lp);

            if (lpIndexInGroup >= 0 && lpIndexInGroup < candidateTeam.length) {
              const candidateSlot = candidateTeam[lpIndexInGroup];
              const slotKey = candidateSlot.puuid || candidateSlot.cellId;
              // 确保该槽位未被占用，且该槽位此前没有绑定不同的有效英雄
              if (
                (slotKey === undefined || !matchedTargetKeys.has(slotKey)) &&
                (!candidateSlot.championId ||
                  candidateSlot.championId <= 0 ||
                  candidateSlot.championId === champId)
              ) {
                targetPlayer = candidateSlot;
                // 仅当 LiveClient 返回的是真实实名时才覆写名字，脱敏英雄名/占位名严禁覆写！
                if (isRealName && realName && targetPlayer.displayName !== realName) {
                  targetPlayer.gameName = realName;
                  targetPlayer.displayName = realName;
                }
              }
            }
          }

          if (targetPlayer) {
            const targetKey = targetPlayer.puuid || targetPlayer.cellId;
            if (targetKey !== undefined) {
              matchedTargetKeys.add(targetKey);
            }

            const defaultEnemyOffset = store.mapSide === "red" ? 0 : 5;
            const cellKey =
              targetPlayer.cellId ??
              (isEnemyLp
                ? defaultEnemyOffset
                : defaultEnemyOffset === 0
                  ? 5
                  : 0);
            const pd =
              (targetPlayer.puuid
                ? playerData.value[targetPlayer.puuid]
                : undefined) ||
              (targetPlayer.summonerId
                ? playerData.value[targetPlayer.summonerId]
                : undefined) ||
              playerData.value[cellKey];

            console.debug(
              `[GamePlayerData] 成功匹配 LiveClient 玩家 -> 槽位目标: name=${targetPlayer.gameName || targetPlayer.displayName}, cellId=${targetPlayer.cellId}, champId=${champId}, champName=${lp.championName}, team=${lp.team}`,
            );

            if (pd && pd.championId !== champId) {
              pd.championId = champId;
              hasUpdated = true;
            }
            if (targetPlayer.championId !== champId) {
              targetPlayer.championId = champId;
              hasUpdated = true;
            }
            if (targetPlayer.puuid && playerData.value[targetPlayer.puuid]) {
              playerData.value[targetPlayer.puuid].championId = champId;
            }
            // 回填真实姓名并对原本无真实战绩的玩家触发重新拉取
            // 【关键防御】：
            // 1. realName 必须是真实实名（isRealName 为 true），脱敏英雄名（如“迅捷斥候”）与占位名严禁覆写！
            // 2. 已有 puuid 的实名玩家严禁被覆盖！
            // 3. 严禁用英雄名发起召唤师查询！
            if (
              isRealName &&
              realName &&
              !targetPlayer.puuid &&
              !pd?.info?.puuid
            ) {
              writeFrontendLog(
                "info",
                "GameInfo:LiveClientName",
                `LiveClient 回填实名: 原名="${targetPlayer.gameName || pd?.info?.gameName}", 回填真实名="${realName}", cellId=${targetPlayer.cellId}, champ=${lp.championName}`,
              );
              targetPlayer.gameName = realName;
              targetPlayer.displayName = realName;
              if (pd?.info) {
                pd.info.gameName = realName;
                pd.info.displayName = realName;
              }
              hasUpdated = true;

              // 若该槽位此前缺少 puuid/战绩数据，异步通过真实名字查询召唤师并拉取战绩（去重防止多次轮询并发重复请求）
              if (
                (!targetPlayer.puuid ||
                  !pd?.info?.puuid ||
                  pd.matches.length === 0) &&
                !queriedLiveClientNames.has(realName)
              ) {
                queriedLiveClientNames.add(realName);
                (async () => {
                  try {
                    let realSummoner: SummonerDisplay | null = null;
                    // 1. 若含有 # 或具备 riotIdTagLine，优先使用 RiotID alias/lookup 查找
                    if (realName.includes("#") || lp.riotIdTagLine) {
                      const hashIdx = realName.indexOf("#");
                      const gName =
                        hashIdx >= 0
                          ? realName.slice(0, hashIdx)
                          : lp.riotIdGameName || realName;
                      const tLine =
                        hashIdx >= 0
                          ? realName.slice(hashIdx + 1)
                          : lp.riotIdTagLine || "";
                      if (gName && tLine) {
                        const aliasResp = await lcuRequest<SummonerDisplay>(
                          "GET",
                          `/lol-summoner/v1/alias/lookup?gameName=${encodeURIComponent(gName)}&tagLine=${encodeURIComponent(tLine)}`,
                        );
                        if (aliasResp.success && aliasResp.data) {
                          realSummoner = aliasResp.data;
                        }
                      }
                    }

                    // 2. 尝试常规 summoners?name 查找
                    if (!realSummoner) {
                      const lookupResp = await lcuRequest<SummonerDisplay>(
                        "GET",
                        `/lol-summoner/v1/summoners?name=${encodeURIComponent(realName)}`,
                      );
                      if (lookupResp.success && lookupResp.data) {
                        realSummoner = lookupResp.data;
                      }
                    }

                    // 3. 若纯名字查询失败，尝试去除 tag 或以纯 gameName 结合当前大区尝试
                    if (!realSummoner && realName.includes("#")) {
                      const pureName = realName.split("#")[0];
                      const pureResp = await lcuRequest<SummonerDisplay>(
                        "GET",
                        `/lol-summoner/v1/summoners?name=${encodeURIComponent(pureName)}`,
                      );
                      if (pureResp.success && pureResp.data) {
                        realSummoner = pureResp.data;
                      }
                    }

                    if (realSummoner) {
                      // 若查到的 realSummoner 缺少 privacy 字段，通过 puuid 补全完整召唤师信息
                      if (!realSummoner.privacy && realSummoner.puuid) {
                        try {
                          const fullResp = await lcuRequest<SummonerDisplay>(
                            "GET",
                            `/lol-summoner/v2/summoners/puuid/${realSummoner.puuid}`,
                          );
                          if (fullResp.success && fullResp.data) {
                            realSummoner = fullResp.data;
                          }
                        } catch {
                          /* ignore */
                        }
                      }
                      targetPlayer.summonerId = realSummoner.summonerId;
                      targetPlayer.puuid = realSummoner.puuid;
                      targetPlayer.gameName =
                        realSummoner.gameName ||
                        realSummoner.displayName ||
                        realName;
                      targetPlayer.displayName =
                        realSummoner.displayName || realName;
                      targetPlayer.tagLine = realSummoner.tagLine;
                      targetPlayer.profileIconId = realSummoner.profileIconId;

                      await loadPlayerData(
                        cellKey,
                        realSummoner.summonerId,
                        realSummoner.puuid,
                        targetPlayer,
                      );
                      writeReserveData();
                    }
                  } catch (lookupErr) {
                    queriedLiveClientNames.delete(realName);
                    console.debug(
                      "[GamePlayerData] 查询 LiveClient 召唤师身份失败:",
                      lookupErr,
                    );
                  }
                })();
              }
            }
          } else {
            console.debug(
              `[GamePlayerData] LiveClient 玩家未能对齐队伍: lpSummoner=${lp.summonerName}, lpRiotId=${lp.riotIdGameName}, lpChamp=${lp.championName}, team=${lp.team}`,
            );
          }
        }

        if (hasUpdated) {
          writeReserveData();
        }
      }
    } catch (e) {
      // 游戏还在加载中时 2999 端口可能尚未就绪，正常重试
      console.debug(`[GamePlayerData] LiveClient 端口暂未就绪或请求失败:`, e);
    } finally {
      isPollingLiveClient = false;
    }

    // 若依然在游戏加载/进行阶段且次数未超限（最多重试 15 次，约 30 秒），继续轮询尝试
    if (
      (store.gamePhase === "GameStart" || store.gamePhase === "InProgress") &&
      retryCount < 15
    ) {
      if (liveClientPollTimer) clearTimeout(liveClientPollTimer);
      liveClientPollTimer = setTimeout(() => {
        pollLiveClientChampions(retryCount + 1);
      }, 2000);
    }
  }

  // 团队内容签名：成员 cellId + 英雄 ID（包含锁定、预选及 actions 挑选）。session 高频事件中仅倒计时变化时签名不变，跳过无效重载
  // 必须在下方带 immediate 的 phase watcher 之前声明（该回调会同步重置此变量）
  let lastSessionTeamSig = "";
  let lastSessionRosterSig = "";

  // 对局结束延时等待池与会话版本：清理时必须先 resolve 再 clearTimeout。
  // 仅 clearTimeout 会让 `await new Promise(resolve => setTimeout(resolve, delay))` 永不返回，
  // 等待中的协程连同其闭包引用将永久挂起泄漏。
  const gameEndedWaits = new Set<() => void>();
  let currentGameEndedSessionId = 0;

  function waitGameEndedDelay(delay: number): Promise<void> {
    return new Promise<void>((resolve) => {
      const cancel = () => {
        clearTimeout(timer);
        gameEndedWaits.delete(cancel);
        resolve();
      };
      const timer = setTimeout(() => {
        gameEndedWaits.delete(cancel);
        resolve();
      }, delay);
      gameEndedWaits.add(cancel);
    });
  }

  function clearGameEndedTimers() {
    currentGameEndedSessionId++;
    for (const cancel of [...gameEndedWaits]) {
      cancel();
    }
  }

  // 监听 Watchers
  watch(isGameActive, (active) => {
    if (!active) {
      clearGameEndedTimers();
      // 离开游戏活跃状态（回到 Lobby / EndOfGame 等）：
      const hasPlayerData =
        gameflowMyTeam.value.length > 0 &&
        Object.keys(playerData.value).length > 0;
      if (hasPlayerData) {
        // 内存中已有刚打完的对局，确保落盘
        writeReserveData();
      } else if (appConfig.value?.Functions?.EnableReserveGameinfo) {
        // 否则（如中途启动或选人秒退离开），仅在配置开启时尝试从 localStorage 恢复上一次对局快照
        restoreReserveDataFromLocalStorage();
      }
      champSelectTeamSnapshot.value = [];
      champSelectTheirTeamSnapshot.value = [];
      lastSessionTeamSig = "";
      lastSessionRosterSig = "";
    } else {
      champSelectTeamSnapshot.value = [];
      champSelectTheirTeamSnapshot.value = [];
      lastSessionTeamSig = "";
      lastSessionRosterSig = "";
      // 刚进入选人阶段时，清空当前内存视图与快照以展示当前新选人
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
        queriedLiveClientNames.clear();
        if (liveClientPollTimer) {
          clearTimeout(liveClientPollTimer);
          liveClientPollTimer = null;
        }
      }
      if (phase === "ChampSelect" || phase === "None") {
        currentGameId.value = null;
        clearSessionCache();
        gameflowMyTeam.value = [];
        gameflowTheirTeam.value = [];
        champSelectTeamSnapshot.value = [];
        champSelectTheirTeamSnapshot.value = [];
        sessionAllyTeam.value = [];
        sessionEnemyTeam.value = [];
        lastSessionTeamSig = "";
        lastSessionRosterSig = "";
        playerData.value = {};
        premadeColorsMy.value = {};
        premadeColorsTheir.value = {};
        if (phase === "ChampSelect") {
          // 选人阶段不立即清空 localStorage，避免秒退导致已有完整对局丢失
          refreshState();
        }
      }
      if (phase === "InProgress" || phase === "GameStart") {
        // 未经历本局选人快照过渡（例如直连进游戏、大乱斗或游戏中途启动软件），彻底清空残留数据，防止上一局旧快照污染新对局
        if (champSelectTeamSnapshot.value.length === 0) {
          playerData.value = {};
          gameflowMyTeam.value = [];
          gameflowTheirTeam.value = [];
        } else {
          // 清理选人阶段以数字 0..9 临时缓存的旧键，避免红方我方 5..9 槽位残留与对局敌方 5..9 发生碰撞
          for (let i = 0; i < 10; i++) {
            delete playerData.value[i];
          }
        }
        // 从选人阶段进入载入或游戏时，清空 session 缓存
        clearSessionCache();
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
      if (session.gameData.gameId)
        currentGameId.value = session.gameData.gameId;
      seedSessionCache(session);
      if (store.gamePhase === "ChampSelect") {
        fetchPremadeColors();
        return;
      }
      if (store.gamePhase !== "InProgress" && store.gamePhase !== "GameStart")
        return;
      const { teamOne, teamTwo } = session.gameData;
      if ((teamOne && teamOne.length > 0) || (teamTwo && teamTwo.length > 0)) {
        // 队伍为空时加载；会话人数增长（如残缺会话补齐）时重处理，补回缺失的列
        const sessionTotal = (teamOne?.length ?? 0) + (teamTwo?.length ?? 0);
        const currentTotal =
          gameflowMyTeam.value.length + gameflowTheirTeam.value.length;
        if (
          gameflowMyTeam.value.length === 0 ||
          gameflowTheirTeam.value.length === 0 ||
          (currentTotal > 0 && sessionTotal > currentTotal)
        ) {
          processTeamData(teamOne || [], teamTwo || []);
        } else if (
          !hasPremadeGroup(premadeColorsMy.value) ||
          !hasPremadeGroup(premadeColorsTheir.value)
        ) {
          // 若人数已满但此前尚未拿到组队 ID，尝试从最新推送的 session 补齐组队颜色
          fetchPremadeColors();
        }
      }
    },
  );

  const teamSig = (team: ChampSelectPlayer[], session?: any) =>
    (team || [])
      .map((p) => {
        const champId = resolvePlayerChampionId(p, session);
        return `${p.cellId}:${champId}:${p.puuid || ""}:${p.teamParticipantId ?? p.partyId ?? ""}`;
      })
      .join(",");

  watch(
    () => store.champSelectSession,
    (session) => {
      if (session && store.gamePhase === "ChampSelect") {
        const rawMyTeam = session.myTeam || [];
        const rawTheirTeam = session.theirTeam || [];

        // 队伍归属以 LCU 原生语义为准：myTeam（含自定义人机 isHumanoid）= 我方，
        // theirTeam = 敌方。人机仅标记为 bot（下游走本地占位，不请求 LCU 战绩接口），不改变归属。
        const flagBots = (list: ChampSelectPlayer[]): ChampSelectPlayer[] =>
          list.map((p) =>
            p.isHumanoid && !p.bot && !p.isBot
              ? { ...p, bot: true, isBot: true }
              : p,
          );
        // 自定义会话（敌方 cell 可能与我方共用编号空间）：敌方快照需重映射到 5+ 稳定区间隔离
        const isCustomSession =
          session.isCustomGame === true ||
          rawMyTeam.some((p) => p.isHumanoid) ||
          rawTheirTeam.some((p) => p.isHumanoid);
        const myTeam = flagBots(rawMyTeam).map((p) => ({
          ...p,
          isEnemy: false,
        }));
        const theirTeamPlayers = flagBots(rawTheirTeam).map((p) => ({
          ...p,
          isEnemy: true,
        }));
        const sig =
          teamSig(myTeam, session) + "|" + teamSig(theirTeamPlayers, session);
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
            const finalTpid = getValidTpid(p) ?? getValidTpid(prev);
            return {
              ...p,
              championId: finalChampId,
              teamParticipantId: finalTpid ?? p.teamParticipantId,
              partyId: finalTpid ?? p.partyId,
            };
          });
        };

        champSelectTeamSnapshot.value = mergeSnapshot(
          myTeam,
          champSelectTeamSnapshot.value,
        );
        // 增量合并敌方快照：自定义会话映射到 5+ 稳定区间；常规会话保持原 cellId 空间
        const mergedTheir = mergeSnapshot(
          theirTeamPlayers,
          champSelectTheirTeamSnapshot.value,
        );
        champSelectTheirTeamSnapshot.value = isCustomSession
          ? mergedTheir.map((p, idx) => ({ ...p, cellId: 5 + idx }))
          : mergedTheir;

        loading.value = false;
        error.value = "";
        gameflowMyTeam.value = champSelectTeamSnapshot.value;
        gameflowTheirTeam.value = champSelectTheirTeamSnapshot.value;

        const rosterSig = (team: ChampSelectPlayer[]) =>
          (team || [])
            .map(
              (p) =>
                `${p.cellId}:${p.puuid || ""}:${p.summonerId || ""}:${p.teamParticipantId ?? p.partyId ?? ""}`,
            )
            .join(";");

        const currentRosterSig =
          rosterSig(myTeam) + "|" + rosterSig(theirTeamPlayers);
        const isRosterChanged = currentRosterSig !== lastSessionRosterSig;
        lastSessionRosterSig = currentRosterSig;

        // 同步已缓存在 playerData 字典中的 championId
        for (const p of [
          ...champSelectTeamSnapshot.value,
          ...champSelectTheirTeamSnapshot.value,
        ]) {
          const cid = p.championId ?? 0;
          if (cid > 0) {
            const entry =
              (p.cellId !== undefined
                ? playerData.value[p.cellId]
                : undefined) ||
              (p.puuid ? playerData.value[p.puuid] : undefined) ||
              (p.summonerId ? playerData.value[p.summonerId] : undefined);
            if (entry && entry.championId !== cid) {
              entry.championId = cid;
            }
          }
        }

        // 若仅英雄变更/悬停且队员名单无增减变动，仅增量更新条目英雄，不重复触发全队战绩重拉与组队分析
        if (!isRosterChanged) {
          for (const p of [
            ...champSelectTeamSnapshot.value,
            ...champSelectTheirTeamSnapshot.value,
          ]) {
            const cid = p.championId ?? 0;
            if (cid > 0) {
              const entry =
                (p.cellId !== undefined
                  ? playerData.value[p.cellId]
                  : undefined) ||
                (p.puuid ? playerData.value[p.puuid] : undefined) ||
                (p.summonerId ? playerData.value[p.summonerId] : undefined);
              if (entry && !entry.masteryFetched) {
                const targetPuuid = p.puuid || entry.info?.puuid;
                const targetSid =
                  (p.summonerId && p.summonerId !== p.cellId
                    ? p.summonerId
                    : 0) || entry.info?.summonerId;
                const isMe =
                  targetSid === currentSummonerId.value ||
                  (!!targetPuuid &&
                    targetPuuid === currentSummonerPuuid.value);
                void fetchPlayerMastery(targetPuuid, targetSid, isMe)
                  .then((m) => {
                    if (m && m.length > 0) {
                      entry.masteries = m;
                      entry.masteryFetched = true;
                    }
                  })
                  .catch(() => {
                    // 保持 entry.masteryFetched 为 falsy，以便下次 hover 可重试
                  });
              }
            }
          }
          return;
        }

        loadAllPlayers();
        fetchPremadeColors();
      }
    },
  );

  watch(activeTab, () => loadAllPlayers());

  // 监听对局结束事件：链式梯度轮询拉取刚结束对局的最新战绩，依次间隔等待且支持随时安全清理
  watch(
    () => store.gameEndedTrigger,
    () => {
      clearGameEndedTimers();
      const sessionId = currentGameEndedSessionId;
      const delays = [2000, 3500, 5000];
      void (async () => {
        for (const delay of delays) {
          await waitGameEndedDelay(delay);
          if (sessionId !== currentGameEndedSessionId) return;
          try {
            await syncCurrentPlayerMatches(true);
          } catch {
            /* ignore */
          }
          if (sessionId !== currentGameEndedSessionId) return;
        }
      })();
    },
  );

  watch(
    () => store.currentSummoner,
    (summoner) => {
      if (summoner?.summonerId) currentSummonerId.value = summoner.summonerId;
      if (summoner?.puuid) currentSummonerPuuid.value = summoner.puuid;
      if (
        summoner?.puuid &&
        store.gamePhase !== "InProgress" &&
        store.gamePhase !== "GameStart" &&
        store.gamePhase !== "ChampSelect"
      ) {
        syncCurrentPlayerMatches();
      }
    },
  );

  watch(
    () => store.isConnected,
    async (connected) => {
      if (connected) {
        if (!currentSummonerPuuid.value || !currentSummonerId.value) {
          try {
            const s = await fetchCurrentSummoner();
            if (s?.summonerId) currentSummonerId.value = s.summonerId;
            if (s?.puuid) currentSummonerPuuid.value = s.puuid;
          } catch {
            /* ignore */
          }
        }
        refreshState();
      }
    },
  );

  watch(
    () => store.currentPage,
    (newPage) => {
      if (newPage === "gameinfo") {
        if (
          store.gamePhase === "InProgress" ||
          store.gamePhase === "GameStart"
        ) {
          loadFromGameflowSession();
        } else if (store.gamePhase === "ChampSelect") {
          loadAllPlayers();
        } else {
          // 非活跃游戏状态下（如打完一局回到大厅或离开对局信息切回）：
          // 优先检查内存中是否已有上局对局数据，或从 localStorage 恢复保留数据
          const hasPlayerData =
            gameflowMyTeam.value.length > 0 &&
            Object.keys(playerData.value).length > 0;
          if (!hasPlayerData) {
            if (appConfig.value?.Functions?.EnableReserveGameinfo) {
              restoreReserveDataFromLocalStorage();
            }
          } else {
            syncCurrentPlayerMatches();
          }
          // 仅拉取当前 phase 和 champSelectSession，切勿覆盖已保留的对局
          refreshState();
        }
      }
    },
  );

  onMounted(async () => {
    window.addEventListener("beforeunload", handleFlushOnUnload);
    document.addEventListener("visibilitychange", handleFlushOnHidden);

    if (!appConfig.value) {
      try {
        appConfig.value = await fetchConfig();
      } catch {
        /* ignore */
      }
    }

    if (
      appConfig.value?.Functions?.EnableReserveGameinfo &&
      store.gamePhase !== "ChampSelect" &&
      store.gamePhase !== "GameStart" &&
      store.gamePhase !== "InProgress"
    ) {
      if (restoreReserveDataFromLocalStorage()) {
        if (
          Object.keys(premadeColorsMy.value).length === 0 &&
          gameflowMyTeam.value.length > 0
        )
          premadeColorsMy.value = computePremadeColors(gameflowMyTeam.value);
        if (
          Object.keys(premadeColorsTheir.value).length === 0 &&
          gameflowTheirTeam.value.length > 0
        )
          premadeColorsTheir.value = computePremadeColors(
            gameflowTheirTeam.value,
          );
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

  onUnmounted(() => {
    clearGameEndedTimers();
    // 卸载前把仍在防抖窗口内未落盘的快照立刻写出，避免丢失最后一次数据
    flushPendingSave();
    window.removeEventListener("beforeunload", handleFlushOnUnload);
    document.removeEventListener("visibilitychange", handleFlushOnHidden);
    extraMatchesQueue.length = 0;
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
