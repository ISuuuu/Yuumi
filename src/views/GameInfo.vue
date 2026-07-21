<script setup lang="ts">
import { ref, watch, computed, onMounted, inject, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import { useLcuStore } from "../store/lcuStore";
import {
  getGameflowPhase,
  getChampSelectSession,
  fetchMatchHistory,
  fetchCurrentSummoner,
  lcuRequest,
  fetchConfig,
  fetchPlayerFateInfo,
} from "../api/lcu";
import type { MatchDisplay, AppConfig } from "../api/lcu";
import LcuImage from "../components/LcuImage.vue";

const store = useLcuStore();
const { t, te } = useI18n();

function getQueueName(queueId: number, backendName: string): string {
  const key = `gameModes.${queueId}`;
  if (te(key)) {
    const translation = t(key);
    if (
      (translation.includes("云顶") || translation.includes("TFT")) &&
      !backendName.includes("云顶") && !backendName.includes("TFT")
    ) {
      return backendName;
    }
    return translation;
  }
  return backendName;
}

const loading = ref(false);
const error = ref("");
const activeTab = ref<"my" | "their">("my");

// 应用配置（用于获取对局卡片颜色）
const appConfig =
  inject<Ref<AppConfig | null>>("appConfig") || ref<AppConfig | null>(null);

const currentSummonerId = ref<number>(0);
const currentSummonerPuuid = ref<string>("");

// ── 排位数据缓存（puuid → { data, timestamp }），避免重复请求同一玩家的排位信息
const rankCache = new Map<string, { data: any; timestamp: number }>();
const RANK_CACHE_TTL = 5 * 60 * 1000; // 5 分钟

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

// ── localStorage 写入防抖（10 个玩家并发加载时避免重复序列化）
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

// 预组队颜色方案（降低饱和度与透明度，非常柔和剔透，不刺眼）
const PREMADE_COLORS = [
  {
    border: "rgba(255, 140, 0, 0.6)",
    bg: "rgba(255, 140, 0, 0.15)",
    dot: "#ff9010",
  }, // 亮橙 / 橘色
  {
    border: "rgba(236, 72, 153, 0.4)",
    bg: "rgba(236, 72, 153, 0.10)",
    dot: "#ec4899",
  }, // 柔粉
  {
    border: "rgba(59, 130, 246, 0.4)",
    bg: "rgba(59, 130, 246, 0.10)",
    dot: "#3b82f6",
  }, // 柔蓝
  {
    border: "rgba(16, 185, 129, 0.4)",
    bg: "rgba(16, 185, 129, 0.10)",
    dot: "#10b981",
  }, // 柔绿
  {
    border: "rgba(168, 85, 247, 0.4)",
    bg: "rgba(168, 85, 247, 0.10)",
    dot: "#a855f7",
  }, // 柔紫
];

// summonerId → colorIndex（-1 = 单排，0+ = 组队颜色索引）
const premadeColorsMy = ref<Record<number, number>>({});
const premadeColorsTheir = ref<Record<number, number>>({});

/** 根据 teamParticipantId 分组，计算组队颜色映射 */
function computePremadeColors(team: any[]): Record<number, number> {
  if (!team || team.length === 0) return {};
  const tIdToSIds: Record<number, number[]> = {};

  for (const p of team) {
    const sid = p.summonerId;
    const tpid = p.teamParticipantId;
    if (!sid || tpid === undefined || tpid === null) continue;
    if (!tIdToSIds[tpid]) tIdToSIds[tpid] = [];
    tIdToSIds[tpid].push(sid);
  }

  const result: Record<number, number> = {};
  let currentColor = 0;

  for (const ids of Object.values(tIdToSIds)) {
    if (ids.length === 1) {
      result[ids[0]] = -1; // 单排
    } else {
      for (const id of ids) result[id] = currentColor;
      currentColor++;
    }
  }
  return result;
}

// 补充缓存 session 团队以确保选人阶段能拿到敌方真实数据
const sessionAllyTeam = ref<any[]>([]);
const sessionEnemyTeam = ref<any[]>([]);

/** 从 gameflow session 获取组队信息（ChampSelect 阶段使用） */
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

/** 获取玩家组队颜色索引 */
function getPremadeIdx(summonerId: number, side: "my" | "their"): number {
  const colors =
    side === "my" ? premadeColorsMy.value : premadeColorsTheir.value;
  return colors[summonerId] ?? -1;
}

/** 左侧玩家卡片组队样式 */
function getPremadeCardStyle(
  summonerId: number,
  side: "my" | "their",
): Record<string, string> {
  const idx = getPremadeIdx(summonerId, side);
  if (idx < 0) return {};
  const c = PREMADE_COLORS[idx % PREMADE_COLORS.length];
  return {
    backgroundColor: c.bg,
    borderColor: c.border,
  };
}

interface PremadeMember {
  summonerId: number;
  displayName: string;
  championId: number;
}

interface PremadeGroup {
  colorIdx: number;
  members: PremadeMember[];
}

function buildPremadeGroups(team: any[], colors: Record<number, number>): PremadeGroup[] {
  if (!team || team.length === 0 || !colors) return [];
  const map: Record<number, PremadeMember[]> = {};

  for (const p of team) {
    const sid = p.summonerId;
    if (!sid) continue;
    const cIdx = colors[sid];
    if (cIdx === undefined || cIdx < 0) continue;

    if (!map[cIdx]) map[cIdx] = [];
    const champId = p.championId || p.championPickIntent || 0;
    const name =
      playerData.value[p.cellId || sid]?.info?.gameName ||
      playerData.value[p.cellId || sid]?.info?.displayName ||
      p.displayName ||
      p.summonerName ||
      "";

    map[cIdx].push({
      summonerId: sid,
      displayName: name,
      championId: champId,
    });
  }

  return Object.entries(map).map(([cIdxStr, members]) => ({
    colorIdx: Number(cIdxStr),
    members,
  })).sort((a, b) => a.colorIdx - b.colorIdx);
}

// 己方组队列表
const myPremadeGroups = computed(() => {
  const teamList = myTeam.value.length > 0 ? myTeam.value : sessionAllyTeam.value;
  return buildPremadeGroups(teamList, premadeColorsMy.value);
});

// 敌方组队列表
const theirPremadeGroups = computed(() => {
  const teamList = theirTeam.value.length > 0 ? theirTeam.value : sessionEnemyTeam.value;
  return buildPremadeGroups(teamList, premadeColorsTheir.value);
});

// 是否有任何组队信息
const hasAnyPremadeInfo = computed(() => {
  return myPremadeGroups.value.length > 0 || theirPremadeGroups.value.length > 0;
});

interface PremadeRow {
  ally?: PremadeGroup;
  enemy?: PremadeGroup;
}

/** 逐行交错配对我方与敌方组队（优先保证友方显示在第一行，人数 >= 4 独占一行） */
const premadeRows = computed<PremadeRow[]>(() => {
  const my = [...myPremadeGroups.value];
  const their = [...theirPremadeGroups.value];
  const rows: PremadeRow[] = [];

  let mIdx = 0;
  let tIdx = 0;

  while (mIdx < my.length || tIdx < their.length) {
    const mGroup = my[mIdx];
    const tGroup = their[tIdx];

    if (mGroup) {
      // 我方组队 >= 4 人时，我方独占一行
      if (mGroup.members.length >= 4) {
        rows.push({ ally: mGroup });
        mIdx++;
      } else {
        // 我方组队 < 4 人，如果敌方组队也 < 4 人，则同行并排展示
        if (tGroup && tGroup.members.length < 4) {
          rows.push({ ally: mGroup, enemy: tGroup });
          mIdx++;
          tIdx++;
        } else {
          // 如果敌方无组队或敌方组队 >= 4 人，优先将我方安排在当前行
          rows.push({ ally: mGroup });
          mIdx++;
        }
      }
    } else if (tGroup) {
      // 我方组队处理完毕，仅剩敌方组队
      rows.push({ enemy: tGroup });
      tIdx++;
    }
  }

  return rows;
});

// 当前游戏模式对应的队列 ID（用于过滤战绩）
const currentQueueId = ref<number | null>(null);
const isTftMode = ref(false);

async function updateCurrentQueueId() {
  try {
    const data = await fetchSessionCached();
    if (data?.gameData?.queue?.id !== undefined) {
      currentQueueId.value = data.gameData.queue.id;
      console.log(
        "[GameInfo] Detected current game mode queueId:",
        currentQueueId.value,
      );

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

// 每个玩家的完整数据
interface PlayerData {
  info: any;
  matches: MatchDisplay[];
  ranked: { solo: any; flex: any };
  loading: boolean;
  avgKda?: number;
  winRate?: number;
  winCount?: number;
  lossesCount?: number;
  fateFlag?: "ally" | "enemy" | null; // 上一局宿命：队友/对手/无
  recentlyChampionName?: string; // 上一局使用的英雄名
}
const playerData = ref<Record<number, PlayerData>>({});

const TIER_MAP = computed<Record<string, string>>(() => ({
  NONE: "",
  IRON: t("tools.spoofTier.IRON"),
  BRONZE: t("tools.spoofTier.BRONZE"),
  SILVER: t("tools.spoofTier.SILVER"),
  GOLD: t("tools.spoofTier.GOLD"),
  PLATINUM: t("tools.spoofTier.PLATINUM"),
  EMERALD: t("tools.spoofTier.EMERALD"),
  DIAMOND: t("tools.spoofTier.DIAMOND"),
  MASTER: t("tools.spoofTier.MASTER"),
  GRANDMASTER: t("tools.spoofTier.GRANDMASTER"),
  CHALLENGER: t("tools.spoofTier.CHALLENGER"),
}));

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

// InProgress/GameStart 阶段的队伍数据（从 gameflow session 获取）
const gameflowMyTeam = ref<any[]>([]);
const gameflowTheirTeam = ref<any[]>([]);

/** 当前是否处于可展示战绩的阶段 */
const isGameActive = computed(
  () =>
    store.gamePhase === "ChampSelect" ||
    store.gamePhase === "GameStart" ||
    store.gamePhase === "InProgress",
);

/** 是否应该在界面中展示对局信息（如果开启了“保留对局信息界面内容”且有历史数据，则即使不在对局中也展示） */
const shouldShowContent = computed(() => {
  if (isTftMode.value) return false;
  if (isGameActive.value) return true;
  if (appConfig.value?.Functions?.EnableReserveGameinfo) {
    return Object.keys(playerData.value).length > 0;
  }
  return false;
});

// 监听游戏活跃状态的变化，当退出游戏或选人阶段时，若开启了保留上一局配置，则从本地存储中加载数据以在内存中还原对局选手和战绩
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
    // 游戏再次活跃时，先清空以加载新状态
    gameflowMyTeam.value = [];
    gameflowTheirTeam.value = [];
    playerData.value = {};
  }
});

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

/** 缓存 key 与 Career.vue 共享，避免各自独立缓存导致数据不互通 */
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

/** 加载单个玩家的战绩/排位数据（通用，ChampSelect 和 InProgress 共用） */
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
            // 优先从缓存获取排位数据
            const cached = rankCache.get(info.puuid);
            if (cached && Date.now() - cached.timestamp < RANK_CACHE_TTL) {
              return Promise.resolve({ success: true, data: cached.data });
            }
            return lcuRequest<any>("GET", `/lol-ranked/v1/ranked-stats/${info.puuid}`)
              .then((resp) => {
                if (resp.success && resp.data) {
                  rankCache.set(info.puuid, { data: resp.data, timestamp: Date.now() });
                }
                return resp;
              });
          })()
        : Promise.resolve({ success: false } as any),
    ]);

    const isCurrentPlayer =
      summonerId === currentSummonerId.value ||
      (!!info.puuid && info.puuid === currentSummonerPuuid.value);

    // 仅对"自己"的战绩与缓存合并去重（其他玩家不缓存）
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

    // 宿命检测：取该玩家最近一局，查自己是否在其中（上局队友/对手），排除自己本身
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

