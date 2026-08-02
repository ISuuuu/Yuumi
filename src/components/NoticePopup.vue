<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";

const emit = defineEmits<{
  close: [];
}>();

const NOTICE_URL =
  "https://raw.githubusercontent.com/ISuuuu/Yuumi/main/NOTICE.md";

const AUTO_DISMISS_MS = 8000;

const noticeHtml = ref("");
const loading = ref(true);
const hasError = ref(false);
let dismissTimer: ReturnType<typeof setTimeout> | null = null;

async function renderMarkdown(md: string): Promise<string> {
  try {
    const { marked } = await import("marked");
    return marked.parse(md) as string;
  } catch (e) {
    console.error("[NoticePopup] 解析 Markdown 失败:", e);
    return md;
  }
}

async function fetchNotice() {
  loading.value = true;
  hasError.value = false;
  try {
    const text = await invoke<string>("fetch_github_text", {
      url: NOTICE_URL,
    });
    noticeHtml.value = await renderMarkdown(text);
    console.log("[NoticePopup] 成功拉取 GitHub 公告");
  } catch (err) {
    console.warn("[NoticePopup] 拉取 GitHub 公告失败:", err);
    hasError.value = true;
  } finally {
    loading.value = false;
  }
}

function close() {
  emit("close");
}

function startDismissTimer() {
  if (dismissTimer) clearTimeout(dismissTimer);
  dismissTimer = setTimeout(close, AUTO_DISMISS_MS);
}

function retry() {
  fetchNotice();
  startDismissTimer();
}

onMounted(() => {
  fetchNotice();
  startDismissTimer();
});

onUnmounted(() => {
  if (dismissTimer) clearTimeout(dismissTimer);
});
</script>

<template>
  <div class="notice-popup">
    <div class="notice-popup-header">
      <span class="notice-popup-title">
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <path
            d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9M13.73 21a2 2 0 0 1-3.46 0"
          />
        </svg>
        <span>{{ $t("noticePage.title") }}</span>
      </span>
      <span class="notice-popup-close" @click="close">×</span>
    </div>

    <div v-if="loading" class="notice-popup-body notice-popup-status">
      <span class="notice-popup-spinner" />
      <span class="notice-popup-hint">{{
        $t("noticePage.loadingText")
      }}</span>
    </div>

    <div
      v-else-if="hasError"
      class="notice-popup-body notice-popup-status"
    >
      <span class="notice-popup-hint">{{
        $t("noticePage.errorText")
      }}</span>
      <button class="notice-popup-retry" @click="retry">
        {{ $t("noticePage.retryBtn") }}
      </button>
    </div>

    <div
      v-else
      class="notice-popup-body markdown-body"
      v-html="noticeHtml"
    />
  </div>
</template>

<style scoped>
.notice-popup {
  position: fixed;
  right: 20px;
  bottom: 20px;
  z-index: 1000;
  width: 320px;
  max-width: calc(100vw - 100px);
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: 14px;
  box-shadow: var(--shadow-md);
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
  overflow: hidden;
  animation: noticeSlideUp 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
}

.notice-popup-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 14px;
  border-bottom: 1px solid var(--border-color);
  background: var(--primary-color-alpha-10);
}

.notice-popup-title {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  font-size: 0.84rem;
  font-weight: 800;
  color: var(--text-color);
}

.notice-popup-title svg {
  width: 15px;
  height: 15px;
  color: var(--primary-color);
}

.notice-popup-close {
  width: 22px;
  height: 22px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 6px;
  cursor: pointer;
  font-size: 1rem;
  line-height: 1;
  color: var(--text-muted);
  transition: all 0.2s ease;
  user-select: none;
}

.notice-popup-close:hover {
  background: var(--hover-bg-strong);
  color: var(--text-color);
}

.notice-popup-body {
  padding: 12px 14px;
  max-height: 240px;
  overflow-y: auto;
  font-size: 0.8rem;
  line-height: 1.7;
  color: var(--text-muted);
  word-break: break-word;
}

.notice-popup-body :deep(h1) {
  margin: 0 0 6px;
  font-size: 1.05rem;
  color: var(--text-color);
  font-weight: 800;
}

.notice-popup-body :deep(h2) {
  margin: 8px 0 4px;
  font-size: 0.92rem;
  color: var(--text-color);
  font-weight: 700;
}

.notice-popup-body :deep(h3) {
  margin: 6px 0 4px;
  font-size: 0.86rem;
  color: var(--text-color);
  font-weight: 700;
}

.notice-popup-body :deep(p) {
  margin: 3px 0 8px;
}

.notice-popup-body :deep(ul),
.notice-popup-body :deep(ol) {
  margin: 3px 0 8px;
  padding-left: 18px;
}

.notice-popup-body :deep(li) {
  margin-bottom: 3px;
}

.notice-popup-body :deep(a) {
  color: var(--primary-color);
  text-decoration: none;
}

.notice-popup-body :deep(code) {
  background: var(--hover-bg-strong);
  padding: 1px 5px;
  border-radius: 4px;
  font-family: Consolas, Monaco, monospace;
  font-size: 0.82em;
  color: var(--primary-color);
}

.notice-popup-body :deep(strong) {
  color: var(--text-color);
  font-weight: 700;
}

.notice-popup-status {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  padding: 24px 16px;
  color: var(--text-dimmed);
}

.notice-popup-spinner {
  width: 22px;
  height: 22px;
  border: 2.5px solid var(--border-color);
  border-top-color: var(--primary-color);
  border-radius: 50%;
  animation: noticeSpin 0.8s linear infinite;
}

.notice-popup-hint {
  font-size: 0.78rem;
  color: var(--text-muted);
}

.notice-popup-retry {
  background: var(--primary-color-alpha-15);
  color: var(--primary-color);
  border: 1px solid var(--border-color);
  padding: 4px 16px;
  border-radius: 6px;
  font-size: 0.78rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease;
}

.notice-popup-retry:hover {
  background: var(--primary-color);
  color: white;
  border-color: var(--primary-color);
}

@keyframes noticeSlideUp {
  from {
    opacity: 0;
    transform: translateY(16px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@keyframes noticeSpin {
  to {
    transform: rotate(360deg);
  }
}
</style>
