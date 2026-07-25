<script setup lang="ts">
import { ref, computed } from "vue";
import { useI18n } from "vue-i18n";

const { t } = useI18n();

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

const selectedComp = ref<number | null>(0);
</script>

<template>
  <div class="meta-comps-tab">
    <div class="comp-grid">
      <div
        v-for="(comp, idx) in teamComps"
        :key="idx"
        :class="['comp-card', { selected: selectedComp === idx }]"
        @click="selectedComp = idx"
      >
        <div class="comp-tier" :class="comp.tier.toLowerCase()">
          Tier {{ comp.tier }}
        </div>
        <div class="comp-name">{{ comp.name }}</div>
        <div class="comp-core">
          {{ t("tftPage.core") }}: {{ comp.core }}
        </div>
        <div class="comp-traits">{{ comp.traits }}</div>
      </div>
    </div>

    <!-- 选中阵容详情说明 -->
    <div v-if="selectedComp !== null" class="comp-detail">
      <h4>{{ teamComps[selectedComp].name }} — {{ t("tftPage.detail") }}</h4>
      <div class="detail-placeholder">
        <div class="detail-section">
          <span class="detail-label">{{ t("tftPage.earlyGame") }}</span>
          <span class="detail-value">{{ t("tftPage.earlyGamePlaceholder") }}</span>
        </div>
        <div class="detail-section">
          <span class="detail-label">{{ t("tftPage.items") }}</span>
          <span class="detail-value">{{ t("tftPage.itemsPlaceholder") }}</span>
        </div>
        <div class="detail-section">
          <span class="detail-label">{{ t("tftPage.position") }}</span>
          <span class="detail-value">{{ t("tftPage.positionPlaceholder") }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.comp-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 12px;
}

.comp-card {
  padding: 14px;
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
}
.comp-tier.b {
  background: #3b82f6;
}

.comp-name {
  font-weight: bold;
  color: var(--text-color);
  margin-bottom: 4px;
  font-size: 0.95rem;
}

.comp-core,
.comp-traits {
  font-size: 0.78rem;
  color: var(--text-muted);
  margin-top: 2px;
}

.comp-detail {
  margin-top: 16px;
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
  font-size: 0.95rem;
  font-weight: 700;
  color: var(--text-color);
}

.detail-section {
  margin-bottom: 8px;
  font-size: 0.82rem;
}

.detail-label {
  font-weight: 600;
  color: var(--text-color);
  margin-right: 8px;
}

.detail-value {
  color: var(--text-muted);
}
</style>
