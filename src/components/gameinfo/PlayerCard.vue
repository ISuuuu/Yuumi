<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  getChampionIcon,
  getPlayerMasteryDetail,
  type PlayerData,
  type PremadePlayerLike,
} from "../../types/gameInfo";
import { usePlayerSearch } from "../../composables/usePlayerSearch";
import { useFateBadge } from "../../composables/useFateBadge";
import { usePlayerChampionId } from "../../composables/usePlayerChampionId";
import type { SavedPlayerMarker } from "../../api/lcu";
import LcuImage from "../LcuImage.vue";
import PositionIcon from "../PositionIcon.vue";
import RankedIcon from "../RankedIcon.vue";
import { NPopover, NTooltip } from "naive-ui";
import { POSITION_NAMES } from "../../utils/matchAnalysis";

const props = defineProps<{
  player: PremadePlayerLike;
  playerData?: PlayerData;
  premadeIdx: number;
  premadeCardStyle: Record<string, string>;
  savedMap?: Record<string, SavedPlayerMarker>;
  selfPuuid?: string;
  index?: number;
}>();

const { t } = useI18n();
const { getPlayerSearchName, getPlayerDisplayName, handleCareerClick } = usePlayerSearch();

const resolvedChampId = usePlayerChampionId(
  () => props.player,
  () => props.playerData,
);

const masteryDetail = computed(() => {
  return getPlayerMasteryDetail(props.playerData, resolvedChampId.value);
});

const { savedInfo, savedBadgeTitle, getFateBadgeText, getFateTitle } = useFateBadge(
  () => props.playerData?.info?.puuid,
  () => props.savedMap,
  () => props.selfPuuid,
);

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

const soloQueue = computed(() => props.playerData?.ranked?.solo);
const flexQueue = computed(() => props.playerData?.ranked?.flex);

const hasSoloRank = computed(() => {
  const t = soloQueue.value?.tier;
  return Boolean(t && t !== "NONE" && t !== "UNRANKED");
});

const hasFlexRank = computed(() => {
  const t = flexQueue.value?.tier;
  return Boolean(t && t !== "NONE" && t !== "UNRANKED");
});

const isDualRank = computed(() => hasSoloRank.value && hasFlexRank.value);

// 单双排信息
const soloTier = computed(() => soloQueue.value?.tier);
const soloTierName = computed(() => {
  if (!soloTier.value || soloTier.value === "NONE" || soloTier.value === "UNRANKED") return t("gameInfo.noRank");
  return TIER_MAP.value[soloTier.value] || soloTier.value;
});
const soloDivision = computed(() => {
  const q = soloQueue.value;
  if (!q || !q.rank || q.rank === "NA" || q.tier === "NONE" || q.tier === "UNRANKED") return "";
  return q.rank;
});
const soloLp = computed(() => {
  const q = soloQueue.value;
  if (!q || q.leaguePoints === undefined || !q.tier || q.tier === "NONE" || q.tier === "UNRANKED") return "";
  return `${q.leaguePoints} LP`;
});
const soloStats = computed(() => {
  const q = soloQueue.value;
  if (!q || !q.tier || q.tier === "NONE" || q.tier === "UNRANKED") return null;
  const wins = q.wins || 0;
  if (wins <= 0) return null;
  return { wins };
});

// 灵活组排信息
const flexTier = computed(() => flexQueue.value?.tier);
const flexTierName = computed(() => {
  if (!flexTier.value || flexTier.value === "NONE" || flexTier.value === "UNRANKED") return "";
  return TIER_MAP.value[flexTier.value] || flexTier.value;
});
const flexDivision = computed(() => {
  const q = flexQueue.value;
  if (!q || !q.rank || q.rank === "NA" || q.tier === "NONE" || q.tier === "UNRANKED") return "";
  return q.rank;
});
const flexLp = computed(() => {
  const q = flexQueue.value;
  if (!q || q.leaguePoints === undefined || !q.tier || q.tier === "NONE" || q.tier === "UNRANKED") return "";
  return `${q.leaguePoints} LP`;
});
const flexStats = computed(() => {
  const q = flexQueue.value;
  if (!q || !q.tier || q.tier === "NONE" || q.tier === "UNRANKED") return null;
  const wins = q.wins || 0;
  if (wins <= 0) return null;
  return { wins };
});

// 完整排位 Tooltip 提示
const soloTooltip = computed(() => {
  if (!hasSoloRank.value || !soloQueue.value) return t("gameInfo.soloRankTitle") || "单双排: 无段位";
  const q = soloQueue.value;
  const tierName = soloTierName.value;
  const div = soloDivision.value ? ` ${soloDivision.value}` : "";
  const lp = soloLp.value ? ` ${soloLp.value}` : "";
  const wins = q.wins || 0;
  const losses = q.losses || 0;
  let statsStr = "";
  if (wins > 0 || losses > 0) {
    const total = wins + losses;
    const rate = total > 0 ? Math.round((wins / total) * 100) : 0;
    statsStr = ` (${wins}胜${losses > 0 ? ` ${losses}负` : ""} · ${rate}%胜率)`;
  }
  return `${t("gameInfo.soloRank") || "单双排"}: ${tierName}${div}${lp}${statsStr}`;
});

