<script setup lang="ts">
import { useAutoSaveConfig } from "../../composables/useAutoSaveConfig";
import { useI18n } from "vue-i18n";
import { NSwitch, NCollapse, NCollapseItem } from "naive-ui";

const { config, triggerAutoSave } = useAutoSaveConfig();
const { t } = useI18n();

function updateInvite(value: boolean) {
  if (!config?.value) return;
  config.value.Functions.EnableAutoHandleInvite = value;
  triggerAutoSave();
}

function updateHonor(value: boolean) {
  if (!config?.value) return;
  config.value.Functions.EnableAutoHonor = value;
  triggerAutoSave();
}

function updatePlayAgain(value: boolean) {
  if (!config?.value) return;
  config.value.Functions.EnableAutoPlayAgain = value;
  triggerAutoSave();
}

function updateAramSide(value: boolean) {
  if (!config?.value) return;
  config.value.Functions.EnableAutoAramTeamSide = value;
  triggerAutoSave();
}

function updateAramSideVisible(value: boolean) {
  if (!config?.value) return;
  config.value.Functions.AramTeamSideVisibleToTeam = value;
  triggerAutoSave();
}
</script>

<template>
  <n-collapse v-if="config" arrow-placement="right" class="collapse-card">
    <n-collapse-item name="autogameflow">
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
              <h3 class="card-title">{{ t("tools.autoGameflow.title") }}</h3>
              <span class="card-desc">{{ t("tools.autoGameflow.desc") }}</span>
            </div>
          </div>
          <div class="collapse-right-status">
            <span class="status-preview">
              {{
                config.Functions.EnableAutoHandleInvite ||
                config.Functions.EnableAutoHonor ||
                config.Functions.EnableAutoPlayAgain ||
                config.Functions.EnableAutoAramTeamSide
                  ? t("tools.autoGameflow.statusEnabled")
                  : t("tools.autoGameflow.statusDisabled")
              }}
            </span>
          </div>
        </div>
      </template>
      <div class="setting-row">
        <span class="setting-label">{{ t("tools.autoGameflow.inviteLabel") }}</span>
        <n-switch
          :value="config.Functions.EnableAutoHandleInvite"
          @update:value="updateInvite"
        />
      </div>
      <div class="setting-row">
        <span class="setting-label">{{ t("tools.autoGameflow.honorLabel") }}</span>
        <n-switch
          :value="config.Functions.EnableAutoHonor"
          @update:value="updateHonor"
        />
      </div>
      <div class="setting-row">
        <span class="setting-label">{{ t("tools.autoGameflow.playAgainLabel") }}</span>
        <n-switch
          :value="config.Functions.EnableAutoPlayAgain"
          @update:value="updatePlayAgain"
        />
      </div>
      <div class="setting-row">
        <span class="setting-label">{{ t("tools.autoGameflow.aramSideLabel") }}</span>
        <n-switch
          :value="config.Functions.EnableAutoAramTeamSide"
          @update:value="updateAramSide"
        />
      </div>
      <div class="setting-row">
        <span class="setting-label">{{ t("tools.autoGameflow.aramSideVisibleLabel") }}</span>
        <n-switch
          :value="config.Functions.AramTeamSideVisibleToTeam"
          :disabled="!config.Functions.EnableAutoAramTeamSide"
          @update:value="updateAramSideVisible"
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
