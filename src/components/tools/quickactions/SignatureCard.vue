<script setup lang="ts">
import { ref } from "vue";
import { lcuRequest } from "../../../api/lcu";
import { useToast } from "../../../composables/useToast";
import { useI18n } from "vue-i18n";
import { NInput, NButton, NCollapse, NCollapseItem } from "naive-ui";
import { useQuickActionsLoading } from "./shared";

const { showToast } = useToast();
const { t } = useI18n();
const loading = useQuickActionsLoading();

// 个人签名
const statusInput = ref("");

// 更换状态签名
async function handleApplyStatus() {
  if (!statusInput.value.trim()) return;
  loading.value = true;
  try {
    const resp = await lcuRequest<unknown>("PUT", "/lol-chat/v1/me", {
      statusMessage: statusInput.value.trim(),
    });
    if (resp.success) {
      showToast(t("tools.signature.success"));
      statusInput.value = "";
    } else {
      showToast(t("tools.signature.failed", { error: resp.error }), "error");
    }
  } catch (e: unknown) {
    showToast(t("tools.signature.error", { error: String(e) }), "error");
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <!-- 个人签名 -->
  <n-collapse arrow-placement="right" class="collapse-card">
    <n-collapse-item name="signature">
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
                <path d="M12 20h9"></path>
                <path
                  d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"
                ></path>
              </svg>
            </div>
            <div class="title-container">
              <h3 class="card-title">{{ t("tools.signature.title") }}</h3>
              <span class="card-desc">{{ t("tools.signature.desc") }}</span>
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
        <span class="setting-label">{{
          t("tools.signature.delayLabel") || "输入新的个性化签名:"
        }}</span>
        <n-input
          v-model:value="statusInput"
          :placeholder="t('tools.signature.placeholder')"
          clearable
          style="max-width: 300px"
          size="small"
        >
          <template #suffix>
            <n-button
              size="small"
              type="primary"
              :disabled="loading || !statusInput.trim()"
              @click="handleApplyStatus"
            >
              {{ t("tools.signature.updateBtn") }}
            </n-button>
          </template>
        </n-input>
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