const flexTooltip = computed(() => {
  if (!hasFlexRank.value || !flexQueue.value) return t("gameInfo.flexRankTitle") || "灵活排: 无段位";
  const q = flexQueue.value;
  const tierName = flexTierName.value;
  const div = flexDivision.value ? ` ${flexDivision.value}` : "";
  const lp = flexLp.value ? ` ${flexLp.value}` : "";
  const wins = q.wins || 0;
  const losses = q.losses || 0;
  let statsStr = "";
  if (wins > 0 || losses > 0) {
    const total = wins + losses;
    const rate = total > 0 ? Math.round((wins / total) * 100) : 0;
    statsStr = ` (${wins}胜${losses > 0 ? ` ${losses}负` : ""} · ${rate}%胜率)`;
  }
  return `${t("gameInfo.flexRank") || "灵活组排"}: ${tierName}${div}${lp}${statsStr}`;
});

// 单排位主展示队列（单双优先，无单双则取灵活）
const mainQueue = computed(() => {
  if (hasSoloRank.value) return soloQueue.value;
  if (hasFlexRank.value) return flexQueue.value;
  return null;
});

const isMainFlex = computed(() => !hasSoloRank.value && hasFlexRank.value);

const mainTier = computed(() => mainQueue.value?.tier);
const mainTierName = computed(() => {
  if (!mainTier.value || mainTier.value === "NONE" || mainTier.value === "UNRANKED") return t("gameInfo.noRank");
  return TIER_MAP.value[mainTier.value] || mainTier.value;
});
const mainDivision = computed(() => {
  const q = mainQueue.value;
  if (!q || !q.rank || q.rank === "NA" || q.tier === "NONE" || q.tier === "UNRANKED") return "";
  return q.rank;
});
const mainLp = computed(() => {
  const q = mainQueue.value;
  if (!q || q.leaguePoints === undefined || !q.tier || q.tier === "NONE" || q.tier === "UNRANKED") return "";
  return `${q.leaguePoints} LP`;
});
const mainStats = computed(() => {
  const q = mainQueue.value;
  if (!q || !q.tier || q.tier === "NONE" || q.tier === "UNRANKED") return null;
  const wins = q.wins || 0;
  if (wins <= 0) return null;
  return { wins };
});

// 近期对局数据统计计算（胜率与 KDA 悬停解释）
const matchStatsDetail = computed(() => {
  const matches = props.playerData?.matches || [];
  let totalKills = 0;
  let totalDeaths = 0;
  let totalAssists = 0;
  let validMatches = 0;
  for (const m of matches) {
    if (!m.remake) {
      validMatches++;
      totalKills += m.kills || 0;
      totalDeaths += m.deaths || 0;
      totalAssists += m.assists || 0;
    }
  }
  const fallbackTotal = (props.playerData?.winCount || 0) + (props.playerData?.lossesCount || 0);
  const games = validMatches > 0 ? validMatches : (matches.length > 0 ? matches.length : fallbackTotal);
  const avgKills = games > 0 && validMatches > 0 ? (totalKills / games).toFixed(1) : null;
  const avgDeaths = games > 0 && validMatches > 0 ? (totalDeaths / games).toFixed(1) : null;
  const avgAssists = games > 0 && validMatches > 0 ? (totalAssists / games).toFixed(1) : null;

  return {
    totalGames: games,
    winCount: props.playerData?.winCount ?? 0,
    lossesCount: props.playerData?.lossesCount ?? 0,
    winRate: props.playerData?.winRate,
    avgKda: props.playerData?.avgKda,
    avgKills,
    avgDeaths,
    avgAssists,
  };
});

const primaryPos = computed(() => props.playerData?.positions?.primaryPosition);
const primaryPosPercent = computed(() => props.playerData?.positions?.primaryPercentage);
const secondaryPos = computed(() => props.playerData?.positions?.secondaryPosition);
const secondaryPosPercent = computed(() => props.playerData?.positions?.secondaryPercentage);
const topChamps = computed(() => props.playerData?.topChampions || []);
</script>

