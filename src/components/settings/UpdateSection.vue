<script setup lang="ts">
import { ref, inject, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useI18n } from "vue-i18n";
import type { UpdateInfo } from "../UpdateDialog.vue";
import { useSettingsAutoSave } from "../../composables/useSettingsAutoSave";
import { useToast } from "../../composables/useToast";

const props = defineProps<{
  appVersion: string;
  isPortable: boolean;
}>();

const { t } = useI18n();

const { config, autoSave } = useSettingsAutoSave();
const { showToast } = useToast();
const hasUpdate = inject<Ref<boolean>>("hasUpdate", ref(false));
const globalUpdateInfo = inject<Ref<UpdateInfo | null>>("updateInfo", ref(null));

// App.vue 提供的更新弹窗推送函数（便携版手动检查 / 立即更新时用，expanded=true 直达带日志的大弹窗）
const showUpdateInfo = inject<(info: UpdateInfo, expanded?: boolean) => void>(
  "showUpdateInfo",
  () => {},
);

// 手动检查更新状态
const checkingUpdate = ref(false);

interface PortableUpdateInfo {
  version: string;
  date?: string;
  body?: string;
}

async function manualCheckUpdate() {
  if (checkingUpdate.value) return;
  checkingUpdate.value = true;
  try {
    if (props.isPortable) {
      const result = await invoke<PortableUpdateInfo | null>(
        "check_portable_update",
      );
      if (result) {
        // 便携版：直接展开带日志的大弹窗
        showUpdateInfo(
          {
            version: result.version,
            currentVersion: props.appVersion || "",
            notes: result.body,
            pubDate: result.date,
          },
          true,
        );
        showToast(t("settings.newVersionToast", { version: result.version }), "success");
      } else {
        showToast(t("settings.alreadyLatest"), "success");
      }
    } else {
      const result = await invoke<UpdateInfo | null>("check_update");
      if (result) {
        // 手动检查：直接展开带日志的大弹窗（跳过右下角小气泡）
        showUpdateInfo(result, true);
        showToast(t("settings.downloadStarted", { version: result.version }), "success");
      } else {
        showToast(t("settings.alreadyLatest"), "success");
      }
    }
  } catch (e: unknown) {
    showToast(t("settings.checkUpdateFailed") + ": " + String(e), "error");
  } finally {
    checkingUpdate.value = false;
  }
}

// 开机检测到的可更新版本，自动更新关闭时需点击此按钮才开始后台下载
const immediateUpdating = ref(false);
async function handleImmediateUpdate() {
  if (!globalUpdateInfo.value) return;
  if (immediateUpdating.value) return;
  immediateUpdating.value = true;
  // 未开启自动更新时：点击“立即更新”直接展开带日志的大弹窗并触发后台下载（跳过小气泡）
  showUpdateInfo(globalUpdateInfo.value, true);
  try {
    const result = await invoke<UpdateInfo | null>("check_update");
    if (result) {
      showToast(`已开始下载 v${result.version}`, "success");
    }
  } catch (e: unknown) {
    showToast(t("settings.downloadFailed", { error: String(e) }), "error");
  } finally {
    immediateUpdating.value = false;
  }
}

/** 便携版：跳转 GitHub Releases 手动下载新版 */
function goToReleases() {
  openUrl("https://github.com/ISuuuu/Yuumi/releases/latest").catch(
    (err: unknown) => console.error("打开 Releases 页面失败:", err),
  );
}
</script>

