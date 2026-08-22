<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { useToast } from "../../../composables/useToast";
import { useI18n } from "vue-i18n";
import { useQuickActionsLoading } from "./shared";

const { showToast } = useToast();
const { t } = useI18n();
const loading = useQuickActionsLoading();

// 修复客户端窗口
async function handleFixWindow() {
  loading.value = true;
  try {
    await invoke("fix_lcu_window");
    showToast(t("tools.fixWindow.success"));
  } catch (e: unknown) {
    showToast(t("tools.fixWindow.failed", { error: String(e) }), "error");
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <!-- 修复客户端窗口 -->
  <div class="card-item border-bottom">
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
          <path
            d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"
          ></path>
        </svg>
      </div>
      <div class="title-container">
        <h3 class="card-title">{{ t("tools_extra.fixWindow") }}</h3>
        <span class="card-desc">{{ t("tools_extra.fixWindowDesc") }}</span>
      </div>
    </div>
    <div class="card-right">
      <button
        class="action-btn"
        @click="handleFixWindow"
        :disabled="loading"
      >
        {{ t("tools_extra.fixBtn") }}
      </button>
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

/* Action button styles */
.action-btn {
  background: var(--settings-card-bg);
  border: 1px solid var(--settings-card-border);
  color: var(--text-color);
  padding: 6px 20px;
  height: auto;
  border-radius: 6px;
  font-size: 0.82rem;
  font-weight: 600;
  cursor: pointer;
  outline: none;
  transition: all 0.2s cubic-bezier(0.25, 0.8, 0.25, 1);
  box-shadow: var(--shadow-sm);
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.action-btn:hover {
  border-color: var(--primary-color);
  background-color: var(--settings-card-bg-hover);
  box-shadow: 0 0 0 1px rgba(0, 159, 170, 0.3);
  transform: translateY(-0.5px);
}

.action-btn:active {
  background: var(--settings-card-bg);
  transform: translateY(0.5px);
}

.action-btn:disabled {
  opacity: 0.3;
  cursor: not-allowed;
  transform: none !important;
  box-shadow: none !important;
}
</style>