<template>
  <div
    class="player-card"
    :class="{
      'premade-card': premadeIdx >= 0,
    }"
    :style="premadeCardStyle"
  >
    <!-- 第一层：主体（左侧头像 + 右侧召唤师名称 / 段位 / KDA，独占整行宽度） -->
    <div class="pc-top-section">
      <div class="pc-avatar-area">
        <div
          class="profile-icon-wrapper-mini"
          :class="{ 'top-champ-ring': masteryDetail?.isTopChampion }"
        >
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
                  playerData?.info?.percentCompleteForNextLevel || 0,
              }"
            />
          </svg>
          <div class="avatar-container-mini">
            <!-- 选人阶段/对局中：选了英雄显示英雄头像，未选英雄显示占位符 -->
            <LcuImage
              v-if="resolvedChampId > 0"
              :src="getChampionIcon(resolvedChampId)"
              class="profile-avatar-mini"
              alt="champ"
            />
            <div v-else class="profile-avatar-mini profile-avatar-empty-mini">
              ?
            </div>
          </div>
          <!-- 等级数字 -->
          <div
            v-if="playerData?.info?.summonerLevel"
            class="level-badge-mini"
          >
            {{ playerData.info.summonerLevel }}
          </div>
        </div>
      </div>

      <div class="pc-info">
          <!-- 第 1 行：召唤师名称 + 战绩隐藏锁 + 宿命/标记 + 连胜连败 -->
        <div class="pc-row pc-name-row">
          <span
            class="name-text"
            :title="getPlayerSearchName(player, playerData) ? `${$t('nav.career')} ${getPlayerSearchName(player, playerData)}` : undefined"
            @click="(e) => handleCareerClick(e, player, playerData)"
          >{{
            getPlayerDisplayName(player, playerData) ||
            $t("gameInfo.playerIndex", { index: (index ?? 0) + 1 })
          }}</span>

          <!-- 战绩隐藏锁图标 -->
          <span
            v-if="playerData?.matchHistoryHidden || playerData?.isProfilePrivate"
            class="pc-name-lock"
            :title="$t('gameInfo.matchHistoryHidden')"
          >
            <svg
              class="pc-name-lock-svg"
              viewBox="0 0 24 24"
              fill="currentColor"
            >
              <path
                fill-rule="evenodd"
                clip-rule="evenodd"
                d="M12 2a5 5 0 0 0-5 5v3H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-8a2 2 0 0 0-2-2h-1V7a5 5 0 0 0-5-5zm-3 5a3 3 0 0 1 6 0v3H9V7zm3 6a1.5 1.5 0 0 0-1 2.618V18a1 1 0 1 0 2 0v-2.382A1.5 1.5 0 0 0 12 13z"
              />
            </svg>
          </span>

          <div class="name-tags-group">
            <!-- 宿命徽标 -->
            <span
              v-if="playerData?.fateFlag"
              :class="['fate-badge', playerData.fateFlag]"
              :title="getFateTitle(playerData)"
            >{{ getFateBadgeText(playerData) }}</span>

            <!-- 标记玩家 -->
            <span
              v-if="savedInfo && (savedInfo.tag || !playerData?.fateFlag)"
              :class="['saved-badge', { 'met-only': !savedInfo.tag }]"
              :title="savedBadgeTitle(savedInfo)"
            >{{ savedInfo.tag || $t("gameInfo.savedPlayerMark") }}</span>

            <!-- 连胜 / 连败微徽标（从底栏提至第1行，强化当前战斗状态） -->
            <span
              v-if="playerData?.streak && playerData.streak.count >= 2"
              :class="['streak-badge', playerData.streak.type]"
              :title="
                playerData.streak.type === 'win'
                  ? $t('gameInfo.winStreak', { count: playerData.streak.count })
                  : $t('gameInfo.lossStreak', { count: playerData.streak.count })
              "
            >
              <svg
                v-if="playerData.streak.type === 'win'"
                class="streak-svg flame"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2.3"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <path d="M8.5 14.5A2.5 2.5 0 0 0 11 12c0-1.38-.5-2-1-3-1.072-2.143-.224-4.054 2-6 .5 2.5 2 4.9 4 6.5 2 1.6 3 3.5 3 5.5a7 7 0 1 1-14 0c0-1.153.433-2.294 1-3a2.5 2.5 0 0 0 2.5 2.5z" />
              </svg>
              <svg
                v-else
                class="streak-svg snow"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2.3"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <line x1="12" y1="2" x2="12" y2="22" />
                <line x1="2" y1="12" x2="22" y2="12" />
                <line x1="4.93" y1="4.93" x2="19.07" y2="19.07" />
                <line x1="19.07" y1="4.93" x2="4.93" y2="19.07" />
              </svg>
              <span class="streak-count">{{ playerData.streak.count }}{{ playerData.streak.type === 'win' ? $t('career.win') : $t('career.lose') }}</span>
            </span>
          </div>
        </div>

        <!-- 排位段位展示：若同时存在单双与灵活排位，分两行各展示一行 -->
        <template v-if="isDualRank">
          <!-- 单双排行 -->
          <div class="pc-row pc-rank-row" :title="soloTooltip">
            <RankedIcon :tier="soloTier" :size="13" class="rank-tier-icon" />
            <span class="rank-type-badge solo">{{ $t("gameInfo.soloRank") || "单双" }}</span>
            <span :class="['tier-text', soloTier]">
              {{ soloTierName }}{{ soloDivision ? ' ' + soloDivision : '' }}
            </span>
            <span v-if="soloLp" class="tier-lp">{{ soloLp }}</span>
            <span v-if="soloStats" class="rank-winrate-stats">
              <span class="stats-divider">·</span>
              <span class="stats-num">{{ soloStats.wins }}{{ $t("career.win") }}</span>
            </span>
          </div>

          <!-- 灵活组排行 -->
          <div class="pc-row pc-rank-row" :title="flexTooltip">
            <RankedIcon :tier="flexTier" :size="13" class="rank-tier-icon" />
            <span class="rank-type-badge flex">{{ $t("gameInfo.flexRank") || "灵活" }}</span>
            <span :class="['tier-text', flexTier]">
              {{ flexTierName }}{{ flexDivision ? ' ' + flexDivision : '' }}
            </span>
            <span v-if="flexLp" class="tier-lp">{{ flexLp }}</span>
            <span v-if="flexStats" class="rank-winrate-stats">
              <span class="stats-divider">·</span>
              <span class="stats-num">{{ flexStats.wins }}{{ $t("career.win") }}</span>
            </span>
          </div>
        </template>

        <!-- 单排位展示（仅单双排或仅灵活排位） -->
        <div v-else class="pc-row pc-rank-row" :title="mainQueue === flexQueue ? flexTooltip : soloTooltip">
          <template v-if="mainTier && mainTier !== 'NONE' && mainTier !== 'UNRANKED'">
            <RankedIcon :tier="mainTier" :size="14" class="rank-tier-icon" />
            <span v-if="isMainFlex" class="rank-type-badge flex">{{ $t("gameInfo.flexRank") || "灵活" }}</span>
            <span :class="['tier-text', mainTier]">
              {{ mainTierName }}{{ mainDivision ? ' ' + mainDivision : '' }}
            </span>
            <span v-if="mainLp" class="tier-lp">{{ mainLp }}</span>
            <span v-if="mainStats" class="rank-winrate-stats">
              <span class="stats-divider">·</span>
              <span class="stats-num">{{ mainStats.wins }}{{ $t("career.win") }}</span>
            </span>
          </template>
          <template v-else>
            <span class="tier-unranked">{{ $t("gameInfo.noRank") }}</span>
          </template>
        </div>

        <!-- 战绩行：近 10 场胜率与 KDA -->
        <div
          class="pc-row pc-stats-row"
          v-if="!playerData?.matchHistoryHidden && !playerData?.isProfilePrivate && (playerData?.winRate !== undefined || playerData?.avgKda !== undefined)"
        >
          <!-- 胜率展示 + Hover 详细解释 Tooltip -->
          <n-tooltip
            v-if="playerData?.winRate !== undefined"
            trigger="hover"
            placement="top"
            :delay="120"
          >
            <template #trigger>
              <span
                :class="[
                  'pc-winrate-text',
                  'interactive-stat',
                  getWinRateClass(playerData.winRate),
                ]"
              >
                {{ playerData.winRate }}%
              </span>
            </template>
            <div class="stat-tooltip-box">
              <div class="stat-tooltip-title">{{ $t("career.winRate") || "近期胜率" }}</div>
              <div class="stat-tooltip-line">
                <span class="stat-tooltip-label">统计范围:</span>
                <span class="stat-tooltip-val">近 {{ matchStatsDetail.totalGames }} 场对局</span>
              </div>
              <div class="stat-tooltip-line">
                <span class="stat-tooltip-label">战绩详情:</span>
                <span class="stat-tooltip-val">
                  <span class="hl-win">{{ matchStatsDetail.winCount }}{{ $t("career.win") }}</span>
                  <span class="hl-sep"> / </span>
                  <span class="hl-loss">{{ matchStatsDetail.lossesCount }}{{ $t("career.lose") }}</span>
                  <span class="hl-rate"> ({{ matchStatsDetail.winRate }}%)</span>
                </span>
              </div>
              <div class="stat-tooltip-formula">
                近期有效对局的胜负比例
              </div>
            </div>
          </n-tooltip>

          <span
            v-if="playerData?.winRate !== undefined && playerData?.avgKda !== undefined"
            class="stats-divider"
          >·</span>

          <!-- KDA 展示 + Hover 详细解释 Tooltip -->
          <n-tooltip
            v-if="playerData?.avgKda !== undefined"
            trigger="hover"
            placement="top"
            :delay="120"
          >
            <template #trigger>
              <span
                :class="[
                  'pc-kda-text',
                  'interactive-stat',
                  getKdaClass(playerData.avgKda),
                ]"
              >
                KDA {{ playerData?.avgKda?.toFixed(2) ?? "0.00" }}
              </span>
            </template>
            <div class="stat-tooltip-box">
              <div class="stat-tooltip-title">近期场均 KDA</div>
              <div v-if="matchStatsDetail.avgKills !== null" class="stat-tooltip-line">
                <span class="stat-tooltip-label">场均数据:</span>
                <span class="stat-tooltip-val">
                  {{ matchStatsDetail.avgKills }} / {{ matchStatsDetail.avgDeaths }} / {{ matchStatsDetail.avgAssists }}
                </span>
              </div>
              <div class="stat-tooltip-line">
                <span class="stat-tooltip-label">综合数值:</span>
                <span class="stat-tooltip-val font-bold">
                  {{ playerData.avgKda.toFixed(2) }}
                </span>
              </div>
              <div class="stat-tooltip-formula">
                计算公式: (击杀 + 助攻) ÷ 死亡
              </div>
            </div>
          </n-tooltip>
        </div>
      </div>
    </div>

    <!-- 第二层：底部资产行（左侧熟练度两行，右侧位置 + 常用英雄两行） -->
    <div
      v-if="masteryDetail || (primaryPos && primaryPosPercent !== undefined) || topChamps.length > 0"
      class="pc-bottom-bar"
    >
      <!-- 左侧：所选英雄熟练度（等级点数 / 招牌·练英雄 分两行） -->
      <div v-if="masteryDetail" class="pc-bottom-left">
        <div
          class="avatar-sub-pill"
          :class="{ 'is-top': masteryDetail.isTopChampion }"
          :title="$t('gameInfo.masteryTooltip', { level: masteryDetail.level, points: masteryDetail.points.toLocaleString() })"
        >
          <span class="mastery-lvl">Lv.{{ masteryDetail.level }}</span>
          <span class="mastery-pts">{{ masteryDetail.formattedPoints }}</span>
        </div>

        <span
          v-if="masteryDetail.isTopChampion"
          class="mastery-role-badge top"
          :title="$t('gameInfo.topChampionTitle')"
        >
          <svg class="role-svg star" viewBox="0 0 24 24" fill="currentColor">
            <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
          </svg>
          {{ $t('gameInfo.topChampion') }}
        </span>
        <span
          v-else-if="masteryDetail.isPracticing"
          class="mastery-role-badge practice"
          :title="$t('gameInfo.practicingTitle')"
        >
          {{ $t('gameInfo.practicing') }}
        </span>
      </div>

      <!-- 右侧：近期画像（上行位置分布，下行常用英雄） -->
      <div
        v-if="(primaryPos && primaryPosPercent !== undefined) || topChamps.length > 0"
        class="pc-bottom-right"
      >
          <!-- 第一行：常玩位置微标与分布弹窗 -->
          <n-popover
            v-if="primaryPos && primaryPosPercent !== undefined"
            trigger="hover"
            placement="top"
            :delay="120"
            style="border-radius: 8px; padding: 8px 12px; max-width: 260px;"
          >
            <template #trigger>
              <span class="pc-pos-pill" :title="$t('career.positions') || '常玩位置'">
                <span class="pc-pos-item">
                  <PositionIcon
                    :position="primaryPos"
                    :use-position-color="true"
                    :size="11"
                  />
                  <span class="pos-text">{{ POSITION_NAMES[primaryPos] }} {{ primaryPosPercent }}%</span>
                </span>
                <template v-if="secondaryPos && secondaryPosPercent">
                  <span class="pc-pos-sep">/</span>
                  <span
                    class="pc-pos-item is-secondary"
                    :title="`${POSITION_NAMES[secondaryPos]} ${secondaryPosPercent}%`"
                  >
                    <PositionIcon
                      :position="secondaryPos"
                      :use-position-color="true"
                      :size="11"
                    />
                    <span class="pos-text">{{ secondaryPosPercent }}%</span>
                  </span>
                </template>
              </span>
            </template>
            <div class="pos-popover-box">
              <div class="pos-popover-title">近期位置分布 ({{ playerData?.positions?.totalValid }}场)</div>
              <div class="pos-popover-list">
                <div
                  v-for="e in playerData?.positions?.entries"
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

          <!-- 第二行：近期常玩英雄小头像组（前 3 个） -->
          <div v-if="topChamps.length > 0" class="pc-top-champs-bar">
            <n-popover
              v-for="c in topChamps.slice(0, 3)"
              :key="c.championId"
              trigger="hover"
              placement="top"
              :delay="120"
              style="border-radius: 8px; padding: 8px 10px;"
            >
              <template #trigger>
                <div class="pc-mini-champ-tag">
                  <LcuImage :src="c.championIconUrl" class="mini-champ-avatar" alt="champ" />
                  <span
                    class="mini-champ-badge"
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
                      <span :class="getWinRateClass(c.winRate)">{{ c.winRate }}% 胜率</span>
                      <span class="stats-divider">·</span>
                      <span>{{ c.wins }}胜 {{ c.losses }}负</span>
                      <span class="stats-divider">·</span>
                      <span :class="getKdaClass(parseFloat(c.kda))">KDA {{ c.kda }}</span>
                    </div>
                  </div>
                </div>
              </div>
            </n-popover>
          </div>
        </div>
      </div>
    </div>
