<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

const props = defineProps<{
  appVersion: string;
}>();

// ─── 版本更新历史（GitHub Releases）───
interface VersionEntry {
  tag: string;
  date: string;
  html: string;
}

const versionHistory = ref<VersionEntry[]>([]);
const historyLoading = ref(false);
const historyError = ref(false);
const showChangelog = ref(false);

// 预取版本更新历史：等待父组件获取到真实版本号后再请求，
// 避免以空版本号调用导致后端跳过"缓存缺少当前版本则强制刷新"的校验。
watch(
  () => props.appVersion,
  (val) => {
    if (val) fetchReleaseHistory();
  },
  { immediate: true },
);

function normalizeVersion(v: string) {
  return (v || "").replace(/^v/i, "").trim();
}

const currentRelease = computed(() => {
  if (!versionHistory.value.length) return null;
  const local = normalizeVersion(props.appVersion);
  if (local) {
    const match = versionHistory.value.find(
      (e) => normalizeVersion(e.tag) === local,
    );
    if (match) return match;
  }
  return versionHistory.value[0];
});

function openChangelog() {
  showChangelog.value = true;
  if (!versionHistory.value.length) fetchReleaseHistory();
}

function openRepo() {
  openUrl("https://github.com/ISuuuu/Yuumi").catch((err) => {
    console.warn("[Settings] 无法打开开源仓库链接:", err);
  });
}

