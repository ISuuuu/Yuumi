<script setup lang="ts">
import { ref, watch, onMounted } from "vue";
import { useLcuStore } from "../store/lcuStore";
import { invoke } from "@tauri-apps/api/core";
import { fetchConfig } from "../api/lcu";

const store = useLcuStore();
const connectionDetails = ref<{ pid: number; port: number; token: string } | null>(null);
const lolPaths = ref<string[]>([]);
const selectedPath = ref<string>("");

async function loadConnectionDetails() {
  if (store.isConnected) {
    try {
      const details = await invoke<any>("get_lcu_connection_info");
      connectionDetails.value = details;
    } catch (e) {
      console.error("加载连接信息失败:", e);
    }
  } else {
    connectionDetails.value = null;
  }
}

async function loadPaths() {
  try {
    const cfg = await fetchConfig();
    lolPaths.value = cfg.General?.LolPath || [];
    if (lolPaths.value.length > 0) {
      selectedPath.value = lolPaths.value[0];
    }
  } catch (e) {
    console.error("加载客户端路径失败:", e);
  }
}

async function handleLaunchClient() {
  try {
    await invoke("launch_lol_client", { path: selectedPath.value || null });
  } catch (e: any) {
    console.warn("启动客户端失败:", e);
  }
}

onMounted(() => {
  loadPaths();
});

watch(() => store.isConnected, (connected) => {
  if (connected) {
    loadConnectionDetails();
  } else {
    connectionDetails.value = null;
  }
}, { immediate: true });
</script>

<template>
  <div class="home-view">
    <!-- 连接成功状态 -->
    <div v-if="store.isConnected" class="connection-success">
      <h1 class="status-title">{{ $t('home.connected') }}</h1>

      <div v-if="connectionDetails" class="details-box">
        <div class="detail-row">
          <span class="detail-name">PID =</span>
          <span class="detail-value">{{ connectionDetails.pid }}</span>
        </div>
        <div class="detail-row">
          <span class="detail-name">--app-port =</span>
          <span class="detail-value">{{ connectionDetails.port }}</span>
        </div>
        <div class="detail-row token-row">
          <span class="detail-name">--remoting-auth-token =</span>
          <span class="detail-value">{{ connectionDetails.token }}</span>
        </div>
      </div>
      <div v-else class="details-loading">
        {{ $t('home.fetchingDetails') }}
      </div>
    </div>

    <!-- 连接成功后猫咪移到窗体右下角 -->
    <img
      v-if="store.isConnected"
      src="/logo.png"
      class="connected-logo"
      alt="Yuumi"
    />

    <!-- 未连接状态 -->
    <div v-else class="connection-offline">
      <img src="/logo.png" class="offline-logo" alt="Yuumi" />
      <h1 class="status-title">{{ $t('home.notConnected') }}</h1>
      <p class="offline-desc">
        {{ $t('home.launchDesc') }}
      </p>
      <button class="launch-btn" @click="handleLaunchClient">
        <svg class="launch-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <polygon points="5 3 19 12 5 21 5 3"/>
        </svg>
        {{ $t('home.launchBtn') }}
      </button>
      <div v-if="lolPaths.length > 0" class="path-list">
        <span class="path-label">{{ $t('home.lolPathLabel') }}</span>
        <div
          v-for="(p, i) in lolPaths"
          :key="i"
          :class="['path-item', { active: selectedPath === p }]"
          @click="selectedPath = p"
        >
          <span class="path-radio" :class="{ checked: selectedPath === p }"></span>
          <span class="path-text">{{ p }}</span>
        </div>
      </div>
      <div class="loading-ring">
        <div class="ring-dot"></div>
        <span>{{ $t('home.waitingForLol') }}</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.home-view {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 100%;
  background-color: transparent;
  padding: 2.5rem 1.5rem;
  user-select: text;
}

.status-title {
  font-size: 1.8rem;
  font-weight: 800;
  color: var(--text-color);
  margin: 0 0 1.25rem;
  text-align: center;
  letter-spacing: 0.5px;
  text-shadow: 0 2px 10px rgba(0, 0, 0, 0.05);
}

/* 连接成功 */
.connection-success {
  display: flex;
  flex-direction: column;
  align-items: center;
  max-width: 600px;
  width: 100%;
  animation: fadeIn 0.4s ease-out;
}

.change-client-btn {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  padding: 8px 16px;
  border-radius: 8px;
  font-size: 0.82rem;
  font-weight: 600;
  color: var(--text-color);
  cursor: pointer;
  margin-bottom: 2rem;
  transition: all 0.25s ease;
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
}

.change-client-btn:hover {
  background: var(--card-bg-hover);
  color: var(--text-color);
  border-color: var(--primary-color);
  box-shadow: 0 4px 12px var(--primary-color-alpha-15);
  transform: translateY(-1px);
}

.btn-icon {
  width: 14px;
  height: 14px;
}

.details-box {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  font-family: ui-monospace, SFMono-Regular, SF Mono, Menlo, Consolas, monospace;
  font-size: 0.82rem;
  color: var(--text-muted);
  background: var(--card-bg);
  padding: 1.5rem;
  border-radius: var(--radius-lg);
  width: 100%;
  max-width: 500px;
  box-shadow: var(--shadow-md);
  border: 1px solid var(--border-color);
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
  transition: all 0.3s ease;
  text-align: center;
}

