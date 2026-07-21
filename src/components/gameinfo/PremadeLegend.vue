<script setup lang="ts">
import type { PremadeRow } from "../../types/gameInfo";
import { PREMADE_COLORS } from "../../types/gameInfo";
import LcuImage from "../LcuImage.vue";

defineProps<{
  premadeRows: PremadeRow[];
}>();

function getChampionIcon(id: number): string {
  return id > 0 ? `/lol-game-data/assets/v1/champion-icons/${id}.png` : "";
}
</script>

<template>
  <div class="premade-legend">
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
</template>

<style scoped>
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
</style>
