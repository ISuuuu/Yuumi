<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { fetchConfig } from "../api/lcu";
import { updateThemeColor, updateCardColors } from "../utils/theme";

import { useDialog } from "naive-ui";
import { useToast } from "../composables/useToast";
import { useSettingsAutoSave } from "../composables/useSettingsAutoSave";
import LolPathSection from "../components/settings/LolPathSection.vue";
import CloudServiceSection from "../components/settings/CloudServiceSection.vue";
import ScreenshotSection from "../components/settings/ScreenshotSection.vue";
import PersonalizationSection from "../components/settings/PersonalizationSection.vue";
import UpdateSection from "../components/settings/UpdateSection.vue";
import AboutSection from "../components/settings/AboutSection.vue";

const { config, autoSave } = useSettingsAutoSave();
const dialog = useDialog();

// ─── 隐藏菜单（展开式开关组）───
const hideMenuActive = computed(
  () =>
    !!config.value?.Functions.HideTft ||
    !!config.value?.Functions.HideSavedPlayers,
);

// 当前版本号
const appVersion = ref("");

// 是否为便携版（便携版更新走 zip 覆盖方案）
const isPortable = ref(false);

const { showToast } = useToast();

onMounted(async () => {
  // 获取当前应用版本号
  try {
    appVersion.value = await getVersion();
  } catch (e) {
    console.warn("获取版本号失败:", e);
  }

  // 识别便携版（便携版不支持自动更新，界面隐藏更新入口）
  try {
    isPortable.value = await invoke<boolean>("is_portable");
  } catch (e) {
    console.warn("识别便携版失败:", e);
  }

  if (!config.value) {
    try {
      config.value = await fetchConfig();
    } catch (e) {
      console.error("加载系统配置失败:", e);
    }
  }
  if (config.value && config.value.Personalization) {
    if (config.value.Personalization.ThemeColor) {
      updateThemeColor(config.value.Personalization.ThemeColor);
    }
    updateCardColors(
      config.value.Personalization.WinCardColor,
      config.value.Personalization.LoseCardColor,
      config.value.Personalization.RemakeCardColor,
    );
  }
});

// 清除缓存
function handleClearCache() {
  dialog.warning({
    title: "清除缓存",
    content: "确定要清除所有游戏资源缓存吗？清除后需要重新加载游戏资源。",
    positiveText: "确定",
    negativeText: "取消",
    positiveButtonProps: { type: "primary" },
    onPositiveClick: async () => {
      try {
        const result = await invoke<string>("clear_game_cache");
        showToast(result);
      } catch (e: unknown) {
        showToast("清除缓存失败", "error");
      }
    },
  });
}

// 打开日志文件夹
async function handleOpenLogFolder() {
  try {
    await invoke("open_log_folder");
  } catch (e: unknown) {
    showToast("打开日志文件夹失败", "error");
  }
}
</script>

