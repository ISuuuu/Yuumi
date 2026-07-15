<script setup lang="ts">
import { inject, type Ref } from "vue";
import { updateConfig } from "../../api/lcu";
import type { AppConfig } from "../../api/lcu";
import { useI18n } from "vue-i18n";
import { NSwitch, NCollapse, NCollapseItem } from "naive-ui";

const config = inject<Ref<AppConfig | null>>("appConfig");
const updateConfigFn = inject<(config: AppConfig) => void>("updateConfig");
const { t } = useI18n();

async function triggerAutoSave() {
  if (!config?.value || !updateConfigFn) return;
  const newConfig = { ...config.value };
  newConfig.Functions = { ...newConfig.Functions };
  newConfig.Functions.EnableAutoSelectChampion =
    newConfig.Functions.EnableAutoHoverChampion;
  updateConfigFn(newConfig);
  try {
    await updateConfig(newConfig);
  } catch (e) {
    console.error("自动保存设置失败:", e);
  }
}

function updateCeilSwap(value: boolean) {
  if (!config?.value) return;
  config.value.Functions.AutoAcceptCeilSwap = value;
  triggerAutoSave();
}

function updateChampTrade(value: boolean) {
  if (!config?.value) return;
  config.value.Functions.AutoAcceptChampTrade = value;
  triggerAutoSave();
}
</script>

<template>
  <n-collapse v-if="config" arrow-placement="right" class="collapse-card">
    <n-collapse-item name="autoswap">
      <template #header>
        <div class="collapse-header-wrapper">
          <div class="collapse-left">
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
                <line x1="8" y1="6" x2="21" y2="6"></line>
                <line x1="8" y1="12" x2="21" y2="12"></line>
                <line x1="8" y1="18" x2="21" y2="18"></line>
                <line x1="3" y1="6" x2="3.01" y2="6"></line>
                <line x1="3" y1="12" x2="3.01" y2="12"></line>
                <line x1="3" y1="18" x2="3.01" y2="18"></line>
              </svg>
            </div>
            <div class="title-container">
              <h3 class="card-title">{{ t("tools.autoSwap.title") }}</h3>
              <span class="card-desc">{{ t("tools.autoSwap.desc") }}</span>
            </div>
          </div>
          <div class="collapse-right-status">
            <span class="status-preview">
              {{
                config.Functions.AutoAcceptCeilSwap ||
                config.Functions.AutoAcceptChampTrade
                  ? t("tools.autoSwap.statusEnabled")
                  : t("tools.autoSwap.statusDisabled")
              }}
            </span>
          </div>
        </div>
      </template>
      <div class="setting-row">
        <span class="setting-label">{{ t("tools.autoSwap.floorLabel") }}</span>
        <n-switch
          :value="config.Functions.AutoAcceptCeilSwap"
          @update:value="updateCeilSwap"
        />
      </div>
      <div class="setting-row">
        <span class="setting-label">{{ t("tools.autoSwap.champLabel") }}</span>
        <n-switch
          :value="config.Functions.AutoAcceptChampTrade"
          @update:value="updateChampTrade"
        />
      </div>
    </n-collapse-item>
  </n-collapse>
</template>

<style scoped>
.collapse-header-wrapper {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
}

.collapse-left {
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

.collapse-right-status {
  margin-left: auto;
  display: flex;
  align-items: center;
}

.status-preview {
  font-size: 0.78rem;
  color: var(--text-dimmed);
  margin-right: 10px;
}

.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 0;
  border-bottom: 1px dashed var(--border-color);
}

.setting-row:last-child {
  border-bottom: none;
}

.setting-label {
  font-size: 0.82rem;
  color: var(--text-muted);
}
</style>