/** ChampSelect 阶段：加载当前 Tab 下的己方/敌方玩家 */
async function loadAllPlayers() {
  const team = currentTeam.value;
  if (!team || team.length === 0) return;
  await updateCurrentQueueId();
  await Promise.all(
    team.map((p: any) => loadPlayerData(p.cellId, p.summonerId)),
  );
}

/**
 * InProgress / GameStart 阶段：通过 /lol-gameflow/v1/session 加载双方数据。
 * 该接口的 teamOne/teamTwo 包含完整 championId，可同时获取敌方信息。
 */
async function loadFromGameflowSession() {
  loading.value = true;
  error.value = "";
  
  const reqId = ++currentGameflowSessionRequestId; // 分配当前请求的唯一 ID
  
  // 强制清除 gameflow session 缓存，确保拿到当前局的 teamOne/teamTwo，
  // 避免 30 秒过期时间内快速重开导致读到上一局的旧数据
  cachedSession = null;
  await updateCurrentQueueId();
  if (isTftMode.value) {
    loading.value = false;
    return;
  }

  // 保底获取当前玩家 summonerId（onMounted 可能还没拿到）
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
    if (reqId !== currentGameflowSessionRequestId) return; // 拦截旧请求的执行

    if (!data?.gameData) {
      error.value = "无法获取对局 Session";
      loading.value = false;
      return;
    }

    const { teamOne, teamTwo } = data.gameData;
    // 队伍数据缺失或为空时延迟重试（GameStart 初期 LCU 可能尚未填充 teamOne/teamTwo）
    if (!teamOne || !teamTwo || teamOne.length === 0 || teamTwo.length === 0) {
      let retried = 0;
      while (retried < 5) {
        await new Promise((r) => setTimeout(r, 3000));
        
        // 1. 重试期间安全终止校验：如果已被新的请求覆盖，或者对局阶段已退出，则立刻终止
        if (reqId !== currentGameflowSessionRequestId) return;
        if (store.gamePhase !== "InProgress" && store.gamePhase !== "GameStart") {
          console.log("[GameInfo] 对局已不再活跃，终止队伍数据重试");
          loading.value = false;
          return;
        }

        cachedSession = null; // 强制刷新缓存
        const retryData = await fetchSessionCached();
        if (reqId !== currentGameflowSessionRequestId) return;

        const rt = retryData?.gameData;
        if (rt?.teamOne?.length > 0 && rt?.teamTwo?.length > 0) {
          // 用重试获取到的数据继续后续逻辑
          const rTeamOne = rt.teamOne;
          const rTeamTwo = rt.teamTwo;
          return processTeamData(rTeamOne, rTeamTwo);
        }
        retried++;
        console.log(`[GameInfo] 队伍数据为空，重试 ${retried}/5`);
      }
      console.warn("[GameInfo] 重试 5 次后队伍数据仍为空");
      loading.value = false;
      return;
    }

    if (reqId !== currentGameflowSessionRequestId) return; // 拦截旧请求的执行
    await processTeamData(teamOne, teamTwo);
  } catch (e) {
    if (reqId !== currentGameflowSessionRequestId) return; // 拦截旧请求的执行
    console.error("加载 gameflow session 失败:", e);
    error.value = "加载对局数据失败";
  }
  loading.value = false;
}

