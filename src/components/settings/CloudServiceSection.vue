<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useSettingsAutoSave } from "../../composables/useSettingsAutoSave";
import { useI18n } from "vue-i18n";

const { config, autoSave } = useSettingsAutoSave();
const { t } = useI18n();

// SignalR 连接状态
const signalrStatus = ref<
  "disconnected" | "connecting" | "connected" | "error"
>("disconnected");
const signalrError = ref("");
const unlistenFns = ref<Array<() => void>>([]);

onMounted(async () => {
  // 获取初始 SignalR 状态
  try {
    signalrStatus.value = await invoke<"disconnected" | "connecting" | "connected" | "error">(
      "get_signalr_status",
    );
  } catch (e) {
    console.error("获取 SignalR 状态失败:", e);
  }

  // 监听后端 SignalR 事件
  try {
    const unConnecting = await listen("signalr-connecting", () => {
      signalrStatus.value = "connecting";
      signalrError.value = "";
    });
    const unConnected = await listen("signalr-connected", () => {
      signalrStatus.value = "connected";
      signalrError.value = "";
    });
    const unDisconnected = await listen("signalr-disconnected", () => {
      signalrStatus.value = "disconnected";
    });
    const unError = await listen<string>("signalr-error", (event) => {
      signalrStatus.value = "error";
      signalrError.value = event.payload;
    });

    unlistenFns.value.push(unConnecting);
    unlistenFns.value.push(unConnected);
    unlistenFns.value.push(unDisconnected);
    unlistenFns.value.push(unError);
  } catch (e) {
    console.error("注册 SignalR 监听器失败:", e);
  }
});

onUnmounted(() => {
  unlistenFns.value.forEach((fn) => fn());
});
</script>

<template>
  <!-- 云端服务 -->
  <n-collapse v-if="config" arrow-placement="right" class="collapse-card">
    <n-collapse-item name="upload_and_signalr">
      <template #header>
        <div class="collapse-header-wrapper">
          <div class="collapse-left-simple">
            <span class="card-title">{{
              $t("settings.cloudServiceTitle")
            }}</span>
            <span class="card-desc">{{
              $t("settings.cloudServiceDesc")
            }}</span>
          </div>
          <div class="collapse-right-status">
            <span class="status-preview">
              {{
                config.General.UploadApiUrl
                  ? $t("settings.uploadConfigured")
                  : $t("settings.uploadNotConfigured")
              }}
              <template v-if="config.Functions.LcuRealtimeEnabled">
                /
                <span :class="['signalr-status-badge', signalrStatus]">
                  {{
                    signalrStatus === "connected"
                      ? $t("settings.cloudConnected")
                      : signalrStatus === "connecting"
                        ? $t("settings.cloudConnecting")
                        : signalrStatus === "error"
                          ? $t("settings.cloudConnectFailed")
                          : $t("settings.cloudDisconnected")
                  }}
                </span>
              </template>
            </span>
          </div>
        </div>
      </template>
      <div class="setting-input-row">
        <span class="setting-input-label">{{
          $t("settings.apiServerAddrLabel")
        }}</span>
        <n-input
          v-model:value="config.General.UploadApiUrl"
          placeholder="http://example.com"
          clearable
          @change="
            if (
              config.Functions.LcuRealtimeEnabled &&
              config.General.UploadApiUrl
            ) {
              signalrStatus = 'connecting';
            }
            autoSave();
          "
          style="max-width: 300px"
        />
      </div>
      <div class="setting-input-row">
        <span class="setting-input-label">{{
          $t("settings.realtimeLcuLabel")
        }}</span>
        <n-switch
          v-model:value="config.Functions.LcuRealtimeEnabled"
          @update:value="
            if (
              config.Functions.LcuRealtimeEnabled &&
              config.General.UploadApiUrl
            ) {
              signalrStatus = 'connecting';
            } else {
              signalrStatus = 'disconnected';
            }
            autoSave();
          "
        />
      </div>
      <div
        v-if="signalrStatus === 'error' && signalrError"
        class="setting-error-tip"
      >
        {{ $t("settings.connectionError") }}{{ signalrError }}
      </div>
      <div class="setting-input-row">
        <span class="setting-input-label">userid:</span>
        <n-input
          v-model:value="config.General.SignalrUserId"
          :placeholder="t('settings.useridPlaceholder')"
          clearable
          @change="autoSave"
          style="max-width: 300px"
        />
      </div>
    </n-collapse-item>
  </n-collapse>
</template>

<style scoped src="./shared.css"></style>

<style scoped>
.setting-input-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  width: 100%;
  margin-bottom: 12px;
}

.setting-input-row:last-child {
  margin-bottom: 0;
}

.setting-input-label {
  font-size: 0.82rem;
  font-weight: 600;
  color: var(--text-muted);
  width: 130px;
  flex-shrink: 0;
}
.signalr-status-badge {
  font-weight: 600;
  transition: color 0.2s ease;
}
.signalr-status-badge.connected {
  color: var(--win-color);
}
.signalr-status-badge.connecting {
  color: #f59e0b;
}
.signalr-status-badge.error {
  color: var(--loss-color);
}
.signalr-status-badge.disconnected {
  color: var(--text-dimmed);
}
.setting-error-tip {
  font-size: 0.76rem;
  color: var(--loss-color);
  margin-top: -6px;
  margin-bottom: 10px;
  padding-left: 142px;
  text-align: left;
}
</style>
