import { ref, computed, watch, onMounted, type Ref } from "vue";
import { useLcuStore, type ChampSelectPlayer } from "../store/lcuStore";
import {
  getGameflowPhase,
  getChampSelectSession,
  fetchMatchHistorySmart,
  fetchCurrentSummoner,
  lcuRequest,
  fetchConfig,
  fetchPlayerFateInfo,
  querySavedPlayersMap,
  getLiveClientPlayerList,
  getLiveClientActivePlayerName,
  getMapSide,
  writeFrontendLog,
  type MatchDisplay,
  type AppConfig,
} from "../api/lcu";
import type {
  PlayerData,
  PremadePlayerLike,
  ChampionMasteryItem,
  StreakInfo,
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
import {
  fetchChampions,
  findChampionIdByLiveClientName,
} from "../utils/championCache";

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

// ── 英雄熟练度数据缓存（puuid → { data, timestamp }）
const masteryCache = new Map<
  string,
  { data: ChampionMasteryItem[]; timestamp: number }
>();
const MASTERY_CACHE_TTL = 5 * 60 * 1000; // 5 分钟
const MASTERY_CACHE_MAX_SIZE = 100;

function getMasteryFromCache(puuid: string) {
  const cached = masteryCache.get(puuid);
  if (!cached) return null;
  if (Date.now() - cached.timestamp >= MASTERY_CACHE_TTL) {
    masteryCache.delete(puuid);
    return null;
  }
  return cached.data;
}

function setMasteryToCache(puuid: string, data: ChampionMasteryItem[]) {
  if (masteryCache.size >= MASTERY_CACHE_MAX_SIZE) {
    const firstKey = masteryCache.keys().next().value;
    if (firstKey) masteryCache.delete(firstKey);
  }
  masteryCache.set(puuid, { data, timestamp: Date.now() });
}

export async function fetchPlayerMastery(
  puuid?: string,
  summonerId?: number,
  isMe?: boolean,
): Promise<ChampionMasteryItem[]> {
  if (!puuid && !summonerId) return [];

  if (puuid) {
    const cached = getMasteryFromCache(puuid);
    if (cached && cached.length > 0) {
      return cached;
    }
  }

  // 1. 优先按 puuid 查询
  let mResp: { success: boolean; data?: any; error?: any } = { success: false };
  if (puuid) {
    mResp = await lcuRequest<any>(
      "GET",
      `/lol-champion-mastery/v1/${puuid}/champion-mastery`,
    );
  }

  // 2. 若是当前玩家且按 puuid 失败（或无 puuid），降级到 local-player
  if ((!mResp.success || !mResp.data) && isMe) {
    mResp = await lcuRequest<any>(
      "GET",
      "/lol-champion-mastery/v1/local-player/champion-mastery",
    );
  }

  // 3. 如果仍未成功，尝试按 summonerId 查询
  if ((!mResp.success || !mResp.data) && summonerId) {
    mResp = await lcuRequest<any>(
      "GET",
      `/lol-champion-mastery/v1/summoners/${summonerId}/champion-mastery`,
    );
  }

  if (mResp.success && Array.isArray(mResp.data)) {
    const normalized: ChampionMasteryItem[] = mResp.data.map((item: any) => ({
      championId: Number(item.championId ?? item.champion_id ?? 0),
      championLevel: Number(
        item.championLevel ?? item.masteryLevel ?? item.level ?? 0,
      ),
      championPoints: Number(
        item.championPoints ?? item.points ?? item.score ?? 0,
      ),
      highestGrade: item.highestGrade ?? item.highest_grade,
      championPointsSinceLastLevel: item.championPointsSinceLastLevel,
      championPointsUntilNextLevel: item.championPointsUntilNextLevel,
      tokensEarned: item.tokensEarned,
    }));
    if (puuid) {
      setMasteryToCache(puuid, normalized);
    }
    return normalized;
  }

  return [];
}

// ── 槽位英雄 ID 继承：保留已有英雄 ID，只有条目与 incoming 身份明确冲突时才严禁串用
function inheritPlaceholderChampion(
  entry: PlayerData | undefined,
  cellId: number,
  incoming?: { puuid?: string; summonerId?: number },
): number {
  const champ = entry?.championId ?? 0;
  if (!champ || champ <= 0) return 0;
  if (incoming) {
    const inPuuid = (incoming.puuid || "").trim();
    const inSid = incoming.summonerId || 0;
    const inSidReal = Boolean(inSid) && inSid !== cellId;
    const ePuuid = (entry?.info?.puuid || "").trim();
    const eRawSid = entry?.info?.summonerId || 0;
    const eSid = eRawSid === cellId ? 0 : eRawSid;
    // 仅在明确与不同真实身份冲突时拒绝继承
    if (inPuuid && ePuuid && inPuuid !== ePuuid) return 0;
    if (inSidReal && eSid && inSid !== eSid) return 0;
  }
  return champ;
}

// ── 新号判定上限：空战绩 + 等级在此之下视为从未打过的新号，不标隐藏
export const NEW_PLAYER_MAX_LEVEL = 30;

// ── 身份门禁（canonical）：incoming 的真实身份键与条目是否冲突。
// puuid 非空即真实；summonerId 仅在非 0 且不等于 cell 槽位时视为真实
// （选人/对局切换时 cellId 会被充作 sid 兜底，不可参与比对）。
// 条目侧同理：info 缺失（loading 占位）或 sid 恰为 cell 槽位都视为无身份，不构成冲突。
// 注意这是“宽松版”（无冲突即兼容）：seed 预填充与视图层核验直接用它；
// loadPlayerData 复用已加载项时另需 finished 包装（loading 占位不可复用，占位→真实必须重拉）。
export function isIdentityCompatible(
  entry: PlayerData | undefined,
  incoming: { puuid?: string; summonerId?: number; cellId?: number },
): boolean {
  if (!entry?.info) return true;
  const cell = incoming.cellId ?? -1;
  const inPuuid = (incoming.puuid || "").trim();
  const inSid = incoming.summonerId || 0;
  const inSidReal = Boolean(inSid) && inSid !== cell;
  const ePuuid = (entry.info.puuid || "").trim();
  const eRawSid = entry.info.summonerId || 0;
  const eSid = eRawSid === cell ? 0 : eRawSid;

  if (inPuuid && ePuuid && inPuuid !== ePuuid) return false;
  if (inSidReal && eSid && inSid !== eSid) return false;

  // 严防选人阶段脱敏匿名玩家复用已有实名玩家的数据（避免选人红方 5..9 槽位残留被敌方 5..9 误复用）
  // 仅在选人阶段（ChampSelect）严格拦截；进入游戏后（GameStart/InProgress），脱敏槽位需要读取 LiveClient 回填的实名数据
  const store = useLcuStore();
  if (store.gamePhase === "ChampSelect") {
    const incomingHasIdentity = Boolean(inPuuid || inSidReal);
    const entryHasIdentity = Boolean(ePuuid || eSid);
    if (incomingHasIdentity !== entryHasIdentity) {
      return false;
    }
  }

  return true;
}

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
    const resp = await lcuRequest<GameflowSession>(
      "GET",
      "/lol-gameflow/v1/session",
    );
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
      localStorage.setItem(
        "yuumi_last_game_player_data",
        JSON.stringify(playerData.value),
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

  function restoreReserveDataFromLocalStorage(): boolean {
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
        if (parsed && Object.keys(parsed).length > 0) {
          playerData.value = parsed;
          hasRestored = true;
          // 异步检查并补齐缺少熟练度的玩家（例如旧版本保留的数据）
          backfillMissingMasteries();
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
                p.puuid === currentSummonerPuuid.value),
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
    const fromResolver = resolvePlayerChampionId(
      fallbackPlayer,
      store.champSelectSession,
    );
    if (fromResolver > 0) return fromResolver;
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

  async function loadPlayerData(
    cellId: number,
    summonerId: number,
    playerPuuid?: string,
    fallbackPlayer?: PremadePlayerLike,
  ) {
    if (!summonerId && !playerPuuid && !fallbackPlayer) return;

    // 防止 cellId 误传为 summonerId（如 0..9 的 cellId）
    const realSummonerId = summonerId && summonerId !== cellId ? summonerId : 0;

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
    if (reusable) {
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
      return;
    }
    // 无可复用的同身份已加载项，继续往下加载真实数据

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
      const botName =
        fallbackPlayer.displayName ||
        fallbackPlayer.summonerName ||
        fallbackPlayer.botName ||
        fallbackPlayer.gameName ||
        `电脑${cellId + 1}`;
      const iconId = fallbackPlayer.profileIconId ?? 29;
      const botInfo: SummonerDisplay = {
        accountId: 0,
        summonerId: realSummonerId || 0,
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
        championId:
          fallbackPlayer.championId || fallbackPlayer.botChampionId || 0,
      };
      playerData.value[cellId] = botDataObj;
      if (realSummonerId) playerData.value[realSummonerId] = botDataObj;
      if (playerPuuid) playerData.value[playerPuuid] = botDataObj;
      debouncedSavePlayerData();
      return;
    }

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
      const maxMatches = filterEnabled ? 50 : 10;

      const [rawMatches, rankedResp, masteryData] = await Promise.all([
        safeInfo.puuid
          ? fetchMatchHistorySmart(safeInfo.puuid, 0, maxMatches)
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
          validMatches > 0
            ? Math.round((currentWinCount / validMatches) * 100)
            : 0;
        const deathsForCalc = totalDeaths === 0 ? 1 : totalDeaths;
        avgKda = (totalKills + totalAssists) / deathsForCalc;
      }

      let streak: StreakInfo | null = null;
      if (matches && matches.length > 0) {
        const validStreakMatches = matches.filter((m) => !m.remake);
        if (validStreakMatches.length > 0) {
          const firstWin = validStreakMatches[0].win;
          let count = 0;
          for (const m of validStreakMatches) {
            if (m.win === firstWin) {
              count++;
            } else {
              break;
            }
          }
          if (count >= 2) {
            streak = {
              type: firstWin ? "win" : "loss",
              count,
            };
          }
        }
      }

      let fateFlag: "ally" | "enemy" | null = null;
      let recentlyChampionName = "";
      let fateIsLastGame = true;
      let fateAllyCount = 0;
      let fateEnemyCount = 0;
      if (
        currentSummonerId.value &&
        !isCurrentPlayer &&
        safeInfo.puuid
      ) {
        try {
          // 候选 game_id 列表：优先使用路人集记录的最近相遇对局 ID，随后按时间倒序填入目标玩家近 5 场战绩
          const candidateIds: number[] = [];
          if (currentSummonerPuuid.value) {
            try {
              const savedMap = await querySavedPlayersMap(currentSummonerPuuid.value);
              const targetSaved = savedMap[safeInfo.puuid];
              if (targetSaved?.lastEncounteredGameId) {
                candidateIds.push(targetSaved.lastEncounteredGameId);
              }
            } catch {
              // ignore saved map lookup failure
            }
          }
          for (const m of matches.slice(0, 5)) {
            if (m.gameId && !candidateIds.includes(m.gameId)) {
              candidateIds.push(m.gameId);
            }
          }

          if (candidateIds.length > 0) {
            const fateInfo = await fetchPlayerFateInfo(
              candidateIds[0],
              safeInfo.puuid,
              currentSummonerId.value,
              candidateIds,
            );
            if (fateInfo && fateInfo.fateFlag) {
              fateFlag = fateInfo.fateFlag;
              recentlyChampionName = fateInfo.recentlyChampionName || "";
              fateAllyCount = fateInfo.allyCount ?? 0;
              fateEnemyCount = fateInfo.enemyCount ?? 0;
              // 如果命中的对局不是 matches[0]，或者是好几局之前的，则标记为非最上一局
              if (matches[0] && fateInfo.gameId && fateInfo.gameId !== matches[0].gameId) {
                fateIsLastGame = false;
              }
            }
          }
        } catch (e) {
          console.error("宿命检测失败:", e);
        }
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
        fateFlag,
        recentlyChampionName,
        fateIsLastGame,
        fateAllyCount,
        fateEnemyCount,
        masteries: masteryData,
        streak,
      };
      playerData.value[cellId] = dataObj;
      if (summonerId && summonerId !== cellId) {
        playerData.value[summonerId] = dataObj;
      }
      if (safeInfo.puuid) {
        playerData.value[safeInfo.puuid] = dataObj;
      }
      console.log(
        `[GamePlayerData] 玩家数据已组装: cellId=${cellId}, name=${safeInfo.gameName || safeInfo.displayName}, champId=${targetChampId}, hidden=${matchHistoryHidden}, private=${isProfilePrivate}, matchesCount=${matches.length}`,
      );
      debouncedSavePlayerData();
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
    // 先加载当前可见队伍，再后台加载另一队，避免请求风暴
    const visible = filterValidPlayers(isMyVisible ? my : their, !isMyVisible);
    const background = filterValidPlayers(
      isMyVisible ? their : my,
      isMyVisible,
    );

    await runWithConcurrency(visible, 3, (p) => {
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
    void runWithConcurrency(background, 3, (p) => {
      const cid = p.cellId ?? p.summonerId ?? 0;
      const sid = p.summonerId ?? 0;
      if (
        cid !== undefined &&
        (sid || p.puuid || p.displayName || p.summonerName)
      ) {
        return loadPlayerData(cid, sid, p.puuid, p);
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
          (p.summonerName &&
            cs.displayName &&
            cs.displayName === p.summonerName) ||
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
      store.setMapSide("blue");
      console.info(
        "[GamePlayerData] checkMatches 确定队伍: 我方为蓝方 (teamOne)",
      );
    } else if (isTeamTwo) {
      allyTeam = teamTwo;
      enemyTeam = teamOne;
      store.setMapSide("red");
      console.info(
        "[GamePlayerData] checkMatches 确定队伍: 我方为红方 (teamTwo)",
      );
    } else {
      // 脱敏对局（排位赛等）：未直接匹配到当前玩家，结合多重信息推断阵营
      let determinedSide: "blue" | "red" | null = store.mapSide;

      // 1. 若当前 store.mapSide 未设置，从选人阶段快照 cellId 推断（蓝方 0..4，红方 5..9）
      if (!determinedSide && champSelectTeamSnapshot.value.length > 0) {
        const hasRedCell = champSelectTeamSnapshot.value.some(
          (cs) => cs.cellId !== undefined && cs.cellId >= 5,
        );
        const hasBlueCell = champSelectTeamSnapshot.value.some(
          (cs) => cs.cellId !== undefined && cs.cellId < 5,
        );
        if (hasRedCell && !hasBlueCell) {
          determinedSide = "red";
        } else if (hasBlueCell && !hasRedCell) {
          determinedSide = "blue";
        }
      }

      // 2. 若仍未确定，异步向 Rust 端获取（含 LCU session 及 2999 端口 LiveClientData 探测）
      if (!determinedSide) {
        try {
          const side = await getMapSide();
          if (side === "blue" || side === "red") {
            determinedSide = side;
          }
        } catch {
          /* ignore */
        }
      }

      // 3. 根据最终推断的阵营分配队伍：LOL 中 teamOne 永远为蓝方，teamTwo 永远为红方
      if (determinedSide === "red") {
        allyTeam = teamTwo.length > 0 ? teamTwo : teamOne;
        enemyTeam = teamTwo.length > 0 ? teamOne : teamTwo;
        store.setMapSide("red");
        console.info(
          "[GamePlayerData] 脱敏队伍推断成功: 我方为红方 (teamTwo), 敌方为蓝方 (teamOne)",
        );
      } else if (determinedSide === "blue") {
        allyTeam = teamOne.length > 0 ? teamOne : teamTwo;
        enemyTeam = teamOne.length > 0 ? teamTwo : teamOne;
        store.setMapSide("blue");
        console.info(
          "[GamePlayerData] 脱敏队伍推断成功: 我方为蓝方 (teamOne), 敌方为红方 (teamTwo)",
        );
      } else {
        // 最终兜底：非空队伍优先作为 allyTeam
        allyTeam = teamOne.length > 0 ? teamOne : teamTwo;
        enemyTeam = teamOne.length > 0 ? teamTwo : teamOne;
        if (teamOne.length > 0) {
          store.setMapSide("blue");
        } else if (teamTwo.length > 0) {
          store.setMapSide("red");
        }
        console.warn(
          "[GamePlayerData] 脱敏队伍推断兜底: 默认分配 teamOne 为我方",
        );
      }
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
      const resolvedName = p.gameName
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
      // 严格匹配：必须满足以下强标识之一，绝不使用没有强身份关联的 cellId 盲目兜底
      const snap = targetSnapshot.find(
        (cs) =>
          (p.puuid && cs.puuid && cs.puuid === p.puuid) ||
          (p.summonerId && cs.summonerId && cs.summonerId === p.summonerId) ||
          (p.summonerName &&
            cs.displayName &&
            cs.displayName === p.summonerName) ||
          (p.gameName && cs.gameName && cs.gameName === p.gameName) ||
          (p.displayName && cs.displayName && cs.displayName === p.displayName),
      );
      if (snap) {
        if (!resolvedChampId || resolvedChampId <= 0) {
          resolvedChampId = snap.championId ?? snap.championPickIntent ?? 0;
        }
        if (!profileIconId && snap.profileIconId) {
          profileIconId = snap.profileIconId;
        }
      }
      // 最终兜底：从当前已渲染的队伍数据中找同一玩家的 championId
      // 这解决 InProgress 阶段 gameflow session 不再携带 championId 导致覆盖 GameStart 时有效值的问题
      if (!resolvedChampId || resolvedChampId <= 0) {
        const prevTeam = isEnemy
          ? gameflowTheirTeam.value
          : gameflowMyTeam.value;
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
        isEnemy,
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

    // 严谨对应英雄联盟底层阵营机制：
    // 蓝方 (ORDER / 100) 对应 cellId 0..4
    // 红方 (CHAOS / 200) 对应 cellId 5..9
    // 我方是红方还是蓝方，决定了 0..4 与 5..9 属于我方还是敌方！
    const isAllyRed = store.mapSide === "red";
    const allyOffset = isAllyRed ? 5 : 0;
    const enemyOffset = isAllyRed ? 0 : 5;

    const mappedMy = allyTeam.map((p, idx) =>
      mapParticipant(p, idx, allyOffset, false),
    );
    const mappedTheir = enemyTeam.map((p, idx) =>
      mapParticipant(p, idx, enemyOffset, true),
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
    const seedInitialPlayerData = (players: PremadePlayerLike[]) => {
      for (const p of players) {
        const key = p.cellId ?? 0;
        const isBot = Boolean(p.bot || p.isBot || p.botChampionId);
        // 如果已有选人阶段完整加载的数据，保留并继承。
        // 身份优先查找：puuid → summonerId → cellId。选人/对局顺序变化时同 cell 槽
        // 可能残留异队旧数据，身份键优先才能命中同一玩家的已加载项，避免误杀重拉
        const byPuuid = p.puuid ? playerData.value[p.puuid] : undefined;
        const bySid = p.summonerId ? playerData.value[p.summonerId] : undefined;
        const byCell = playerData.value[key];
        const incoming = {
          puuid: p.puuid,
          summonerId: p.summonerId,
          cellId: key,
        };
        const existing = [byPuuid, bySid, byCell].find((e) =>
          isIdentityCompatible(e, incoming),
        );
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
        // cell 槽被异队旧数据占据：清槽后按新身份重建，避免把我方 info/matches 挂到敌方 key 上。
        // 若被清的是无身份占位且带有英雄 ID，先继承给 p，保证加载中英雄不断档
        if (byCell) {
          if (!p.championId || p.championId <= 0) {
            const carried = inheritPlaceholderChampion(byCell, key, p);
            if (carried > 0) p.championId = carried;
          }
          delete playerData.value[key];
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
        }
        if (
          p.summonerId &&
          p.summonerId !== key &&
          !playerData.value[p.summonerId]
        ) {
          // 防止选人阶段占位数据（cellId 充当 summonerId）污染真实 summonerId 键
          const existingSid = playerData.value[key]?.info?.summonerId;
          if (
            !existingSid ||
            existingSid === p.summonerId ||
            existingSid === key
          ) {
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
    loadTeam(visible).catch((e) =>
      console.debug("[GameInfo] 队伍数据加载异常:", e),
    );

    loadTeam(background)
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
        console.debug("[GameInfo] 队伍数据预加载失败:", err);
        // 异常兜底，只要队伍齐备也保存
        writeReserveData();
        if (
          store.gamePhase === "GameStart" ||
          store.gamePhase === "InProgress"
        ) {
          pollLiveClientChampions();
        }
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
      const liveGameId = data.gameData.gameId ?? null;
      if (liveGameId) currentGameId.value = liveGameId;
      // InProgress 阶段 LCU 会把 gameflow 队伍精简（甚至只剩我方或单边）：
      // 当落盘有同 gameId 的完整快照，且当前队伍为空或当前队伍少于落盘人数或当前 session 少于落盘人数时，
      // 先从快照恢复整局数据，后续逻辑中的长度守卫与 identity 合并会保留更全的数据，防止敌方丢失
      const sessionTotal = t1.length + t2.length;
      const isCustomGame = data.gameData.queue?.isCustom === true;
      if (liveGameId && !isCustomGame) {
        try {
          const savedId =
            Number(localStorage.getItem("yuumi_last_game_id")) || 0;
          const savedTotal =
            Number(localStorage.getItem("yuumi_last_game_team_count")) || 0;
          const currentTotal =
            gameflowMyTeam.value.length + gameflowTheirTeam.value.length;
          if (
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
        if (retryCount === 0) {
          writeFrontendLog(
            "debug",
            "GameInfo:LiveClient",
            `拉取 2999 端口 LiveClientData 成功: ${clientList.length} 人, 详情: ${JSON.stringify(
              clientList.map((p) => ({
                name: p.riotIdGameName || p.summonerName || p.riotId,
                champion: p.championName,
                rawChampion: p.rawChampionName,
                team: p.team,
              })),
            )}`,
          );
        }
        const champList = await fetchChampions();

        let hasUpdated = false;

        // 规范化名字比较辅助工具：去掉 #tag、空格及不可见字符，转小写
        const normalizeName = (name?: string) => {
          if (!name) return "";
          return name.split("#")[0].replace(/\s+/g, "").toLowerCase();
        };

        // 1. 判断我方在 LiveClientData 中的 team 阵营（"ORDER" 为蓝方，"CHAOS" 为红方）
        // 优先通过 2999 端口 activeplayername 获取当前本机操作的玩家名字，百分之百精准
        let myLiveClientTeam: string | null = null;
        try {
          const activeName = await getLiveClientActivePlayerName();
          if (activeName) {
            const activeNorm = normalizeName(activeName);
            const meInLiveClient = clientList.find((lp) => {
              const lpNames = [
                normalizeName(lp.riotIdGameName),
                normalizeName(lp.summonerName),
                normalizeName(lp.riotId),
              ].filter(Boolean);
              return lpNames.some((ln) => ln === activeNorm);
            });
            if (meInLiveClient?.team) {
              myLiveClientTeam = meInLiveClient.team;
              console.debug(
                `[GamePlayerData] 通过 activeplayername (${activeName}) 锁定我方阵营: ${myLiveClientTeam}`,
              );
            }
          }
        } catch {
          /* ignore */
        }

        if (!myLiveClientTeam) {
          for (const lp of clientList) {
            const lpNames = [
              normalizeName(lp.riotIdGameName),
              normalizeName(lp.summonerName),
              normalizeName(lp.riotId),
            ].filter(Boolean);
            const isMyAlly = gameflowMyTeam.value.some((p) => {
              const pNames = [
                normalizeName(p.gameName),
                normalizeName(p.displayName),
                normalizeName(p.summonerName),
              ].filter(Boolean);
              return pNames.some((pn) => lpNames.some((ln) => pn === ln));
            });
            if (isMyAlly && lp.team) {
              myLiveClientTeam = lp.team;
              break;
            }
          }
        }
        if (!myLiveClientTeam) {
          if (store.mapSide) {
            myLiveClientTeam = store.mapSide === "blue" ? "ORDER" : "CHAOS";
          } else {
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

          // 1) 优先进行严格全等名字匹配
          let targetPlayer = candidateTeam.find((p) => {
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
                if (realName && targetPlayer.displayName !== realName) {
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
            if (
              realName &&
              (targetPlayer.displayName !== realName || !targetPlayer.puuid)
            ) {
              writeFrontendLog(
                "info",
                "GameInfo:LiveClientName",
                `LiveClient 回填名字: 原名="${targetPlayer.gameName || pd?.info?.gameName}", 回填真实名="${realName}", cellId=${targetPlayer.cellId}, champ=${lp.championName}`,
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
      // 确保敌方队伍快照在离开对局时若处于内存状态，得以稳定维持
      if (
        champSelectTheirTeamSnapshot.value.length === 0 &&
        gameflowTheirTeam.value.length > 0
      ) {
        champSelectTheirTeamSnapshot.value = [...gameflowTheirTeam.value];
      }
      if (
        champSelectTeamSnapshot.value.length === 0 &&
        gameflowMyTeam.value.length > 0
      ) {
        champSelectTeamSnapshot.value = [...gameflowMyTeam.value];
      }
    } else {
      // 刚进入选人阶段时，清空当前内存视图与快照以展示当前新选人
      if (store.gamePhase === "ChampSelect") {
        gameflowMyTeam.value = [];
        gameflowTheirTeam.value = [];
        champSelectTeamSnapshot.value = [];
        champSelectTheirTeamSnapshot.value = [];
        lastSessionTeamSig = "";
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
      if (phase === "ChampSelect") {
        currentGameId.value = null;
        gameflowMyTeam.value = [];
        gameflowTheirTeam.value = [];
        champSelectTeamSnapshot.value = [];
        champSelectTheirTeamSnapshot.value = [];
        lastSessionTeamSig = "";
        playerData.value = {};
        premadeColorsMy.value = {};
        premadeColorsTheir.value = {};
        // 选人阶段不立即清空 localStorage，避免秒退导致已有完整对局丢失
        refreshState();
      }
      if (phase === "InProgress" || phase === "GameStart") {
        // 清理选人阶段以数字 0..9 临时缓存的旧键，避免红方我方 5..9 槽位残留与对局敌方 5..9 发生碰撞
        for (let i = 0; i < 10; i++) {
          delete playerData.value[i];
        }
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
      if (session.gameData.gameId)
        currentGameId.value = session.gameData.gameId;
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
        }
      }
    },
  );

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
            restoreReserveDataFromLocalStorage();
          }
          // 仅拉取当前 phase 和 champSelectSession，切勿覆盖已保留的对局
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

    if (appConfig.value?.Functions?.EnableReserveGameinfo) {
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