/** 处理双方队伍数据：迁移己方缓存、注入队伍列表、加载玩家详情 */
async function processTeamData(teamOne: any[], teamTwo: any[]) {
    // 用当前玩家 summonerId 判断哪队是己方
    console.log("[GameInfo] currentSummonerId:", currentSummonerId.value);
    const isTeamOne = teamOne.some(
      (p: any) => p.summonerId === currentSummonerId.value,
    );
    const allyTeam = isTeamOne ? teamOne : teamTwo;
    const enemyTeam = isTeamOne ? teamTwo : teamOne;

    // 迁移 ChampSelect 阶段已加载的己方数据（key: cellId → summonerId）
    // 此时内存中的 gameflowMyTeam 仍保留着选人阶段的旧席位快照，避免依赖已置空的 store.champSelectSession
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

    // 将 gameflow session 的数据注入队伍列表
    // 使用 summonerId 作为 cellId（gameflow 无 cellId 字段）
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

    // 计算组队颜色
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

    // 并行加载双方全部玩家（己方若已迁移则 loadPlayerData 内部会跳过）
    await Promise.all([
      ...allyTeam.map((p: any) => loadPlayerData(p.summonerId, p.summonerId)),
      ...enemyTeam.map((p: any) => loadPlayerData(p.summonerId, p.summonerId)),
    ]);
}

