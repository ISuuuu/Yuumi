<script setup lang="ts">
import LcuImage from "../LcuImage.vue";
import type { GameDataAssets } from "../../types/lcu";
import type { OpggIdWinRate } from "../../types/opgg";
import { pct, resolveItemIcon } from "./shared";

const props = defineProps<{
  starterItems?: OpggIdWinRate[];
  coreItems?: OpggIdWinRate[];
  boots?: OpggIdWinRate[];
  lastItems?: OpggIdWinRate[];
  gameDataAssets: GameDataAssets | null;
}>();

function getItemIcon(id: number | undefined): string {
  return resolveItemIcon(props.gameDataAssets, id);
}
</script>

<template>
  <!-- 出装：初始装备 + 鞋子 | 核心装备 → | 可选装备 -->
  <div
    v-if="
      boots?.length ||
      coreItems?.length ||
      starterItems?.length ||
      lastItems?.length
    "
    class="build-card item-card"
  >
    <!-- 初始装备 + 鞋子（并排） -->
    <div class="item-row-flex" v-if="starterItems?.length || boots?.length">
      <div class="item-group">
        <div
          v-for="(item, i) in (starterItems || []).slice(0, 3)"
          :key="'s' + i"
          class="item-entry"
        >
          <LcuImage
            v-for="id in item.ids"
            :key="id"
            :src="getItemIcon(id)"
            class="item-icon"
          />
          <span class="item-entry-wr">{{ pct(item.win / item.play) }}</span>
          <span class="item-entry-games">{{
            $t("career.gamesCount", { count: item.play })
          }}</span>
        </div>
      </div>
      <div class="item-v-divider"></div>
      <div class="item-group">
        <div
          v-for="(item, i) in (boots || []).slice(0, 3)"
          :key="'b' + i"
          class="item-entry"
        >
          <LcuImage
            v-for="id in item.ids"
            :key="id"
            :src="getItemIcon(id)"
            class="item-icon"
          />
          <span class="item-entry-wr">{{ pct(item.win / item.play) }}</span>
          <span class="item-entry-games">{{
            $t("career.gamesCount", { count: item.play })
          }}</span>
        </div>
      </div>
    </div>

    <div class="item-h-divider" v-if="coreItems?.length"></div>

    <!-- 核心装备（图标链 + 箭头 + 胜率） -->
    <div
      v-for="(item, i) in (coreItems || []).slice(0, 5)"
      :key="'c' + i"
      class="item-entry-row"
    >
      <div class="item-icons-chain">
        <template v-for="(id, j) in item.ids" :key="j">
          <LcuImage :src="getItemIcon(id)" class="item-icon" />
          <span v-if="j < item.ids.length - 1" class="item-arrow">›</span>
        </template>
      </div>
      <div class="item-entry-stats">
        <span class="item-entry-wr">{{ pct(item.win / item.play) }}</span>
        <span class="item-entry-games">{{
          $t("career.gamesCount", { count: item.play })
        }}</span>
      </div>
      <span class="item-entry-pick">{{ pct(item.pick_rate) }}</span>
    </div>

    <div class="item-h-divider" v-if="lastItems?.length"></div>

    <!-- 可选装备（一排图标） -->
    <div v-if="lastItems?.length" class="last-items-row">
      <LcuImage
        v-for="(item, i) in lastItems.slice(0, 10)"
        :key="'l' + i"
        :src="getItemIcon(item.ids?.[0])"
        class="item-icon"
      />
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

/* 出装 */
.item-row-flex {
  display: flex;
  gap: 0;
}
.item-group {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.item-v-divider {
  width: 1px;
  background: var(--border-color);
  margin: 0 8px;
}
.item-h-divider {
  height: 1px;
  background: var(--border-color);
  margin: 6px 0;
}
.item-entry {
  display: flex;
  align-items: center;
  gap: 4px;
}
.item-icon {
  width: 26px;
  height: 26px;
  border-radius: 4px;
  border: 1px solid var(--border-color);
}
.item-entry-wr {
  font-size: 0.72rem;
  font-weight: 600;
  color: var(--text-color);
  margin-left: auto;
}
.item-entry-games {
  font-size: 0.62rem;
  color: var(--text-dimmed);
  min-width: 50px;
}
.item-entry-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 3px 0;
}
.item-icons-chain {
  display: flex;
  align-items: center;
  gap: 3px;
}
.item-arrow {
  color: var(--text-dimmed);
  font-size: 0.7rem;
  margin: 0 2px;
}
.item-entry-stats {
  display: flex;
  flex-direction: column;
  align-items: center;
  margin-left: auto;
}
.item-entry-pick {
  font-size: 0.72rem;
  font-weight: 700;
  color: var(--text-muted);
  min-width: 40px;
  text-align: right;
}
.last-items-row {
  display: flex;
  gap: 4px;
  flex-wrap: wrap;
}
</style>
