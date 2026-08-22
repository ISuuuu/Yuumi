<script setup lang="ts">
import { lcuRequest } from "../../../api/lcu";
import { useToast } from "../../../composables/useToast";
import { useI18n } from "vue-i18n";
import { NButton, NCollapse, NCollapseItem } from "naive-ui";
import { useQuickActionsLoading } from "./shared";

const { showToast } = useToast();
const { t } = useI18n();
const loading = useQuickActionsLoading();

// 在线状态更改
async function handleApplyAvailability(avail: string) {
  loading.value = true;
  try {
    const resp = await lcuRequest<unknown>("PUT", "/lol-chat/v1/me", {
      availability: avail,
    });
    if (resp.success) {
      const availText =
        avail === "chat"
          ? t("tools.status.online")
          : avail === "away"
            ? t("tools.status.away")
            : t("tools.status.invisible");
      showToast(t("tools.status.success", { status: availText }));
    } else {
      showToast(t("tools.status.failed", { error: resp.error }), "error");
    }
  } catch (e: unknown) {
    showToast(t("tools.status.error", { error: String(e) }), "error");
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <!-- 在线状态 -->
  <n-collapse arrow-placement="right" class="collapse-card">
    <n-collapse-item name="onlinestate">
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
                <path
                  d="M21 11.5a8.38 8.38 0 0 1-.9 3.8 8.5 8.5 0 0 1-7.6 4.7 8.38 8.38 0 0 1-3.8-.9L3 21l1.9-5.7a8.38 8.38 0 0 1-.9-3.8 8.5 8.5 0 0 1 4.7-7.6 8.38 8.38 0 0 1 3.8-.9h.5a8.48 8.48 0 0 1 8 8v.5z"
                ></path>
              </svg>
            </div>
            <div class="title-container">
              <h3 class="card-title">{{ t("tools.status.title") }}</h3>
              <span class="card-desc">{{ t("tools.status.title") }}</span>
            </div>
          </div>
          <div class="collapse-right-status">
            <span class="status-preview">{{
              t("tools.spectate.expand")
            }}</span>
          </div>
        </div>
      </template>
      <div class="setting-row">
        <span class="setting-label">{{ t("tools.status.title") }}</span>
        <div class="btn-group">
          <n-button
            class="status-btn online"
            size="small"
            @click="handleApplyAvailability('chat')"
            :disabled="loading"
            >{{ t("tools.status.online") }}</n-button
          >
          <n-button
            class="status-btn away"
            size="small"
            @click="handleApplyAvailability('away')"
            :disabled="loading"
            >{{ t("tools.status.away") }}</n-button
          >
          <n-button
            class="status-btn offline"
            size="small"
            @click="handleApplyAvailability('offline')"
            :disabled="loading"
            >{{ t("tools.status.invisible") }}</n-button
          >
        </div>
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

.btn-group {
  display: flex;
  gap: 8px;
}

/* Status buttons */
.status-btn {
  border: 1px solid var(--border-color);
  background: var(--card-bg);
  padding: 6px 20px;
  border-radius: 6px;
  font-size: 0.78rem;
  font-weight: bold;
  cursor: pointer;
  color: var(--text-color);
  transition: all 0.2s;
}

.status-btn.online {
  color: var(--win-color);
  border-color: var(--win-border);
  background-color: var(--win-bg);
}

.status-btn.online:hover {
  background-color: var(--win-color);
  color: white;
}

.status-btn.away {
  color: #e6a23c;
  border-color: rgba(230, 162, 60, 0.2);
  background-color: rgba(230, 162, 60, 0.08);
}

.status-btn.away:hover {
  background-color: #e6a23c;
  color: white;
}

.status-btn.offline {
  color: var(--text-muted);
  border-color: var(--border-color);
  background-color: rgba(0, 0, 0, 0.02);
}

.status-btn.offline:hover {
  background-color: var(--text-dimmed);
  color: white;
}
</style>
