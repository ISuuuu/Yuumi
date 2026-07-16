<script setup lang="ts">
import { inject, provide, type Ref } from "vue";
import type { AppConfig } from "../api/lcu";
import { useLcuStore } from "../store/lcuStore";

// 子组件导入
import AutoAcceptCard from "../components/tools/AutoAcceptCard.vue";
import AutoSwapCard from "../components/tools/AutoSwapCard.vue";
import AutoHoverCard from "../components/tools/AutoHoverCard.vue";
import SpectateCard from "../components/tools/SpectateCard.vue";
import LcuQuickActionsCard from "../components/tools/LcuQuickActionsCard.vue";

const config = inject<Ref<AppConfig | null>>("appConfig");
const store = useLcuStore();

// 提供更新配置的方法给子组件
function updateConfig(newConfig: AppConfig) {
  if (config) {
    config.value = newConfig;
  }
}
provide("updateConfig", updateConfig);
</script>

<template>
  <div class="tools-view">
    <div v-if="!config" class="tip-container">
      <div class="loading-spinner"></div>
      <p class="tip">{{ $t("tools.loading") }}</p>
    </div>

    <div v-else :class="['tools-container', { 'offline-disabled': !store.isConnected }]">
      <h1 class="page-title">{{ $t("tools.title") }}</h1>

      <!-- 1. 英雄选择组 -->
      <div class="group-header">{{ $t("tools.groupChampSelect") }}</div>
      <AutoAcceptCard />
      <AutoSwapCard />
      <AutoHoverCard />

      <!-- 2. 游戏组 -->
      <div class="group-header">{{ $t("tools.groupGame") }}</div>
      <SpectateCard />
      <LcuQuickActionsCard />
    </div>
  </div>
</template>

<style scoped>
.tools-view {
  padding: 1rem 1.5rem 1rem 0.6rem;
  background-color: transparent;
}

.tools-container {
  max-width: 1000px;
  width: 100%;
  margin: 0 auto;
  animation: fadeIn 0.3s ease-out;
  padding-bottom: 2rem;
}

.tip-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 6rem 2rem;
  color: var(--text-muted);
  flex: 1;
}

.tip {
  font-size: 0.95rem;
  color: var(--text-dimmed);
  margin: 0;
}

.page-title {
  font-size: 1.4rem;
  font-weight: 800;
  color: var(--text-color);
  margin: 0 0 1.2rem 0;
}

.group-header {
  font-size: 0.78rem;
  font-weight: 700;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.5px;
  margin: 1.2rem 0 0.6rem 0;
  padding-left: 4px;
}

/* 离线禁用状态 - 禁用所有交互元素但保持滚动 */
.offline-disabled :deep(.n-collapse),
.offline-disabled :deep(.card-item) {
  opacity: 0.5;
  pointer-events: none;
}

.offline-disabled :deep(.n-collapse-item__header) {
  pointer-events: none !important;
  cursor: not-allowed !important;
}

.offline-disabled :deep(.n-collapse-item__content) {
  pointer-events: none;
}

.offline-disabled :deep(.n-switch),
.offline-disabled :deep(.n-input),
.offline-disabled :deep(.n-input-number),
.offline-disabled :deep(.n-select),
.offline-disabled :deep(.n-button),
.offline-disabled :deep(button),
.offline-disabled :deep(input),
.offline-disabled :deep(select) {
  pointer-events: none;
  opacity: 0.5;
  cursor: not-allowed;
}

.loading-spinner {
  width: 32px;
  height: 32px;
  border: 3px solid var(--border-color);
  border-top-color: var(--primary-color);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
  margin-bottom: 12px;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@keyframes fadeIn {
  from { opacity: 0; transform: translateY(8px); }
  to { opacity: 1; transform: translateY(0); }
}
</style>
