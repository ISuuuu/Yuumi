<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

const { t } = useI18n();

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
</script>

<template>
  <div class="augments-tab">
    <div class="augment-list">
      <div v-for="(a, idx) in augments" :key="idx" class="augment-card">
        <span
          :class="[
            'augment-tier',
            a.tier === t('tftPage.gold') ? 'gold' : 'silver',
          ]"
        >
          {{ a.tier }}
        </span>
        <div class="augment-info">
          <span class="augment-name">{{ a.name }}</span>
          <span class="augment-desc">{{ a.desc }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.augment-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.augment-card {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 12px 16px;
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
  font-size: 0.75rem;
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
  font-size: 0.9rem;
}

.augment-desc {
  font-size: 0.78rem;
  color: var(--text-muted);
  display: block;
  margin-top: 2px;
}
</style>