</template>

<style scoped>
.player-card {
  --pc-sub-row-h: 16px;
  position: relative;
  overflow: hidden;
  flex: 1;
  min-height: 0;
  max-height: 142px;
  display: flex;
  flex-direction: column;
  justify-content: center;
  padding: 6px 8px;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  transition: all 0.25s cubic-bezier(0.25, 0.8, 0.25, 1);
  box-sizing: border-box;
  box-shadow: var(--shadow-sm);
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
  gap: 4px;
}
.player-card:hover {
  background: var(--card-bg-hover);
  transform: translateY(-2px) scale(1.01);
  border-color: var(--primary-color-alpha-40);
  box-shadow:
    var(--shadow-md),
    0 4px 16px var(--primary-color-alpha-15);
}
.player-card.premade-card:hover {
  filter: brightness(1.08);
  transform: translateY(-2px) scale(1.01);
}

/* ─── 主体：头像 + 召唤师名称/段位/KDA（保证完整宽度，不被画像挤压） ─── */
.pc-top-section {
  display: flex;
  align-items: center;
  gap: 7px;
  width: 100%;
  min-width: 0;
}

/* ─── 头像区 ─── */
.pc-avatar-area {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}
.avatar-sub-pill {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  font-size: 0.60rem;
  line-height: 1;
  padding: 1px 4.5px;
  border-radius: 3px;
  background: var(--hover-bg-strong);
  border: 1px solid var(--border-color);
  white-space: nowrap;
}
.avatar-sub-pill.is-top {
  background: rgba(245, 158, 11, 0.08);
  border-color: rgba(245, 158, 11, 0.3);
}
.avatar-sub-pill.is-top .mastery-lvl {
  color: #d97706;
}
.mastery-lvl {
  font-weight: 800;
  color: var(--text-color);
}
.mastery-pts {
  color: var(--text-muted);
  font-weight: 600;
}

