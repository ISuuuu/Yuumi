<script setup lang="ts">
import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-opener";

export interface UpdateInfo {
  version: string;
  currentVersion: string;
  notes?: string;
  pubDate?: string;
}

interface DownloadProgress {
  downloaded: number;
  total?: number;
  percent?: number;
}

const { updateInfo } = defineProps<{
  updateInfo: UpdateInfo;
}>();

const emit = defineEmits<{
  (e: "dismiss"): void;
}>();

const installing = ref(false);
const progress = ref<DownloadProgress | null>(null);
const errorMsg = ref("");

// 进度百分比（用于进度条显示）
const progressPercent = computed(() => {
  if (!progress.value) return 0;
  if (progress.value.percent != null) return Math.min(progress.value.percent, 100);
  // 没有 total 时，模拟不确定进度条
  return -1;
});

// 格式化已下载大小
function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

// 监听下载进度事件
let unlistenProgress: (() => void) | null = null;

async function startInstall() {
  installing.value = true;
  errorMsg.value = "";
  progress.value = { downloaded: 0, total: undefined, percent: 0 };

  // 注册进度事件监听
  unlistenProgress = await listen<DownloadProgress>("updater://progress", (event) => {
    progress.value = event.payload;
  });

  try {
    await invoke("install_update");
    // install_update 会重启应用，不会走到这里
  } catch (e: any) {
    errorMsg.value = String(e);
    installing.value = false;
    if (unlistenProgress) {
      unlistenProgress();
      unlistenProgress = null;
    }
  }
}

function dismiss() {
  if (installing.value) return; // 安装过程中不允许关闭
  emit("dismiss");
}

function openReleasePage() {
  open("https://github.com/ISuuuu/Yuumi/releases/latest").catch((err) => {
    console.error("Failed to open release page:", err);
  });
}
</script>

<template>
  <Teleport to="body">
    <div class="update-overlay" @click.self="dismiss">
      <div class="update-dialog">
        <!-- 头部 -->
        <div class="update-header">
          <div class="update-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
              <polyline points="17 8 12 3 7 8"/>
              <line x1="12" y1="3" x2="12" y2="15"/>
            </svg>
          </div>
          <div class="update-title-group">
            <h2 class="update-title">发现新版本</h2>
            <p class="update-subtitle">
              <span class="version-current">v{{ updateInfo.currentVersion }}</span>
              <svg class="version-arrow" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                <line x1="5" y1="12" x2="19" y2="12"/>
                <polyline points="12 5 19 12 12 19"/>
              </svg>
              <span class="version-new">v{{ updateInfo.version }}</span>
            </p>
          </div>
          <button v-if="!installing" class="update-close" @click="dismiss" title="稍后提醒">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
            </svg>
          </button>
        </div>

        <!-- 更新说明 -->
        <div v-if="updateInfo.notes && !installing" class="update-notes">
          <div class="notes-label">更新说明</div>
          <pre class="notes-content">{{ updateInfo.notes }}</pre>
        </div>

        <!-- 下载进度 -->
        <div v-if="installing" class="update-progress-section">
          <div class="progress-label">
            <span>{{ progress?.percent != null ? '下载中...' : '准备中...' }}</span>
            <span v-if="progress?.downloaded">{{ formatBytes(progress.downloaded) }}{{ progress.total ? ` / ${formatBytes(progress.total)}` : '' }}</span>
          </div>
          <div class="progress-bar-track">
            <div
              v-if="progressPercent >= 0"
              class="progress-bar-fill"
              :style="{ width: `${progressPercent}%` }"
            />
            <!-- 不确定进度动画 -->
            <div v-else class="progress-bar-indeterminate" />
          </div>
          <p class="progress-hint">下载完成后将自动安装并重启应用</p>
        </div>

        <!-- 错误提示 -->
        <div v-if="errorMsg" class="update-error">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14">
            <circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/>
          </svg>
          {{ errorMsg }}
        </div>

        <!-- 按钮区 -->
        <div v-if="!installing" class="update-actions">
          <button v-if="errorMsg" class="btn-manual" @click="openReleasePage">浏览器下载</button>
          <button class="btn-dismiss" @click="dismiss">稍后提醒</button>
          <button class="btn-install" @click="startInstall">{{ errorMsg ? '重试' : '立即更新' }}</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.update-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.55);
  backdrop-filter: blur(4px);
  -webkit-backdrop-filter: blur(4px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
  animation: overlay-in 0.2s ease;
}

@keyframes overlay-in {
  from { opacity: 0; }
  to   { opacity: 1; }
}

