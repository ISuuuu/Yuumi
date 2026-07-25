<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import LcuImage from "../LcuImage.vue";
import type { TftMatchDisplay } from "../../composables/useTftData";

const props = defineProps<{
  match: TftMatchDisplay;
}>();

const { t } = useI18n();

const isFirst = computed(() => props.match.placement === 1);
const isTop4 = computed(() => props.match.placement >= 1 && props.match.placement <= 4);

const placementClass = computed(() => {
  if (isFirst.value) return "rank-first";
  if (isTop4.value) return "rank-top4";
  return "rank-bottom";
});

// 筛选并精简羁绊：只展示高层级激活的前4个核心羁绊
const displayTraits = computed(() => {
  if (!props.match.traits) return [];
  return [...props.match.traits]
    .filter((t) => t.tierCurrent > 0 && t.numUnits > 0)
    .sort((a, b) => b.tierCurrent - a.tierCurrent || b.numUnits - a.numUnits)
    .slice(0, 4);
});

// 悬浮提示完整羁绊列表
const fullTraitsTooltip = computed(() => {
  if (!props.match.traits) return "";
  return props.match.traits
    .filter((t) => t.tierCurrent > 0)
    .map((t) => `${t.name}: ${t.numUnits}`)
    .join("\n");
});
</script>

<template>
  <div :class="['tft-match-card', placementClass]">
    <!-- 左侧：名次与基础统计 -->
    <div class="match-left">
      <div class="placement-badge">
        <span class="placement-num">#{{ match.placement }}</span>
        <span class="placement-text">
          {{ isFirst ? t("tftPage.match.firstPlace") : isTop4 ? t("tftPage.match.top4") : t("tftPage.match.rank", { n: match.placement }) }}
        </span>
      </div>

      <div class="meta-info">
        <span class="queue-tag">{{ match.queueName }}</span>
        <span class="time-str">{{ match.timeStr }}</span>
        <span class="duration-str">{{ match.durationStr }}</span>
      </div>

      <div class="economy-info">
        <span class="level-badge">{{ t("tftPage.match.level", { n: match.level }) }}</span>
        <span class="gold-badge">{{ t("tftPage.match.goldLeft", { n: match.goldLeft }) }}</span>
      </div>
    </div>

    <!-- 中间：出战棋子矩阵（主体） -->
    <div class="match-center">
      <div class="units-grid">
        <div
          v-for="(unit, idx) in match.units"
          :key="idx"
          class="unit-item"
          :title="`${unit.name || unit.characterId} (${unit.tier}星)${unit.itemNames && unit.itemNames.length ? '\n装备: ' + unit.itemNames.join(', ') : ''}`"
        >
          <!-- 星级图标 -->
          <div class="star-level">
            <span v-for="s in unit.tier" :key="s" class="star">★</span>
          </div>

          <!-- 棋子头像 -->
          <div class="unit-avatar-box">
            <LcuImage :src="unit.iconUrl" class="unit-avatar" />
          </div>

          <!-- 装备小图标横栏（若无装备则保留固定高度占位） -->
          <div class="unit-items-row">
            <template v-if="unit.itemIconUrls && unit.itemIconUrls.length > 0">
              <div
                v-for="(itemIcon, iIdx) in unit.itemIconUrls"
                :key="iIdx"
                class="item-icon-box"
                :title="unit.itemNames[iIdx] || '装备'"
              >
                <LcuImage :src="itemIcon" class="item-img" />
              </div>
            </template>
            <template v-else-if="unit.itemNames && unit.itemNames.length > 0">
              <div class="unit-items-dot" :title="`装备: ${unit.itemNames.join(', ')}`">
                {{ unit.itemNames.length }}
              </div>
            </template>
          </div>
        </div>
      </div>
    </div>

    <!-- 右侧：核心羁绊（精简展示） -->
    <div class="match-right">
      <div v-if="displayTraits.length > 0" class="traits-list" :title="fullTraitsTooltip">
        <div
          v-for="(trait, idx) in displayTraits"
          :key="idx"
          class="trait-badge"
        >
          <LcuImage v-if="trait.iconUrl" :src="trait.iconUrl" class="trait-icon" />
          <span class="trait-name">{{ trait.name || '羁绊' }}</span>
          <span class="trait-count">{{ trait.numUnits }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.tft-match-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-radius: var(--radius-md);
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
  box-shadow: var(--shadow-sm);
  transition: all 0.2s cubic-bezier(0.25, 0.8, 0.25, 1);
  gap: 16px;
  cursor: pointer;
}

