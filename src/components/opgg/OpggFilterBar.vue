<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";

defineProps<{
  region: string;
  mode: string;
  tier: string;
  position: string;
}>();

const emit = defineEmits<{
  (e: "update:region", value: string): void;
  (e: "update:mode", value: string): void;
  (e: "update:tier", value: string): void;
  (e: "update:position", value: string): void;
}>();

const REGIONS = [
  { value: "kr", label: "韩服" },
  { value: "global", label: "全球" },
];

const MODES = [
  { value: "ranked", label: "召唤师峡谷 (排位)" },
  { value: "normal", label: "召唤师峡谷 (匹配)" },
  { value: "aram", label: "极地大乱斗" },
  { value: "arena", label: "斗魂竞技场" },
];

const TIERS = [
  { value: "all", label: "全部" },
  { value: "gold_plus", label: "黄金+" },
  { value: "platinum_plus", label: "铂金+" },
  { value: "emerald_plus", label: "翡翠+" },
  { value: "diamond_plus", label: "钻石+" },
  { value: "master", label: "大师" },
  { value: "master_plus", label: "大师+" },
  { value: "grandmaster", label: "宗师" },
  { value: "challenger", label: "王者" },
];

const POSITIONS = [
  { value: "TOP", label: "上单" },
  { value: "JUNGLE", label: "打野" },
  { value: "MID", label: "中单" },
  { value: "ADC", label: "ADC" },
  { value: "SUPPORT", label: "辅助" },
];

// 自定义下拉状态
const showRegionDropdown = ref(false);
const showModeDropdown = ref(false);
const showTierDropdown = ref(false);
const showPositionDropdown = ref(false);
function closeAllDropdowns() {
  showRegionDropdown.value = false;
  showModeDropdown.value = false;
  showTierDropdown.value = false;
  showPositionDropdown.value = false;
}

onMounted(() => {
  document.addEventListener("click", closeAllDropdowns);
});

onUnmounted(() => {
  document.removeEventListener("click", closeAllDropdowns);
});
</script>

<template>
  <!-- 筛选栏 -->
  <div class="opgg-filters">
    <div
      class="filter-trigger"
      @click.stop="showRegionDropdown = !showRegionDropdown"
    >
      <span>{{ $t("opgg.regions." + region) }}</span>
      <svg
        :class="['filter-arrow', { expanded: showRegionDropdown }]"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
      >
        <polyline points="6 9 12 15 18 9" />
      </svg>
      <div v-if="showRegionDropdown" class="filter-menu" @click.stop>
        <div
          v-for="r in REGIONS"
          :key="r.value"
          :class="['filter-item', { active: region === r.value }]"
          @click="
            emit('update:region', r.value);
            showRegionDropdown = false;
          "
        >
          {{ $t("opgg.regions." + r.value) }}
        </div>
      </div>
    </div>
    <div
      class="filter-trigger"
      @click.stop="showModeDropdown = !showModeDropdown"
    >
      <span>{{ $t("opgg.modes." + mode) }}</span>
      <svg
        :class="['filter-arrow', { expanded: showModeDropdown }]"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
      >
        <polyline points="6 9 12 15 18 9" />
      </svg>
      <div v-if="showModeDropdown" class="filter-menu" @click.stop>
        <div
          v-for="m in MODES"
          :key="m.value"
          :class="['filter-item', { active: mode === m.value }]"
          @click="
            emit('update:mode', m.value);
            showModeDropdown = false;
          "
        >
          {{ $t("opgg.modes." + m.value) }}
        </div>
      </div>
    </div>
    <div
      class="filter-trigger"
      @click.stop="showTierDropdown = !showTierDropdown"
    >
      <span>{{ $t("opgg.tiers." + tier) }}</span>
      <svg
        :class="['filter-arrow', { expanded: showTierDropdown }]"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
      >
        <polyline points="6 9 12 15 18 9" />
      </svg>
      <div v-if="showTierDropdown" class="filter-menu" @click.stop>
        <div
          v-for="t in TIERS"
          :key="t.value"
          :class="['filter-item', { active: tier === t.value }]"
          @click="
            emit('update:tier', t.value);
            showTierDropdown = false;
          "
        >
          {{ $t("opgg.tiers." + t.value) }}
        </div>
      </div>
    </div>
    <div
      v-if="mode === 'ranked'"
      class="filter-trigger"
      @click.stop="showPositionDropdown = !showPositionDropdown"
    >
      <span>{{ $t("opgg.positions." + position) }}</span>
      <svg
        :class="['filter-arrow', { expanded: showPositionDropdown }]"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
      >
        <polyline points="6 9 12 15 18 9" />
      </svg>
      <div v-if="showPositionDropdown" class="filter-menu" @click.stop>
        <div
          v-for="p in POSITIONS"
          :key="p.value"
          :class="['filter-item', { active: position === p.value }]"
          @click="
            emit('update:position', p.value);
            showPositionDropdown = false;
          "
        >
          {{ $t("opgg.positions." + p.value) }}
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.opgg-filters {
  display: flex;
  gap: 6px;
  padding: 8px 16px;
  border-bottom: 1px solid var(--border-color);
  flex-shrink: 0;
  position: relative;
  z-index: 2;
  background: var(--bg-color);
}

.filter-trigger {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 4px 8px;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  font-size: 0.78rem;
  background: var(--card-bg);
  color: var(--text-color);
  cursor: pointer;
  position: relative;
  transition: all 0.2s;
  flex-shrink: 0;
  min-width: 0;
}
.filter-trigger:hover {
  background: var(--card-bg);
  border-color: var(--primary-color);
}
.filter-arrow {
  width: 11px;
  height: 11px;
  transition: transform 0.2s;
  flex-shrink: 0;
}
.filter-arrow.expanded {
  transform: rotate(180deg);
}
.filter-menu {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  box-shadow: var(--shadow-lg);
  z-index: 100;
  min-width: max-content;
  padding: 4px 0;
}
.filter-item {
  padding: 6px 14px;
  font-size: 0.78rem;
  color: var(--text-muted);
  cursor: pointer;
  transition: all 0.2s;
  white-space: nowrap;
}
.filter-item:hover {
  background: rgba(0, 0, 0, 0.02);
  color: var(--text-color);
}
.filter-item.active {
  color: var(--primary-color);
  font-weight: 600;
  background: var(--primary-color-alpha-15);
}

/* 过滤下拉菜单悬浮背景 */
.filter-item:hover {
  background: var(--hover-bg);
  color: var(--text-color);
}
</style>
