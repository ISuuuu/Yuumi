<script setup lang="ts">
import { computed, ref, watch, nextTick } from "vue";
import { NModal } from "naive-ui";
import LcuImage from "../LcuImage.vue";
import type { TftMatchDisplay, TftParticipantDisplay } from "../../composables/useTftData";

const props = defineProps<{
  show: boolean;
  match: TftMatchDisplay | null;
}>();

const emit = defineEmits<{
  (e: "update:show", value: boolean): void;
}>();

const visible = computed({
  get: () => props.show,
  set: (val) => emit("update:show", val),
});

const modalBodyRef = ref<HTMLElement | null>(null);

watch(
  () => props.show,
  (newVal) => {
    if (newVal) {
      nextTick(() => {
        modalBodyRef.value?.focus();
      });
    }
  }
);

function getParticipantTraits(p: TftParticipantDisplay) {
  if (!p.traits) return [];
  return [...p.traits]
    .filter((t) => t.tierCurrent > 0 && t.numUnits > 0)
    .sort((a, b) => b.tierCurrent - a.tierCurrent || b.numUnits - a.numUnits)
    .slice(0, 6);
}
</script>

<template>
  <n-modal
    v-model:show="visible"
    preset="card"
    class="tft-detail-modal"
    :style="{ width: '1100px', maxWidth: '95vw' }"
    :segmented="{ content: 'soft', footer: 'soft' }"
    :auto-focus="false"
  >
    <template #header>
      <div v-if="match" class="modal-header-content">
        <div class="header-left">
          <span :class="['my-rank-badge', `rank-${match.placement}`]">
            第 {{ match.placement }} 名
          </span>
          <span class="queue-title">对局结算榜 (8人对决)</span>
          <span class="match-meta">{{ match.timeStr }} • {{ match.durationStr }}</span>
        </div>
      </div>
    </template>

    <div v-if="match" ref="modalBodyRef" tabindex="-1" class="tft-modal-body" style="outline: none;">
      <div class="participants-list">
        <div
          v-for="p in match.participants"
          :key="p.puuid || p.placement"
          :class="['p-card', { 'is-self': p.isSelf, 'top4': p.placement <= 4 }]"
        >
          <!-- 召唤师名称 & 等级 -->
          <div class="p-info-col">
            <!-- 排名 -->
            <div class="p-rank-row">
              <span :class="['rank-tag', `rank-${p.placement}`]">
                #{{ p.placement }}
              </span>
            </div>
            <!-- 召唤师名称 -->
            <div class="p-name-row">
              <span class="p-name" :title="p.summonerName">{{ p.summonerName }}</span>
              <span v-if="p.isSelf" class="self-badge">我</span>
            </div>
            <!-- 等级等其它信息 -->
            <div class="p-meta-row">
              <span class="p-lvl">Lvl {{ p.level }}</span>
              <span class="p-gold">💰 {{ p.goldLeft }}</span>
              <span v-if="p.totalDamageToPlayers > 0" class="p-dmg">⚔️ {{ p.totalDamageToPlayers }}</span>
            </div>
          </div>

          <!-- 出战棋子列表 -->
          <div class="p-units-col">
            <div class="units-flex">
              <div
                v-for="(unit, uIdx) in p.units"
                :key="uIdx"
                class="unit-card"
                :title="`${unit.name || unit.characterId} (${unit.tier}星)${unit.itemNames && unit.itemNames.length ? '\n装备: ' + unit.itemNames.join(', ') : ''}`"
              >
                <!-- 星级 -->
                <div class="unit-stars">
                  <span v-for="s in unit.tier" :key="s" class="star">★</span>
                </div>
                <!-- 棋子头像 -->
                <div class="unit-img-box">
                  <LcuImage :src="unit.iconUrl" class="unit-img" />
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
                    <div class="item-badge" :title="`装备: ${unit.itemNames.join(', ')}`">
                      {{ unit.itemNames.length }}
                    </div>
                  </template>
                </div>
              </div>
            </div>
          </div>

          <!-- 激活羁绊 -->
          <div class="p-traits-col">
            <div class="traits-flex">
              <div
                v-for="(tr, tIdx) in getParticipantTraits(p)"
                :key="tIdx"
                class="trait-chip"
                :title="`${tr.name}: ${tr.numUnits}`"
              >
                <LcuImage v-if="tr.iconUrl" :src="tr.iconUrl" class="trait-img" />
                <span class="trait-name">{{ tr.name }}</span>
                <span class="trait-val">{{ tr.numUnits }}</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </n-modal>
</template>

<style scoped>
.tft-detail-modal {
  border-radius: var(--radius-lg, 12px);
}