function getChampionIcon(id: number): string {
  return id > 0 ? `/lol-game-data/assets/v1/champion-icons/${id}.png` : "";
}

/** 获取对局卡片颜色样式 */
function getMatchCardStyle(m: MatchDisplay): Record<string, string> {
  if (!appConfig.value?.Personalization) return {};

  const colors = appConfig.value.Personalization;
  let color = "";

  if (m.remake) {
    color = colors.RemakeCardColor || "";
  } else if (m.win) {
    color = colors.WinCardColor || "";
  } else {
    color = colors.LoseCardColor || "";
  }

  if (color) {
    // 处理不同的颜色格式
    if (color.startsWith("#") && color.length === 9) {
      // #AARRGGBB 格式：转换为 rgba()
      const alpha = parseInt(color.slice(1, 3), 16) / 255;
      const r = parseInt(color.slice(3, 5), 16);
      const g = parseInt(color.slice(5, 7), 16);
      const b = parseInt(color.slice(7, 9), 16);
      return { background: `rgba(${r}, ${g}, ${b}, ${alpha.toFixed(2)})` };
    } else if (color.startsWith("#") && color.length === 7) {
      // #RRGGBB 格式：添加半透明背景
      return { background: `${color}1a` }; // 1a = 约 10% 透明度
    }
    // 其他格式（rgba 等）直接使用
    return { background: color };
  }
  return {};
}

function formatRank(q: any): string {
  if (!q || !q.tier || q.tier === "NONE") return "";
  const tier = TIER_MAP.value[q.tier] || q.tier;
  const div = q.rank && q.rank !== "NA" ? q.rank : "";
  const lp = q.leaguePoints !== undefined ? ` ${q.leaguePoints} LP` : "";
  return `${tier}${div}${lp}`;
}

onMounted(async () => {
  // 1. 获取应用配置（用于判定是否加载持久化数据）
  if (!appConfig.value) {
    try {
      appConfig.value = await fetchConfig();
    } catch {
      /* ignore */
    }
  }

  // 2. 从 localStorage 加载上一局数据以支持持久化显示（仅在非游戏活跃且开启保留配置时恢复）
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

      // 从已加载的队伍数据恢复组队颜色
      if (gameflowMyTeam.value.length > 0)
        premadeColorsMy.value = computePremadeColors(gameflowMyTeam.value);
      if (gameflowTheirTeam.value.length > 0)
        premadeColorsTheir.value = computePremadeColors(gameflowTheirTeam.value);
    } catch {
      /* ignore */
    }
  }

  // 3. 获取当前玩家 summonerId，用于 InProgress 阶段分离队伍
  try {
    const s = await fetchCurrentSummoner();
    if (s?.summonerId) currentSummonerId.value = s.summonerId;
    if (s?.puuid) currentSummonerPuuid.value = s.puuid;
  } catch {
    /* ignore */
  }

  refreshState();
});

