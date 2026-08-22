<script setup lang="ts">
import { computed } from "vue";
import LcuImage from "../LcuImage.vue";
import type { TierListItem } from "../../types/opgg";
import { pct, fmtKda, champIcon as getChampIcon } from "./shared";
import tierIcon1 from "../../assets/tier/tier-1.svg";
import tierIcon2 from "../../assets/tier/tier-2.svg";
import tierIcon3 from "../../assets/tier/tier-3.svg";
import tierIcon4 from "../../assets/tier/tier-4.svg";

const props = defineProps<{
  items: TierListItem[];
  championsMap: Map<number, string>;
  dataTheme: string;
}>();

const emit = defineEmits<{
  (e: "select", championId: number): void;
}>();

// Tier 等级图标（盾牌 + 数字）
const TIER_ICONS: Record<string, string> = {
  "0": tierIcon1,
  "1": tierIcon1,
  "2": tierIcon2,
  "3": tierIcon3,
  "4": tierIcon4,
  "5": tierIcon4,
};

// 梯队卡片行背景色（适配深色模式和亮色模式）
const TIER_CARD_BG = computed<Record<string, string>>(() => {
  const isDark = props.dataTheme === "dark";
  if (isDark) {
    return {
      "0": "rgba(59, 130, 246, 0.12)",
      "1": "rgba(59, 130, 246, 0.12)", // T1 蓝
      "2": "rgba(20, 184, 166, 0.12)", // T2 青
      "3": "rgba(234, 179, 8, 0.08)", // T3 金
      "4": "rgba(148, 163, 184, 0.08)", // T4 灰
      "5": "rgba(148, 163, 184, 0.08)",
      "": "transparent",
    };
  }
  return {
    "0": "#CDE5F8",
    "1": "#CDE5F8", // T1 蓝
    "2": "#CDECEA", // T2 青
    "3": "#F4EAD1", // T3 暖
    "4": "#E5E8EC", // T4 灰
    "5": "#E5E8EC",
    "": "transparent",
  };
});

// 卡片描边配色（原 TIER_CARD_BORDER，取自出装标题栏边框配色表）
const TIER_CARD_BORDER = computed<Record<string, string>>(() => {
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
</script>

<template>
  <!-- 表头 -->
  <div class="tier-header">
    <span class="tier-h-rank">#</span>
    <span class="tier-h-champ">{{
      $t("championPicker.title").substring(2) || "英雄"
    }}</span>
    <span class="tier-h-spacer"></span>
    <span class="tier-h-tier">Tier</span>
    <span class="tier-h-stat">{{ $t("career.winRate") }}</span>
    <span class="tier-h-stat">{{ $t("opgg.pickRate") }}</span>
    <span class="tier-h-stat">{{ $t("opgg.banRate") }}</span>
    <span class="tier-h-stat">KDA</span>
    <span class="tier-h-counters">{{ $t("opgg.counters") }}</span>
  </div>
  <!-- 卡片列表 -->
  <div class="tier-cards">
    <div
      v-for="(c, i) in items"
      :key="c.id"
      class="tier-card"
      :style="{
        background: TIER_CARD_BG[String(c.tier)] || 'transparent',
        borderColor:
          TIER_CARD_BORDER[String(c.tier)] || 'rgba(0, 0, 0, 0.095)',
      }"
      @click="emit('select', c.id)"
    >
      <span class="tier-c-rank">{{ i + 1 }}</span>
      <LcuImage :src="getChampIcon(c.id)" class="tier-c-icon" />
      <span class="tier-c-name">{{
        championsMap.get(c.id) || c.name
      }}</span>
      <span class="tier-c-spacer"></span>
      <img
        v-if="TIER_ICONS[String(c.tier)]"
        :src="TIER_ICONS[String(c.tier)]"
        class="tier-c-icon-svg"
        alt=""
      />
      <span v-else class="tier-c-tier-badge">{{ c.tier || "-" }}</span>
      <span class="tier-c-stat">{{ pct(c.win_rate) }}</span>
      <span class="tier-c-stat">{{ pct(c.pick_rate) }}</span>
      <span class="tier-c-stat">{{ pct(c.ban_rate) }}</span>
      <span class="tier-c-stat">{{ fmtKda(c.kda) }}</span>
      <div class="tier-c-counters">
        <LcuImage
          v-for="cid in (c.counters || []).slice(0, 3)"
          :key="cid"
          :src="getChampIcon(cid)"
          class="tier-counter-icon"
        />
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 梯队卡片列表 */
.tier-header {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 10px;
  margin-bottom: 3px;
  font-size: 0.72rem;
  font-weight: 600;
  color: var(--text-muted);
  text-transform: uppercase;
  border-bottom: 1px solid var(--border-color);
}
.tier-h-rank {
  width: 30px;
  text-align: center;
  flex-shrink: 0;
}
.tier-h-champ {
  flex: 1;
}
.tier-h-spacer {
  flex: 1;
}
.tier-h-tier {
  width: 50px;
  text-align: center;
  flex-shrink: 0;
}
.tier-h-stat {
  width: 65px;
  text-align: center;
  flex-shrink: 0;
}

.tier-cards {
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.tier-card {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 12px 14px;
  border: 1px solid rgba(0, 0, 0, 0.095);
  border-radius: 6px;
  cursor: pointer;
  transition: filter 0.15s;
  font-size: 0.78rem;
  color: var(--text-muted);
}
.tier-card:hover {
  filter: brightness(0.93);
}

.tier-c-rank {
  width: 30px;
  text-align: center;
  flex-shrink: 0;
  color: var(--text-dimmed);
}
.tier-c-icon {
  width: 34px;
  height: 34px;
  border-radius: 50%;
  border: 2px solid var(--border-color);
  flex-shrink: 0;
}
.tier-c-name {
  flex-shrink: 0;
  font-weight: 500;
  color: var(--text-color);
}
.tier-c-spacer {
  flex: 1;
}
.tier-c-tier-badge {
  width: 50px;
  text-align: center;
  flex-shrink: 0;
  font-weight: 700;
  font-size: 0.7rem;
}
.tier-c-icon-svg {
  width: 24px;
  height: 24px;
  flex-shrink: 0;
}
.tier-c-stat {
  width: 65px;
  text-align: center;
  flex-shrink: 0;
}
.tier-h-counters {
  width: 90px;
  text-align: center;
  flex-shrink: 0;
}
.tier-c-counters {
  display: flex;
  align-items: center;
  gap: 3px;
  flex-shrink: 0;
  width: 90px;
  justify-content: center;
}
.tier-counter-icon {
  width: 22px;
  height: 22px;
  flex-shrink: 0;
  border-radius: 50%;
  border: 1px solid var(--border-color);
  object-fit: cover;
}
</style>
