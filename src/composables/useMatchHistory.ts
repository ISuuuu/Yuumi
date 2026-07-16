import { ref, computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  fetchCurrentSummoner,
  fetchMatchHistory,
  fetchMatchHistorySgp,
  fetchRecentTeammates,
  lcuRequest,
} from "../api/lcu";
import type { SummonerDisplay, MatchDisplay, RecentTeammate } from "../api/lcu";

// 模块作用域内存缓存单例
let cachedSummoner: SummonerDisplay | null = null;
let cachedMatches: MatchDisplay[] = [];
let cachedRecentMatches: MatchDisplay[] = [];
let cachedRankedQueues: any[] = [];
let lastFetchedTime = 0;

export function useMatchHistory() {
  const { t, te } = useI18n();

  const summoner = ref<SummonerDisplay | null>(null);
  const matches = ref<MatchDisplay[]>([]);
  const recentMatches = ref<MatchDisplay[]>([]);
  const rankedQueues = ref<any[]>([]);
  const loading = ref(false);
  const error = ref("");
  const copied = ref(false);
  const careerGamesNumber = ref(20);

  // 游戏模式筛选
  const selectedQueue = ref<number | null>(null);
  const QUEUE_OPTIONS = [
    { id: null, label: "全部" },
    { id: 2400, label: "海克斯大乱斗" },
    { id: 450, label: "极地大乱斗" },
    { id: 430, label: "匹配模式" },
    { id: 420, label: "单双排位" },
    { id: 440, label: "灵活排位" },
  ];
  const showQueueDropdown = ref(false);

  const TIER_MAP: Record<string, string> = {
    NONE: "无段位",
    IRON: "坚韧黑铁",
    BRONZE: "英勇黄铜",
    SILVER: "不屈白银",
    GOLD: "荣耀黄金",
    PLATINUM: "华贵铂金",
    EMERALD: "流光翡翠",
    DIAMOND: "璀璨钻石",
    MASTER: "超凡大师",
    GRANDMASTER: "傲世宗师",
    CHALLENGER: "最强王者",
  };

  // 计算属性
  const filteredMatches = computed(() => {
    if (selectedQueue.value === null) return matches.value;
    return matches.value.filter(
      (m: MatchDisplay) => m.queueId === selectedQueue.value,
    );
  });

  const soloQueue = computed(() => {
    return rankedQueues.value.find((q) => q.queueType === "RANKED_SOLO_5x5") || null;
  });

  const flexQueue = computed(() => {
    return rankedQueues.value.find((q) => q.queueType === "RANKED_FLEX_SR") || null;
  });

  const statsSummary = computed(() => {
    if (recentMatches.value.length === 0) return null;
    let wins = 0;
    let losses = 0;
    let kills = 0;
    let deaths = 0;
    let assists = 0;
    const champMap: Record<number, { id: number; icon: string; count: number }> = {};

    for (const m of recentMatches.value) {
      if (m.win) wins++;
      else losses++;
      kills += m.kills;
      deaths += m.deaths;
      assists += m.assists;

      if (!champMap[m.championId]) {
        champMap[m.championId] = {
          id: m.championId,
          icon: m.championIconUrl,
          count: 0,
        };
      }
      champMap[m.championId].count++;
    }

    const topChamps = Object.values(champMap)
      .sort((a, b) => b.count - a.count)
      .slice(0, 6);

    const kdaRatio =
      deaths === 0 ? "Perfect" : ((kills + assists) / deaths).toFixed(1);

    return { wins, losses, kills, deaths, assists, kda: kdaRatio, topChamps };
  });

  // 最近队友
  const loadingTeammates = ref(false);
  const recentTeammates = ref<RecentTeammate[]>([]);
  let currentTeammatePuuid = "";

  // ─── 数据加载 ───

  const MATCHES_CACHE_KEY = (puuid: string) => `yuumi_matches_cache_${puuid}`;

  async function fetchMatchHistoryWithFallback(
    puuid: string,
    begIndex: number,
    endIndex: number,
    isGameEndSync = false,
  ): Promise<MatchDisplay[]> {
    let raw = await fetchMatchHistory(puuid, begIndex, endIndex);
    const prevLatestId =
      recentMatches.value[0]?.gameId ?? matches.value[0]?.gameId ?? null;
    const latestId = raw[0]?.gameId ?? null;

    const shouldFallback =
      raw.length === 0 ||
      (isGameEndSync && prevLatestId && latestId === prevLatestId);

    if (shouldFallback) {
      try {
        const sgpRaw = await fetchMatchHistorySgp(puuid, begIndex, endIndex);
        if (sgpRaw && sgpRaw.length > 0) {
          console.log("[Career] LCU 战绩未更新，已降级 SGP 加速拉取新战绩");
          raw = sgpRaw;
        }
      } catch (e) {
        console.warn("[Career] SGP 战绩降级/加速获取失败:", e);
      }
    }
    return raw;
  }

  async function loadSummoner(forceRefresh = false) {
    const now = Date.now();
    if (!forceRefresh && cachedSummoner && now - lastFetchedTime < 20000) {
      summoner.value = cachedSummoner;
      rankedQueues.value = cachedRankedQueues;
      matches.value = cachedMatches;
      recentMatches.value = cachedRecentMatches;
      return;
    }

    loading.value = true;
    error.value = "";
    try {
      summoner.value = await fetchCurrentSummoner();
      if (summoner.value?.puuid) {
        await Promise.all([
          loadRankedStats(summoner.value.puuid),
          loadMatches(summoner.value.puuid),
          loadRecentMatches(summoner.value.puuid),
        ]);

        cachedSummoner = summoner.value;
        cachedRankedQueues = rankedQueues.value;
        cachedMatches = matches.value;
        cachedRecentMatches = recentMatches.value;
        lastFetchedTime = Date.now();
      }
    } catch (e: any) {
      error.value = e.toString();
    } finally {
      loading.value = false;
    }
  }

  async function loadRankedStats(puuid: string) {
    try {
      const resp = await lcuRequest<any>(
        "GET",
        `/lol-ranked/v1/ranked-stats/${puuid}`,
      );
      if (resp.success && resp.data && resp.data.queues) {
        rankedQueues.value = resp.data.queues;
        cachedRankedQueues = rankedQueues.value;
      }
    } catch (e) {
      console.error("获取排位段位数据失败:", e);
    }
  }

  async function loadMatches(puuid: string, isGameEndSync = false) {
    try {
      loading.value = true;
      const targetCount = careerGamesNumber.value;
      matches.value = await fetchMatchHistoryWithFallback(
        puuid, 0, targetCount - 1, isGameEndSync,
      );
      cachedMatches = matches.value;
    } catch (e) {
      console.error("获取战绩历史失败:", e);
    } finally {
      loading.value = false;
    }
  }

  async function loadRecentMatches(puuid: string, isGameEndSync = false) {
    try {
      const targetCount = careerGamesNumber.value;
      const fresh = await fetchMatchHistoryWithFallback(
        puuid, 0, targetCount, isGameEndSync,
      );
      updateRecentMatchesCache(puuid, fresh);
    } catch (e) {
      console.error("获取近期战绩统计失败:", e);
    }
  }

  function updateRecentMatchesCache(puuid: string, fresh: MatchDisplay[]) {
    let cached: MatchDisplay[] = [];
    try {
      const raw = localStorage.getItem(MATCHES_CACHE_KEY(puuid));
      if (raw) cached = JSON.parse(raw);
    } catch { /* ignore */ }

    const seen = new Set<number>();
    const merged = [...fresh, ...cached]
      .filter((m) => {
        if (seen.has(m.gameId)) return false;
        seen.add(m.gameId);
        return true;
      })
      .sort((a, b) => b.timeStamp - a.timeStamp)
      .slice(0, careerGamesNumber.value);

    recentMatches.value = merged;
    cachedRecentMatches = merged;

    try {
      localStorage.setItem(MATCHES_CACHE_KEY(puuid), JSON.stringify(merged));
    } catch { /* ignore */ }

    calculateRecentTeammates();
  }

  async function calculateRecentTeammates() {
    if (!summoner.value?.puuid || recentMatches.value.length === 0) {
      recentTeammates.value = [];
      return;
    }

    const targetPuuid = summoner.value.puuid;
    currentTeammatePuuid = targetPuuid;
    loadingTeammates.value = true;
    try {
      const gameIds = recentMatches.value.map((m) => m.gameId);
      const resp = await fetchRecentTeammates(gameIds, targetPuuid);
      if (currentTeammatePuuid === targetPuuid) {
        recentTeammates.value = resp.summoners || [];
      }
    } catch (err) {
      console.error("计算最近队友失败:", err);
      if (currentTeammatePuuid === targetPuuid) {
        recentTeammates.value = [];
      }
    } finally {
      if (currentTeammatePuuid === targetPuuid) {
        loadingTeammates.value = false;
      }
    }
  }

  // ─── 辅助函数 ───

  function selectQueue(id: number | null) {
    selectedQueue.value = id;
    showQueueDropdown.value = false;
  }

  function formatRank(queue: any) {
    if (!queue || !queue.tier || queue.tier === "NONE") return "--";
    const tierCn = TIER_MAP[queue.tier] || queue.tier;
    const division = queue.rank === "NA" ? "" : " " + queue.rank;
    return `${tierCn}${division}`;
  }

  function formatHighestRank(queue: any) {
    if (!queue || !queue.highestTier || queue.highestTier === "NONE") return "--";
    return TIER_MAP[queue.highestTier] || queue.highestTier;
  }

  function formatPrevSeasonRank(queue: any) {
    if (!queue || !queue.previousSeasonEndTier || queue.previousSeasonEndTier === "NONE") return "--";
    return TIER_MAP[queue.previousSeasonEndTier] || queue.previousSeasonEndTier;
  }

  async function copyRiotId() {
    if (!summoner.value) return;
    const fullId = `${summoner.value.gameName || summoner.value.displayName}#${summoner.value.tagLine}`;
    try {
      await navigator.clipboard.writeText(fullId);
      copied.value = true;
      setTimeout(() => { copied.value = false; }, 1500);
    } catch {
      const ta = document.createElement("textarea");
      ta.value = fullId;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand("copy");
      document.body.removeChild(ta);
      copied.value = true;
      setTimeout(() => { copied.value = false; }, 1500);
    }
  }

  function getSpellIcon(m: MatchDisplay, slot: 1 | 2): string {
    return slot === 1 ? m.spell1IconUrl : m.spell2IconUrl;
  }

  function formatTime(ts: number): string {
    const d = new Date(ts);
    const pad = (n: number) => n.toString().padStart(2, "0");
    return `${d.getFullYear()}/${pad(d.getMonth() + 1)}/${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
  }

  function translateMapName(name: string): string {
    if (!name) return "";
    if (name.includes("峡谷") || name.includes("Rift")) return t("maps.11");
    if (name.includes("深渊") || name.includes("Abyss")) return t("maps.12");
    if (name.includes("闪击") || name.includes("Blitz")) return t("maps.21");
    if (name.includes("大厅") || name.includes("Lobby")) return t("maps.22");
    return name;
  }

  function getQueueName(m: MatchDisplay): string {
    const key = `gameModes.${m.queueId}`;
    if (te(key)) {
      const translation = t(key);
      if (
        (translation.includes("云顶") || translation.includes("TFT")) &&
        !m.name.includes("云顶") && !m.name.includes("TFT")
      ) {
        return m.name;
      }
      return translation;
    }
    return m.name;
  }

  function getKdaClass(kda: string): string {
    const val = parseFloat(kda);
    if (isNaN(val)) return "kda-perfect";
    if (val >= 5) return "kda-great";
    if (val >= 3) return "kda-good";
    return "kda-normal";
  }

  // 清除缓存
  function clearCache() {
    cachedSummoner = null;
    cachedMatches = [];
    cachedRecentMatches = [];
    cachedRankedQueues = [];
    lastFetchedTime = 0;
  }

  return {
    // 状态
    summoner,
    matches,
    recentMatches,
    rankedQueues,
    loading,
    error,
    copied,
    careerGamesNumber,
    selectedQueue,
    QUEUE_OPTIONS,
    showQueueDropdown,
    filteredMatches,
    soloQueue,
    flexQueue,
    statsSummary,
    loadingTeammates,
    recentTeammates,

    // 方法
    loadSummoner,
    loadMatches,
    loadRecentMatches,
    loadRankedStats,
    selectQueue,
    formatRank,
    formatHighestRank,
    formatPrevSeasonRank,
    copyRiotId,
    getSpellIcon,
    formatTime,
    translateMapName,
    getQueueName,
    getKdaClass,
    clearCache,
    calculateRecentTeammates,
    fetchMatchHistoryWithFallback,
  };
}
