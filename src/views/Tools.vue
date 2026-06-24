<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

const lobbyName = ref("训练营");
const lobbyPassword = ref("");
const result = ref("");
const loading = ref(false);

async function createLobby() {
  loading.value = true;
  result.value = "";
  try {
    const msg = await invoke<string>("create_5v5_practice_lobby", {
      params: {
        lobbyName: lobbyName.value,
        password: lobbyPassword.value || null,
      },
    });
    result.value = msg;
  } catch (e: any) {
    result.value = "❌ " + e.toString();
  } finally {
    loading.value = false;
  }
}

async function aramReroll() {
  loading.value = true;
  result.value = "";
  try {
    const msg = await invoke<string>("aram_reroll_and_swap_back");
    result.value = msg;
  } catch (e: any) {
    result.value = "❌ " + e.toString();
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <div class="settings">
    <h2>工具箱</h2>

    <!-- 创建 5v5 训练营 -->
    <section class="tool-section">
      <h3>创建 5v5 训练营</h3>
      <div class="form-row">
        <label>房间名</label>
        <input v-model="lobbyName" placeholder="房间名称" />
      </div>
      <div class="form-row">
        <label>密码</label>
        <input v-model="lobbyPassword" type="password" placeholder="可选" />
      </div>
      <button @click="createLobby" :disabled="loading">创建房间</button>
    </section>

    <!-- 大乱斗摇号换回 -->
    <section class="tool-section">
      <h3>大乱斗摇号换回</h3>
      <p class="desc">先摇号再换回原英雄，适用于 ARAM</p>
      <button @click="aramReroll" :disabled="loading">摇号换回</button>
    </section>

    <div v-if="result" :class="['result', result.startsWith('❌') ? 'err' : 'ok']">
      {{ result }}
    </div>
  </div>
</template>

<style scoped>
.settings { padding: 1rem; }
.tool-section {
  background: #f9f9f9; border-radius: 8px; padding: 16px; margin-bottom: 1rem;
}
.tool-section h3 { margin: 0 0 8px; }
.desc { color: #888; font-size: 0.85rem; margin-bottom: 8px; }
.form-row { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; }
.form-row label { min-width: 60px; font-size: 0.9rem; }
.form-row input { padding: 6px 10px; border: 1px solid #ddd; border-radius: 4px; flex: 1; }
.result { margin-top: 12px; padding: 8px; border-radius: 4px; font-size: 0.9rem; }
.result.ok { background: #e8f5e9; color: #2e7d32; }
.result.err { background: #fbe9e7; color: #c62828; }
</style>