// 监听阶段变化：ChampSelect → 加载己方；InProgress/GameStart → 加载双方
watch(
  () => store.gamePhase,
  (phase: string) => {
    if (phase === "None" || phase === "Lobby") {
      isTftMode.value = false;
    }
    if (phase === "ChampSelect") {
      isTftMode.value = false;
      // 开启新一局选人时才清理上局快照数据以备载入新数据
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

// ChampSelect session 更新时加载己方玩家
watch(
  () => store.champSelectSession,
  (session: any) => {
    if (session && store.gamePhase === "ChampSelect") {
      loading.value = false;
      error.value = "";
      // 实现在 ChampSelect 选人更新时就同步填充 gameflowMyTeam 列表快照，防范进入游戏加载页时数据由于 Session 销毁发生短暂的清空闪烁
      gameflowMyTeam.value = session.myTeam || [];
      loadAllPlayers();
      // 获取组队颜色（gameflow session 在选人阶段也有 teamParticipantId）
      fetchPremadeColors();

      // 存储当前选人快照数据至 localStorage，以防秒退后头像等数据丢失
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

function getKdaClass(kda: number | undefined): string {
  if (kda === undefined) return "kda-gray";
  if (kda >= 5.0) return "kda-orange";
  if (kda >= 4.0) return "kda-blue";
  if (kda >= 3.0) return "kda-green";
  return "kda-gray";
}

function getWinRateClass(rate: number | undefined): string {
  if (rate === undefined) return "wr-low";
  if (rate >= 70) return "wr-high";
  if (rate >= 50) return "wr-medium";
  return "wr-low";
}

watch(activeTab, () => loadAllPlayers());

// 监听全局路由变化，当重新切回该页面且对局处于活跃状态时，主动触发数据拉取，防止缓存滞后
watch(
  () => store.currentPage,
  (newPage) => {
    if (newPage === "gameinfo") {
      console.log("[GameInfo] 页面切换到对局信息，执行主动刷新");
      if (store.gamePhase === "InProgress" || store.gamePhase === "GameStart") {
        loadFromGameflowSession();
      } else if (store.gamePhase === "ChampSelect") {
        loadAllPlayers();
      } else {
        refreshState();
      }
    }
  }
);
</script>

<template>
  <div class="game-info">
    <div v-if="!store.isConnected" class="tip-container">
      <div class="offline-logo">🎮</div>
      <p class="tip">{{ $t("gameInfo.launchLolPrompt") }}</p>
    </div>

    <div v-else-if="isTftMode" class="tip-container">
      <div class="offline-logo">♟️</div>
      <p class="tip">云顶之弈对局中，对局信息页面不显示数据</p>
    </div>

    <div v-else-if="!shouldShowContent" class="tip-container">
      <div class="offline-logo">⏳</div>
      <p class="tip">{{ $t("gameInfo.awaitingLoad") }}</p>
    </div>

    <div v-else class="game-layout">
      <!-- 左侧：队伍切换 + 玩家列表 -->
      <div class="left-panel">
        <div class="team-tabs">
          <button
            :class="['tab-btn', { active: activeTab === 'my' }]"
            @click="activeTab = 'my'"
          >
            {{ $t("gameInfo.myTeam", { count: myTeam.length }) }}
          </button>
          <button
            :class="['tab-btn', { active: activeTab === 'their' }]"
            @click="activeTab = 'their'"
          >
            {{ $t("gameInfo.theirTeam", { count: theirTeam.length }) }}
          </button>
        </div>

        <div class="player-list">
          <div
            v-for="(p, i) in currentTeam"
            :key="p.cellId ?? i"
            class="player-card"
            :class="{
              'premade-card': getPremadeIdx(p.summonerId, activeTab) >= 0,
            }"
            :style="getPremadeCardStyle(p.summonerId, activeTab)"
          >
            <div class="pc-avatar-area">
              <div class="profile-icon-wrapper-mini">
                <!-- 等级进度环形条 -->
                <svg class="gauge-ring-svg-mini" viewBox="0 0 100 100">
                  <circle class="gauge-track-mini" cx="50" cy="50" r="45" />
                  <circle
                    class="gauge-progress-mini"
                    cx="50"
                    cy="50"
                    r="45"
                    :style="{
                      '--progress':
                        playerData[p.cellId]?.info
                          ?.percentCompleteForNextLevel || 0,
                    }"
                  />
                </svg>
                <div class="avatar-container-mini">
                  <!-- 选人阶段：选了英雄或预选了英雄才显示英雄头像 -->
                  <template v-if="p.championId || p.championPickIntent">
                    <LcuImage
                      :src="
                        getChampionIcon(p.championId || p.championPickIntent)
                      "
                      class="profile-avatar-mini"
                      alt="champ"
                    />
                  </template>
                  <template v-else>
                    <div class="profile-avatar-mini profile-avatar-empty-mini">
                      ?
                    </div>
                  </template>
                </div>
                <!-- 等级数字 -->
                <div
                  v-if="playerData[p.cellId]?.info?.summonerLevel"
                  class="level-badge-mini"
                >
                  {{ playerData[p.cellId].info.summonerLevel }}
                </div>
              </div>
            </div>

            <div class="pc-info centered">
              <div class="pc-row pc-name-row-centered">
                <span
                  v-if="getPremadeIdx(p.summonerId, activeTab) >= 0"
                  class="premade-dot"
                  :style="{
                    background:
                      PREMADE_COLORS[
                        getPremadeIdx(p.summonerId, activeTab) %
                          PREMADE_COLORS.length
                      ].dot,
                  }"
                  :title="
                    t('gameInfo.premadeIdx', {
                      idx: getPremadeIdx(p.summonerId, activeTab) + 1,
                    })
                  "
                ></span>
                <span class="name-group">
                  <span class="name-text">{{
                    playerData[p.cellId]?.info?.gameName ||
                    playerData[p.cellId]?.info?.displayName ||
                    p.displayName ||
                    "未知"
                  }}</span>
                  <span
                    v-if="playerData[p.cellId]?.fateFlag"
                    :class="['fate-badge', playerData[p.cellId].fateFlag]"
                    :title="
                      playerData[p.cellId].recentlyChampionName
                        ? (playerData[p.cellId].fateFlag === 'ally'
                          ? `${$t('gameInfo.fateAllyTitle')} (使用: ${playerData[p.cellId].recentlyChampionName})`
                          : `${$t('gameInfo.fateEnemyTitle')} (使用: ${playerData[p.cellId].recentlyChampionName})`)
                        : (playerData[p.cellId].fateFlag === 'ally'
                          ? $t('gameInfo.fateAllyTitle')
                          : $t('gameInfo.fateEnemyTitle'))
                    "
                  >{{
                      playerData[p.cellId].fateFlag === "ally"
                        ? $t("gameInfo.fateAllyText")
                        : $t("gameInfo.fateEnemyText")
                    }}</span
                  >
                </span>
              </div>

              <div
                class="pc-row pc-winrate-row"
                v-if="playerData[p.cellId]?.winRate !== undefined"
              >
                <span
                  :class="[
                    'pc-winrate-badge-clean',
                    getWinRateClass(playerData[p.cellId].winRate),
                  ]"
                >
                  {{ playerData[p.cellId].winRate
                  }}{{ $t("gameInfo.winRateSuffix") }}
                </span>
              </div>

              <div
                class="pc-row pc-kda-row"
                v-if="playerData[p.cellId]?.avgKda !== undefined"
              >
                <span
                  :class="[
                    'pc-kda-text',
                    getKdaClass(playerData[p.cellId].avgKda),
                  ]"
                >
                  KDA: {{ playerData[p.cellId]?.avgKda?.toFixed(2) ?? "0.00" }}
                </span>
              </div>

              <div class="pc-row pc-rank-row">
                <span
                  class="rank-badge-clean"
                  :title="
                    formatRank(playerData[p.cellId]?.ranked?.solo) ||
                    $t('gameInfo.soloRankTitle')
                  "
                >
                  {{ $t("gameInfo.soloRank") }}:
                  {{
                    playerData[p.cellId]?.ranked?.solo?.tier
                      ? TIER_MAP[playerData[p.cellId].ranked.solo.tier]
                      : $t("gameInfo.noRank")
                  }}
                </span>
              </div>

              <div class="pc-row pc-rank-row">
                <span
                  class="rank-badge-clean"
                  :title="
                    formatRank(playerData[p.cellId]?.ranked?.flex) ||
                    $t('gameInfo.flexRankTitle')
                  "
                >
                  {{ $t("gameInfo.flexRank") }}:
                  {{
                    playerData[p.cellId]?.ranked?.flex?.tier
                      ? TIER_MAP[playerData[p.cellId].ranked.flex.tier]
                      : $t("gameInfo.noRank")
                  }}
                </span>
              </div>
            </div>
          </div>
          <div v-if="currentTeam.length === 0" class="tip">
            {{ $t("gameInfo.noTeamData") }}
          </div>
        </div>
        <!-- 组队图例 (按行交错配对) -->
        <div v-if="hasAnyPremadeInfo" class="premade-legend">
          <div
            v-for="(row, idx) in premadeRows"
            :key="idx"
            class="premade-legend-row"
          >
            <!-- 我方组队 (靠左) -->
            <div class="premade-slot ally-slot">
              <div
                v-if="row.ally"
                class="premade-group-chip"
                :style="{
                  borderColor: PREMADE_COLORS[row.ally.colorIdx % PREMADE_COLORS.length].border,
                  backgroundColor: PREMADE_COLORS[row.ally.colorIdx % PREMADE_COLORS.length].bg,
                }"
              >
                <span
                  class="legend-dot"
                  :style="{
                    background: PREMADE_COLORS[row.ally.colorIdx % PREMADE_COLORS.length].dot,
                  }"
                ></span>
                <div class="premade-avatars">
                  <template v-for="m in row.ally.members" :key="m.summonerId">
                    <LcuImage
                      v-if="m.championId > 0"
                      :src="getChampionIcon(m.championId)"
                      class="premade-avatar"
                      :title="m.displayName"
                    />
                    <div v-else class="premade-avatar premade-avatar-empty" :title="m.displayName">?</div>
                  </template>
                </div>
              </div>
            </div>

            <!-- 敌方组队 (靠右) -->
            <div class="premade-slot enemy-slot">
              <div
                v-if="row.enemy"
                class="premade-group-chip"
                :style="{
                  borderColor: PREMADE_COLORS[row.enemy.colorIdx % PREMADE_COLORS.length].border,
                  backgroundColor: PREMADE_COLORS[row.enemy.colorIdx % PREMADE_COLORS.length].bg,
                }"
              >
                <span
                  class="legend-dot"
                  :style="{
                    background: PREMADE_COLORS[row.enemy.colorIdx % PREMADE_COLORS.length].dot,
                  }"
                ></span>
                <div class="premade-avatars">
                  <template v-for="m in row.enemy.members" :key="m.summonerId">
                    <LcuImage
                      v-if="m.championId > 0"
                      :src="getChampionIcon(m.championId)"
                      class="premade-avatar"
                      :title="m.displayName"
                    />
                    <div v-else class="premade-avatar premade-avatar-empty" :title="m.displayName">?</div>
                  </template>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- 右侧：5 列战绩 -->
      <div class="right-panel">
        <div class="columns-container">
          <div
            v-for="(p, i) in currentTeam"
            :key="p.cellId ?? i"
            class="player-column"
          >
            <div class="col-header">
              <div class="col-header-top">
                <div class="col-header-info">
                  <span class="col-name">{{
                    playerData[p.cellId]?.info?.gameName ||
                    p.displayName ||
                    `玩家${i + 1}`
                  }}</span>
                  <span
                    v-if="playerData[p.cellId]?.winRate !== undefined"
                    class="col-summary"
                  >
                    <span class="summary-wins"
                      >{{ playerData[p.cellId].winCount }}胜</span
                    >
                    <span class="summary-losses"
                      >{{ playerData[p.cellId].lossesCount }}负</span
                    >
                  </span>
                </div>
              </div>
            </div>

            <template v-if="playerData[p.cellId]?.loading">
              <div class="col-loading">
                <div class="mini-spinner"></div>
              </div>
            </template>
            <template v-else-if="playerData[p.cellId]?.matches?.length">
              <div class="col-matches-list">
                <div
                  v-for="m in playerData[p.cellId].matches"
                  :key="m.gameId"
                  :class="[
                    'col-match',
                    m.remake ? 'remake' : m.win ? 'win' : 'lose',
                  ]"
                  :style="getMatchCardStyle(m)"
                >
                  <div class="cm-champ">
                    <LcuImage
                      :src="m.championIconUrl"
                      class="cm-champ-img"
                      alt="champ"
                    />
                    <span class="cm-level">{{ m.champLevel }}</span>
                  </div>
                  <div class="cm-detail">
                    <div class="cm-top-row">
                      <span class="cm-mode">{{ getQueueName(m.queueId, m.name) }}</span>
                    </div>
                    <div class="cm-bottom">
                      <span class="cm-kda">
                        <span class="k">{{ m.kills }}</span
                        >/ <span class="d">{{ m.deaths }}</span
                        >/
                        <span class="a">{{ m.assists }}</span>
                      </span>
                      <span class="cm-date">{{
                        m.shortTime.split(" ")[0]
                      }}</span>
                    </div>
                  </div>
                </div>
              </div>
            </template>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.game-info {
  padding: 1rem 1.5rem 1rem 0.6rem;
  background-color: transparent;
  flex: 1;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.page-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 1rem;
  flex-shrink: 0;
}
.page-title {
  font-size: 1.4rem;
  font-weight: 800;
  color: var(--text-color);
  margin: 0;
  letter-spacing: 0.5px;
}

