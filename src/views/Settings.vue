<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useLcuStore } from "../store/lcuStore";

const store = useLcuStore();
const loading = ref(false);
const result = ref("");

async function setProfileIcon() {
  const iconId = prompt("输入头像 Icon ID:");
  if (!iconId) return;
  loading.value = true;
  try {
    await invoke("call_lcu_api", {
      method: "PUT",
      path: "/lol-summoner/v1/current-summoner/icon",
      body: { profileIconId: parseInt(iconId) },
    });
    result.value = "头像已更新";
  } catch (e: any) {
    result.value = "❌ " + e.toString();
  } finally {
    loading.value = false;
  }
}

async function setStatus() {
  const status = prompt("输入新的状态签名:");
  if (status === null) return;
  loading.value = true;
  try {
    await invoke("call_lcu_api", {
      method: "PUT",
      path: "/lol-chat/v1/me",
      body: { statusMessage: status },
    });
    result.value = "状态签名已更新";
  } catch (e: any) {
    result.value = "❌ " + e.toString();
  } finally {
    loading.value = false;
  }
}

async function setAvailability(avail: string) {
  loading.value = true;
  try {
    await invoke("call_lcu_api", {
      method: "PUT",
      path: "/lol-chat/v1/me",
      body: { availability: avail },
    });
    result.value = `状态已设为: ${avail}`;
  } catch (e: any) {
    result.value = "❌ " + e.toString();
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <div class="settings">
    <h2>设置</h2>

    <section class="section">
      <h3>个人信息</h3>
      <div class="btn-group">
        <button @click="setProfileIcon" :disabled="!store.isConnected || loading">修改头像</button>
        <button @click="setStatus" :disabled="!store.isConnected || loading">修改签名</button>
      </div>
    </section>

    <section class="section">
      <h3>在线状态</h3>
      <div class="btn-group">
        <button @click="setAvailability('online')" :disabled="!store.isConnected || loading">在线</button>
        <button @click="setAvailability('away')" :disabled="!store.isConnected || loading">离开</button>
        <button @click="setAvailability('offline')" :disabled="!store.isConnected || loading">隐身</button>
      </div>
    </section>

    <div v-if="result" class="result">{{ result }}</div>

    <section class="section">
      <h3>自动化功能</h3>
      <p class="hint">配置自动选人/禁人/接受匹配等功能请修改配置文件：</p>
      <code class="path">%APPDATA%/Yuumi/config.json</code>
      <p class="hint">配置热更新将在后续版本支持。</p>
    </section>
  </div>
</template>

<style scoped>
.settings { padding: 1rem; }
.section {
  background: #f9f9f9; border-radius: 8px; padding: 16px; margin-bottom: 1rem;
}
.section h3 { margin: 0 0 12px; }
.btn-group { display: flex; gap: 8px; flex-wrap: wrap; }
.btn-group button {
  padding: 8px 16px; border: 1px solid #ddd; border-radius: 6px;
  background: #fff; cursor: pointer; transition: all 0.15s;
}
.btn-group button:hover:not(:disabled) { border-color: #6c5ce7; color: #6c5ce7; }
.btn-group button:disabled { opacity: 0.5; cursor: not-allowed; }
.result { margin-top: 12px; padding: 8px; border-radius: 4px; font-size: 0.9rem; background: #e8f5e9; color: #2e7d32; }
.hint { color: #888; font-size: 0.85rem; margin: 4px 0; }
.path { display: block; background: #eee; padding: 6px 10px; border-radius: 4px; font-size: 0.8rem; margin: 8px 0; }
</style>
