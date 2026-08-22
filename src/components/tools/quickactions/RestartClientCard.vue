<script setup lang="ts">
import { lcuRequest } from "../../../api/lcu";
import { useToast } from "../../../composables/useToast";
import { useI18n } from "vue-i18n";
import { NButton, useDialog } from "naive-ui";
import { useQuickActionsLoading } from "./shared";

const { showToast } = useToast();
const { t } = useI18n();
const dialog = useDialog();
const loading = useQuickActionsLoading();

// 重启客户端
function handleRestartClient() {
  dialog.warning({
    title: t("tools.restartClient.dialogTitle"),
    content: t("tools.restartClient.dialogContent"),
    positiveText: t("tools.confirm"),
    negativeText: t("tools.cancel"),
    positiveButtonProps: { type: "primary" },
    onPositiveClick: async () => {
      loading.value = true;
      try {
        const resp = await lcuRequest<unknown>(
          "POST",
          "/riotclient/kill-and-restart-ux",
        );
        if (resp.success) {
          showToast(t("tools.restartClient.success"));
        } else {
          showToast(
            t("tools.restartClient.failed", { error: resp.error }),
            "error",
          );
        }
      } catch (e: unknown) {
        showToast(
          t("tools.restartClient.error", { error: String(e) }),
          "error",
        );
      } finally {
        loading.value = false;
      }
    },
  });
}
</script>

<template>
  <!-- 重启客户端 -->
  <div class="card-item attached-below">
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
            d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67"
          ></path>
        </svg>
      </div>
      <div class="title-container">
        <h3 class="card-title">{{ t("tools_extra.restartClient") }}</h3>
        <span class="card-desc">{{
          t("tools_extra.restartClientDesc")
        }}</span>
      </div>
    </div>
    <div class="card-right">
      <n-button
        class="action-btn"
        @click="handleRestartClient"
        :loading="loading"
        >{{ t("tools_extra.restartBtn") }}</n-button
      >
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

/* 拆分前由 `.border-bottom + .card-item` 相邻选择器实现的上卡片贴合效果 */
.card-item.attached-below {
  border-radius: 0;
  margin-top: 0;
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
.action-btn,
.action-btn.n-button {
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

.action-btn:hover,
.action-btn.n-button:hover {
  border-color: var(--primary-color);
  background-color: var(--settings-card-bg-hover);
  box-shadow: 0 0 0 1px rgba(0, 159, 170, 0.3);
  transform: translateY(-0.5px);
}

.action-btn:active,
.action-btn.n-button:active {
  background: var(--settings-card-bg);
  transform: translateY(0.5px);
}

.action-btn:disabled,
.action-btn.n-button:disabled {
  opacity: 0.3;
  cursor: not-allowed;
  transform: none !important;
  box-shadow: none !important;
}

.action-btn.n-button .n-button__content {
  color: inherit !important;
}

.action-btn.n-button .n-base-loading {
  color: inherit !important;
}
</style>
