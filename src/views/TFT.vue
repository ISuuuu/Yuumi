<script setup lang="ts">
import { ref, computed } from "vue";
import { useI18n } from "vue-i18n";
import { useLcuStore } from "../store/lcuStore";

const store = useLcuStore();
const { t } = useI18n();

// 占位数据 — 实际数据需接入 OP.GG / CDragon
const teamComps = computed(() => [
  {
    name: t("tftPage.comps.c1"),
    core: t("tftPage.cores.c1"),
    traits: t("tftPage.traits.c1"),
    tier: "S",
  },
  {
    name: t("tftPage.comps.c2"),
    core: t("tftPage.cores.c2"),
    traits: t("tftPage.traits.c2"),
    tier: "S",
  },
  {
    name: t("tftPage.comps.c3"),
    core: t("tftPage.cores.c3"),
    traits: t("tftPage.traits.c3"),
    tier: "A",
  },
  {
    name: t("tftPage.comps.c4"),
    core: t("tftPage.cores.c4"),
    traits: t("tftPage.traits.c4"),
    tier: "A",
  },
  {
    name: t("tftPage.comps.c5"),
    core: t("tftPage.cores.c5"),
    traits: t("tftPage.traits.c5"),
    tier: "B",
  },
]);

const augments = computed(() => [
  {
    name: t("tftPage.bag"),
    desc: t("tftPage.bagDesc"),
    tier: t("tftPage.gold"),
  },
  {
    name: t("tftPage.level"),
    desc: t("tftPage.levelDesc"),
    tier: t("tftPage.silver"),
  },
  {
    name: t("tftPage.interest"),
    desc: t("tftPage.interestDesc"),
    tier: t("tftPage.gold"),
  },
]);

const selectedComp = ref<number | null>(null);
</script>

<template>
  <div class="tft">
    <h2>{{ $t("tftPage.title") }}</h2>

    <div v-if="!store.isConnected" class="tip">
      {{ $t("gameInfo.launchLolPrompt") }}
    </div>

    <div v-else>
      <!-- 阵容推荐 -->
      <section class="section">
        <h3>{{ $t("tftPage.recommendTitle") }}</h3>
        <p class="section-desc">{{ $t("tftPage.recommendDesc") }}</p>

        <div class="comp-grid">
          <div
            v-for="(comp, idx) in teamComps"
            :key="idx"
            :class="['comp-card', { selected: selectedComp === idx }]"
            @click="selectedComp = idx"
          >
            <div class="comp-tier" :class="comp.tier.toLowerCase()">
              {{ comp.tier }}
            </div>
            <div class="comp-name">{{ comp.name }}</div>
            <div class="comp-core">
              {{ $t("tftPage.core") }}: {{ comp.core }}
            </div>
            <div class="comp-traits">{{ comp.traits }}</div>
          </div>
        </div>

        <!-- 选中阵容详情 -->
        <div v-if="selectedComp !== null" class="comp-detail">
          <h4>
            {{ teamComps[selectedComp].name }} — {{ $t("tftPage.detail") }}
          </h4>
          <div class="detail-placeholder">
            <div class="detail-section">
              <span class="detail-label">{{ $t("tftPage.earlyGame") }}</span>
              <span class="detail-value">{{
                $t("tftPage.earlyGamePlaceholder")
              }}</span>
            </div>
            <div class="detail-section">
              <span class="detail-label">{{ $t("tftPage.items") }}</span>
              <span class="detail-value">{{
                $t("tftPage.itemsPlaceholder")
              }}</span>
            </div>
            <div class="detail-section">
              <span class="detail-label">{{ $t("tftPage.position") }}</span>
              <span class="detail-value">{{
                $t("tftPage.positionPlaceholder")
              }}</span>
            </div>
          </div>
        </div>
      </section>

      <!-- 海克斯推荐 -->
      <section class="section">
        <h3>{{ $t("tftPage.augments") }}</h3>
        <p class="section-desc">{{ $t("tftPage.augmentsDesc") }}</p>

        <div class="augment-list">
          <div v-for="(a, idx) in augments" :key="idx" class="augment-card">
            <span
              :class="[
                'augment-tier',
                a.tier === $t('tftPage.gold') ? 'gold' : 'silver',
              ]"
              >{{ a.tier }}</span
            >
            <div class="augment-info">
              <span class="augment-name">{{ a.name }}</span>
              <span class="augment-desc">{{ a.desc }}</span>
            </div>
          </div>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.tft {
  padding: 1.5rem 1.5rem 1.5rem 0.6rem;
  color: var(--text-color);
}
.tip {
  color: var(--text-dimmed);
  text-align: center;
  padding: 2rem;
  font-size: 0.95rem;
}

