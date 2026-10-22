<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted, inject, type Ref } from "vue";
import { useLcuStore } from "../../store/lcuStore";
import { fetchRecentTeammates, fetchConfig } from "../../api/lcu";
import type { SummonerDisplay, MatchDisplay, RecentTeammate } from "../../api/lcu";
import type { RankDisplaySource, RankedQueueEntry } from "../../types/lcu";
import LcuImage from "../LcuImage.vue";
import PositionIcon from "../PositionIcon.vue";
import RankedIcon from "../RankedIcon.vue";
import MatchCard from "../match-card/MatchCard.vue";
import { formatLocalDateTime } from "../../utils/matchTime";
import { useVirtualList } from "../../composables/useVirtualList";
import {
  computeAggregatedPositions,
  computeAggregatedChampions,
  POSITION_NAMES,
  type AggregatedPositions,
  type AggregatedChampion,
} from "../../utils/matchAnalysis";

const store = useLcuStore();

// 从 Career.vue inject 共享的 composable 状态（单一实例）
const mh = inject<{
  summoner: Ref<SummonerDisplay | null>;
  matches: Ref<MatchDisplay[]>;
  recentMatches: Ref<MatchDisplay[]>;
  rankedQueues: Ref<RankedQueueEntry[]>;
  loading: Ref<boolean>;
  isViewingOther: Ref<boolean>;
  loadSummoner: (force?: boolean) => Promise<void>;
  loadCareerSummoner: (puuid: string, force?: boolean, preloadedSummoner?: SummonerDisplay) => Promise<void>;
  backToMyCareer: () => Promise<void>;
  loadCareerData: (puuid: string, sync?: boolean) => Promise<void>;
  loadRankedStats: (puuid: string) => Promise<void>;
  clearCache: () => void;
  fetchMatchHistoryWithFallback: (puuid: string, beg: number, end: number, sync?: boolean) => Promise<MatchDisplay[]>;
}>("matchHistoryState")!;

const { summoner, matches, recentMatches, rankedQueues, loading, loadCareerSummoner } = mh;

// ─── 本地 UI 状态（仅本组件使用）───
const careerGamesNumber = ref(20);
const selectedQueue = ref<number | null>(null);
const showQueueDropdown = ref(false);
const loadingTeammates = ref(false);
const recentTeammates = ref<RecentTeammate[]>([]);
let currentTeammatePuuid = "";
let lastCalculatedGameIds = "";

const QUEUE_OPTIONS = [
  { id: null, label: "全部" },
  { id: 2400, label: "海克斯大乱斗" },
  { id: 2450, label: "经典海斗" },
  { id: 450, label: "极地大乱斗" },
  { id: 430, label: "匹配模式" },
  { id: 420, label: "单双排位" },
  { id: 440, label: "灵活排位" },
];

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

// ─── 计算属性 ───
const filteredMatches = computed(() => {
  if (selectedQueue.value === null) return matches.value;
  return matches.value.filter((m: MatchDisplay) => m.queueId === selectedQueue.value);
});

const soloQueue = computed(() =>
  rankedQueues.value.find((q) => q.queueType === "RANKED_SOLO_5x5") || null,
);

const flexQueue = computed(() =>
  rankedQueues.value.find((q) => q.queueType === "RANKED_FLEX_SR") || null,
);

const aggregatedPositions = computed<AggregatedPositions | null>(() => {
  return computeAggregatedPositions(recentMatches.value);
});

const aggregatedTopChamps = computed<AggregatedChampion[]>(() => {
  return computeAggregatedChampions(recentMatches.value, 6);
});

// ─── 虚拟列表状态 ───
// 单张卡片固定高度：头像 52px + 上下 padding 14px*2 + 列表 gap 10px ≈ 90px
const VIRTUAL_ITEM_HEIGHT = 90;
const scrollTop = ref(0);
const viewportHeight = ref(0);
const scrollContainerRef = ref<HTMLElement | null>(null);

const virtualList = useVirtualList({
  total: computed(() => filteredMatches.value.length),
  scrollTop,
  viewportHeight,
  itemHeight: VIRTUAL_ITEM_HEIGHT,
  overscan: 3,
});
const { containerHeight, offsetY, visibleItems } = virtualList;

function onScroll() {
  const el = scrollContainerRef.value;
  if (!el) return;
  scrollTop.value = el.scrollTop;
  viewportHeight.value = el.clientHeight;
}

function updateViewportHeight() {
  const el = scrollContainerRef.value;
  if (el) viewportHeight.value = el.clientHeight;
}

