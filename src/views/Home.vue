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
  background-color: #fafbfc;
  padding: 2rem;
  user-select: text;
}

.status-title {
  font-size: 2.2rem;
  font-weight: 800;
  color: #1a1a1a;
  margin: 0 0 1.5rem;
  text-align: center;
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
  background: white;
  border: 1px solid #dcdfe6;
  padding: 8px 16px;
  border-radius: 6px;
  font-size: 0.88rem;
  color: #606266;
  cursor: pointer;
  margin-bottom: 2rem;
  transition: all 0.2s;
  box-shadow: 0 1px 3px rgba(0,0,0,0.02);
}

.change-client-btn:hover {
  background: #f5f7fa;
  color: #1a73e8;
  border-color: #c0c4cc;
}

.btn-icon {
  width: 16px;
  height: 16px;
}

.details-box {
  display: flex;
  flex-direction: column;
  gap: 8px;
  font-family: ui-monospace, SFMono-Regular, SF Mono, Menlo, Consolas, Liberation Mono, monospace;
  font-size: 0.9rem;
  color: #8c8c8c;
  background: #fafafa;
  padding: 1.5rem 2rem;
  border-radius: 8px;
  width: 100%;
  max-width: 500px;
  box-shadow: inset 0 1px 2px rgba(0,0,0,0.02);
  border: 1px solid #f0f0f0;
}

.detail-row {
  display: flex;
  justify-content: center;
  align-items: center;
  gap: 6px;
  white-space: nowrap;
}

.detail-name {
  color: #888888;
}

.detail-value {
  color: #888888;
  font-weight: 500;
}

.token-row {
  word-break: break-all;
  white-space: normal;
}

.details-loading {
  font-size: 0.9rem;
  color: #909399;
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
  font-size: 4rem;
  margin-bottom: 1rem;
}

.offline-desc {
  color: #606266;
  line-height: 1.6;
  font-size: 0.95rem;
  margin-bottom: 2rem;
}

.loading-ring {
  display: flex;
  align-items: center;
  gap: 10px;
  color: #909399;
  font-size: 0.88rem;
}

.ring-dot {
  width: 8px;
  height: 8px;
  background-color: #909399;
  border-radius: 50%;
  animation: pulse 1.2s infinite ease-in-out;
}

@keyframes fadeIn {
  from { opacity: 0; transform: translateY(10px); }
  to { opacity: 1; transform: translateY(0); }
}

@keyframes pulse {
  0%, 100% { transform: scale(0.8); opacity: 0.5; }
  50% { transform: scale(1.3); opacity: 1; }
}
</style>