function formatDate(isoStr: string) {
  if (!isoStr) return "";
  const d = new Date(isoStr);
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${d.getFullYear()}/${pad(d.getMonth() + 1)}/${pad(d.getDate())}`;
}

async function fetchReleaseHistory() {
  if (historyLoading.value) return;
  historyLoading.value = true;
  historyError.value = false;
  try {
    const releases = await invoke<
      { tag: string; publishedAt: string; body: string }[]
    >("get_release_changelog", { currentVersion: props.appVersion });
    const { marked } = await import("marked");
    versionHistory.value = releases.map((rel) => ({
      tag: rel.tag,
      date: formatDate(rel.publishedAt),
      html: marked.parse(rel.body || "") as string,
    }));
    console.log("[Settings] 成功获取版本更新日志");
  } catch (err) {
    console.warn("[Settings] 获取版本更新日志失败:", err);
    historyError.value = true;
  } finally {
    historyLoading.value = false;
  }
}
</script>

<template>
  <!-- 7. 关于 -->
  <div class="group-header">{{ $t("settings.aboutGroup") }}</div>

  <div class="about-card">
    <div class="about-brand">
      <span class="about-logo">Y</span>
      <span class="about-name">Yuumi</span>
      <span class="about-version">{{
        appVersion ? `v${appVersion}` : $t("settings.loading")
      }}</span>
    </div>
    <div class="about-intro">{{ $t("settings.aboutIntro") }}</div>
    <div class="about-actions">
      <n-button
        size="tiny"
        quaternary
        class="about-action-btn"
        @click="openChangelog"
      >
        {{ $t("settings.aboutHistoryBtn") }}
      </n-button>
      <n-button
        size="tiny"
        quaternary
        class="about-action-btn"
        @click="openRepo"
      >
        <template #icon>
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            width="12"
            height="12"
          >
            <path d="M9 19c-5 1.5-5-2.5-7-3m14 6v-3.87a3.37 3.37 0 0 0-.94-2.61c3.14-.35 6.44-1.54 6.44-7A5.44 5.44 0 0 0 20 4.77 5.07 5.07 0 0 0 19.91 1S18.73.65 16 2.48a13.38 13.38 0 0 0-7 0C6.27.65 5.09 1 5.09 1A5.07 5.07 0 0 0 5 4.77a5.44 5.44 0 0 0-1.5 3.78c0 5.42 3.3 6.61 6.44 7A3.37 3.37 0 0 0 9 18.13V22" />
          </svg>
        </template>
        {{ $t("settings.aboutRepository") }}
      </n-button>
    </div>
  </div>

  <n-modal
    v-model:show="showChangelog"
    preset="card"
    class="changelog-modal"
    :style="{ width: '640px', maxWidth: '95vw' }"
    :auto-focus="false"
  >
    <template #header>
      <div class="changelog-modal-header">
        <span class="changelog-title">
          {{ $t("settings.aboutHistoryTitle") }}
        </span>
        <span v-if="currentRelease" class="changelog-version-tag">
          {{ currentRelease.tag }}
        </span>
        <span v-if="currentRelease" class="changelog-date">
          {{ currentRelease.date }}
        </span>
      </div>
    </template>
    <div v-if="historyLoading" class="history-status">
      <span class="history-status-text">{{
        $t("settings.aboutHistoryLoading")
      }}</span>
    </div>
    <div v-else-if="historyError" class="history-status">
      <span class="history-status-text">{{
        $t("settings.aboutHistoryError")
      }}</span>
      <n-button size="small" quaternary @click="fetchReleaseHistory">
        {{ $t("settings.aboutHistoryRetry") }}
      </n-button>
    </div>
    <div
      v-else-if="currentRelease"
      class="history-body markdown-body"
      v-html="currentRelease.html"
    />
    <div v-else class="history-status">
      <span class="history-status-text">{{
        $t("settings.aboutHistoryLoading")
      }}</span>
    </div>
  </n-modal>
</template>

<style scoped src="./shared.css"></style>

<style scoped>
/* ── 关于卡片 ── */
.about-card {
  background: var(--settings-card-bg);
  border: 1px solid var(--settings-card-border);
  border-radius: 12px;
  padding: 28px 24px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 14px;
  margin-bottom: 8px;
  box-shadow: var(--shadow-sm);
  text-align: center;
}

.about-brand {
  display: flex;
  align-items: center;
  gap: 10px;
}

.about-logo {
  width: 40px;
  height: 40px;
  border-radius: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 1.15rem;
  font-weight: 900;
  color: #fff;
  background: linear-gradient(135deg, var(--primary-color), #8ec5ff);
  box-shadow: 0 4px 14px var(--primary-color-alpha-30);
}

.about-name {
  font-size: 1.35rem;
  font-weight: 800;
  color: var(--text-color);
  letter-spacing: 0.5px;
  background: linear-gradient(135deg, var(--primary-color), #8ec5ff);
  -webkit-background-clip: text;
  background-clip: text;
  -webkit-text-fill-color: transparent;
}

.about-version {
  font-size: 0.75rem;
  font-weight: 700;
  color: var(--primary-color);
  background: var(--primary-color-alpha-15);
  border: 1px solid var(--primary-color-alpha-30);
  padding: 3px 10px;
  border-radius: 999px;
  white-space: nowrap;
}

.about-intro {
  font-size: 0.82rem;
  color: var(--text-muted);
  line-height: 1.7;
  max-width: 520px;
}

.about-actions {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 12px;
}

.about-action-btn {
  color: var(--text-dimmed) !important;
  font-size: 0.72rem !important;
  padding: 2px 8px !important;
  height: auto !important;
  border-radius: 6px !important;
}

.about-action-btn:hover {
  color: var(--primary-color) !important;
  background: var(--primary-color-alpha-15) !important;
}

/* ── 更新日志弹窗 ── */
.changelog-modal :deep(.n-card-header) {
  padding: 16px 20px;
}

.changelog-modal-header {
  display: flex;
  align-items: center;
  gap: 10px;
}

.changelog-title {
  font-size: 0.95rem;
  font-weight: 700;
  color: var(--text-color);
}

.changelog-version-tag {
  font-size: 0.72rem;
  font-weight: 700;
  color: var(--primary-color);
  background: var(--primary-color-alpha-15);
  border: 1px solid var(--primary-color-alpha-30);
  padding: 2px 9px;
  border-radius: 999px;
  white-space: nowrap;
}

.changelog-date {
  font-size: 0.75rem;
  color: var(--text-dimmed);
  font-weight: 500;
}

/* ── 版本更新历史（关于） ── */
.history-status {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 2px;
}

.history-status-text {
  font-size: 0.82rem;
  color: var(--text-dimmed);
}

.history-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.history-item {
  background: var(--bg-secondary, rgba(255, 255, 255, 0.04));
  border: 1px solid var(--border-color, rgba(255, 255, 255, 0.1));
  border-radius: 10px;
  padding: 12px 14px;
}

.history-item-header {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 8px;
}

.history-tag {
  display: inline-flex;
  align-items: center;
  background: var(--primary-color-alpha-15);
  color: var(--primary-color);
  padding: 2px 10px;
  border-radius: 6px;
  font-weight: 800;
  font-size: 0.76rem;
  letter-spacing: 0.3px;
  border: 1px solid var(--primary-color-alpha-30);
}

.history-tag.latest {
  background: linear-gradient(135deg, #f59e0b, #f97316);
  color: white;
  border: none;
  box-shadow: 0 2px 8px rgba(245, 158, 11, 0.35);
}

.history-date {
  font-size: 0.76rem;
  color: var(--text-dimmed);
  font-weight: 500;
}

.history-body {
  margin: 0;
  font-size: 0.8rem;
  line-height: 1.7;
  color: var(--text-muted);
  word-break: break-word;
}

.history-body :deep(h1),
.history-body :deep(h2),
.history-body :deep(h3) {
  margin-top: 8px;
  margin-bottom: 4px;
  color: var(--text-color);
  font-weight: 700;
  font-size: 0.9rem;
}

.history-body :deep(p) {
  margin: 4px 0 8px;
  line-height: 1.6;
}

.history-body :deep(ul),
.history-body :deep(ol) {
  margin: 4px 0 8px;
  padding-left: 18px;
}

.history-body :deep(li) {
  margin-bottom: 3px;
}

.history-body :deep(code) {
  background: var(--hover-bg-strong);
  padding: 1px 5px;
  border-radius: 4px;
  font-family: Consolas, Monaco, monospace;
  font-size: 0.82em;
  color: var(--primary-color);
}

.history-body :deep(strong) {
  color: var(--text-color);
  font-weight: 700;
}
</style>
