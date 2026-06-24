<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useLcuStore, initLcuListeners } from "./store/lcuStore";
import Home from "./views/Home.vue";
import Career from "./views/Career.vue";
import Settings from "./views/Settings.vue";
import Tools from "./views/Tools.vue";

const store = useLcuStore();
const currentPage = ref("home");

onMounted(() => {
  initLcuListeners();
});

function navigate(page: string) {
  currentPage.value = page;
}
</script>

<template>
  <div class="app">
    <!-- 顶部导航栏 -->
    <nav class="navbar">
      <span class="brand" @click="navigate('home')">Yuumi</span>
      <div class="nav-links">
        <a :class="{ active: currentPage === 'home' }" @click="navigate('home')">首页</a>
        <a :class="{ active: currentPage === 'career' }" @click="navigate('career')">战绩</a>
        <a :class="{ active: currentPage === 'settings' }" @click="navigate('settings')">设置</a>
        <a :class="{ active: currentPage === 'tools' }" @click="navigate('tools')">工具</a>
      </div>
      <div class="status-chip">
        <span :class="['dot', store.isConnected ? 'online' : 'offline']" />
        <span>{{ store.isConnected ? store.gamePhase : "离线" }}</span>
      </div>
    </nav>

    <!-- 页面内容 -->
    <main class="content">
      <Home v-if="currentPage === 'home'" @navigate="navigate" />
      <Career v-else-if="currentPage === 'career'" />
      <Settings v-else-if="currentPage === 'settings'" />
      <Tools v-else-if="currentPage === 'tools'" />
    </main>
  </div>
</template>

<style>
* { box-sizing: border-box; }
body { margin: 0; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; }
</style>

<style scoped>
.app { display: flex; flex-direction: column; height: 100vh; }

.navbar {
  display: flex; align-items: center; gap: 16px;
  padding: 0 16px; height: 48px;
  background: #fff; border-bottom: 1px solid #eee;
}
.brand { font-weight: 700; font-size: 1.1rem; color: #6c5ce7; cursor: pointer; }
.nav-links { display: flex; gap: 4px; margin-left: 24px; }
.nav-links a {
  padding: 6px 12px; border-radius: 6px; cursor: pointer;
  font-size: 0.9rem; color: #555; transition: background 0.15s;
}
.nav-links a:hover { background: #f0f0f0; }
.nav-links a.active { background: #6c5ce7; color: #fff; }
.status-chip {
  margin-left: auto; display: flex; align-items: center; gap: 6px;
  font-size: 0.8rem; color: #888;
}
.dot { width: 8px; height: 8px; border-radius: 50%; }
.dot.online { background: #2ecc71; }
.dot.offline { background: #e74c3c; }

.content { flex: 1; overflow-y: auto; }
</style>
