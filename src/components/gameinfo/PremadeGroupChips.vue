<script setup lang="ts">
import { PREMADE_COLORS, getChampionIcon, type PremadeGroup } from "../../types/gameInfo";
import LcuImage from "../LcuImage.vue";

defineProps<{
  groups: PremadeGroup[];
}>();
</script>

<template>
  <div v-if="groups.length > 0" class="premade-chips-wrapper">
    <div
      v-for="group in groups"
      :key="group.colorIdx"
      class="premade-group-chip"
      :style="{
        borderColor: PREMADE_COLORS[group.colorIdx % PREMADE_COLORS.length].border,
        backgroundColor: PREMADE_COLORS[group.colorIdx % PREMADE_COLORS.length].bg,
      }"
      :title="$t('gameInfo.premadeIdx', { idx: group.colorIdx + 1 })"
    >
      <span
        class="legend-dot"
        :style="{
          background: PREMADE_COLORS[group.colorIdx % PREMADE_COLORS.length].dot,
        }"
      ></span>
      <div class="premade-avatars">
        <template v-for="m in group.members" :key="m.summonerId">
          <LcuImage
            v-if="m.championId > 0"
            :src="getChampionIcon(m.championId)"
            class="premade-avatar"
            :title="m.displayName"
          />
          <div v-else class="premade-avatar premade-avatar-empty" :title="m.displayName">
            {{ m.displayName ? m.displayName.slice(0, 1) : '?' }}
          </div>
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
.premade-chips-wrapper {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: nowrap;
  overflow: hidden;
}

.premade-group-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 2px 7px;
  border-width: 1px;
  border-style: solid;
  border-radius: 999px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.06);
  flex-shrink: 0;
  transition: all 0.2s ease;
  backdrop-filter: blur(4px);
  -webkit-backdrop-filter: blur(4px);
}
.premade-group-chip:hover {
  filter: brightness(1.1);
  transform: translateY(-1px);
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.12);
}

.legend-dot {
  display: inline-block;
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex-shrink: 0;
  box-shadow: 0 0 5px currentColor;
}

.premade-avatars {
  display: flex;
  align-items: center;
}

.premade-avatar {
  width: 20px;
  height: 20px;
  border-radius: 50%;
  object-fit: cover;
  border: 1.5px solid rgba(255, 255, 255, 0.8);
  box-sizing: border-box;
  margin-left: -4px;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.2);
}
.premade-avatar:first-child {
  margin-left: 0;
}

.premade-avatar-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.2);
  color: var(--text-dimmed);
  font-size: 0.6rem;
  font-weight: 800;
  border: 1.5px solid rgba(255, 255, 255, 0.8);
  box-sizing: border-box;
  margin-left: -4px;
}
.premade-avatar-empty:first-child {
  margin-left: 0;
}
</style>
