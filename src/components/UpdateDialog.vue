<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";

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
  updateInfo: UpdateInfo | null | undefined;
}>();

const emit = defineEmits<{
  (e: "dismiss"): void;
}>();

// ─── 状态 ───
const installing = ref(false);
const isMinimized = ref(true); // 默认从迷你气泡开始，不遮挡主界面
const downloadReady = ref(false);
const progress = ref<DownloadProgress | null>(null);
const errorMsg = ref("");

// 记录历史最大 total，防止服务器中途改变 Content-Length 导致百分比倒退
let stableTotal: number | null = null;

// 进度百分比
const progressPercent = computed(() => {
  if (!progress.value) return 0;
  // 优先用 Rust 端计算的百分比（但用 stableTotal 做上限保护）
  const p = progress.value;
  if (p.percent != null && stableTotal != null && p.total != null && p.total < stableTotal) {
    // total 变小了：用 stableTotal 重新计算，防止百分比倒退
    return Math.min((p.downloaded / stableTotal) * 100, 100);
  }
  if (p.percent != null) return Math.min(p.percent, 100);
  return -1;
});

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

// ─── 事件监听 ───
let unlistenProgress: (() => void) | null = null;
let unlistenDownloadReady: (() => void) | null = null;
let unlistenDownloadError: (() => void) | null = null;

onMounted(async () => {
  // 监听 Rust 后台下载进度（start_background_download 已在 Rust 端自动启动）
  unlistenProgress = await listen<DownloadProgress>("updater://progress", (event) => {
    const p = event.payload;
    // 更新稳定 total（只增不减）
    if (p.total != null) {
      if (stableTotal == null || p.total > stableTotal) {
        stableTotal = p.total;
      }
      // 如果 Rust 给的 total 比 stableTotal 小，补全一个基于 stableTotal 的 percent
      if (p.total < stableTotal) {
        p.percent = (p.downloaded / stableTotal) * 100;
        p.total = stableTotal;
      }
    }
    progress.value = p;
  });
  // 监听下载完成事件
  unlistenDownloadReady = await listen<UpdateInfo>("updater://download-ready", () => {
    downloadReady.value = true;
    console.log("后台更新下载完成，等待用户确认安装");
  });
  // 监听下载失败事件
  unlistenDownloadError = await listen<string>("updater://download-error", (event) => {
    errorMsg.value = String(event.payload);
    isMinimized.value = false; // 出错时弹出显示错误
  });
});

onUnmounted(() => {
  if (unlistenProgress) unlistenProgress();
  if (unlistenDownloadReady) unlistenDownloadReady();
  if (unlistenDownloadError) unlistenDownloadError();
});

// ─── 操作 ───

/** 安装已下载的待更新版本（download-ready 后调用） */
async function installPending() {
  installing.value = true;
  errorMsg.value = "";
  try {
    await invoke("install_pending_update");
    // install_pending_update 会重启应用，不会走到这里
  } catch (e: any) {
    errorMsg.value = String(e);
    installing.value = false;
    isMinimized.value = false;
  }
}

/** 手动使用旧版 install_update（Settings 页面手动检查场景） */
async function installNow() {
  installing.value = true;
  errorMsg.value = "";
  progress.value = { downloaded: 0, total: undefined, percent: 0 };

  try {
    await invoke("install_update");
  } catch (e: any) {
    errorMsg.value = String(e);
    installing.value = false;
    isMinimized.value = false;
  }
}

function dismiss() {
  if (installing.value) return;
  emit("dismiss");
}

function minimize() {
  isMinimized.value = true;
}

function restore() {
  isMinimized.value = false;
}

function openReleasePage() {
  openUrl("https://github.com/ISuuuu/Yuumi/releases/latest").catch((err: any) => {
    console.error("Failed to open release page:", err);
  });
}
</script>