/* ─── 底部资产行（左侧熟练度两行 + 右侧位置/常用英雄两行） ─── */
.pc-bottom-bar {
  width: 100%;
  box-sizing: border-box;
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 3px 5px;
  background: rgba(0, 0, 0, 0.025);
  border-top: 1px dashed var(--border-color);
  border-radius: 4px;
}
.pc-bottom-left {
  display: grid;
  grid-auto-rows: var(--pc-sub-row-h);
  align-items: center;
  justify-items: start;
  gap: 2px;
  flex-shrink: 0;
}
.pc-bottom-right {
  display: grid;
  grid-auto-rows: var(--pc-sub-row-h);
  align-items: center;
  justify-items: end;
  gap: 2px;
  flex-shrink: 0;
  margin-left: auto;
}

.profile-icon-wrapper-mini {
  position: relative;
  width: 48px;
  height: 48px;
  flex-shrink: 0;
  margin: 0 auto;
}
.profile-icon-wrapper-mini.top-champ-ring .avatar-container-mini {
  box-shadow: 0 0 0 1.5px rgba(245, 158, 11, 0.65), 0 0 8px rgba(245, 158, 11, 0.35);
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
  stroke-width: 4.5;
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
  inset: 5.5px;
  border-radius: 50%;
  overflow: hidden;
  transition: box-shadow 0.25s ease;
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
  bottom: -3px;
  left: 50%;
  transform: translateX(-50%);
  color: var(--text-color);
  font-size: 0.60rem;
  font-weight: 800;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  padding: 0 3px;
  border-radius: 4px;
  z-index: 2;
  white-space: nowrap;
  line-height: 1.15;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.08);
}
.pc-name-lock {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: #d97706;
  flex-shrink: 0;
  opacity: 0.9;
}
.pc-name-lock-svg {
  width: 11px;
  height: 11px;
}