.modal-header-content {
  display: flex;
  justify-content: space-between;
  align-items: center;
  width: 100%;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.queue-title {
  font-size: 1.1rem;
  font-weight: 800;
  color: var(--text-color);
}

.match-meta {
  font-size: 0.8rem;
  color: var(--text-muted);
}

.my-rank-badge {
  font-size: 0.9rem;
  font-weight: 800;
  padding: 3px 10px;
  border-radius: 6px;
  background: rgba(0, 0, 0, 0.05);
}

.my-rank-badge.rank-1 {
  background: rgba(245, 158, 11, 0.15);
  color: #f59e0b;
}

.my-rank-badge.rank-2,
.my-rank-badge.rank-3,
.my-rank-badge.rank-4 {
  background: var(--win-bg);
  color: var(--win-color);
}

.tft-modal-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.participants-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 76vh;
  overflow-y: auto;
  padding-right: 4px;
}

.p-card {
  display: flex;
  align-items: center;
  padding: 6px 12px;
  border-radius: 8px;
  background: var(--card-bg, rgba(0, 0, 0, 0.02));
  border: 1px solid var(--border-color);
  gap: 12px;
  transition: all 0.15s ease;
}

.p-card:hover {
  background: var(--card-bg-hover, rgba(0, 0, 0, 0.04));
}

.p-card.is-self {
  background: linear-gradient(90deg, rgba(59, 130, 246, 0.12) 0%, var(--card-bg) 70%);
  border-color: #3b82f6;
  box-shadow: 0 0 8px rgba(59, 130, 246, 0.15);
}

.p-card.top4 {
  border-left: 4px solid var(--win-color);
}

.p-rank-row {
  line-height: 1.2;
}

.rank-tag {
  font-size: 0.95rem;
  font-weight: 900;
  color: var(--text-muted);
}

.rank-tag.rank-1 { color: #f59e0b; }
.rank-tag.rank-2,
.rank-tag.rank-3,
.rank-tag.rank-4 { color: var(--win-color); }

.p-info-col {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 4px;
  min-width: 130px;
  max-width: 160px;
  text-align: left;
}

.p-name-row {
  display: flex;
  align-items: center;
  justify-content: flex-start;
  gap: 4px;
}

.p-name {
  font-size: 0.82rem;
  font-weight: 700;
  color: var(--text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  text-align: left;
}

.self-badge {
  font-size: 0.65rem;
  background: #3b82f6;
  color: white;
  padding: 0 4px;
  border-radius: 3px;
  font-weight: bold;
}

.p-meta-row {
  display: flex;
  gap: 6px;
  font-size: 0.7rem;
  color: var(--text-muted);
}

.p-units-col {
  flex: 1;
  min-width: 0;
}

.units-flex {
  display: flex;
  align-items: flex-end;
  gap: 6px;
  overflow-x: auto;
  padding: 2px 0;
}

.unit-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  flex-shrink: 0;
}

.unit-stars {
  height: 11px;
  display: flex;
  gap: 1px;
}

.star {
  font-size: 0.65rem;
  color: #f59e0b;
  line-height: 1;
}

.unit-img-box {
  position: relative;
  width: 32px;
  height: 32px;
  border-radius: 6px;
  overflow: hidden;
  border: 1px solid var(--border-color);
  background: #000;
}

.unit-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.unit-items-row {
  display: flex;
  justify-content: center;
  align-items: center;
  gap: 2px;
  margin-top: 2px;
  height: 15px;
  min-height: 15px;
}

.item-icon-box {
  width: 14px;
  height: 14px;
  border-radius: 3px;
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

.item-badge {
  position: absolute;
  bottom: 0;
  right: 0;
  background: linear-gradient(135deg, #3b82f6, #1d4ed8);
  color: white;
  font-size: 0.58rem;
  font-weight: bold;
  padding: 0 3px;
  border-top-left-radius: 4px;
}

.p-traits-col {
  min-width: 240px;
  max-width: 300px;
  flex-shrink: 0;
}

.traits-flex {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 3px;
  max-height: 56px;
  overflow: hidden;
}

.trait-chip {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  padding: 1px 5px;
  border-radius: 4px;
  background: rgba(0, 0, 0, 0.05);
  border: 1px solid var(--border-color);
  font-size: 0.68rem;
  max-width: 95px;
}

.trait-img {
  width: 13px;
  height: 13px;
  object-fit: contain;
}

.trait-name {
  font-size: 0.68rem;
  color: var(--text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 60px;
}

.trait-val {
  font-weight: bold;
  color: var(--primary-color);
}
</style>