.tip-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 6rem 2rem;
  color: var(--text-muted);
  flex: 1;
}
.offline-logo {
  font-size: 3rem;
  margin-bottom: 1rem;
}
.tip {
  font-size: 0.95rem;
  color: var(--text-dimmed);
  margin: 0;
}

/* 左右分栏 */
.game-layout {
  display: grid;
  grid-template-columns: 240px 1fr;
  gap: 16px;
  flex: 1;
  min-height: 0;
}

/* 左侧面板 */
.left-panel {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  overflow: hidden;
  box-shadow: var(--shadow-sm);
  display: flex;
  flex-direction: column;
  height: 100%;
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
}
.team-tabs {
  display: flex;
  border-bottom: 1px solid var(--border-color);
  background: rgba(0, 0, 0, 0.02);
}
.tab-btn {
  flex: 1;
  padding: 14px;
  border: none;
  background: transparent;
  font-size: 0.85rem;
  font-weight: 700;
  color: var(--text-muted);
  cursor: pointer;
  border-bottom: 2px solid transparent;
  transition: all 0.2s ease-in-out;
}
.tab-btn:hover {
  background: rgba(0, 0, 0, 0.03);
  color: var(--text-color);
}
.tab-btn.active {
  color: var(--primary-color);
  border-bottom-color: var(--primary-color);
  background: var(--card-bg);
}