/* ─── 信息区 ─── */
.pc-info {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  justify-content: center;
  text-align: left;
  gap: 2.5px;
  min-width: 0;
  flex: 1;
}
.pc-row {
  display: flex;
  align-items: center;
  width: 100%;
  min-height: 16px;
  line-height: 1.2;
}

/* 第一行：名字与微标 */
.pc-name-row {
  display: flex;
  align-items: center;
  gap: 4px;
  min-width: 0;
}
.name-text {
  font-size: 0.82rem;
  font-weight: 700;
  color: var(--text-color);
  max-width: 95px;
  min-width: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  cursor: pointer;
  transition: color 0.15s ease-in-out;
}
.name-text:hover {
  color: var(--primary-color);
}
.name-tags-group {
  display: inline-flex;
  align-items: center;
  gap: 3.5px;
  min-width: 0;
  flex-wrap: nowrap;
}

/* 宿命徽标 */
.fate-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 0.58rem;
  font-weight: 800;
  width: 14px;
  height: 14px;
  border-radius: 3px;
  flex-shrink: 0;
  line-height: 1;
}
.fate-badge.ally {
  background: rgba(16, 185, 129, 0.15);
  color: #10b981;
  border: 1px solid rgba(16, 185, 129, 0.35);
}
.fate-badge.enemy {
  background: rgba(239, 68, 68, 0.15);
  color: #ef4444;
  border: 1px solid rgba(239, 68, 68, 0.35);
}

