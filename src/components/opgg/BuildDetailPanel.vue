<script setup lang="ts">
import { computed } from "vue";
import LcuImage from "../LcuImage.vue";
import RunesSection from "./RunesSection.vue";
import SkillOrderSection from "./SkillOrderSection.vue";
import ItemsSection from "./ItemsSection.vue";
import type { GameDataAssets } from "../../types/lcu";
import type {
  OpggBuildData,
  OpggRunePreset,
  PerkStyle,
  Perk,
} from "../../types/opgg";
import {
  pct,
  champIcon as getChampIcon,
  resolveSpellIcon,
} from "./shared";

const props = defineProps<{
  build: OpggBuildData;
  championId: number | null;
  position: string | null;
  championsMap: Map<number, string>;
  gameDataAssets: GameDataAssets | null;
  perkStyles: PerkStyle[];
  perksMap: Map<number, Perk>;
  dataTheme: string;
}>();

const emit = defineEmits<{
  (e: "apply-rune", rune: OpggRunePreset): void;
}>();

// 梯队徽章颜色（深色文字，用于 Tier 标签）
const TIER_COLORS: Record<string, string> = {
  "0": "#5B8DB8",
  "1": "#5B8DB8", // T1 蓝
  "2": "#5BA8A3", // T2 青
  "3": "#B89B52", // T3 金
  "4": "#7D8185", // T4 灰
  "5": "#7D8185",
  "": "#7D8185",
};

// 出装标题栏复用同样配色
const TIER_BG_COLORS = computed<Record<string, string>>(() => {
  const isDark = props.dataTheme === "dark";
  if (isDark) {
    return {
      "0": "rgba(59, 130, 246, 0.12)",
      "1": "rgba(59, 130, 246, 0.12)",
      "2": "rgba(20, 184, 166, 0.12)",
      "3": "rgba(234, 179, 8, 0.08)",
      "4": "rgba(148, 163, 184, 0.08)",
      "5": "rgba(148, 163, 184, 0.08)",
      "": "var(--card-bg)",
    };
  }
  return {
    "0": "#CDE5F8",
    "1": "#CDE5F8",
    "2": "#CDECEA",
    "3": "#F4EAD1",
    "4": "#E5E8EC",
    "5": "#E5E8EC",
    "": "#f8f9fa",
  };
});

const TIER_BORDER_COLORS = computed<Record<string, string>>(() => {
  const isDark = props.dataTheme === "dark";
  if (isDark) {
    return {
      "0": "rgba(59, 130, 246, 0.25)",
      "1": "rgba(59, 130, 246, 0.25)",
      "2": "rgba(20, 184, 166, 0.25)",
      "3": "rgba(234, 179, 8, 0.2)",
      "4": "rgba(148, 163, 184, 0.2)",
      "5": "rgba(148, 163, 184, 0.2)",
      "": "var(--border-color)",
    };
  }
  return {
    "0": "rgba(0, 0, 0, 0.095)",
    "1": "rgba(0, 0, 0, 0.095)",
    "2": "rgba(0, 0, 0, 0.095)",
    "3": "rgba(0, 0, 0, 0.095)",
    "4": "rgba(0, 0, 0, 0.095)",
    "5": "rgba(0, 0, 0, 0.095)",
    "": "rgba(0, 0, 0, 0.095)",
  };
});

// 出装页计算属性
const selectedStats = computed(() => {
  if (!props.build.summary) return null;
  const s = props.build.summary;
  if (props.position && s.positions) {
    const p = s.positions.find((p) => p.name === props.position);
    return p?.stats || s.average_stats || null;
  }
  return s.average_stats || null;
});
const selectedTier = computed(
  () => selectedStats.value?.tier_data?.tier || selectedStats.value?.tier,
);

const strongCounters = computed(() => {
  if (!props.build.counters) return [];
  return props.build.counters
    .map((c) => ({ ...c, rate: c.win / c.play }))
    .filter((c) => c.rate >= 0.5)
    .sort((a, b) => b.rate - a.rate)
    .slice(0, 5);
});
const weakCounters = computed(() => {
  if (!props.build.counters) return [];
  return props.build.counters
    .map((c) => ({ ...c, rate: c.win / c.play }))
    .filter((c) => c.rate < 0.5)
    .sort((a, b) => a.rate - b.rate)
    .slice(0, 5);
});

function getSpellIcon(id: number): string {
  return resolveSpellIcon(props.gameDataAssets, id);
}
</script>