.tft-match-card:hover {
  background: var(--card-bg-hover);
  border-color: var(--border-color-hover);
  box-shadow: var(--shadow-md);
  transform: translateY(-1px);
}

.rank-first {
  border-left: 4px solid #f59e0b;
  background: linear-gradient(90deg, rgba(245, 158, 11, 0.08) 0%, var(--card-bg) 40%);
}

.rank-top4 {
  border-left: 4px solid var(--win-color);
  background: linear-gradient(90deg, var(--win-bg) 0%, var(--card-bg) 40%);
}

.rank-bottom {
  border-left: 4px solid var(--loss-color);
  opacity: 0.88;
}

.match-left {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 120px;
}

.placement-badge {
  display: flex;
  align-items: baseline;
  gap: 6px;
}

.placement-num {
  font-size: 1.1rem;
  font-weight: 900;
  color: var(--text-color);
}

.rank-first .placement-num {
  color: #f59e0b;
}

.rank-top4 .placement-num {
  color: var(--win-color);
}

.placement-text {
  font-size: 0.78rem;
  font-weight: bold;
  color: var(--text-muted);
}

.meta-info {
  display: flex;
  gap: 6px;
  font-size: 0.72rem;
  color: var(--text-muted);
}

.economy-info {
  display: flex;
  gap: 6px;
  margin-top: 2px;
}

.level-badge,
.gold-badge {
  font-size: 0.72rem;
  padding: 1px 5px;
  border-radius: 4px;
  background: rgba(0, 0, 0, 0.05);
  color: var(--text-color);
  font-weight: 600;
}

.match-center {
  flex: 1;
  min-width: 0;
  overflow: hidden;
}

.units-grid {
  display: flex;
  flex-direction: row;
  align-items: flex-end;
  flex-wrap: nowrap;
  gap: 10px;
  overflow-x: auto;
  padding: 4px 2px;
}

.units-grid::-webkit-scrollbar {
  height: 3px;
}

.units-grid::-webkit-scrollbar-thumb {
  background: var(--border-color);
  border-radius: 3px;
}

.unit-item {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  flex-shrink: 0;
}

.star-level {
  display: flex;
  gap: 1px;
  height: 14px;
  align-items: center;
}

.star {
  font-size: 0.72rem;
  color: #f59e0b;
  line-height: 1;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.4);
}

.unit-avatar-box {
  width: 46px;
  height: 46px;
  border-radius: 8px;
  overflow: hidden;
  border: 1px solid var(--border-color);
  background: #000;
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.15);
  transition: transform 0.15s ease;
}

.unit-item:hover .unit-avatar-box {
  transform: translateY(-2px);
  border-color: var(--primary-color);
}

.unit-avatar {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.unit-items-row {
  display: flex;
  justify-content: center;
  align-items: center;
  gap: 2px;
  margin-top: 3px;
  height: 17px;
  min-height: 17px;
}

.item-icon-box {
  width: 16px;
  height: 16px;
  border-radius: 4px;
  overflow: hidden;
  border: 1px solid rgba(255, 255, 255, 0.4);
  background: #000;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
}

.item-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.unit-items-dot {
  position: absolute;
  bottom: -3px;
  right: -3px;
  background: linear-gradient(135deg, #3b82f6, #1d4ed8);
  color: white;
  font-size: 0.65rem;
  font-weight: bold;
  border-radius: 50%;
  width: 17px;
  height: 17px;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.4);
  border: 1px solid rgba(255, 255, 255, 0.6);
}

.match-right {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 4px;
  min-width: 90px;
  max-width: 120px;
  flex-shrink: 0;
}

.traits-list {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 3px;
}

.trait-badge {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 1px 4px;
  border-radius: 3px;
  background: rgba(0, 0, 0, 0.04);
  border: 1px solid var(--border-color);
  font-size: 0.65rem;
  color: var(--text-color);
}

.trait-icon {
  width: 14px;
  height: 14px;
  object-fit: contain;
}
</style>
