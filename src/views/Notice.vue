<script setup lang="ts">
import { ref, onMounted } from "vue";
import { marked } from "marked";
import { useI18n } from "vue-i18n";

const { t } = useI18n();

interface VersionEntry {
  tag: string;
  date: string;
  body: string;
}

const versions = ref<VersionEntry[]>([]);
const loading = ref(false);
const hasError = ref(false);

function renderMarkdown(md: string): string {
  if (!md) return "";
  try {
    return marked.parse(md) as string;
  } catch (e) {
    console.error("[Notice] 解析 Markdown 失败:", e);
    return md;
  }
}

function formatDate(isoStr: string) {
  if (!isoStr) return "";
  const d = new Date(isoStr);
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${d.getFullYear()}/${pad(d.getMonth() + 1)}/${pad(d.getDate())}`;
}

async function fetchReleases() {
  loading.value = true;
  hasError.value = false;
  try {
    const resp = await fetch(
      "https://api.github.com/repos/ISuuuu/Yuumi/releases",
    );
    if (!resp.ok) throw new Error("HTTP status " + resp.status);
    const data = await resp.json();
    if (Array.isArray(data) && data.length > 0) {
      versions.value = data.slice(0, 3).map((rel: any) => ({
        tag: rel.tag_name,
        date: formatDate(rel.published_at),
        body: rel.body || t("noticePage.noDesc"),
      }));
      console.log("[Notice] 成功拉取 GitHub 最近 3 个版本日志");
    } else {
      throw new Error("No release entries found");
    }
  } catch (err) {
    console.warn("[Notice] 拉取 GitHub 动态日志失败:", err);
    hasError.value = true;
  } finally {
    loading.value = false;
  }
}

onMounted(() => {
  fetchReleases();
});
</script>

<template>
  <div class="notice-view">
    <div class="notice-header">
      <div class="header-icon">
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <path
            d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9M13.73 21a2 2 0 0 1-3.46 0"
          />
        </svg>
      </div>
      <div class="header-text">
        <h2>{{ $t("noticePage.title") }}</h2>
        <p class="header-sub">{{ $t("noticePage.sub") }}</p>
      </div>
    </div>

    <!-- 动态同步 Loading 骨架屏 -->
    <div v-if="loading && versions.length === 0" class="loading-container">
      <div class="loading-spinner" />
      <p class="loading-text">{{ $t("noticePage.loadingText") }}</p>
    </div>

    <!-- 错误状态提示与重试 -->
    <div v-else-if="hasError && versions.length === 0" class="error-container">
      <div class="offline-logo">⚠️</div>
      <p class="error-text">{{ $t("noticePage.errorText") }}</p>
      <button class="retry-btn" @click="fetchReleases">
        {{ $t("noticePage.retryBtn") }}
      </button>
    </div>

    <div v-else class="timeline">
      <div
        v-for="(ver, idx) in versions"
        :key="ver.tag"
        class="timeline-node"
        :style="{ '--delay': `${idx * 0.1}s` }"
      >
        <div class="timeline-dot">
          <div class="dot-inner" />
        </div>

        <div class="changelog-card">
          <div class="card-header">
            <div class="version-tag" :class="{ latest: idx === 0 }">
              {{ ver.tag }}
            </div>
            <span class="version-date">{{ ver.date }}</span>
          </div>
          <div
            class="release-body markdown-body"
            v-html="renderMarkdown(ver.body)"
          />
          <div class="card-glow" />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.notice-view {
  max-width: 820px;
  margin: 0 auto;
  padding: 1.5rem 2rem 3rem;
  animation: fadeIn 0.35s ease-out;
}

.notice-header {
  display: flex;
  align-items: center;
  gap: 16px;
  margin-bottom: 2.5rem;
  padding-bottom: 1.5rem;
  border-bottom: 1px solid var(--border-color);
}

.header-icon {
  width: 42px;
  height: 42px;
  border-radius: 12px;
  background: var(--primary-color-alpha-15);
  color: var(--primary-color);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  box-shadow: 0 0 20px var(--primary-color-alpha-15);
}
.header-icon svg {
  width: 22px;
  height: 22px;
}

.header-text h2 {
  margin: 0;
  font-size: 1.5rem;
  font-weight: 800;
  color: var(--text-color);
  letter-spacing: 0.5px;
}

.header-sub {
  margin: 4px 0 0;
  font-size: 0.82rem;
  color: var(--text-dimmed);
}

.timeline {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 0;
}

.timeline::before {
  content: "";
  position: absolute;
  left: 19px;
  top: 12px;
  bottom: 12px;
  width: 2px;
  background: linear-gradient(
    180deg,
    var(--primary-color) 0%,
    var(--primary-color-alpha-30) 60%,
    transparent 100%
  );
  opacity: 0.5;
}

.timeline-node {
  position: relative;
  display: flex;
  align-items: flex-start;
  gap: 20px;
  padding-bottom: 2rem;
  animation: slideUp 0.4s ease-out both;
  animation-delay: var(--delay);
}

.timeline-node:last-child {
  padding-bottom: 0;
}

.timeline-dot {
  position: relative;
  z-index: 1;
  width: 40px;
  flex-shrink: 0;
  display: flex;
  justify-content: center;
  padding-top: 16px;
}

.dot-inner {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: var(--primary-color);
  border: 3px solid var(--bg-color);
  box-shadow:
    0 0 0 3px var(--primary-color-alpha-30),
    0 0 12px var(--primary-color-alpha-30);
  transition:
    transform 0.2s,
    box-shadow 0.2s;
}

.timeline-node:hover .dot-inner {
  transform: scale(1.25);
  box-shadow:
    0 0 0 4px var(--primary-color-alpha-40),
    0 0 20px var(--primary-color-alpha-40);
}

.changelog-card {
  flex: 1;
  background: var(--card-bg);
  border-radius: 14px;
  padding: 1.5rem;
  border: 1px solid var(--border-color);
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
  box-shadow: var(--shadow-sm);
  transition: all 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
  position: relative;
  overflow: hidden;
}

.changelog-card:hover {
  border-color: var(--primary-color-alpha-30);
  box-shadow:
    0 8px 30px -8px var(--primary-color-alpha-15),
    var(--shadow-md);
  transform: translateY(-2px);
}

.card-glow {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  height: 2px;
  background: linear-gradient(
    90deg,
    transparent 0%,
    var(--primary-color-alpha-30) 30%,
    var(--primary-color) 50%,
    var(--primary-color-alpha-30) 70%,
    transparent 100%
  );
  opacity: 0;
  transition: opacity 0.3s;
}

.changelog-card:hover .card-glow {
  opacity: 1;
}

.card-header {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 0.8rem;
}

.version-tag {
  display: inline-flex;
  align-items: center;
  background: linear-gradient(135deg, var(--primary-color), #00b3a7);
  color: white;
  padding: 3px 12px;
  border-radius: 6px;
  font-weight: 800;
  font-size: 0.78rem;
  letter-spacing: 0.3px;
  box-shadow: 0 2px 8px var(--primary-color-alpha-30);
}

.version-tag.latest {
  background: linear-gradient(135deg, #f59e0b, #f97316);
  box-shadow: 0 2px 10px rgba(245, 158, 11, 0.4);
}

.version-date {
  font-size: 0.78rem;
  color: var(--text-dimmed);
  font-weight: 500;
}

.release-body {
  margin: 0;
  font-size: 0.82rem;
  line-height: 1.7;
  color: var(--text-muted);
  word-break: break-word;
  font-family: inherit;
}

/* Markdown 富文本深度美化 */
.release-body :deep(h1),
.release-body :deep(h2),
.release-body :deep(h3) {
  margin-top: 10px;
  margin-bottom: 6px;
  color: var(--text-color);
  font-weight: 700;
  font-size: 0.95rem;
}

.release-body :deep(p) {
  margin: 4px 0 8px;
  line-height: 1.6;
}

.release-body :deep(ul),
.release-body :deep(ol) {
  margin: 4px 0 10px;
  padding-left: 20px;
}

.release-body :deep(li) {
  margin-bottom: 4px;
}

.release-body :deep(code) {
  background: var(--hover-bg-strong);
  padding: 2px 6px;
  border-radius: 4px;
  font-family: Consolas, Monaco, monospace;
  font-size: 0.82em;
  color: var(--primary-color);
}

.release-body :deep(strong) {
  color: var(--text-color);
  font-weight: 700;
}

.loading-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 4rem 2rem;
  color: var(--text-dimmed);
}

.loading-spinner {
  width: 28px;
  height: 28px;
  border: 2.5px solid var(--border-color);
  border-top-color: var(--primary-color);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
  margin-bottom: 1rem;
}

.loading-text {
  font-size: 0.85rem;
  color: var(--text-muted);
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.error-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 5rem 2rem;
  color: var(--text-dimmed);
  animation: fadeIn 0.3s ease-out;
}

.error-text {
  font-size: 0.9rem;
  color: var(--text-muted);
  margin: 1rem 0 1.5rem;
  text-align: center;
}

.retry-btn {
  background: var(--primary-color-alpha-15);
  color: var(--primary-color);
  border: 1px solid var(--border-color);
  padding: 8px 24px;
  border-radius: 8px;
  font-size: 0.85rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease;
}

.retry-btn:hover {
  background: var(--primary-color);
  color: white;
  border-color: var(--primary-color);
  box-shadow: 0 4px 12px var(--primary-color-alpha-30);
  transform: translateY(-1px);
}

.retry-btn:active {
  transform: translateY(0);
}

@keyframes fadeIn {
  from {
    opacity: 0;
    transform: translateY(8px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@keyframes slideUp {
  from {
    opacity: 0;
    transform: translateY(16px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>