<template>
  <!-- 迷你气泡：直接渲染，不经过 Teleport，永远属于 App.vue 的 DOM 树 -->
  <div v-if="updateInfo && isMinimized" class="update-mini-badge" @click="restore" :title="downloadReady ? '点击重启安装' : '点击展开更新进度'">
    <div class="mini-progress-ring">
      <svg class="ring-svg" viewBox="0 0 36 36">
        <path class="ring-bg" d="M18 2.0845 a 15.9155 15.9155 0 0 1 0 31.831 a 15.9155 15.9155 0 0 1 0 -31.831"/>
        <path
          class="ring-fill"
          :stroke-dasharray="`${downloadReady ? 100 : (progressPercent >= 0 ? progressPercent : 25)}, 100`"
          :class="{ 'ring-animate': progressPercent < 0 && !downloadReady }"
          d="M18 2.0845 a 15.9155 15.9155 0 0 1 0 31.831 a 15.9155 15.9155 0 0 1 0 -31.831"
        />
      </svg>
      <div class="mini-percent">
        {{ downloadReady ? '✓' : (progressPercent >= 0 ? Math.round(progressPercent) + '%' : '...') }}
      </div>
    </div>
    <div class="mini-info">
      <div class="mini-title">{{ downloadReady ? '更新就绪' : '正在后台更新' }}</div>
      <div class="mini-version">{{ downloadReady ? '点击重启安装' : `新版本 v${updateInfo.version}` }}</div>
    </div>
  </div>

  <!-- 全屏弹窗：用 Teleport 放到 body 最上层 -->
  <Teleport to="body" v-if="updateInfo && !isMinimized">
    <div class="update-overlay" @click.self="minimize">
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
            <!-- 下载完成 → 准备安装 -->
            <template v-if="downloadReady">
              <h2 class="update-title">更新已就绪</h2>
              <p class="update-subtitle">
                <span class="version-current">v{{ updateInfo.currentVersion }}</span>
                <svg class="version-arrow" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                  <line x1="5" y1="12" x2="19" y2="12"/>
                  <polyline points="12 5 19 12 12 19"/>
                </svg>
                <span class="version-new">v{{ updateInfo.version }}</span>
              </p>
            </template>
            <!-- 下载中 / 手动更新 -->
            <template v-else>
              <h2 class="update-title">发现新版本</h2>
              <p class="update-subtitle">
                <span class="version-current">v{{ updateInfo.currentVersion }}</span>
                <svg class="version-arrow" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                  <line x1="5" y1="12" x2="19" y2="12"/>
                  <polyline points="12 5 19 12 12 19"/>
                </svg>
                <span class="version-new">v{{ updateInfo.version }}</span>
              </p>
            </template>
          </div>
          <!-- 右上角关闭/最小化按钮 -->
          <button v-if="!installing" class="update-close" @click="downloadReady ? dismiss() : minimize()" :title="downloadReady ? '稍后' : '后台下载'">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line v-if="downloadReady" x1="18" y1="6" x2="6" y2="18"/><line v-if="downloadReady" x1="6" y1="6" x2="18" y2="18"/>
              <line v-else x1="5" y1="12" x2="19" y2="12"/>
            </svg>
          </button>
          <button v-else class="update-close" @click="minimize" title="后台运行">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="5" y1="12" x2="19" y2="12"/>
            </svg>
          </button>
        </div>

        <!-- 更新说明（仅手动更新/下载完成时显示） -->
        <div v-if="updateInfo.notes && (downloadReady || !progress)" class="update-notes">
          <div class="notes-label">更新说明</div>
          <pre class="notes-content">{{ updateInfo.notes }}</pre>
        </div>

        <!-- 下载进度（自动后台下载时显示） -->
        <div v-if="!downloadReady && progress" class="update-progress-section">
          <div class="progress-label">
            <span>后台下载中...</span>
            <span v-if="progress?.downloaded">{{ formatBytes(progress.downloaded) }}{{ progress.total ? ` / ${formatBytes(progress.total)}` : '' }}</span>
          </div>
          <div class="progress-bar-track">
            <div
              v-if="progressPercent >= 0"
              class="progress-bar-fill"
              :style="{ width: `${progressPercent}%` }"
            />
            <div v-else class="progress-bar-indeterminate" />
          </div>
          <p class="progress-hint">下载完成后将提示安装</p>
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
          <!-- 下载完成 → 立即重启 -->
          <template v-if="downloadReady">
            <button class="btn-dismiss" @click="dismiss">稍后</button>
            <button class="btn-install" @click="installPending">立即重启</button>
          </template>
          <!-- 手动更新（Settings 页面场景，无后台下载） -->
          <template v-else-if="!progress">
            <button v-if="errorMsg" class="btn-manual" @click="openReleasePage">浏览器下载</button>
            <button class="btn-dismiss" @click="dismiss">稍后提醒</button>
            <button class="btn-install" @click="installNow">{{ errorMsg ? '重试' : '立即更新' }}</button>
          </template>
          <!-- 下载中 -->
          <template v-else>
            <button class="btn-dismiss" @click="minimize">后台运行</button>
          </template>
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