/* 组队图例：逐行配对交错显示 */
.premade-legend {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 6px 12px;
  border-top: 1px solid var(--border-color);
  flex-shrink: 0;
  background: rgba(0, 0, 0, 0.03);
  box-sizing: border-box;
}

.premade-legend-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  width: 100%;
}

.premade-slot {
  display: flex;
  align-items: center;
  flex: 1;
  min-width: 0;
}

.premade-slot.ally-slot {
  justify-content: flex-start;
}

.premade-slot.enemy-slot {
  justify-content: flex-end;
}

.premade-group-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 3px 7px;
  border-width: 1px;
  border-style: solid;
  border-radius: 16px;
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.08);
}

.legend-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.premade-avatars {
  display: flex;
  align-items: center;
  gap: 4px;
}

.premade-avatar {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  object-fit: cover;
  border: 1.5px solid rgba(255, 255, 255, 0.3);
  box-sizing: border-box;
}

.premade-avatar-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(255, 255, 255, 0.15);
  color: var(--text-dimmed);
  font-size: 0.7rem;
  font-weight: 700;
}

.player-list {
  display: flex;
  flex-direction: column;
  flex: 1;
  padding: 12px;
  gap: 10px;
  overflow: hidden;
}

/* 玩家卡片卡片化升级 */
.player-card {
  flex: 1;
  min-height: 0;
  max-height: 142px;
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 12px 16px;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  transition: all 0.25s cubic-bezier(0.25, 0.8, 0.25, 1);
  box-sizing: border-box;
  box-shadow: var(--shadow-sm);
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
}
.player-card:hover {
  background: var(--card-bg-hover);
  transform: translateY(-2px) scale(1.01);
  border-color: var(--primary-color-alpha-40);
  box-shadow:
    var(--shadow-md),
    0 4px 16px var(--primary-color-alpha-15);
}

/* 组队标记圆点 */
.premade-dot {
  display: inline-block;
  width: 6px;
  height: 6px;
  border-radius: 50%;
  margin-right: 4px;
  vertical-align: middle;
  box-shadow: 0 0 4px currentColor;
}

/* 宿命标记徽章（上一局队友/对手） */
.fate-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 0.6rem;
  font-weight: 800;
  width: 14px;
  height: 14px;
  border-radius: 3px;
  margin-left: 4px;
  vertical-align: middle;
  flex-shrink: 0;
  letter-spacing: 0;
}
.fate-badge.ally {
  background: rgba(5, 119, 72, 0.18);
  color: #10b981;
  border: 1px solid rgba(16, 185, 129, 0.4);
}
.fate-badge.enemy {
  background: rgba(191, 36, 42, 0.15);
  color: #f87171;
  border: 1px solid rgba(248, 113, 113, 0.4);
}

/* 头像区域：等级圆环 mini 版 */
.profile-icon-wrapper-mini {
  position: relative;
  width: 58px;
  height: 58px;
  flex-shrink: 0;
  margin: 0 auto;
}
.gauge-ring-svg-mini {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  overflow: visible;
}
.gauge-track-mini,
.gauge-progress-mini {
  fill: none;
  stroke-width: 5;
  stroke-linecap: round;
  stroke-dasharray: 235.62 282.74;
  transform: rotate(120deg);
  transform-origin: center;
}
.gauge-track-mini {
  stroke: var(--border-color);
}
.gauge-progress-mini {
  stroke: var(--primary-color);
  stroke-dashoffset: calc(235.62px * (1 - var(--progress) / 100));
  transition: stroke-dashoffset 0.8s ease;
}
.avatar-container-mini {
  position: absolute;
  inset: 7px;
  border-radius: 50%;
  overflow: hidden;
}
.profile-avatar-mini {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}
.profile-avatar-empty-mini {
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--hover-bg);
  color: var(--text-dimmed);
  font-size: 0.85rem;
  font-weight: 700;
}
.level-badge-mini {
  position: absolute;
  bottom: -2px;
  left: 50%;
  transform: translateX(-50%);
  color: var(--text-color);
  font-size: 0.65rem;
  font-weight: 800;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  padding: 0 4px;
  border-radius: 4px;
  z-index: 2;
  white-space: nowrap;
  line-height: 1.2;
}