<template>
  <div class="settings-view">
    <div v-if="!config" class="tip-container">
      <div class="loading-spinner"></div>
      <p class="tip">{{ $t("settings.loadingData") }}</p>
    </div>

    <div v-else class="settings-container">
      <h1 class="page-title">{{ $t("settings.title") }}</h1>

      <div class="group-header">{{ $t("settings.groupFeatures") }}</div>

      <!-- LCU API 并发数 -->
      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">{{ $t("settings.lcuConcurrencyTitle") }}</h3>
          <span class="card-desc">{{ $t("settings.lcuConcurrencyDesc") }}</span>
        </div>
        <div class="card-right">
          <n-input-number
            v-model:value="config.Functions.ApiConcurrencyNumber"
            :min="1"
            :max="10"
            @update:value="autoSave"
            style="width: 140px"
            size="small"
          />
        </div>
      </div>

      <!-- 默认对局数量 -->
      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">{{ $t("settings.defaultGamesTitle") }}</h3>
          <span class="card-desc">{{ $t("settings.defaultGamesDesc") }}</span>
        </div>
        <div class="card-right">
          <n-input-number
            v-model:value="config.Functions.CareerGamesNumber"
            :min="1"
            :max="100"
            :step="5"
            @update:value="autoSave"
            style="width: 140px"
            size="small"
          />
        </div>
      </div>

      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">{{ $t("settings.gameInfoFilterTitle") }}</h3>
          <span class="card-desc">{{ $t("settings.gameInfoFilterDesc") }}</span>
        </div>
        <div class="card-right">
          <n-switch
            v-model:value="config.Functions.GameInfoFilter"
            @update:value="autoSave"
          />
        </div>
      </div>

      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">{{ $t("settings.reserveGameInfoTitle") }}</h3>
          <span class="card-desc">{{
            $t("settings.reserveGameInfoDesc")
          }}</span>
        </div>
        <div class="card-right">
          <n-switch
            v-model:value="config.Functions.EnableReserveGameinfo"
            @update:value="autoSave"
          />
        </div>
      </div>

      <div class="card-item">
        <div class="card-left">
          <h3 class="card-title">
            {{ $t("settings.showTierInGameInfoTitle") }}
          </h3>
          <span class="card-desc">{{
            $t("settings.showTierInGameInfoDesc")
          }}</span>
        </div>
        <div class="card-right">
          <n-switch
            v-model:value="config.Functions.ShowTierInGameInfo"
            @update:value="autoSave"
          />
        </div>
      </div>

      <!-- 2. OP.GG -->
      <div class="group-header">{{ $t("settings.opggGroup") }}</div>

      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">{{ $t("settings.autoShowOpggTitle") }}</h3>
          <span class="card-desc">{{ $t("settings.autoShowOpggDesc") }}</span>
        </div>
        <div class="card-right">
          <n-switch
            v-model:value="config.Functions.AutoShowOpgg"
            @update:value="autoSave"
          />
        </div>
      </div>

      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">{{ $t("settings.pinOpggTitle") }}</h3>
          <span class="card-desc">{{ $t("settings.pinOpggDesc") }}</span>
        </div>
        <div class="card-right">
          <n-switch
            v-model:value="config.Functions.EnableOpggOnTop"
            @update:value="autoSave"
          />
        </div>
      </div>

      <!-- 3. 通用 -->
      <div class="group-header">{{ $t("settings.generalGroup") }}</div>

      <LolPathSection />

      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">{{ $t("settings.autoStartLolTitle") }}</h3>
          <span class="card-desc">{{ $t("settings.autoStartLolDesc") }}</span>
        </div>
        <div class="card-right">
          <n-switch
            v-model:value="config.General.EnableStartLolWithApp"
            @update:value="autoSave"
          />
        </div>
      </div>

      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">{{ $t("settings.clearCacheTitle") }}</h3>
          <span class="card-desc">{{ $t("settings.clearCacheDesc") }}</span>
        </div>
        <div class="card-right">
          <n-button size="small" @click="handleClearCache">{{
            $t("settings.deleteBtn")
          }}</n-button>
        </div>
      </div>

      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">{{ $t("settings.closeToTrayTitle") }}</h3>
          <span class="card-desc">{{ $t("settings.closeToTrayDesc") }}</span>
        </div>
        <div class="card-right">
          <n-switch
            :value="!!config?.General?.EnableCloseToTray"
            @update:value="
              (val) => {
                if (config) {
                  config.General.EnableCloseToTray = val;
                  autoSave();
                }
              }
            "
          />
        </div>
      </div>

      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">{{ $t("settings.startMinimizedTitle") }}</h3>
          <span class="card-desc">{{ $t("settings.startMinimizedDesc") }}</span>
        </div>
        <div class="card-right">
          <n-switch
            v-model:value="config.General.EnableGameStartMinimize"
            @update:value="autoSave"
          />
        </div>
      </div>

      <CloudServiceSection />

      <!-- 隐藏菜单 -->
      <n-collapse arrow-placement="right" class="collapse-card">
        <n-collapse-item name="hidemenu">
          <template #header>
            <div class="collapse-header-wrapper">
              <div class="collapse-left-simple">
                <span class="card-title">{{ $t("settings.hideMenuTitle") }}</span>
                <span class="card-desc">{{ $t("settings.hideMenuDesc") }}</span>
              </div>
              <div class="collapse-right-status">
                <span class="status-preview">{{
                  hideMenuActive
                    ? $t("settings.enabled")
                    : $t("settings.disabled")
                }}</span>
              </div>
            </div>
          </template>
          <div class="setting-row">
            <span class="setting-label">{{
              $t("settings.hideMenuOptionTft")
            }}</span>
            <n-switch
              v-model:value="config.Functions.HideTft"
              @update:value="autoSave"
            />
          </div>
          <div class="setting-row">
            <span class="setting-label">{{
              $t("settings.hideMenuOptionSavedPlayers")
            }}</span>
            <n-switch
              v-model:value="config.Functions.HideSavedPlayers"
              @update:value="autoSave"
            />
          </div>
        </n-collapse-item>
      </n-collapse>

      <ScreenshotSection />

      <!-- 4. 日志 -->
      <div class="group-header">{{ $t("settings.logGroup") }}</div>

      <!-- 日志等级 -->
      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">{{ $t("settings.logLevelTitle") }}</h3>
          <span class="card-desc">{{ $t("settings.logLevelDesc") }}</span>
        </div>
        <div class="card-right">
          <n-select
            v-model:value="config.General.LogLevel"
            :options="[
              { label: 'Debug', value: 0 },
              { label: 'Info', value: 1 },
              { label: 'Error', value: 2 },
            ]"
            @update:value="autoSave"
            style="width: 120px"
            size="small"
          />
        </div>
      </div>

      <div class="card-item">
        <div class="card-left">
          <h3 class="card-title">{{ $t("settings.logFileTitle") }}</h3>
          <span class="card-desc">{{ $t("settings.logFileDesc") }}</span>
        </div>
        <div class="card-right">
          <n-button size="small" @click="handleOpenLogFolder">{{
            $t("settings.openFolderBtn")
          }}</n-button>
        </div>
      </div>

      <PersonalizationSection />

      <UpdateSection :app-version="appVersion" :is-portable="isPortable" />

      <n-collapse arrow-placement="right" class="collapse-card">
        <n-collapse-item name="httpproxy">
          <template #header>
            <div class="collapse-header-wrapper">
              <div class="collapse-left-simple">
                <span class="card-title">{{
                  $t("settings.httpProxyGroup")
                }}</span>
                <span class="card-desc">{{
                  $t("settings.httpProxyDesc")
                }}</span>
              </div>
              <div class="collapse-right-status">
                <span class="status-preview">{{
                  config.General.EnableHttpProxy
                    ? $t("settings.enabled")
                    : $t("settings.disabled")
                }}</span>
              </div>
            </div>
          </template>
          <div class="setting-row">
            <n-input
              v-model:value="config.General.HttpProxyAddr"
              placeholder="127.0.0.1:7897"
              :disabled="!config.General.EnableHttpProxy"
              clearable
              @change="autoSave"
              style="max-width: 300px"
            />
            <n-switch
              v-model:value="config.General.EnableHttpProxy"
              @update:value="autoSave"
            />
          </div>
        </n-collapse-item>
      </n-collapse>

      <AboutSection :app-version="appVersion" />
    </div>
  </div>