/* 标记玩家 */
.saved-badge {
  display: inline-flex;
  align-items: center;
  padding: 1px 4px;
  border-radius: 999px;
  font-size: 0.55rem;
  font-weight: 700;
  line-height: 1.2;
  background: var(--primary-color-alpha-15);
  color: var(--primary-color);
  border: 1px solid var(--primary-color-alpha-40);
  max-width: 50px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex-shrink: 0;
}
.saved-badge.met-only {
  background: var(--border-color);
  color: var(--text-muted);
  border: none;
}

/* 连胜 / 连败微徽标 */
.streak-badge {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  padding: 1px 4px;
  border-radius: 3px;
  font-size: 0.58rem;
  font-weight: 800;
  line-height: 1;
  flex-shrink: 0;
}
.streak-badge.win {
  background: linear-gradient(135deg, rgba(249, 115, 22, 0.14), rgba(239, 68, 68, 0.14));
  color: #ea580c;
  border: 1px solid rgba(249, 115, 22, 0.35);
}
.streak-badge.loss {
  background: linear-gradient(135deg, rgba(56, 189, 248, 0.14), rgba(59, 130, 246, 0.14));
  color: #2563eb;
  border: 1px solid rgba(59, 130, 246, 0.35);
}
.streak-svg {
  width: 9px;
  height: 9px;
  flex-shrink: 0;
}
.streak-svg.flame {
  color: #ea580c;
}
.streak-svg.snow {
  color: #2563eb;
}
.streak-count {
  line-height: 1;
}

/* 排位段位行 */
.pc-rank-row {
  display: flex;
  align-items: center;
  gap: 3px;
  font-size: 0.70rem;
  width: 100%;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
}
.rank-tier-icon {
  margin-right: 1px;
  flex-shrink: 0;
}
.rank-type-badge {
  font-size: 0.58rem;
  font-weight: 700;
  padding: 0.5px 3.5px;
  border-radius: 3px;
  line-height: 1.15;
  margin-right: 2px;
  flex-shrink: 0;
}
.rank-type-badge.solo {
  background: var(--primary-color-alpha-15);
  color: var(--primary-color);
  border: 1px solid var(--primary-color-alpha-30);
}
.rank-type-badge.flex {
  background: rgba(16, 185, 129, 0.12);
  color: #059669;
  border: 1px solid rgba(16, 185, 129, 0.25);
}