/* 右侧文本完全居中，一行一个样式 */
.pc-info.centered {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  text-align: center;
  gap: 5px;
  width: 100%;
}
.pc-row {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  min-height: 20px;
}
.name-group {
  display: inline-flex;
  align-items: center;
  min-width: 0;
  max-width: 100%;
}
.name-text {
  font-size: 0.82rem;
  font-weight: 700;
  color: var(--text-color);
  max-width: 110px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.pc-winrate-badge-clean {
  font-size: 0.72rem;
  font-weight: 600;
  color: var(--text-muted);
}
.pc-kda-text {
  font-size: 0.72rem;
  font-weight: 700;
}
.pc-kda-text.kda-orange {
  color: var(--accent-color);
}
.pc-kda-text.kda-blue {
  color: var(--tier-blue);
}
.pc-kda-text.kda-green {
  color: var(--win-color);
}
.pc-kda-text.kda-gray {
  color: var(--text-dimmed);
}

.rank-badge-clean {
  font-size: 0.7rem;
  font-weight: 600;
  color: var(--text-dimmed);
}

/* 右侧：5 列战绩 */
.right-panel {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
  overflow: hidden;
  height: 100%;
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
}
.columns-container {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  height: 100%;
}

.player-column {
  border-right: 1px solid var(--border-color);
  border-top: 3px solid var(--border-color);
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}
.player-column:last-child {
  border-right: none;
}

.col-header {
  height: 58px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--border-color);
  background: rgba(0, 0, 0, 0.02);
  flex-shrink: 0;
  display: flex;
  align-items: center;
  box-sizing: border-box;
}
.col-header-top {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
}
.col-header-info {
  display: flex;
  flex-direction: column;
  min-width: 0;
  flex: 1;
}
.col-name {
  font-size: 0.8rem;
  font-weight: 700;
  color: var(--text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.col-summary {
  font-size: 0.68rem;
  color: var(--text-muted);
  margin-top: 2px;
}
.summary-wins {
  color: var(--win-color);
  font-weight: bold;
}
.summary-losses {
  color: var(--loss-color);
  font-weight: bold;
}

.col-loading {
  display: flex;
  align-items: center;
  justify-content: center;
  flex: 1;
}
.mini-spinner {
  width: 22px;
  height: 22px;
  border: 2px solid rgba(0, 0, 0, 0.06);
  border-top-color: var(--primary-color);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}
@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.col-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  flex: 1;
  font-size: 0.75rem;
  color: var(--text-dimmed);
}

.col-matches-list {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  gap: 6px;
  padding: 6px 6px;
  box-sizing: border-box;
}

.col-match {
  flex: 1;
  min-height: 0;
  max-height: 72px;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 8px;
  margin: 0;
  border-radius: 8px;
  border: 1px solid transparent;
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.02);
  transition: all 0.2s ease-in-out;
  box-sizing: border-box;
}

/* 胜/败/重赛 玻璃渐变色背景 */
.col-match.win {
  border-color: var(--win-border);
}
.col-match.win:hover {
  border-color: var(--win-color);
  box-shadow: var(--win-glow);
  transform: translateY(-1px);
}

.col-match.lose {
  border-color: var(--loss-border);
}
.col-match.lose:hover {
  border-color: var(--loss-color);
  box-shadow: var(--loss-glow);
  transform: translateY(-1px);
}

.col-match.remake {
  border-color: var(--border-color);
}

/* 头像区域 */
.cm-champ {
  position: relative;
  width: 34px;
  height: 34px;
  flex-shrink: 0;
}
.cm-champ-img {
  width: 34px;
  height: 34px;
  border-radius: 50%;
  overflow: hidden;
  border: 1px solid var(--border-color);
}
.cm-level {
  position: absolute;
  bottom: -2px;
  right: -2px;
  width: 13px;
  height: 13px;
  line-height: 11px;
  background: var(--card-bg);
  color: var(--text-color);
  border-radius: 50%;
  font-size: 0.55rem;
  font-weight: bold;
  text-align: center;
  border: 1px solid var(--border-color);
  box-shadow: var(--shadow-sm);
}

/* 右侧详情 */
.cm-detail {
  display: flex;
  flex-direction: column;
  min-width: 0;
  flex: 1;
  gap: 2px;
}
.cm-top-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.cm-mode {
  font-size: 0.72rem;
  font-weight: 700;
  color: var(--text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.cm-bottom {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-top: 1px;
  white-space: nowrap;
  overflow: hidden;
}
.cm-kda {
  font-size: 0.7rem;
  font-weight: 700;
  color: var(--text-muted);
  letter-spacing: 0.1px;
  white-space: nowrap;
}
.cm-kda .d {
  color: var(--death-color, #ef4444); /* 强调死亡数 */
}
.cm-date {
  font-size: 0.65rem;
  color: var(--text-dimmed);
  white-space: nowrap;
}
</style>
