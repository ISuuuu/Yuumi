<script setup lang="ts">
import { useAutoSaveConfig } from "../../../composables/useAutoSaveConfig";
import { useI18n } from "vue-i18n";
import { NSwitch } from "naive-ui";

const { config, triggerAutoSave } = useAutoSaveConfig();
const { t } = useI18n();
</script>

<template>
  <!-- 大乱斗板凳席悬浮窗 -->
  <div v-if="config" class="card-item border-bottom">
    <div class="card-left">
      <div class="icon-container">
        <svg
          class="header-icon"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <rect x="2" y="3" width="20" height="14" rx="2" ry="2"></rect>
          <line x1="8" y1="21" x2="16" y2="21"></line>
          <line x1="12" y1="17" x2="12" y2="21"></line>
        </svg>
      </div>
      <div class="title-container">
        <h3 class="card-title">{{ t("tools.benchOverlay.title") }}</h3>
        <span class="card-desc">{{ t("tools.benchOverlay.desc") }}</span>
      </div>
    </div>
    <div class="card-right">
      <n-switch
        :value="config.Functions.EnableBenchOverlay"
        @update:value="
          (v: boolean) => {
            if (config) config.Functions.EnableBenchOverlay = v;
            triggerAutoSave();
          }
        "
      />
    </div>
  </div>
</template>

<style scoped>
.card-item {
  background: var(--settings-card-bg);
  border: 1px solid var(--settings-card-border);
  border-radius: 12px;
  margin-bottom: 8px;
  box-shadow: var(--shadow-sm);
  transition:
    box-shadow 0.25s cubic-bezier(0.25, 0.8, 0.25, 1),
    border-color 0.25s,
    background-color 0.25s,
    transform 0.2s;
  position: relative;
  padding: 16px 24px;
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.card-item:hover {
  border-color: var(--settings-card-border-hover);
  background-color: var(--settings-card-bg-hover);
  box-shadow: var(--card-glow-hover);
  transform: translateY(-1px);
}

.card-item.border-bottom {
  border-radius: 12px 12px 0 0;
  border-bottom: 1px solid var(--settings-separator);
  margin-bottom: 0;
}

.card-left {
  display: flex;
  align-items: center;
  flex: 1;
  gap: 14px;
}

.icon-container {
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-muted);
  flex-shrink: 0;
}

.header-icon {
  width: 18px;
  height: 18px;
  stroke-width: 2px;
}

.title-container {
  display: flex;
  flex-direction: column;
}

.card-title {
  font-size: 0.88rem;
  font-weight: bold;
  color: var(--text-color);
  margin: 0;
}

.card-desc {
  font-size: 0.78rem;
  color: var(--text-muted);
  margin-top: 4px;
  line-height: 1.4;
}

.card-right {
  margin-left: auto;
  display: flex;
  align-items: center;
}
</style>