<template>
  <!-- 英雄标题栏：图标 + 名称 + 胜率/登场率/禁用率 + Tier -->
  <div
    class="build-title-bar"
    v-if="build.summary"
    :style="{
      background: TIER_BG_COLORS[String(selectedTier)] || '#f8f9fa',
      borderColor:
        TIER_BORDER_COLORS[String(selectedTier)] || 'rgba(0, 0, 0, 0.04)',
    }"
  >
    <LcuImage
      :src="getChampIcon(build.summary.id || championId!)"
      class="build-champ-icon"
    />
    <div class="build-title-info">
      <div class="build-title-name">
        {{
          championsMap.get(build.summary.id || championId!) ||
          build.summary.name ||
          "英雄"
        }}
      </div>
      <div class="build-title-pos" v-if="position">
        {{ position }}
      </div>
    </div>
    <div class="build-title-stats">
      <div class="stat-col">
        <span class="stat-label">{{ $t("career.winRate") }}</span>
        <span class="stat-value">{{
          pct(selectedStats?.win_rate || selectedStats?.winRate)
        }}</span>
      </div>
      <div class="stat-divider"></div>
      <div class="stat-col">
        <span class="stat-label">{{ $t("opgg.pickRate") }}</span>
        <span class="stat-value">{{
          pct(selectedStats?.pick_rate || selectedStats?.pickRate)
        }}</span>
      </div>
      <div class="stat-divider"></div>
      <div class="stat-col">
        <span class="stat-label">{{ $t("opgg.banRate") }}</span>
        <span class="stat-value">{{
          pct(selectedStats?.ban_rate || selectedStats?.banRate)
        }}</span>
      </div>
    </div>
    <div class="build-title-tier" v-if="selectedTier">
      <span
        class="tier-badge-lg"
        :style="{
          background: TIER_COLORS[String(selectedTier)] || '#adb5bd',
        }"
      >
        {{ selectedTier }}
      </span>
    </div>
  </div>

  <!-- 召唤师技能：左右并排显示两组推荐 -->
  <div
    v-if="build.summoner_spells?.length"
    class="build-card spell-card"
  >
    <div class="spell-presets-container">
      <div
        class="spell-pair"
        v-for="(s, i) in build.summoner_spells.slice(0, 2)"
        :key="i"
      >
        <div class="spell-icons">
          <LcuImage
            v-for="id in s.ids?.slice(0, 2)"
            :key="id"
            :src="getSpellIcon(id)"
            class="spell-icon"
          />
        </div>
        <div class="spell-stats">
          <span class="spell-wr">{{ pct(s.win / s.play) }}</span>
          <span class="spell-games">{{
            $t("career.gamesCount", { count: s.play })
          }}</span>
        </div>
        <span class="spell-pick">{{ pct(s.pick_rate) }}</span>
      </div>
    </div>
  </div>

  <!-- 符文推荐：OP.GG 风格双符文树与碎片 -->
  <RunesSection
    :runes="build.runes"
    :perk-styles="perkStyles"
    :perks-map="perksMap"
    :game-data-assets="gameDataAssets"
    @apply="emit('apply-rune', $event)"
  />

  <!-- 技能加点：主技能图标 + 升级顺序 + 胜率 -->
  <SkillOrderSection
    :skills="build.skills"
    :masteries="build.skill_masteries"
  />

  <!-- 出装：初始装备 + 鞋子 | 核心装备 → | 可选装备 -->
  <ItemsSection
    :starter-items="build.starter_items"
    :core-items="build.core_items"
    :boots="build.boots"
    :last-items="build.last_items"
    :game-data-assets="gameDataAssets"
  />

  <!-- 克制关系：克制 / 被克 双列 -->
  <div v-if="build.counters?.length" class="build-card counter-card">
    <div class="counter-columns">
      <!-- 克制（胜率 > 50%） -->
      <div class="counter-col">
        <div class="counter-col-title">{{ $t("opgg.strongAgainst") }}</div>
        <div
          v-for="ct in strongCounters"
          :key="ct.champion_id"
          class="counter-row"
        >
          <div class="counter-champ">
            <LcuImage
              :src="getChampIcon(ct.champion_id)"
              class="counter-icon"
            />
            <span class="counter-name">{{
              championsMap.get(ct.champion_id) || ct.name || ct.champion_id
            }}</span>
          </div>
          <span class="counter-games">{{
            $t("career.gamesCount", { count: ct.play })
          }}</span>
          <span
            :class="[
              'counter-wr-val',
              ct.rate >= 0.5 ? 'wr-good' : 'wr-bad',
            ]"
          >
            {{ pct(ct.rate) }}
          </span>
        </div>
      </div>
      <div class="counter-v-divider"></div>
      <!-- 被克（胜率 < 50%） -->
      <div class="counter-col">
        <div class="counter-col-title">{{ $t("opgg.weakAgainst") }}</div>
        <div
          v-for="ct in weakCounters"
          :key="ct.champion_id"
          class="counter-row"
        >
          <div class="counter-champ">
            <LcuImage
              :src="getChampIcon(ct.champion_id)"
              class="counter-icon"
            />
            <span class="counter-name">{{
              championsMap.get(ct.champion_id) || ct.name || ct.champion_id
            }}</span>
          </div>
          <span class="counter-games">{{
            $t("career.gamesCount", { count: ct.play })
          }}</span>
          <span
            :class="[
              'counter-wr-val',
              ct.rate >= 0.5 ? 'wr-good' : 'wr-bad',
            ]"
          >
            {{ pct(ct.rate) }}
          </span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 通用卡片美化 */
