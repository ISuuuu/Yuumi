<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { TftRankDisplay, TftMatchSummary } from "../../composables/useTftData";

const props = defineProps<{
  rankedStats: TftRankDisplay | null;
  summary: TftMatchSummary | null;
}>();

const { t } = useI18n();

const TIER_MAP: Record<string, string> = {
  NONE: "无段位",
  UNRANKED: "未组排位",
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
  GRAY: "灰白",
  GREEN: "翠绿",
  BLUE: "天蓝",
  PURPLE: "华紫",
  ORANGE: "耀橙",
};

function formatTier(tier: string, division: string): string {
  if (!tier || tier === "UNRANKED" || tier === "NONE") {
    return t("tftPage.stats.unranked");
  }
  const tierName = TIER_MAP[tier.toUpperCase()] || tier;
  if (["MASTER", "GRANDMASTER", "CHALLENGER"].includes(tier.toUpperCase()) || division === "NA") {
    return tierName;
  }
  return `${tierName} ${division}`;
}
</script>

<template>
  <div class="tft-rank-header">
    <!-- 排位段位卡片 Grid -->
    <div class="rank-cards-grid">
      <!-- 1. 标准单排 -->
      <div class="rank-card">
        <div class="card-title-row">
          <span class="card-badge solo">单排</span>
          <span class="mode-name">{{ t("tftPage.stats.standardRank") }}</span>
        </div>
        <div class="rank-main">
          <span class="tier-text">{{ formatTier(props.rankedStats?.soloTier || '', props.rankedStats?.soloDivision || '') }}</span>
          <span v-if="props.rankedStats && props.rankedStats.soloTier !== 'UNRANKED'" class="lp-text">
            {{ props.rankedStats.soloLp }} LP
          </span>
        </div>
        <div class="rank-sub" v-if="props.rankedStats">
          <span>胜: {{ props.rankedStats.soloWins }}</span>
          <span>负: {{ props.rankedStats.soloLosses }}</span>
        </div>
      </div>

      <!-- 2. 狂暴模式 -->
      <div class="rank-card">
        <div class="card-title-row">
          <span class="card-badge turbo">狂暴</span>
          <span class="mode-name">{{ t("tftPage.stats.turboRank") }}</span>
        </div>
        <div class="rank-main">
          <span class="tier-text">{{ formatTier(props.rankedStats?.turboTier || '', 'NA') }}</span>
          <span class="lp-text" v-if="props.rankedStats">
            {{ props.rankedStats.turboRating }} {{ t("tftPage.stats.rating") }}
          </span>
        </div>
        <div class="rank-sub" v-if="props.rankedStats">
          <span>胜场: {{ props.rankedStats.turboWins }}</span>
        </div>
      </div>

      <!-- 3. 双人作战 -->
      <div class="rank-card">
        <div class="card-title-row">
          <span class="card-badge double">双人</span>
          <span class="mode-name">{{ t("tftPage.stats.doubleRank") }}</span>
        </div>
        <div class="rank-main">
          <span class="tier-text">{{ formatTier(props.rankedStats?.doubleTier || '', props.rankedStats?.doubleDivision || '') }}</span>
          <span v-if="props.rankedStats && props.rankedStats.doubleTier !== 'UNRANKED'" class="lp-text">
            {{ props.rankedStats.doubleLp }} LP
          </span>
        </div>
        <div class="rank-sub" v-if="props.rankedStats">
          <span>胜: {{ props.rankedStats.doubleWins }}</span>
          <span>负: {{ props.rankedStats.doubleLosses }}</span>
        </div>
      </div>

      <!-- 4. 战绩统计汇总 Tile -->
      <div class="summary-tile">
        <div class="summary-item">
          <span class="summary-label">{{ t("tftPage.stats.top4Rate") }}</span>
          <span class="summary-val highlight-green">
            {{ props.summary ? props.summary.top4Rate.toFixed(1) : 0 }}%
          </span>
        </div>
        <div class="summary-item">
          <span class="summary-label">{{ t("tftPage.stats.winRate") }}</span>
          <span class="summary-val highlight-gold">
            {{ props.summary ? props.summary.winRate.toFixed(1) : 0 }}%
          </span>
        </div>
        <div class="summary-item">
          <span class="summary-label">{{ t("tftPage.stats.avgPlacement") }}</span>
          <span class="summary-val">
            #{{ props.summary ? props.summary.avgPlacement.toFixed(2) : '0.00' }}
          </span>
        </div>
        <div class="summary-item">
          <span class="summary-label">{{ t("tftPage.stats.totalGames") }}</span>
          <span class="summary-val">
            {{ props.summary ? props.summary.totalGames : 0 }}
          </span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.tft-rank-header {
  margin-bottom: 1.2rem;
}

.rank-cards-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 12px;
}

.rank-card,
.summary-tile {
  padding: 14px 16px;
  border-radius: var(--radius-md);
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
  box-shadow: var(--shadow-sm);
  display: flex;
  flex-direction: column;
  justify-content: space-between;
}

.card-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.card-badge {
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 0.7rem;
  font-weight: bold;
  color: white;
}
.card-badge.solo {
  background: #3b82f6;
}
.card-badge.turbo {
  background: #f59e0b;
}
.card-badge.double {
  background: #10b981;
}

.mode-name {
  font-size: 0.8rem;
  font-weight: 700;
  color: var(--text-muted);
}

.rank-main {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  margin: 8px 0;
}

.tier-text {
  font-size: 1.05rem;
  font-weight: 800;
  color: var(--text-color);
}

.lp-text {
  font-size: 0.82rem;
  font-weight: 700;
  color: var(--primary-color);
}

.rank-sub {
  display: flex;
  gap: 12px;
  font-size: 0.75rem;
  color: var(--text-muted);
}

.summary-tile {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px 12px;
}

.summary-item {
  display: flex;
  flex-direction: column;
}

.summary-label {
  font-size: 0.72rem;
  color: var(--text-muted);
  margin-bottom: 2px;
}

.summary-val {
  font-size: 0.95rem;
  font-weight: 800;
  color: var(--text-color);
}

.highlight-green {
  color: var(--win-color);
}

.highlight-gold {
  color: #f59e0b;
}
</style>
