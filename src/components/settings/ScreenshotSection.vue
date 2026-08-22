<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { updateConfig } from "../../api/lcu";
import { useSettingsAutoSave } from "../../composables/useSettingsAutoSave";
import { useToast } from "../../composables/useToast";
import { useI18n } from "vue-i18n";

const { config, autoSave } = useSettingsAutoSave();
const { showToast } = useToast();
const { t } = useI18n();

// 选择自动截图保存目录
async function handleSelectScreenshotFolder() {
  try {
    if (!config.value) return;
    const path = await invoke<string | null>("select_folder", {
      title: "选择自动截图保存目录",
      defaultPath: config.value.Functions.ScreenshotSavePath
    });
    if (path) {
      config.value.Functions.ScreenshotSavePath = path;
      await updateConfig(config.value);
      showToast("已成功更新截图保存目录");
    }
  } catch (e: unknown) {
    showToast("选择文件夹失败: " + String(e), "error");
  }
}

// 在系统资源管理器中打开截图目录
async function handleOpenScreenshotFolder() {
  try {
    await invoke("open_screenshot_folder");
  } catch (e: unknown) {
    showToast("无法打开截图文件夹: " + String(e), "error");
  }
}
</script>

<template>
  <!-- 自动截图设置 -->
  <template v-if="config">
    <div class="group-header">{{ $t("settings.screenshotGroup") }}</div>

  <n-collapse arrow-placement="right" class="collapse-card">
    <n-collapse-item name="screenshot">
      <template #header>
        <div class="collapse-header-wrapper">
          <div class="collapse-left-simple">
            <span class="card-title">{{ $t("settings.enableScreenshotOnMultikillTitle") }}</span>
            <span class="card-desc">{{ $t("settings.enableScreenshotOnMultikillDesc") }}</span>
          </div>
          <div class="collapse-right-status">
            <span class="status-preview">{{
              config.Functions.EnableScreenshotOnMultikill
                ? $t("settings.enabled")
                : $t("settings.disabled")
            }}</span>
          </div>
        </div>
      </template>

      <div class="setting-row">
        <span class="setting-label">{{ $t("settings.enableScreenshotOnMultikillTitle") }}</span>
        <n-switch
          v-model:value="config.Functions.EnableScreenshotOnMultikill"
          @update:value="autoSave"
        />
      </div>

      <template v-if="config.Functions.EnableScreenshotOnMultikill">
        <!-- 截图触发条件选项 -->
        <div class="setting-row">
          <span class="setting-label">{{ $t("settings.screenshotLevelsTitle") }}</span>
          <n-checkbox-group
            v-model:value="config.Functions.ScreenshotOnMultikillLevels"
            @update:value="autoSave"
          >
            <n-space>
              <n-checkbox :value="3">{{ $t("settings.tripleKill") }}</n-checkbox>
              <n-checkbox :value="4">{{ $t("settings.quadraKill") }}</n-checkbox>
              <n-checkbox :value="5">{{ $t("settings.pentaKill") }}</n-checkbox>
              <n-checkbox :value="8">{{ $t("settings.legendary") }}</n-checkbox>
            </n-space>
          </n-checkbox-group>
        </div>

        <!-- 目录配置项 -->
        <div class="setting-row" style="flex-wrap: wrap; gap: 12px;">
          <span class="setting-label" style="width: 100%;">{{ $t("settings.screenshotSavePathTitle") }}</span>
          <div style="display: flex; gap: 8px; align-items: center; width: 100%;">
            <n-input
              v-model:value="config.Functions.ScreenshotSavePath"
              readonly
              :placeholder="t('settings.screenshotSavePathPlaceholder')"
              size="small"
              style="flex: 1;"
            />
            <n-button size="small" type="primary" secondary @click="handleSelectScreenshotFolder">
              {{ $t("settings.browseBtn") }}
            </n-button>
            <n-button size="small" @click="handleOpenScreenshotFolder">
              {{ $t("settings.openScreenshotFolderBtn") }}
            </n-button>
          </div>
        </div>
      </template>
    </n-collapse-item>
  </n-collapse>
  </template>
</template>

<style scoped src="./shared.css"></style>
