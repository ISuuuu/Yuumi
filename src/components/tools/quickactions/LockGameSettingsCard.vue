<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useToast } from "../../../composables/useToast";
import { useI18n } from "vue-i18n";
import { NSwitch } from "naive-ui";

const { showToast } = useToast();
const { t } = useI18n();

// 锁定游戏设置状态
const isGameSettingsLocked = ref(false);
async function checkGameSettingsLock() {
  try {
    isGameSettingsLocked.value = await invoke<boolean>(
      "get_game_settings_readonly",
    );
  } catch (e) {
    console.error("获取游戏设置锁定状态失败:", e);
  }
}

onMounted(async () => {
  await checkGameSettingsLock();
});

// 切换锁定游戏设置
async function handleToggleLockGameSettings() {
  try {
    const nextState = !isGameSettingsLocked.value;
    const msg = await invoke<string>("set_game_settings_readonly", {
      readonly: nextState,
    });
    isGameSettingsLocked.value = nextState;
    showToast(msg);
  } catch (e: unknown) {
    showToast(String(e), "error");
  }
}
</script>

<template>
  <!-- 锁定游戏设置 -->
  <div class="card-item">
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
          <rect x="3" y="11" width="18" height="11" rx="2" ry="2"></rect>
          <path d="M7 11V7a5 5 0 0 1 10 0v4"></path>
        </svg>
      </div>
      <div class="title-container">
        <h3 class="card-title">{{ t("tools.lockGameSettings.title") }}</h3>
        <span class="card-desc">{{
          t("tools.lockGameSettings.desc")
        }}</span>
      </div>
    </div>
    <div class="card-right">
      <n-switch
        :value="isGameSettingsLocked"
        @update:value="handleToggleLockGameSettings"
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