/* ── 后台下载按钮 ── */
.progress-actions {
  display: flex;
  justify-content: center;
  margin-top: 14px;
}

.btn-minimize-text {
  background: transparent;
  border: 1px solid var(--border-color, rgba(255,255,255,0.12));
  color: var(--text-secondary, #999);
  padding: 6px 18px;
  border-radius: 6px;
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
}

.btn-minimize-text:hover {
  background: var(--bg-hover, rgba(255,255,255,0.06));
  color: var(--text-primary, #e8eaf0);
  border-color: var(--theme-color, #009faa);
}

.btn-minimize-text:active {
  transform: scale(0.97);
}

/* ── 迷你悬浮气泡 ── */
.update-mini-badge {
  position: fixed;
  bottom: 24px;
  right: 24px;
  z-index: 9999;
  display: flex;
  align-items: center;
  gap: 10px;
  background: var(--bg-card, #1e2030);
  border: 1px solid var(--border-color, rgba(255,255,255,0.12));
  border-radius: 30px;
  padding: 6px 16px 6px 8px;
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.35);
  cursor: pointer;
  user-select: none;
  animation: badge-in 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
  transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
}

.update-mini-badge:hover {
  transform: translateY(-3px);
  box-shadow: 0 14px 40px rgba(0, 0, 0, 0.45);
  border-color: var(--theme-color, #009faa);
}

.update-mini-badge:active {
  transform: translateY(-1px) scale(0.97);
}

@keyframes badge-in {
  from { opacity: 0; transform: translateY(24px) scale(0.85); }
  to { opacity: 1; transform: translateY(0) scale(1); }
}

.mini-progress-ring {
  position: relative;
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.ring-svg {
  width: 100%;
  height: 100%;
  transform: rotate(-90deg);
}

.ring-bg {
  fill: none;
  stroke: var(--bg-secondary, rgba(255,255,255,0.06));
  stroke-width: 3.5;
}

.ring-fill {
  fill: none;
  stroke: var(--theme-color, #009faa);
  stroke-width: 3.5;
  stroke-linecap: round;
  transition: stroke-dasharray 0.3s ease;
}

.ring-animate {
  animation: ring-dash 2s ease-in-out infinite;
}

@keyframes ring-dash {
  0% { stroke-dasharray: 1, 100; }
  50% { stroke-dasharray: 50, 100; }
  100% { stroke-dasharray: 1, 100; }
}

.mini-percent {
  position: absolute;
  font-size: 9px;
  font-weight: 600;
  color: var(--text-primary, #e8eaf0);
  font-family: monospace;
}

.mini-info {
  display: flex;
  flex-direction: column;
}

.mini-title {
  font-size: 11px;
  font-weight: 600;
  color: var(--text-primary, #e8eaf0);
}

.mini-version {
  font-size: 10px;
  color: var(--theme-color, #009faa);
  font-weight: 500;
  margin-top: 1px;
}
</style>