.build-card {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  margin: 10px 14px;
  padding: 12px 16px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.02);
}

/* 召唤师技能 */
.spell-pair {
  flex: 1;
  display: flex;
  align-items: center;
  background: var(--hover-bg);
  border: 1px solid var(--border-color);
  border-radius: 6px;
  padding: 6px 12px;
  gap: 12px;
}

/* 英雄标题栏 */
.build-title-bar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 14px 18px;
  margin: 10px 14px;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.02);
}
.build-champ-icon {
  width: 46px;
  height: 46px;
  border-radius: 50%;
  border: 2px solid var(--border-color);
}
.build-title-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 60px;
}
.build-title-name {
  font-size: 0.95rem;
  font-weight: 700;
  color: var(--text-color);
}
.build-title-pos {
  font-size: 0.7rem;
  color: var(--text-dimmed);
}
.build-title-stats {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-left: auto;
}
.stat-col {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1px;
}
.stat-label {
  font-size: 0.65rem;
  color: var(--text-dimmed);
  text-transform: uppercase;
}
.stat-value {
  font-size: 0.82rem;
  font-weight: 700;
  color: var(--text-color);
}
.stat-divider {
  width: 1px;
  height: 24px;
  background: var(--border-color);
}
.build-title-tier {
  margin-left: 8px;
}
.tier-badge-lg {
  display: inline-block;
  padding: 2px 10px;
  border-radius: 4px;
  color: white;
  font-weight: 800;
  font-size: 0.82rem;
  text-align: center;
}

/* 召唤师技能：横向并列展示 */
.spell-presets-container {
  display: flex;
  gap: 12px;
  width: 100%;
}
.spell-pair {
  flex: 1;
  display: flex;
  align-items: center;
  background: rgba(0, 0, 0, 0.015);
  border: 1px solid var(--border-color);
  border-radius: 6px;
  padding: 6px 12px;
  gap: 12px;
}
.spell-icons {
  display: flex;
  gap: 4px;
}
.spell-icon {
  width: 28px;
  height: 28px;
  border-radius: 5px;
  border: 1px solid var(--border-color);
}
.spell-stats {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 1px;
  flex: 1;
}
.spell-wr {
  font-size: 0.78rem;
  font-weight: 700;
  color: var(--text-color);
}
.spell-games {
  font-size: 0.62rem;
  color: var(--text-dimmed);
}
.spell-pick {
  font-size: 0.75rem;
  font-weight: 700;
  color: var(--text-muted);
  text-align: right;
}

/* 克制关系 */
.counter-columns {
  display: flex;
  gap: 0;
}
.counter-col {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.counter-v-divider {
  width: 1px;
  background: var(--border-color);
  margin: 0 8px;
}
.counter-col-title {
  font-size: 0.7rem;
  font-weight: 700;
  color: var(--text-dimmed);
  text-transform: uppercase;
  margin-bottom: 4px;
}
.counter-row {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 28px;
}
.counter-champ {
  display: flex;
  align-items: center;
  gap: 5px;
  min-width: 0;
}
.counter-icon {
  width: 22px;
  height: 22px;
  border-radius: 50%;
  border: 1px solid var(--border-color);
  flex-shrink: 0;
}
.counter-name {
  font-size: 0.72rem;
  color: var(--text-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.counter-games {
  font-size: 0.62rem;
  color: var(--text-dimmed);
  margin-left: auto;
  min-width: 45px;
  text-align: center;
}
.counter-wr-val {
  font-size: 0.75rem;
  font-weight: 700;
  min-width: 40px;
  text-align: right;
}

.wr-good {
  color: var(--win-color);
}
.wr-bad {
  color: var(--loss-color);
}
</style>