.details-box:hover {
  border-color: var(--primary-color-alpha-30);
  box-shadow: var(--shadow-lg);
}

.detail-row {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  white-space: nowrap;
}

.detail-name {
  color: var(--text-dimmed);
}

.detail-value {
  color: var(--text-color);
  font-weight: 600;
}

.token-row {
  word-break: break-all;
  white-space: normal;
  line-height: 1.4;
}

.details-loading {
  font-size: 0.85rem;
  color: var(--text-dimmed);
}

/* 离线状态 */
.connection-offline {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  max-width: 500px;
  animation: fadeIn 0.4s ease-out;
}

.offline-logo {
  width: 96px;
  height: 96px;
  object-fit: contain;
  margin-bottom: 1.5rem;
  filter: drop-shadow(0 8px 24px rgba(108, 92, 231, 0.25));
  animation: float 3s ease-in-out infinite;
}

.offline-desc {
  color: var(--text-muted);
  line-height: 1.6;
  font-size: 0.88rem;
  margin-bottom: 2rem;
}

.launch-btn {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  background: transparent;
  color: var(--primary-color);
  border: 1.5px solid var(--primary-color);
  padding: 10px 24px;
  border-radius: 8px;
  font-size: 0.88rem;
  font-weight: 600;
  cursor: pointer;
  margin-bottom: 1.5rem;
  transition: all 0.25s ease;
}
.launch-btn:hover {
  background: var(--primary-color-alpha-15);
  transform: translateY(-1px);
}
.launch-icon {
  width: 14px;
  height: 14px;
}
.path-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-bottom: 1.5rem;
  text-align: center;
}
.path-label {
  font-size: 0.78rem;
  color: var(--text-dimmed);
  margin-bottom: 2px;
}
.path-item {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 0.8rem;
  color: var(--text-muted);
  font-family: ui-monospace, SFMono-Regular, SF Mono, Menlo, Consolas, monospace;
  background: var(--card-bg);
  padding: 8px 12px;
  border-radius: 6px;
  border: 1px solid var(--border-color);
  cursor: pointer;
  transition: all 0.2s;
}
.path-item:hover {
  border-color: var(--primary-color-alpha-30);
}
.path-item.active {
  border-color: var(--primary-color);
  background: var(--primary-color-alpha-15);
}
.path-radio {
  width: 14px;
  height: 14px;
  border-radius: 50%;
  border: 2px solid var(--border-color-hover);
  flex-shrink: 0;
  transition: all 0.2s;
}
.path-radio.checked {
  border-color: var(--primary-color);
  background: var(--primary-color);
  box-shadow: inset 0 0 0 3px white;
}
.path-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.loading-ring {
  display: flex;
  align-items: center;
  gap: 12px;
  color: var(--text-muted);
  font-size: 0.8rem;
  background: var(--card-bg);
  padding: 10px 18px;
  border-radius: 20px;
  border: 1px solid var(--border-color);
  box-shadow: var(--shadow-sm);
}

.ring-dot {
  width: 14px;
  height: 14px;
  border: 2.5px solid rgba(0, 0, 0, 0.06);
  border-top-color: var(--primary-color);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes fadeIn {
  from { opacity: 0; transform: translateY(8px); }
  to { opacity: 1; transform: translateY(0); }
}

@keyframes float {
  0%, 100% { transform: translateY(0); }
  50% { transform: translateY(-6px); }
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.connected-logo {
  position: absolute;
  bottom: 40px;
  right: 40px;
  width: 120px;
  height: 120px;
  object-fit: contain;
  opacity: 0.85;
  filter: drop-shadow(0 4px 12px rgba(108, 92, 231, 0.2));
  animation: logo-enter 0.6s ease-out, logo-float 3s ease-in-out 1.2s infinite;
  transition: opacity 0.3s ease, filter 0.3s ease, transform 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
  cursor: default;
}

.connected-logo:hover {
  opacity: 1;
  filter: drop-shadow(0 6px 20px rgba(108, 92, 231, 0.4));
  animation: logo-enter 0.6s ease-out, logo-wiggle 0.5s ease-in-out infinite;
  transform-origin: center bottom;
}

.connected-logo:active {
  animation: logo-bounce 0.35s ease;
  filter: drop-shadow(0 2px 8px rgba(108, 92, 231, 0.5));
}

@keyframes logo-enter {
  from {
    opacity: 0;
    transform: translate(16px, 16px);
  }
  to {
    opacity: 0.85;
    transform: translate(0, 0);
  }
}

@keyframes logo-float {
  0%, 100% { transform: translateY(0); }
  50% { transform: translateY(-8px); }
}

@keyframes logo-wiggle {
  0%, 100% { transform: rotate(0deg); }
  25% { transform: rotate(-6deg); }
  75% { transform: rotate(6deg); }
}

@keyframes logo-bounce {
  0% { transform: scale(1); }
  30% { transform: scale(0.85); }
  60% { transform: scale(1.12); }
  100% { transform: scale(1); }
}
</style>