.section {
  margin-bottom: 2rem;
}
.section h3 {
  margin: 0 0 6px;
  font-size: 1.1rem;
  font-weight: 800;
  color: var(--text-color);
}
.section-desc {
  color: var(--text-dimmed);
  font-size: 0.78rem;
  margin: 0 0 12px;
}

.comp-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: 10px;
}
.comp-card {
  padding: 12px;
  border-radius: var(--radius-md);
  background: var(--card-bg);
  cursor: pointer;
  transition: all 0.25s cubic-bezier(0.25, 0.8, 0.25, 1);
  border: 1px solid var(--border-color);
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
  box-shadow: var(--shadow-sm);
}
.comp-card:hover {
  background: var(--card-bg-hover);
  border-color: var(--border-color-hover);
  box-shadow: var(--shadow-md);
}
.comp-card.selected {
  border-color: var(--primary-color);
  background: var(--primary-color-alpha-15);
  box-shadow: 0 4px 12px var(--primary-color-alpha-15);
}
.comp-tier {
  display: inline-block;
  padding: 2px 8px;
  border-radius: 4px;
  font-weight: bold;
  font-size: 0.75rem;
  color: white;
  margin-bottom: 6px;
}
.comp-tier.s {
  background: var(--loss-color);
  box-shadow: 0 4px 10px var(--loss-glow);
}
.comp-tier.a {
  background: #f59e0b;
  box-shadow: 0 4px 10px rgba(245, 158, 11, 0.2);
}
.comp-tier.b {
  background: #3b82f6;
  box-shadow: 0 4px 10px rgba(59, 130, 246, 0.2);
}
.comp-name {
  font-weight: bold;
  color: var(--text-color);
  margin-bottom: 4px;
  font-size: 0.85rem;
}
.comp-core,
.comp-traits {
  font-size: 0.78rem;
  color: var(--text-muted);
}

.comp-detail {
  margin-top: 12px;
  padding: 16px;
  background: var(--card-bg);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-color);
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
  box-shadow: var(--shadow-sm);
}
.comp-detail h4 {
  margin: 0 0 12px;
  font-size: 0.9rem;
  font-weight: 700;
  color: var(--text-color);
}
.detail-section {
  margin-bottom: 8px;
  font-size: 0.8rem;
}
.detail-label {
  font-weight: 600;
  color: var(--text-color);
  margin-right: 8px;
}
.detail-value {
  color: var(--text-muted);
}

.augment-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.augment-card {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 14px;
  border-radius: var(--radius-md);
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
  box-shadow: var(--shadow-sm);
}
.augment-tier {
  padding: 3px 8px;
  border-radius: 4px;
  font-weight: bold;
  font-size: 0.72rem;
  color: white;
}
.augment-tier.gold {
  background: #fbbf24;
  color: #000;
  box-shadow: 0 4px 10px rgba(251, 191, 36, 0.2);
}
.augment-tier.silver {
  background: #94a3b8;
  box-shadow: 0 4px 10px rgba(148, 163, 184, 0.2);
}
.augment-name {
  font-weight: bold;
  color: var(--text-color);
  font-size: 0.85rem;
}
.augment-desc {
  font-size: 0.78rem;
  color: var(--text-muted);
  display: block;
  margin-top: 2px;
}
</style>