.tier-text {
  font-weight: 700;
  font-size: 0.69rem;
  white-space: nowrap;
}
.tier-text.IRON { color: #848c96; }
.tier-text.BRONZE { color: #b45309; }
.tier-text.SILVER { color: #8293a7; }
.tier-text.GOLD { color: #d97706; }
.tier-text.PLATINUM { color: #0d9488; }
.tier-text.EMERALD { color: #059669; }
.tier-text.DIAMOND { color: #3b82f6; }
.tier-text.MASTER { color: #8b5cf6; }
.tier-text.GRANDMASTER { color: #ef4444; }
.tier-text.CHALLENGER { color: #f59e0b; }

.tier-lp {
  font-weight: 600;
  color: var(--text-muted);
  font-size: 0.66rem;
  white-space: nowrap;
}
.tier-unranked {
  color: var(--text-dimmed);
  font-size: 0.68rem;
  font-weight: 500;
}
.rank-winrate-stats {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  font-size: 0.64rem;
  color: var(--text-muted);
  white-space: nowrap;
}
.stats-num {
  color: var(--text-dimmed);
  font-size: 0.64rem;
}

/* 战绩行：近 10 场胜率与 KDA */
.pc-stats-row {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 0.68rem;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
}
.stats-divider {
  color: var(--text-dimmed);
  opacity: 0.5;
  font-weight: 700;
}

.interactive-stat {
  cursor: help;
  position: relative;
  transition: opacity 0.15s ease;
  border-bottom: 1px dotted transparent;
}
.interactive-stat:hover {
  opacity: 0.85;
  border-bottom-color: currentColor;
}

.pc-winrate-text {
  font-size: 0.68rem;
  font-weight: 700;
}
.pc-kda-text {
  font-size: 0.68rem;
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

/* 数据悬停详细说明浮层 Tooltip */
.stat-tooltip-box {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 140px;
  padding: 2px 0;
}
.stat-tooltip-title {
  font-size: 0.72rem;
  font-weight: 700;
  color: var(--text-color);
  border-bottom: 1px solid var(--border-color);
  padding-bottom: 3px;
  margin-bottom: 2px;
}
.stat-tooltip-line {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  font-size: 0.65rem;
}
.stat-tooltip-label {
  color: var(--text-muted);
}
.stat-tooltip-val {
  color: var(--text-color);
  font-weight: 600;
}
.stat-tooltip-formula {
  font-size: 0.58rem;
  color: var(--text-dimmed);
  margin-top: 3px;
  border-top: 1px dashed var(--border-color);
  padding-top: 3px;
}
.hl-win {
  color: var(--win-color);
  font-weight: 700;
}
.hl-loss {
  color: var(--loss-color, #ef4444);
  font-weight: 700;
}
.hl-rate {
  font-weight: 700;
}
.hl-sep {
  color: var(--text-dimmed);
}

/* 招牌 / 练英雄微标（位于头像下方第二行） */
.mastery-role-badge {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  padding: 1px 4.5px;
  border-radius: 3px;
  font-size: 0.56rem;
  font-weight: 800;
  line-height: 1;
  flex-shrink: 0;
  letter-spacing: 0.2px;
}
.mastery-role-badge.top {
  background: linear-gradient(135deg, rgba(245, 158, 11, 0.16), rgba(217, 119, 6, 0.16));
  color: #b45309;
  border: 1px solid rgba(245, 158, 11, 0.45);
  box-shadow: 0 1px 3px rgba(245, 158, 11, 0.12);
}
.mastery-role-badge.practice {
  background: var(--hover-bg);
  color: var(--text-muted);
  border: 1px solid var(--border-color);
}
.role-svg.star {
  width: 8px;
  height: 8px;
  color: #f59e0b;
  flex-shrink: 0;
}

.wr-high {
  color: var(--win-color);
}
.wr-medium {
  color: var(--tier-blue);
}
.wr-low {
  color: var(--text-dimmed);
}

/* ─── 常玩位置微标（紧凑版：主位置带名称，次位置仅图标+占比） ─── */
.pc-pos-pill {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 1px 4px;
  background: var(--hover-bg);
  border: 1px solid var(--border-color);
  border-radius: 4px;
  font-size: 0.62rem;
  line-height: 1;
  cursor: pointer;
  margin-left: 0;
  white-space: nowrap;
  transition: all 0.15s ease;
}
.pc-pos-item {
  display: inline-flex;
  align-items: center;
  gap: 2px;
}
.pc-pos-item.is-secondary .pos-text {
  color: var(--text-muted);
  font-weight: 600;
}
.pc-pos-sep {
  color: var(--text-dimmed);
  font-size: 0.6rem;
  margin: 0 1px;
}
.pc-pos-pill:hover {
  background: var(--hover-bg-strong);
  border-color: rgba(var(--primary-color-rgb, 99, 102, 241), 0.3);
}
.pos-popover-box {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 170px;
}
.pos-popover-title {
  font-size: 0.72rem;
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
  font-size: 0.68rem;
}
.pop-name {
  font-weight: 600;
  color: var(--text-color);
  width: 24px;
  flex-shrink: 0;
}
.pop-bar-wrap {
  flex: 1;
  height: 4px;
  background: rgba(0, 0, 0, 0.06);
  border-radius: 2px;
  overflow: hidden;
}
.pop-bar-fill {
  height: 100%;
  border-radius: 2px;
  transition: width 0.3s ease;
}
.pop-val {
  font-size: 0.62rem;
  color: var(--text-muted);
  font-weight: 600;
  text-align: right;
  flex-shrink: 0;
}

/* ─── 近期常用英雄栏 ─── */
.pc-top-champs-bar {
  display: inline-flex;
  align-items: center;
  gap: 3.5px;
}
.pc-mini-champ-tag {
  position: relative;
  width: var(--pc-sub-row-h);
  height: var(--pc-sub-row-h);
  border-radius: 3px;
  cursor: pointer;
  flex-shrink: 0;
  transition: transform 0.15s ease;
}
.pc-mini-champ-tag:hover {
  transform: scale(1.15);
  z-index: 2;
}
.mini-champ-avatar {
  width: 100%;
  height: 100%;
  border-radius: 3px;
  object-fit: cover;
  border: 1px solid var(--border-color);
}
.mini-champ-badge {
  position: absolute;
  right: -3px;
  bottom: -3px;
  font-size: 0.5rem;
  font-weight: 800;
  line-height: 1;
  padding: 0.5px 2px;
  border-radius: 2px;
  background: rgba(15, 23, 42, 0.85);
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
}
.mini-champ-badge.wr-high {
  color: #10b981;
}
.mini-champ-badge.wr-mid {
  color: #60a5fa;
}
.mini-champ-badge.wr-low {
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
</style>
