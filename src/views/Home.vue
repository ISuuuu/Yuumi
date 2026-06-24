<script setup lang="ts">
import { useLcuStore } from "../store/lcuStore";

const store = useLcuStore();
defineEmits<{ navigate: [page: string] }>();
</script>

<template>
  <div class="home">
    <div class="hero">
      <h1>Yuumi</h1>
      <p class="subtitle">英雄联盟辅助工具</p>
    </div>

    <div class="status-panel">
      <div class="status-row">
        <span :class="['dot', store.isConnected ? 'online' : 'offline']" />
        <span>LCU {{ store.isConnected ? "已连接" : "未连接" }}</span>
      </div>
      <div v-if="store.isConnected" class="status-row">
        <span class="dot ws" :class="store.wsConnected ? 'online' : 'offline'" />
        <span>WebSocket {{ store.wsConnected ? "已连接" : "未连接" }}</span>
      </div>
      <div v-if="store.isConnected" class="status-row">
        <span class="label">游戏阶段</span>
        <span class="phase">{{ store.gamePhase }}</span>
      </div>
    </div>

    <div class="quick-nav">
      <button class="nav-btn" @click="$emit('navigate', 'career')">📊 生涯战绩</button>
      <button class="nav-btn" @click="$emit('navigate', 'settings')">⚙️ 设置</button>
      <button class="nav-btn" @click="$emit('navigate', 'tools')">🛠 工具箱</button>
    </div>
  </div>
</template>

<style scoped>
.home { padding: 2rem 1rem; text-align: center; }
.hero h1 { font-size: 2.5rem; margin: 0; color: #6c5ce7; }
.subtitle { color: #888; margin-top: 4px; }

.status-panel {
  display: inline-flex; flex-direction: column; gap: 8px;
  background: #f9f9f9; border-radius: 12px; padding: 16px 24px; margin: 1.5rem 0;
}
.status-row { display: flex; align-items: center; gap: 8px; font-size: 0.95rem; }
.dot { width: 10px; height: 10px; border-radius: 50%; }
.dot.online { background: #2ecc71; }
.dot.offline { background: #e74c3c; }
.label { color: #888; }
.phase { color: #6c5ce7; font-weight: 600; }

.quick-nav { display: flex; gap: 12px; justify-content: center; margin-top: 1.5rem; }
.nav-btn {
  padding: 12px 24px; border: none; border-radius: 8px;
  background: #f0f0f0; cursor: pointer; font-size: 1rem;
  transition: background 0.2s;
}
.nav-btn:hover { background: #e0e0e0; }
</style>