</template>

<style scoped src="../components/settings/shared.css"></style>

<style scoped>
.settings-view {
  padding: 1.5rem 1.5rem 1.5rem 0.6rem;
  background-color: transparent;
  min-height: 100%;
  color: var(--text-color);
}

.tip-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 8rem 2rem;
  color: var(--text-muted);
}
.tip {
  font-size: 0.95rem;
  color: var(--text-dimmed);
  margin-top: 12px;
}

.loading-spinner {
  width: 36px;
  height: 36px;
  border: 3px solid var(--hover-bg);
  border-top-color: var(--primary-color);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}
@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.feedback-btn {
  background: var(--win-color);
  border: 1px solid rgba(16, 185, 129, 0.2);
  color: white;
  padding: 6px 16px;
  border-radius: 6px;
  font-size: 0.82rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s cubic-bezier(0.25, 0.8, 0.25, 1);
  box-shadow: 0 2px 8px rgba(16, 185, 129, 0.2);
}
.feedback-btn:hover {
  background: rgba(16, 185, 129, 0.85);
  transform: translateY(-0.5px);
}
.feedback-btn:active {
  color: var(--text-muted);
  transform: translateY(0.5px);
}

.settings-container {
  max-width: 800px;
  margin: 0 auto;
  animation: fadeIn 0.3s ease-out;
}