.update-dialog {
  background: var(--bg-card, #1e2030);
  border: 1px solid var(--border-color, rgba(255,255,255,0.1));
  border-radius: 16px;
  padding: 28px;
  width: 420px;
  max-width: calc(100vw - 48px);
  box-shadow: 0 24px 64px rgba(0, 0, 0, 0.5);
  animation: dialog-in 0.25s cubic-bezier(0.34, 1.56, 0.64, 1);
}

@keyframes dialog-in {
  from { opacity: 0; transform: translateY(16px) scale(0.96); }
  to   { opacity: 1; transform: translateY(0) scale(1); }
}

/* ── 头部 ── */
.update-header {
  display: flex;
  align-items: flex-start;
  gap: 14px;
  margin-bottom: 20px;
}

.update-icon {
  width: 44px;
  height: 44px;
  border-radius: 12px;
  background: linear-gradient(135deg, var(--theme-color, #009faa), color-mix(in srgb, var(--theme-color, #009faa) 70%, #fff));
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  box-shadow: 0 4px 16px color-mix(in srgb, var(--theme-color, #009faa) 40%, transparent);
}

.update-icon svg {
  width: 22px;
  height: 22px;
  stroke: #fff;
}

.update-title-group {
  flex: 1;
}

.update-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--text-primary, #e8eaf0);
  margin: 0 0 5px;
}

.update-subtitle {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  margin: 0;
}

.version-current {
  color: var(--text-secondary, #888);
  text-decoration: line-through;
  font-family: monospace;
}

.version-arrow {
  width: 14px;
  height: 14px;
  stroke: var(--theme-color, #009faa);
}

.version-new {
  color: var(--theme-color, #009faa);
  font-weight: 600;
  font-family: monospace;
  font-size: 14px;
}

.update-close {
  width: 28px;
  height: 28px;
  border-radius: 8px;
  border: none;
  background: transparent;
  color: var(--text-secondary, #888);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background 0.15s, color 0.15s;
  flex-shrink: 0;
}

.update-close:hover {
  background: var(--bg-hover, rgba(255,255,255,0.07));
  color: var(--text-primary, #e8eaf0);
}

.update-close svg {
  width: 14px;
  height: 14px;
}

/* ── 更新说明 ── */
.update-notes {
  background: var(--bg-secondary, rgba(255,255,255,0.04));
  border: 1px solid var(--border-color, rgba(255,255,255,0.08));
  border-radius: 10px;
  padding: 12px 14px;
  margin-bottom: 20px;
}

.notes-label {
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--text-tertiary, #666);
  margin-bottom: 8px;
}

.notes-content {
  font-size: 13px;
  color: var(--text-secondary, #aaa);
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-word;
  margin: 0;
  max-height: 120px;
  overflow-y: auto;
  font-family: inherit;
}

/* ── 进度 ── */
.update-progress-section {
  margin-bottom: 4px;
}

.progress-label {
  display: flex;
  justify-content: space-between;
  font-size: 12px;
  color: var(--text-secondary, #888);
  margin-bottom: 8px;
}

.progress-bar-track {
  height: 6px;
  background: var(--bg-secondary, rgba(255,255,255,0.08));
  border-radius: 999px;
  overflow: hidden;
  position: relative;
}

.progress-bar-fill {
  height: 100%;
  background: linear-gradient(90deg, var(--theme-color, #009faa), color-mix(in srgb, var(--theme-color, #009faa) 60%, #fff));
  border-radius: 999px;
  transition: width 0.3s ease;
}

.progress-bar-indeterminate {
  position: absolute;
  inset: 0;
  background: linear-gradient(90deg,
    transparent 0%,
    var(--theme-color, #009faa) 40%,
    transparent 100%
  );
  animation: indeterminate 1.4s infinite ease-in-out;
}

@keyframes indeterminate {
  0%   { transform: translateX(-100%); }
  100% { transform: translateX(200%); }
}

.progress-hint {
  font-size: 11px;
  color: var(--text-tertiary, #555);
  text-align: center;
  margin: 10px 0 0;
}

/* ── 错误 ── */
.update-error {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: #e06c75;
  background: rgba(224, 108, 117, 0.1);
  border: 1px solid rgba(224, 108, 117, 0.2);
  border-radius: 8px;
  padding: 8px 12px;
  margin-bottom: 16px;
}

/* ── 按钮 ── */
.update-actions {
  display: flex;
  gap: 10px;
  justify-content: flex-end;
  margin-top: 4px;
}

.btn-manual {
  padding: 8px 18px;
  border-radius: 8px;
  border: 1px solid var(--theme-color, #009faa);
  background: transparent;
  color: var(--theme-color, #009faa);
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}

.btn-manual:hover {
  background: color-mix(in srgb, var(--theme-color, #009faa) 10%, transparent);
}

.btn-dismiss {
  padding: 8px 18px;
  border-radius: 8px;
  border: 1px solid var(--border-color, rgba(255,255,255,0.1));
  background: transparent;
  color: var(--text-secondary, #888);
  font-size: 13px;
  cursor: pointer;
  transition: background 0.15s, color 0.15s, border-color 0.15s;
}

.btn-dismiss:hover {
  background: var(--bg-hover, rgba(255,255,255,0.06));
  color: var(--text-primary, #e8eaf0);
}

.btn-install {
  padding: 8px 22px;
  border-radius: 8px;
  border: none;
  background: var(--theme-color, #009faa);
  color: #fff;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  transition: opacity 0.15s, transform 0.1s;
}

.btn-install:hover {
  opacity: 0.88;
  transform: translateY(-1px);
}

.btn-install:active {
  transform: translateY(0);
}
</style>