<template>
  <!-- 6. 软件更新 -->
  <template v-if="config">
    <div class="group-header">{{ $t("settings.softwareUpdateGroup") }}</div>

  <div class="card-item border-bottom">
    <div class="card-left">
      <h3 class="card-title">{{ $t("settings.checkUpdateTitle") }}</h3>
      <span class="card-desc">
        {{
          isPortable
            ? $t("settings.portableUpdateDesc")
            : $t("settings.checkUpdateDesc")
        }}
        <template v-if="!isPortable"> — {{ $t("settings.checkUpdateAutoDownloadHint") }}</template>
      </span>
    </div>
    <div class="card-right" style="flex-shrink: 0; gap: 10px">
      <!-- 便携版：检查更新（zip 覆盖方案）+ 手动前往 GitHub 下载 -->
      <template v-if="isPortable">
        <n-button
          size="small"
          :disabled="checkingUpdate"
          @click="manualCheckUpdate"
          :title="
            checkingUpdate
              ? $t('settings.checkingUpdate')
              : $t('settings.checkUpdateBtn')
          "
        >
          <template #icon>
            <svg
              v-if="!checkingUpdate"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
              width="13"
              height="13"
            >
              <path d="M21 2v6h-6" />
              <path d="M3 12a9 9 0 0 1 15-6.7L21 8" />
              <path d="M3 22v-6h6" />
              <path d="M21 12a9 9 0 0 1-15 6.7L3 16" />
            </svg>
            <svg
              v-else
              class="spin"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              width="13"
              height="13"
            >
              <path
                d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83"
              />
            </svg>
          </template>
          {{
            checkingUpdate
              ? $t("settings.checkingUpdate")
              : $t("settings.checkUpdateBtn")
          }}
        </n-button>
        <n-button
          size="small"
          type="primary"
          ghost
          @click="goToReleases"
          :title="$t('settings.goToReleases')"
        >
          <template #icon>
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
              width="13"
              height="13"
            >
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
              <polyline points="7 10 12 15 17 10" />
              <line x1="12" y1="15" x2="12" y2="3" />
            </svg>
          </template>
          {{ $t("settings.goToReleases") }}
        </n-button>
        <n-switch
          v-model:value="config.General.EnableCheckUpdate"
          @update:value="autoSave"
        />
      </template>
      <!-- 安装版：检查更新按钮 + 开关 -->
      <template v-else>
        <n-button
          size="small"
          :disabled="checkingUpdate"
          @click="manualCheckUpdate"
          :title="
            checkingUpdate
              ? $t('settings.checkingUpdate')
              : $t('settings.checkUpdateBtn')
          "
        >
          <template #icon>
            <svg
              v-if="!checkingUpdate"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
              width="13"
              height="13"
            >
              <path d="M21 2v6h-6" />
              <path d="M3 12a9 9 0 0 1 15-6.7L21 8" />
              <path d="M3 22v-6h6" />
              <path d="M21 12a9 9 0 0 1-15 6.7L3 16" />
            </svg>
            <svg
              v-else
              class="spin"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              width="13"
              height="13"
            >
              <path
                d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83"
              />
            </svg>
          </template>
          {{
            checkingUpdate
              ? $t("settings.checkingUpdate")
              : $t("settings.checkUpdateBtn")
          }}
        </n-button>
        <n-switch
          v-model:value="config.General.EnableCheckUpdate"
          @update:value="autoSave"
        />
      </template>
    </div>
  </div>

  <!-- 更新可用提示（联动 hasUpdate） -->
  <div v-if="hasUpdate && globalUpdateInfo" class="update-available-card">
    <div class="update-available-left">
      <span class="update-available-dot" />
      <span class="update-available-text">{{
        $t("settings.newVersionToast", { version: globalUpdateInfo.version })
      }}</span>
      <span class="update-available-current">{{
        $t("settings.currentVersionShort", { version: globalUpdateInfo.currentVersion })
      }}</span>
    </div>
    <div class="update-available-right">
      <!-- 自动更新开启时提示后台下载中；关闭时展示“立即更新”按钮 -->
      <template v-if="config?.General?.EnableCheckUpdate">
        <span class="update-available-hint">{{ $t("settings.updateReadyHint") }}</span>
      </template>
      <template v-else>
        <n-button
          size="small"
          type="primary"
          :loading="immediateUpdating"
          @click="handleImmediateUpdate"
        >
          {{ $t("settings.updateNow") }}
        </n-button>
      </template>
    </div>
  </div>
  </template>
</template>

<style scoped src="./shared.css"></style>

<style scoped>
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

@keyframes spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

.spin {
  animation: spin 0.9s linear infinite;
}

/* 更新可用提示卡片 */
.update-available-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  background: linear-gradient(135deg, rgba(244, 63, 94, 0.08), rgba(0, 210, 196, 0.08));
  border: 1px solid rgba(244, 63, 94, 0.2);
  border-radius: 12px;
  padding: 12px 16px;
  margin-bottom: 8px;
  animation: fadeIn 0.3s ease-out;
}

.update-available-left {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.update-available-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #f43f5e;
  box-shadow: 0 0 6px rgba(244, 63, 94, 0.8);
  flex-shrink: 0;
  animation: badge-pulse 2s infinite ease-in-out;
}

.update-available-text {
  font-size: 0.85rem;
  font-weight: 700;
  color: #f43f5e;
}

.update-available-current {
  font-size: 0.76rem;
  color: var(--text-dimmed);
}

.update-available-right {
  flex-shrink: 0;
}

.update-available-hint {
  font-size: 0.76rem;
  color: var(--text-muted);
}
</style>