.page-title {
  font-size: 1.4rem;
  font-weight: 800;
  color: var(--text-color);
  margin: 0 0 1.5rem;
  letter-spacing: 0.5px;
}

.github-icon {
  width: 16px;
  height: 16px;
}

.collapse-left {
  display: flex;
  flex-direction: column;
  flex: 1;
}
.collapse-right {
  margin-left: auto;
  color: var(--text-dimmed);
  display: flex;
  align-items: center;
}
.arrow-icon {
  width: 18px;
  height: 18px;
  transition: transform 0.2s;
}
.arrow-icon.expanded {
  transform: rotate(180deg);
}

.input-row {
  display: flex;
  gap: 8px;
  width: 100%;
  justify-content: flex-end;
}
.input-row.align-center {
  align-items: center;
}

/* 分段控制组件（扁平化按钮组） */
.segmented-control {
  display: inline-flex;
  background: var(--segmented-bg);
  padding: 3px;
  border-radius: 8px;
  border: 1px solid var(--border-color);
}
.segmented-item {
  border: none;
  background: transparent;
  padding: 6px 14px;
  font-size: 0.78rem;
  font-weight: 600;
  color: var(--text-muted);
  cursor: pointer;
  border-radius: 6px;
  transition: all 0.2s ease;
  outline: none;
}
.segmented-item:hover {
  color: var(--text-color);
  background: var(--hover-bg);
}
.segmented-item.active {
  background: var(--card-bg-hover);
  color: var(--primary-color);
  box-shadow:
    var(--shadow-sm),
    0 0 8px rgba(0, 159, 170, 0.2);
}

.dropdown-trigger {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  padding: 8px 12px;
  border-radius: 6px;
  font-size: 0.82rem;
  color: var(--text-color);
  cursor: pointer;
  position: relative;
  transition: all 0.2s;
  min-width: 140px;
}
.dropdown-trigger:hover {
  background: var(--card-bg-hover);
  border-color: var(--primary-color);
}
.dropdown-trigger .arrow-icon {
  width: 12px;
  height: 12px;
  margin-left: auto;
  transition: transform 0.2s;
}
.dropdown-trigger .arrow-icon.expanded {
  transform: rotate(180deg);
}
.dropdown-menu {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  box-shadow: var(--shadow-lg);
  z-index: 100;
  min-width: 100%;
  padding: 4px 0;
}
.dropdown-item {
  padding: 6px 14px;
  font-size: 0.78rem;
  color: var(--text-muted);
  cursor: pointer;
  transition: all 0.2s;
}
.dropdown-item:hover {
  background: var(--hover-bg);
  color: var(--text-color);
}
.dropdown-item.active {
  color: var(--primary-color);
  font-weight: 600;
  background: var(--primary-color-alpha-15);
}

.color-picker-label {
  font-size: 0.82rem;
  color: var(--text-muted);
}
.color-picker {
  border: 1px solid var(--border-color);
  background: var(--card-bg);
  padding: 2px;
  width: 44px;
  height: 28px;
  cursor: pointer;
  border-radius: 4px;
}
.color-pickers-row {
  display: flex;
  gap: 16px;
  flex-wrap: wrap;
  justify-content: flex-end;
}
.color-picker-item {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 0.82rem;
  color: var(--text-muted);
}

@keyframes fadeIn {
  from {
    opacity: 0;
    transform: translateY(6px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>
