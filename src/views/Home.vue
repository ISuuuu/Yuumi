<script setup lang="ts">
import { ref, watch } from "vue";
import { useLcuStore } from "../store/lcuStore";
import { invoke } from "@tauri-apps/api/core";

const store = useLcuStore();
const connectionDetails = ref<{ pid: number; port: number; token: string } | null>(null);

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

watch(() => store.isConnected, (connected) => {
  if (connected) {
    loadConnectionDetails();
  } else {
    connectionDetails.value = null;
  }
}, { immediate: true });

function handleChangeClient() {
  alert("🔍 正在扫描系统中的英雄联盟进程，请确保客户端已运行...");
  loadConnectionDetails();
}
</script>

<template>
  <div class="home-view">
    <!-- 连接成功状态 -->
    <div v-if="store.isConnected" class="connection-success">
      <h1 class="status-title">客户端连接成功 🎉</h1>
      
      <button class="change-client-btn" @click="handleChangeClient">
        <svg class="btn-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <rect x="3" y="3" width="18" height="18" rx="2" ry="2"/>
          <line x1="9" y1="3" x2="9" y2="21"/>
        </svg>
        更改连接的客户端
      </button>

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
        正在拉取 LCU 凭证细节...
      </div>
    </div>

    <!-- 未连接状态 -->
    <div v-else class="connection-offline">
      <div class="offline-icon">❌</div>
      <h1 class="status-title">客户端未连接</h1>
      <p class="offline-desc">
        请先启动国服或外服的《英雄联盟》客户端。<br>
        系统将通过底层进程侦测服务自动捕获凭证并建立长连接。
      </p>
      <div class="loading-ring">
        <div class="ring-dot"></div>
        <span>正在等待 LeagueClient 启动...</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.home-view {
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
  background: rgba(255, 255, 255, 0.5);
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
  background: rgba(255, 255, 255, 0.95);
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
}

.details-box:hover {
  border-color: var(--primary-color-alpha-30);
  box-shadow: var(--shadow-lg);
}

.detail-row {
  display: flex;
  align-items: center;
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
  display: block;
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

.offline-icon {
  font-size: 3rem;
  margin-bottom: 1.25rem;
  filter: drop-shadow(0 4px 10px rgba(0, 0, 0, 0.05));
  animation: float 3s ease-in-out infinite;
}

.offline-desc {
  color: var(--text-muted);
  line-height: 1.6;
  font-size: 0.88rem;
  margin-bottom: 2rem;
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
</style>