const statsSummary = computed(() => {
  if (recentMatches.value.length === 0) return null;
  let wins = 0, losses = 0, kills = 0, deaths = 0, assists = 0;

  for (const m of recentMatches.value) {
    if (m.win) wins++;
    else losses++;
    kills += m.kills;
    deaths += m.deaths;
    assists += m.assists;
  }

  const kdaRatio = deaths === 0 ? "Perfect" : ((kills + assists) / deaths).toFixed(1);
  return { wins, losses, kills, deaths, assists, kda: kdaRatio };
});

// ─── 辅助函数 ───
function selectQueue(id: number | null) {
  selectedQueue.value = id;
  showQueueDropdown.value = false;
}

function formatRank(queue: RankDisplaySource | null) {
  if (!queue || !queue.tier || queue.tier === "NONE") return "--";
  const tierCn = TIER_MAP[queue.tier] || queue.tier;
  const division = !queue.rank || queue.rank === "NA" ? "" : " " + queue.rank;
  return `${tierCn}${division}`;
}

function formatHighestRank(queue: RankDisplaySource | null) {
  if (!queue || !queue.highestTier || queue.highestTier === "NONE") return "--";
  const tierCn = TIER_MAP[queue.highestTier] || queue.highestTier;
  const division = !queue.highestRank || queue.highestRank === "NA" ? "" : " " + queue.highestRank;
  return `${tierCn}${division}`;
}

function formatPrevSeasonRank(queue: RankDisplaySource | null) {
  if (!queue || !queue.previousSeasonEndTier || queue.previousSeasonEndTier === "NONE") return "--";
  const tierCn = TIER_MAP[queue.previousSeasonEndTier] || queue.previousSeasonEndTier;
  const division = !queue.previousSeasonEndRank || queue.previousSeasonEndRank === "NA" ? "" : " " + queue.previousSeasonEndRank;
  return `${tierCn}${division}`;
}

