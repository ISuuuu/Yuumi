<script setup lang="ts">
import { inject, type Ref } from "vue";
import { updateConfig } from "../../api/lcu";
import type { AppConfig } from "../../api/lcu";
import { useI18n } from "vue-i18n";
import { NSwitch, NInputNumber, NCollapse, NCollapseItem } from "naive-ui";

const config = inject<Ref<AppConfig | null>>("appConfig");
const updateConfigFn = inject<(config: AppConfig) => void>("updateConfig");
const { t } = useI18n();

async function triggerAutoSave() {
  if (!config?.value || !updateConfigFn) return;
  const newConfig = { ...config.value };
  newConfig.Functions = { ...newConfig.Functions };
  // 同步自动选用开关和自动亮起开关
  newConfig.Functions.EnableAutoSelectChampion =
    newConfig.Functions.EnableAutoHoverChampion;
  updateConfigFn(newConfig);
  try {
    await updateConfig(newConfig);
  } catch (e) {
    console.error("自动保存设置失败:", e);
  }
}

function updateDelay(value: number | null) {
  if (!config?.value) return;
  config.value.Functions.AutoAcceptMatchingDelay = value ?? 0;
  triggerAutoSave();
}

function updateEnabled(value: boolean) {
  if (!config?.value) return;
  config.value.Functions.EnableAutoAcceptMatching = value;
  triggerAutoSave();
}
</script>

<template>
  <n-collapse v-if="config" arrow-placement="right" class="collapse-card">
    <n-collapse-item name="autoaccept">
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
                <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path>
                <polyline points="22 4 12 14.01 9 11.01"></polyline>
              </svg>
            </div>
            <div class="title-container">
              <h3 class="card-title">{{ t("tools.autoAccept.title") }}</h3>
              <span class="card-desc">{{ t("tools.autoAccept.desc") }}</span>
            </div>
          </div>
          <div class="collapse-right-status">
            <span class="status-preview">
              {{
                config.Functions.EnableAutoAcceptMatching
                  ? t("tools.autoAccept.statusEnabled", {
                      delay: config.Functions.AutoAcceptMatchingDelay,
                    })
                  : t("tools.autoAccept.statusDisabled")
              }}
            </span>
          </div>
        </div>
      </template>
      <div class="setting-row">
        <span class="setting-label">{{ t("tools.autoAccept.delayLabel") }}</span>
        <n-input-number
          :value="config.Functions.AutoAcceptMatchingDelay"
          :min="0"
          :max="11"
          @update:value="updateDelay"
          style="width: 120px"
          size="small"
        />
      </div>
      <div class="setting-row justify-end">
        <n-switch
          :value="config.Functions.EnableAutoAcceptMatching"
          @update:value="updateEnabled"
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

.setting-row.justify-end {
  justify-content: flex-end;
}

.setting-label {
  font-size: 0.82rem;
  color: var(--text-muted);
}
</style>