async function calculateRecentTeammates() {
  if (!summoner.value?.puuid || recentMatches.value.length === 0) {
    recentTeammates.value = [];
    lastCalculatedGameIds = "";
    return;
  }
  const targetPuuid = summoner.value.puuid;
  const gameIds = recentMatches.value.map((m) => m.gameId);
  const gameIdsKey = `${targetPuuid}_${gameIds.join(",")}`;

  currentTeammatePuuid = targetPuuid;
  loadingTeammates.value = true;
  try {
    const resp = await fetchRecentTeammates(gameIds, targetPuuid);
    if (currentTeammatePuuid === targetPuuid) {
      recentTeammates.value = resp.summoners || [];
      lastCalculatedGameIds = gameIdsKey;
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

function handleTeammatesPopoverShow(show: boolean) {
  if (!show) return;
  const currentKey = summoner.value?.puuid
    ? `${summoner.value.puuid}_${recentMatches.value.map((m) => m.gameId).join(",")}`
    : "";
  if (
    !loadingTeammates.value &&
    (recentTeammates.value.length === 0 || lastCalculatedGameIds !== currentKey)
  ) {
    calculateRecentTeammates();
  }
}

// ─── 导航函数 ───
const navigateSearchPayload = inject<
  Ref<{ name: string; gameId: number | null } | null>
>("navigateSearchPayload")!;
const navigateTo = inject<(page: string) => void>("navigateTo");

function goToMatchDetail(gameId: number) {
  if (!summoner.value) return;
  const name = summoner.value.gameName || summoner.value.displayName;
  const fullName = summoner.value.tagLine
    ? `${name}#${summoner.value.tagLine}`
    : name;
  navigateSearchPayload.value = { name: fullName, gameId };
}

function handleClickTeammate(tm: RecentTeammate) {
  if (tm.puuid) {
    let gameName = tm.name;
    let tagLine = "";
    if (tm.name.includes("#")) {
      const parts = tm.name.split("#");
      gameName = parts[0];
      tagLine = parts.slice(1).join("#");
    }
    loadCareerSummoner(tm.puuid, false, {
      accountId: 0,
      displayName: tm.name,
      gameName,
      tagLine,
      percentCompleteForNextLevel: 0,
      profileIconId: 29,
      puuid: tm.puuid,
      summonerId: 0,
      summonerLevel: 0,
      xpSinceLastLevel: 0,
      xpUntilNextLevel: 0,
      profileIconUrl: tm.icon || "/lol-game-data/assets/v1/profile-icons/29.jpg",
    });
    return;
  }
  if (navigateSearchPayload) {
    navigateSearchPayload.value = { name: tm.name, gameId: -1 };
  }
  if (navigateTo) {
    navigateTo("search");
  }
}

// 点击外部关闭下拉菜单
function onDocClick() {
  showQueueDropdown.value = false;
}

// ─── 生命周期与监听 ───

onMounted(async () => {
  try {
    const cfg = await fetchConfig();
    careerGamesNumber.value = cfg.Functions?.CareerGamesNumber ?? 20;
  } catch (e) {
    console.warn("加载 CareerGamesNumber 配置失败，使用默认值 20:", e);
  }
  document.addEventListener("click", onDocClick);
  window.addEventListener("resize", updateViewportHeight);
  updateViewportHeight();
});

onUnmounted(() => {
  document.removeEventListener("click", onDocClick);
  window.removeEventListener("resize", updateViewportHeight);
});

// 召唤师变更时清空队友缓存
watch(
  () => summoner.value?.puuid,
  (newPuuid, oldPuuid) => {
    if (newPuuid !== oldPuuid) {
      recentTeammates.value = [];
      lastCalculatedGameIds = "";
    }
  },
);

// 对局结束后或新对局开始时自动刷新战绩列表（召唤师数据由 Career.vue 统一刷新）
let isGameEndSyncing = false;
watch(
  () => store.gameEndedTrigger,
  async (trigger) => {
    if (!trigger || !summoner.value?.puuid || isGameEndSyncing) return;
    isGameEndSyncing = true;
    const puuid = summoner.value.puuid;
    const prevLatestId =
      recentMatches.value[0]?.gameId ?? matches.value[0]?.gameId ?? null;
    console.log(
      `[Career] 对局结束信号触发，开始快速拉取最新战绩，当前最新 gameId:`,
      prevLatestId,
    );

    try {
      // 立即尝试首次拉取（SGP 权威接口通常在结算时已存在该对局）
      await mh.loadCareerData(puuid, true);
      const firstId = matches.value[0]?.gameId ?? null;
      if (firstId && firstId !== prevLatestId) {
        console.log(`[Career] 立即同步成功，已更新最新对局 ${firstId}`);
        await calculateRecentTeammates();
        return;
      }

      // 若首次拉取未刷新，等待 1.5 秒后以更密集间隔（1.5s、2s、3s、3s）重试
      const retryDelays = [1500, 2000, 3000, 3000];
      for (let attempt = 0; attempt < retryDelays.length; attempt++) {
        await new Promise((r) => setTimeout(r, retryDelays[attempt]));
        try {
          await mh.loadCareerData(puuid, true);
          const latestId = matches.value[0]?.gameId ?? null;
          if (latestId && latestId !== prevLatestId) {
            console.log(
              `[Career] 第 ${attempt + 1} 次重试已检测到新对局 ${latestId}`,
            );
            await calculateRecentTeammates();
            return;
          }
          console.log(
            `[Career] 第 ${attempt + 1} 次重试尚未发现新对局，继续等待`,
          );
        } catch (e) {
          console.warn(`[Career] 第 ${attempt + 1} 次刷新重试失败:`, e);
        }
      }
      await mh.loadRankedStats(puuid);
      await calculateRecentTeammates();
    } finally {
      isGameEndSyncing = false;
    }
  },
);

// 新对局开始时（例如进入BP阶段），重置缓存并拉取最新战绩，避免仍停留在上一把更早的战绩
watch(
  () => store.newGameStartedTrigger,
  async (trigger) => {
    if (!trigger || !summoner.value?.puuid) return;
    console.log("[Career] 检测到新对局开始，静默刷新最新战绩");
    try {
      await mh.loadCareerData(summoner.value.puuid, false);
      await calculateRecentTeammates();
    } catch (e) {
      console.warn("[Career] 新对局静默刷新战绩失败:", e);
    }
  },
);
</script>

<template>
  <div class="match-history-tab">
    <!-- 加载中：召唤师数据尚未就绪 -->
    <div v-if="!summoner && loading" class="tip-container">
      <n-spin size="large" />
      <p class="tip" style="margin-top: 1rem;">{{ $t("career.loading") || "加载中..." }}</p>
    </div>

    <!-- 空态：加载完成但无数据 -->
    <div v-else-if="!summoner" class="tip-container">
      <p class="tip">{{ $t("career.empty") }}</p>
    </div>

    <template v-else>
    <!-- 排位段位信息表 -->
    <div class="rank-table-wrapper">
      <table class="rank-table">
        <thead>
          <tr>
            <th>{{ $t("career.type") }}</th>
            <th>{{ $t("career.totalGames") }}</th>
            <th>{{ $t("career.winRate") }}</th>
            <th>{{ $t("career.winsLabel") }}</th>
            <th>{{ $t("career.lossesLabel") }}</th>
            <th>{{ $t("career.tier") }}</th>
            <th>{{ $t("career.lp") }}</th>
            <th>{{ $t("career.highest") }}</th>
            <th>{{ $t("career.prevSeason") }}</th>
          </tr>
        </thead>
        <tbody>
          <!-- 单双排 -->
          <tr>
            <td class="type-name">{{ $t("gameModes.420") }}</td>
            <td>{{ soloQueue ? soloQueue.wins + soloQueue.losses : 0 }}</td>
            <td>
              {{
                soloQueue && soloQueue.wins + soloQueue.losses > 0
                  ? (
                      (soloQueue.wins / (soloQueue.wins + soloQueue.losses)) *
                      100
                    ).toFixed(0) + "%"
                  : "--"
              }}
            </td>
            <td>{{ soloQueue ? soloQueue.wins : 0 }}</td>
            <td>{{ soloQueue ? soloQueue.losses : 0 }}</td>
            <td class="rank-name">
              <div class="rank-cell-wrapper">
                <RankedIcon
                  v-if="soloQueue?.tier && soloQueue.tier !== 'NONE' && soloQueue.tier !== 'UNRANKED'"
                  :tier="soloQueue.tier"
                  :size="18"
                />
                <span>{{ soloQueue ? formatRank(soloQueue) : "--" }}</span>
              </div>
            </td>
            <td>{{ soloQueue ? soloQueue.leaguePoints : 0 }}</td>
            <td>{{ soloQueue ? formatHighestRank(soloQueue) : "--" }}</td>
            <td>{{ soloQueue ? formatPrevSeasonRank(soloQueue) : "--" }}</td>
          </tr>
          <!-- 灵活排位 -->
          <tr>
            <td class="type-name">{{ $t("gameModes.440") }}</td>
            <td>{{ flexQueue ? flexQueue.wins + flexQueue.losses : 0 }}</td>
            <td>
              {{
                flexQueue && flexQueue.wins + flexQueue.losses > 0
                  ? (
                      (flexQueue.wins / (flexQueue.wins + flexQueue.losses)) *
                      100
                    ).toFixed(0) + "%"
                  : "--"
              }}
            </td>
            <td>{{ flexQueue ? flexQueue.wins : 0 }}</td>
            <td>{{ flexQueue ? flexQueue.losses : 0 }}</td>
            <td class="rank-name">
              <div class="rank-cell-wrapper">
                <RankedIcon
                  v-if="flexQueue?.tier && flexQueue.tier !== 'NONE' && flexQueue.tier !== 'UNRANKED'"
                  :tier="flexQueue.tier"
                  :size="18"
                />
                <span>{{ flexQueue ? formatRank(flexQueue) : "--" }}</span>
              </div>
            </td>
            <td>{{ flexQueue ? flexQueue.leaguePoints : 0 }}</td>
            <td>{{ flexQueue ? formatHighestRank(flexQueue) : "--" }}</td>
            <td>{{ flexQueue ? formatPrevSeasonRank(flexQueue) : "--" }}</td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- 近期数据看板 & 常用英雄 -->
    <div v-if="statsSummary" class="recent-summary-bar">
      <div class="summary-text">
        <span class="summary-title">{{
          $t("career.recentGamesTitle", { count: recentMatches.length })
        }}</span>
        <span class="win-color"
          >{{ $t("career.win") }}: {{ statsSummary.wins }}</span
        >
        <span class="lose-color"
          >{{ $t("career.lose") }}: {{ statsSummary.losses }}</span
        >
        <span class="kda-label">KDA:</span>
        <span class="kda-values">
          {{ statsSummary.kills }} /
          <span class="death-red">{{ statsSummary.deaths }}</span> /
          {{ statsSummary.assists }}
        </span>
        <span class="kda-ratio">({{ statsSummary.kda }})</span>
      </div>

      <!-- 近期常玩位置分布 -->
      <div v-if="aggregatedPositions && aggregatedPositions.totalValid > 0" class="recent-positions-group">
        <n-popover trigger="hover" placement="bottom" :delay="150" style="border-radius: 8px; padding: 10px;">
          <template #trigger>
            <div class="pos-summary-pill">
              <span class="pos-summary-title">常玩位置:</span>
              <div class="pos-summary-tags">
                <span
                  v-for="p in aggregatedPositions.entries.filter(e => e.count > 0).slice(0, 2)"
                  :key="p.position"
                  class="pos-tag-chip"
                >
                  <PositionIcon :position="p.position" :use-position-color="true" :size="13" />
                  <span class="pos-tag-name">{{ p.name }}</span>
                  <span class="pos-tag-pct">{{ p.percentage }}%</span>
                </span>
              </div>
            </div>
          </template>
          <div class="career-pos-popover">
            <div class="pos-popover-title">近期位置分布 ({{ aggregatedPositions.totalValid }} 场峡谷)</div>
            <div class="pos-popover-list">
              <div
                v-for="e in aggregatedPositions.entries"
                :key="e.position"
                class="pos-popover-item"
              >
                <PositionIcon :position="e.position" :use-position-color="true" :size="13" />
                <span class="pop-name">{{ e.name }}</span>
                <div class="pop-bar-wrap">
                  <div
                    class="pop-bar-fill"
                    :style="{ width: `${e.percentage}%`, backgroundColor: e.color }"
                  ></div>
                </div>
                <span class="pop-val">{{ e.count }}场 · {{ e.percentage }}%</span>
              </div>
            </div>
          </div>
        </n-popover>
      </div>

      <!-- 近期常用英雄：带胜率微标 + Popover 详情 -->
      <div class="recent-champs">
        <n-popover
          v-for="c in aggregatedTopChamps"
          :key="c.championId"
          trigger="hover"
          placement="bottom"
          :delay="120"
          style="border-radius: 8px; padding: 8px 10px;"
        >
          <template #trigger>
            <div class="recent-champ-icon-wrap">
              <div class="recent-champ-icon">
                <LcuImage :src="c.championIconUrl" alt="champ" />
              </div>
              <span
                class="recent-champ-badge"
                :class="c.winRate >= 60 ? 'wr-high' : c.winRate < 45 ? 'wr-low' : 'wr-mid'"
              >
                {{ c.games }}
              </span>
            </div>
          </template>
          <div class="champ-hover-card">
            <div class="champ-hover-head">
              <LcuImage :src="c.championIconUrl" class="champ-hover-icon" />
              <div class="champ-hover-info">
                <div class="champ-hover-title">
                  近期出场 {{ c.games }} 次
                  <span v-if="c.primaryPosition" class="champ-hover-pos">
                    <PositionIcon :position="c.primaryPosition" :use-position-color="true" :size="12" />
                    {{ POSITION_NAMES[c.primaryPosition] }}
                  </span>
                </div>
                <div class="champ-hover-line">
                  <span :class="c.winRate >= 60 ? 'win-text' : c.winRate < 45 ? 'lose-text' : ''">{{ c.winRate }}% 胜率</span>
                  <span class="stats-divider">·</span>
                  <span>{{ c.wins }}胜 {{ c.losses }}负</span>
                  <span class="stats-divider">·</span>
                  <span>KDA {{ c.kda }}</span>
                </div>
              </div>
            </div>
          </div>
        </n-popover>
      </div>

      <div class="summary-actions">
        <n-popover
          trigger="click"
          placement="bottom-start"
          scrollable
          class="recent-teammates-popover"
          @update:show="handleTeammatesPopoverShow"
          style="
            max-height: 400px;
            width: 420px;
            border-radius: 12px;
            padding: 12px;
          "
        >
          <template #trigger>
            <button class="summary-action-btn">
              {{ $t("career.recentTeammates") }}
            </button>
          </template>
          <div class="teammates-flyout">
            <div v-if="loadingTeammates" class="loading-container">
              <n-spin size="medium" />
            </div>
            <div v-else-if="recentTeammates.length === 0" class="empty-text">
              暂无最近队友数据
            </div>
            <div v-else class="teammates-list">
              <div
                v-for="tm in recentTeammates"
                :key="tm.puuid"
                class="teammate-card"
                @click="handleClickTeammate(tm)"
              >
                <div class="teammate-avatar">
                  <LcuImage :src="tm.icon" />
                </div>
                <div class="teammate-info-col">
                  <div class="teammate-name">
                    {{ tm.name }}
                    <span v-if="tm.tag" class="teammate-tag-badge">{{ tm.tag }}</span>
                  </div>
                  <div class="teammate-last-time" v-if="tm.lastPlayTime">
                    {{ formatLocalDateTime(tm.lastPlayTime) }}
                  </div>
                </div>
                <div class="teammate-stats">
                  <span class="stat-item">
                    {{ $t("career.total") || "总" }}: <span class="stat-value total">{{ tm.total }}</span>
                  </span>
                  <span class="stat-item">
                    {{ $t("career.win") || "胜" }}: <span class="stat-value win">{{ tm.wins }}</span>
                  </span>
                  <span class="stat-item">
                    {{ $t("career.lose") || "负" }}: <span class="stat-value lose">{{ tm.losses }}</span>
                  </span>
                </div>
              </div>
            </div>
          </div>
        </n-popover>
        <div
          class="dropdown-trigger"
          @click.stop="showQueueDropdown = !showQueueDropdown"
        >
          <span>{{
            selectedQueue === null
              ? $t("career.all")
              : $t("gameModes." + selectedQueue)
          }}</span>
          <svg
            :class="['arrow-icon', { expanded: showQueueDropdown }]"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
          >
            <polyline points="6 9 12 15 18 9" />
          </svg>
          <div
            v-if="showQueueDropdown"
            class="queue-dropdown-menu"
            @click.stop
          >
            <div
              v-for="q in QUEUE_OPTIONS"
              :key="q.id ?? -1"
              :class="[
                'queue-dropdown-item',
                { active: selectedQueue === q.id },
              ]"
              @click="selectQueue(q.id)"
            >
              {{ q.id === null ? $t("career.all") : $t("gameModes." + q.id) }}
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- 局部滚动包裹区域：保留头像、排位与近期对局看板，仅滚动对局战绩列表 -->
    <div
      ref="scrollContainerRef"
      class="career-scroll-area"
      @scroll.passive="onScroll"
    >
      <!-- 战绩对局历史列表（虚拟化：仅渲染可视区附近卡片） -->
      <div v-if="filteredMatches.length > 0" class="match-history-list">
        <div
          class="virtual-list"
          :style="{ height: containerHeight + 'px' }"
        >
          <div
            class="virtual-list-viewport"
            :style="{ transform: `translateY(${offsetY}px)` }"
          >
            <MatchCard
              v-for="item in visibleItems"
              :key="filteredMatches[item.index].gameId"
              :match="filteredMatches[item.index]"
              @select="goToMatchDetail"
            />
          </div>
        </div>
      </div>
      <div v-else-if="!loading" class="tip-container">
        <p class="tip">{{ $t("career.empty") }}</p>
      </div>
    </div>
    </template>
  </div>
</template>

<style scoped>
.match-history-tab {
  display: flex;
  flex-direction: column;
  flex: 1;
  overflow: hidden;
  min-height: 0;
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

.tip {
  font-size: 0.95rem;
  color: var(--text-dimmed);
  margin: 0;
}

/* 排位数据表 */
.rank-table-wrapper {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  overflow: hidden;
  margin-bottom: 1rem;
  box-shadow: var(--shadow-sm);
  transition: all 0.25s ease;
}

.rank-table-wrapper:hover {
  background-color: var(--card-bg-hover);
  box-shadow: var(--shadow-md);
  border-color: var(--primary-color-alpha-30);
}

.rank-table {
  width: 100%;
  border-collapse: collapse;
  text-align: left;
}

.rank-table th,
.rank-table td {
  padding: 6px 16px;
  font-size: 0.82rem;
  border-bottom: 1px solid var(--border-color);
}

.rank-table th {
  background-color: var(--hover-bg);
  color: var(--text-muted);
  font-weight: 600;
  border-bottom: 1.5px solid var(--border-color);
}

.rank-table tr:last-child td {
  border-bottom: none;
}

.type-name {
  font-weight: 600;
  color: var(--text-color);
}

.rank-name {
  font-weight: bold;
  color: var(--primary-color);
}

.rank-cell-wrapper {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  vertical-align: middle;
}

/* 近期数据概览栏 */
.recent-summary-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 16px;
  background-color: var(--settings-collapse-bg, var(--card-bg)) !important;
  backdrop-filter: blur(15px) !important;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  margin-bottom: 1rem;
  box-shadow: var(--shadow-sm);
  transition: all 0.25s ease;
  position: sticky;
  top: 0;
  z-index: 100 !important;
}

.recent-summary-bar:hover {
  background-color: var(--card-bg-hover);
  box-shadow: var(--shadow-md);
  border-color: var(--primary-color-alpha-30);
}

.summary-text {
  font-size: 0.82rem;
  color: var(--text-muted);
  display: flex;
  align-items: center;
  gap: 8px;
}

.summary-title {
  font-weight: bold;
  color: var(--text-color);
  margin-right: 4px;
}

.win-color {
  color: var(--win-color);
  font-weight: 600;
}

.lose-color {
  color: var(--loss-color);
  font-weight: 600;
}

.kda-label {
  color: var(--text-dimmed);
  margin-left: 8px;
}

.kda-values {
  color: var(--text-color);
  font-weight: 600;
}

.death-red {
  color: var(--death-color, var(--loss-color));
}

/* ─── 近期常玩位置胶囊 ─── */
.recent-positions-group {
  display: flex;
  align-items: center;
}
.pos-summary-pill {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 3px 8px;
  background: var(--hover-bg);
  border: 1px solid var(--border-color);
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s ease;
}
.pos-summary-pill:hover {
  background: var(--hover-bg-strong);
  border-color: rgba(var(--primary-color-rgb, 99, 102, 241), 0.3);
}
.pos-summary-title {
  font-size: 0.72rem;
  font-weight: 600;
  color: var(--text-muted);
}
.pos-summary-tags {
  display: flex;
  align-items: center;
  gap: 6px;
}
.pos-tag-chip {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  font-size: 0.72rem;
}
.pos-tag-name {
  font-weight: 700;
  color: var(--text-color);
}
.pos-tag-pct {
  font-weight: 600;
  color: var(--text-muted);
  font-size: 0.68rem;
}

/* 生涯位置分布 Popover */
.career-pos-popover {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 180px;
}

.recent-champs {
  display: flex;
  align-items: center;
  gap: 6px;
}

.recent-champ-icon-wrap {
  position: relative;
  display: inline-block;
  cursor: pointer;
  transition: transform 0.15s ease;
}
.recent-champ-icon-wrap:hover {
  transform: scale(1.15);
  z-index: 2;
}

.recent-champ-icon {
  width: 26px;
  height: 26px;
  border-radius: 50%;
  overflow: hidden;
  border: 1.5px solid var(--border-color);
}

.recent-champ-badge {
  position: absolute;
  right: -4px;
  bottom: -4px;
  font-size: 0.52rem;
  font-weight: 800;
  line-height: 1;
  padding: 1px 3px;
  border-radius: 3px;
  background: rgba(15, 23, 42, 0.88);
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.35);
}
.recent-champ-badge.wr-high {
  color: #10b981;
}
.recent-champ-badge.wr-mid {
  color: #60a5fa;
}
.recent-champ-badge.wr-low {
  color: #94a3b8;
}

/* 常用英雄 Popover 卡片 */
.champ-hover-card {
  min-width: 160px;
}
.champ-hover-head {
  display: flex;
  align-items: center;
  gap: 8px;
}
.champ-hover-icon {
  width: 32px;
  height: 32px;
  border-radius: 6px;
  flex-shrink: 0;
  border: 1px solid var(--border-color);
}
.champ-hover-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.champ-hover-title {
  font-size: 0.72rem;
  font-weight: 700;
  color: var(--text-color);
  display: flex;
  align-items: center;
  gap: 6px;
}
.champ-hover-pos {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  font-size: 0.65rem;
  color: var(--text-muted);
}
.champ-hover-line {
  font-size: 0.65rem;
  color: var(--text-muted);
  display: flex;
  align-items: center;
  gap: 4px;
}
.stats-divider {
  opacity: 0.5;
}

/* Popover 位置列表项公共样式 */
.pos-popover-title {
  font-size: 0.75rem;
  font-weight: 700;
  color: var(--text-color);
  border-bottom: 1px solid var(--border-color);
  padding-bottom: 4px;
}
.pos-popover-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.pos-popover-item {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 0.7rem;
}
.pop-name {
  font-weight: 600;
  color: var(--text-color);
  width: 26px;
  flex-shrink: 0;
}
.pop-bar-wrap {
  flex: 1;
  height: 5px;
  background: var(--hover-bg-strong, var(--border-color));
  border-radius: 2.5px;
  overflow: hidden;
}
.pop-bar-fill {
  height: 100%;
  border-radius: 2.5px;
  transition: width 0.3s ease;
}
.pop-val {
  font-size: 0.65rem;
  color: var(--text-muted);
  font-weight: 600;
  text-align: right;
  flex-shrink: 0;
}

.summary-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.summary-action-btn {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  color: var(--text-color);
  padding: 4px 12px;
  border-radius: 4px;
  font-size: 0.78rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
}

.summary-action-btn:hover {
  background: var(--card-bg-hover);
  border-color: var(--primary-color);
}

.dropdown-trigger {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  padding: 4px 10px;
  border-radius: 4px;
  font-size: 0.78rem;
  color: var(--text-color);
  cursor: pointer;
  position: relative;
  transition: all 0.2s;
}

.dropdown-trigger:hover {
  background: var(--card-bg-hover);
  border-color: var(--primary-color);
}

.dropdown-trigger .arrow-icon {
  width: 12px;
  height: 12px;
  transition: transform 0.2s;
}

.dropdown-trigger .arrow-icon.expanded {
  transform: rotate(180deg);
}

/* 模式筛选下拉菜单 */
.queue-dropdown-menu {
  position: absolute;
  top: calc(100% + 4px);
  right: 0;
  background: var(--popover-bg);
  border: 1px solid var(--popover-border);
  border-radius: 8px;
  box-shadow: var(--shadow-lg);
  backdrop-filter: blur(16px);
  -webkit-backdrop-filter: blur(16px);
  z-index: 100;
  min-width: 130px;
  padding: 4px 0;
}

.queue-dropdown-item {
  padding: 6px 14px;
  font-size: 0.78rem;
  color: var(--text-muted);
  cursor: pointer;
  transition: all 0.2s;
}

.queue-dropdown-item:hover {
  background: var(--hover-bg);
  color: var(--text-color);
}

.queue-dropdown-item.active {
  color: var(--primary-color);
  font-weight: 600;
  background: var(--primary-color-alpha-15);
}

/* 战绩及看板局部滚动区域 */
.career-scroll-area {
  flex: 1;
  overflow-y: auto;

  padding-right: 4px;
  margin-top: 1rem;
  scroll-behavior: smooth;
  /* 顶部与底部边缘 16px 渐变淡出羽化，消除滑动硬切裂感，极致顺滑 */
  -webkit-mask-image: linear-gradient(
    to bottom,
    transparent 0%,
    black 16px,
    black calc(100% - 16px),
    transparent 100%
  );
  mask-image: linear-gradient(
    to bottom,
    transparent 0%,
    black 16px,
    black calc(100% - 16px),
    transparent 100%
  );
}

/* 战绩对局历史卡片 */
.match-history-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

/* 虚拟列表：外层撑起总高，内层按 offsetY 平移定位可视卡片 */
.virtual-list {
  position: relative;
  width: 100%;
}

.virtual-list-viewport {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
  will-change: transform;
}

/*
 * .win-text / .lose-text 原属对局卡片的样式，随模板迁入 components/match-card/MatchCard.vue。
 * 此处保留副本：本组件的英雄胜率标签仍在使用。
 */
.win-text {
  color: var(--win-color);
}

.win-text.gold-text {
  color: #f59e0b;
}

.lose-text {
  color: var(--loss-color);
}

/* 队友弹窗内加载容器 */
.loading-container {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 2rem;
}

.empty-text {
  text-align: center;
  padding: 1rem;
  color: var(--text-dimmed);
  font-size: 0.85rem;
}

/* 队友卡片 */
.teammates-flyout {
  min-width: 380px;
}

.teammates-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.teammate-card {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 12px;
  border-radius: 8px;
  cursor: pointer;
  transition: background-color 0.2s;
}

.teammate-card:hover {
  background-color: var(--hover-bg);
}

.teammate-avatar {
  width: 36px;
  height: 36px;
  border-radius: 50%;
  overflow: hidden;
  flex-shrink: 0;
}

.teammate-info-col {
  flex: 1;
  min-width: 0;
}

.teammate-name {
  font-size: 0.85rem;
  font-weight: 600;
  color: var(--text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.teammate-tag-badge {
  display: inline-block;
  margin-left: 6px;
  padding: 1px 6px;
  font-size: 0.68rem;
  font-weight: 500;
  line-height: 1.4;
  color: #fff;
  background: linear-gradient(135deg, var(--primary-color, #6fa8ff), #9b6fff);
  border-radius: 8px;
  vertical-align: middle;
}

.teammate-last-time {
  font-size: 0.72rem;
  color: var(--text-dimmed);
  margin-top: 2px;
}

.teammate-stats {
  display: flex;
  gap: 10px;
  font-size: 0.78rem;
  color: var(--text-muted);
  flex-shrink: 0;
}

.stat-item {
  white-space: nowrap;
}

.stat-value {
  font-weight: 700;
}

.stat-value.total {
  color: var(--text-color);
}

.stat-value.win {
  color: var(--win-color);
}

.stat-value.lose {
  color: var(--loss-color);
}

/* 海克斯强化悬浮提示 */

</style>
