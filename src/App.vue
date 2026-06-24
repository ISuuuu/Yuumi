<script setup lang="ts">
import { ref, onMounted, watch, computed, provide } from "vue";
import { useLcuStore, initLcuListeners } from "./store/lcuStore";
import { fetchCurrentSummoner, lcuRequest, fetchConfig } from "./api/lcu";
import { updateThemeColor } from "./utils/theme";
import type { SummonerDisplay } from "./api/lcu";
import Home from "./views/Home.vue";
import Career from "./views/Career.vue";
import Search from "./views/Search.vue";
import GameInfo from "./views/GameInfo.vue";
import TFT from "./views/TFT.vue";
import Settings from "./views/Settings.vue";
import Tools from "./views/Tools.vue";
import LcuImage from "./components/LcuImage.vue";
import OpggModal from "./components/OpggModal.vue";

const store = useLcuStore();
const currentPage = ref("home");
const showOpgg = ref(false);
const isSidebarExpanded = ref(false);
const summoner = ref<SummonerDisplay | null>(null);
const platformId = ref("");

// 用于 Career → Search 跳转的共享状态
const navigateSearchPayload = ref<{ name: string; gameId: number | null } | null>(null);

provide("navigateSearchPayload", navigateSearchPayload);

// 供子组件跳转页面
function navigateTo(page: string) {
  currentPage.value = page;
}
provide("navigateTo", navigateTo);

const PLATFORM_MAP: Record<string, string> = {
  HN1: "艾欧尼亚", HN2: "祖安", HN3: "诺克萨斯", HN4: "班德尔城", HN5: "皮尔特沃夫",
  HN6: "战争学院", HN7: "巨神峰", HN8: "雷瑟守备", HN9: "裁决之地", HN10: "黑色玫瑰",
  HN11: "暗影岛", HN12: "钢铁烈阳", HN13: "水晶之痕", HN14: "均衡教派", HN15: "影流",
  HN16: "守望之海", HN17: "征服之海", HN18: "卡拉曼达", HN19: "皮城警备",
  WT1: "比尔吉沃特", WT2: "德玛西亚", WT3: "弗雷尔卓德", WT4: "无畏先锋", WT5: "恕瑞玛", WT6: "扭曲丛林",
  HN20: "巨龙之巢", BGP1: "男爵领域", BGP2: "教育网", BGP3: "巅峰战区",
};

const regionName = computed(() => {
  return PLATFORM_MAP[platformId.value] || platformId.value || "艾欧尼亚";
});

onMounted(() => {
  initLcuListeners();
});

function navigate(page: string) {
  currentPage.value = page;
}

function toggleSidebar() {
  isSidebarExpanded.value = !isSidebarExpanded.value;
}

async function loadLcuState() {
  if (store.isConnected) {
    try {
      // 1. 获取当前召唤师数据
      summoner.value = await fetchCurrentSummoner();
      
      // 2. 获取大区平台
      const resp = await lcuRequest<any>("GET", "/lol-platform-config/v1/namespaces/LoginPlatformLocalization/platformId");
      if (resp.success && resp.data) {
        platformId.value = resp.data;
      }

      // 3. 核心修复：同步拉取当前 LCU 对局状态，以便在已经开始的对局中启动软件时能自动跳转
      const phaseResp = await lcuRequest<string>("GET", "/lol-gameflow/v1/gameflow-phase");
      if (phaseResp.success && phaseResp.data) {
        store.setGamePhase(phaseResp.data);
      }

      // 4. 同步拉取对局 Session
      if (store.gamePhase === "ChampSelect") {
        const sessionResp = await lcuRequest<any>("GET", "/lol-champ-select/v1/session");
        if (sessionResp.success && sessionResp.data) {
          store.setChampSelectSession(sessionResp.data);
        }
      }

      // 5. 初始化加载主题色并应用到全局 CSS 变量中
      const cfg = await fetchConfig();
      if (cfg && cfg.Personalization && cfg.Personalization.ThemeColor) {
        updateThemeColor(cfg.Personalization.ThemeColor);
      }
    } catch (e) {
      console.error("加载 LCU 基础状态失败:", e);
    }
  } else {
    summoner.value = null;
    platformId.value = "";
  }
}

watch(() => store.isConnected, (connected) => {
  if (connected) {
    loadLcuState();
  } else {
    summoner.value = null;
    platformId.value = "";
  }
}, { immediate: true });

// 监听 Career → Search 跳转
watch(navigateSearchPayload, (payload) => {
  if (payload && payload.gameId !== null) {
    currentPage.value = "search";
  }
});

// 进入选人/游戏中时自动跳转到对局信息页
watch(() => store.gamePhase, (phase: string) => {
  if (phase === "ChampSelect" || phase === "InProgress") {
    currentPage.value = "gameinfo";
  }
});

function handleReconnect() {
  initLcuListeners();
  alert("🔄 已成功重置并重新初始化 LCU 监听服务。");
}
</script>

<template>
  <div class="app-layout">
    <!-- 左侧导航栏 -->
    <aside :class="['sidebar', isSidebarExpanded ? 'expanded' : 'collapsed']">
      <!-- 顶部折叠按钮 -->
      <div class="sidebar-header" @click="toggleSidebar">
        <svg class="hamburger-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M3 12h18M3 6h18M3 18h18" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
      </div>

      <!-- 中间功能导航 -->
      <nav class="sidebar-nav">
        <div :class="['nav-item', { active: currentPage === 'home' }]" @click="navigate('home')" title="启动页">
          <span class="nav-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>
              <polyline points="9 22 9 12 15 12 15 22"/>
            </svg>
          </span>
          <span class="nav-label">启动页</span>
        </div>

        <div :class="['nav-item', { active: currentPage === 'career' }]" @click="navigate('career')" title="生涯">
          <span class="nav-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/>
              <circle cx="12" cy="7" r="4"/>
            </svg>
          </span>
          <span class="nav-label">生涯</span>
        </div>

        <div :class="['nav-item', { active: currentPage === 'search' }]" @click="navigate('search')" title="战绩查询">
          <span class="nav-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <circle cx="11" cy="11" r="8"/>
              <line x1="21" y1="21" x2="16.65" y2="16.65"/>
            </svg>
          </span>
          <span class="nav-label">战绩查询</span>
        </div>

        <div :class="['nav-item', { active: currentPage === 'gameinfo' }]" @click="navigate('gameinfo')" title="对局信息">
          <span class="nav-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <rect x="2" y="6" width="20" height="12" rx="3"/>
              <path d="M6 12h4M8 10v4M15 11h.01M18 13h.01"/>
            </svg>
          </span>
          <span class="nav-label">对局信息</span>
        </div>

        <div :class="['nav-item', { active: currentPage === 'tft' }]" @click="navigate('tft')" title="Teamfight Tactics">
          <span class="nav-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <polygon points="12 2 2 7 12 12 22 7 12 2"/>
              <polyline points="2 17 12 22 22 17"/>
              <polyline points="2 12 12 17 22 12"/>
            </svg>
          </span>
          <span class="nav-label">Teamfight Tactics</span>
        </div>

        <div :class="['nav-item', { active: currentPage === 'tools' }]" @click="navigate('tools')" title="其他功能">
          <span class="nav-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"/>
            </svg>
          </span>
          <span class="nav-label">其他功能</span>
        </div>
      </nav>

      <!-- 底部附加操作 -->
      <div class="sidebar-bottom">
        <div class="nav-item" @click="showOpgg = true" title="OP.GG">
          <span class="nav-icon text-icon">OP</span>
          <span class="nav-label">OP.GG</span>
        </div>

        <div class="nav-item" @click="handleReconnect" title="修复无限加载">
          <span class="nav-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M23 4v6h-6M1 20v-6h6M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>
            </svg>
          </span>
          <span class="nav-label">修复无限加载</span>
        </div>

        <div :class="['nav-item', { active: currentPage === 'notice' }]" @click="navigate('notice')" title="公告">
          <span class="nav-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9M13.73 21a2 2 0 0 1-3.46 0"/>
            </svg>
          </span>
          <span class="nav-label">公告</span>
        </div>

        <!-- 召唤师简短信息 -->
        <div v-if="summoner" class="user-card" @click="navigate('career')" :title="`${summoner.displayName} (${regionName})`">
          <div class="user-avatar">
            <LcuImage :src="summoner.profileIconUrl" alt="avatar" />
          </div>
          <div class="user-info">
            <span class="user-name">{{ summoner.displayName }}</span>
            <span class="user-region">{{ regionName }}</span>
          </div>
        </div>

        <div :class="['nav-item', { active: currentPage === 'settings' }]" @click="navigate('settings')" title="设置">
          <span class="nav-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <circle cx="12" cy="12" r="3"/>
              <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>
            </svg>
          </span>
          <span class="nav-label">设置</span>
        </div>
      </div>
    </aside>

    <!-- 右侧内容区域 -->
    <main class="content-wrapper">
      <!-- Search 用 v-show 保持状态，切页面不清空数据 -->
      <div v-show="currentPage === 'search'" style="height:100%;overflow-y:auto;">
        <Search />
      </div>
      <template v-if="currentPage !== 'search'">
        <Home v-if="currentPage === 'home'" @navigate="navigate" />
        <Career v-else-if="currentPage === 'career'" />
        <GameInfo v-else-if="currentPage === 'gameinfo'" />
        <TFT v-else-if="currentPage === 'tft'" />
        <Settings v-else-if="currentPage === 'settings'" />
        <Tools v-else-if="currentPage === 'tools'" />

        <!-- 内建 OP.GG 占位页面 -->
        <div v-else-if="currentPage === 'opgg'" class="placeholder-view">
          <div class="view-header">
            <h2>OP.GG 辅助模块</h2>
          </div>
          <div class="view-card">
            <div class="avatar-circle op-icon">OP</div>
            <h3>OP.GG 数据反代已成功建立</h3>
            <p>Yuumi 已在后台为您开启 OP.GG 数据加速反代，保证国服和外服战绩的流畅拉取。</p>
            <div class="status-box">
              <span class="dot online"></span>
              <span>OP.GG 代理地址: 127.0.0.1:10809</span>
            </div>
            <p class="hint">您可以在系统设置中开启"对局自动上传战绩"，该功能将静默安全地同步数据。</p>
          </div>
        </div>

        <!-- 内建 公告 占位页面 -->
        <div v-else-if="currentPage === 'notice'" class="placeholder-view">
          <div class="view-header">
            <h2>系统公告</h2>
          </div>
          <div class="changelog-card">
            <div class="version-tag">v0.1.0</div>
            <h3>Yuumi 辅助工具重构完成</h3>
            <p class="date">发布时间: 2026-06-24</p>
            <ul class="changelog-list">
              <li>✨ <strong>全新 1:1 Seraphine 风格重构</strong>：支持垂直折叠侧边栏，带来极致沉浸式的原生大厅体验。</li>
              <li>🚀 <strong>Rust (Tauri v2) 高效重写</strong>：用 Rust 重构底层 LCU 数据抓取，内存降低 80%，响应即刻响应。</li>
              <li>⚙️ <strong>对局选人自动化</strong>：自动接受对局、极速自动 Ban/Pick，支持分路配置候选英雄。</li>
              <li>📊 <strong>数据看板美化</strong>：全面美化生涯战绩、单场表现分析以及选人面板 10 人近期段位和胜率展现。</li>
            </ul>
          </div>
        </div>
      </template>
    </main>

    <!-- OP.GG 弹窗 -->
    <OpggModal v-if="showOpgg" @close="showOpgg = false" />
  </div>
</template>

<style>
:root {
  --primary-color: #6c5ce7;
  --primary-color-hover: #5b4cc4;
  --primary-color-alpha-15: rgba(108, 92, 231, 0.15);
  --primary-color-alpha-30: rgba(108, 92, 231, 0.3);
  --primary-color-alpha-40: rgba(108, 92, 231, 0.4);
}
* { box-sizing: border-box; }
body { margin: 0; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif; background-color: #fafbfc; color: #333; overflow: hidden; }
</style>

<style scoped>
.app-layout {
  display: flex;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
  background-color: #f7f9fa;
}

/* 侧边栏样式 */
.sidebar {
  display: flex;
  flex-direction: column;
  height: 100%;
  background-color: #f0f2f5;
  border-right: 1px solid #e1e4e8;
  transition: width 0.25s cubic-bezier(0.4, 0, 0.2, 1);
  overflow: hidden;
  flex-shrink: 0;
  user-select: none;
}

.sidebar.collapsed {
  width: 64px;
}

.sidebar.expanded {
  width: 210px;
}

/* 顶部折叠图标 */
.sidebar-header {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 48px;
  cursor: pointer;
  color: #5f6368;
  transition: background-color 0.2s;
}

.sidebar.expanded .sidebar-header {
  justify-content: flex-start;
  padding-left: 20px;
}

.sidebar-header:hover {
  background-color: #e4e7eb;
}

.hamburger-icon {
  width: 22px;
  height: 22px;
  flex-shrink: 0;
}

/* 导航链接 */
.sidebar-nav {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 8px;
  flex: 1;
}

.nav-item {
  display: flex;
  align-items: center;
  height: 42px;
  padding: 0 12px;
  border-radius: 6px;
  cursor: pointer;
  color: #4f5660;
  position: relative;
  transition: background-color 0.15s, color 0.15s;
}

.sidebar.collapsed .nav-item {
  justify-content: center;
  padding: 0;
}

.nav-item:hover {
  background-color: #e4e7eb;
  color: #2c3e50;
}

.nav-item.active {
  background-color: var(--primary-color-alpha-15);
  color: var(--primary-color);
  font-weight: 500;
}

/* 左侧指示条 (Green line like Seraphine) */
.nav-item.active::before {
  content: "";
  position: absolute;
  left: 0;
  top: 8px;
  bottom: 8px;
  width: 4px;
  background-color: var(--primary-color);
  border-radius: 0 4px 4px 0;
}

.sidebar.collapsed .nav-item.active::before {
  left: 4px;
}

.nav-icon {
  width: 20px;
  height: 20px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  margin-right: 12px;
  color: inherit;
}

.nav-icon svg {
  width: 20px;
  height: 20px;
}

.sidebar.collapsed .nav-icon {
  margin-right: 0;
}

.text-icon {
  font-size: 0.75rem;
  font-weight: 800;
  border: 1.5px solid currentColor;
  border-radius: 4px;
  width: 20px;
  height: 20px;
  line-height: 18px;
  text-align: center;
}

.nav-label {
  font-size: 0.88rem;
  white-space: nowrap;
  opacity: 1;
  transition: opacity 0.15s;
}

.sidebar.collapsed .nav-label {
  opacity: 0;
  width: 0;
  overflow: hidden;
  pointer-events: none;
  display: none;
}

/* 底部功能栏 */
.sidebar-bottom {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 8px;
  border-top: 1px solid #e1e4e8;
}

/* 召唤师信息卡片 */
.user-card {
  display: flex;
  align-items: center;
  height: 48px;
  padding: 0 8px;
  margin: 4px 0;
  border-radius: 6px;
  cursor: pointer;
  transition: background-color 0.2s;
  overflow: hidden;
}

.sidebar.collapsed .user-card {
  justify-content: center;
  padding: 0;
}

.user-card:hover {
  background-color: #e4e7eb;
}

.user-avatar {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  overflow: hidden;
  flex-shrink: 0;
  border: 1px solid #dcdfe6;
}

.user-info {
  display: flex;
  flex-direction: column;
  margin-left: 10px;
  min-width: 0;
}

.sidebar.collapsed .user-info {
  display: none;
}

.user-name {
  font-size: 0.82rem;
  font-weight: bold;
  color: #2c3e50;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.user-region {
  font-size: 0.7rem;
  color: #909399;
}

/* 右侧内容区域 */
.content-wrapper {
  flex: 1;
  height: 100%;
  overflow-y: auto;
  background-color: #fafbfc;
}

/* 占位页面样式 */
.placeholder-view {
  padding: 2rem;
  max-width: 800px;
  margin: 0 auto;
}

.view-header {
  margin-bottom: 2rem;
  border-bottom: 1px solid #eee;
  padding-bottom: 1rem;
}

.view-header h2 {
  font-size: 1.8rem;
  margin: 0;
  color: #2c3e50;
}

.view-card {
  background: white;
  border-radius: 12px;
  padding: 3rem 2rem;
  text-align: center;
  box-shadow: 0 4px 12px rgba(0,0,0,0.05);
  border: 1px solid #ebeef5;
}

.avatar-circle.op-icon {
  width: 60px;
  height: 60px;
  line-height: 56px;
  border: 3px solid var(--primary-color);
  color: var(--primary-color);
  font-size: 1.5rem;
  font-weight: 800;
  border-radius: 50%;
  margin: 0 auto 1.5rem;
  text-align: center;
}

.view-card h3 {
  font-size: 1.4rem;
  margin: 0 0 1rem;
  color: #303133;
}

.view-card p {
  color: #606266;
  font-size: 0.95rem;
  line-height: 1.6;
  max-width: 500px;
  margin: 0 auto 1.5rem;
}

.status-box {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  background: #f0f9eb;
  color: #67c23a;
  padding: 8px 16px;
  border-radius: 20px;
  font-size: 0.9rem;
  font-weight: 500;
  margin-bottom: 1.5rem;
  border: 1px solid #e1f3d8;
}

.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.dot.online {
  background: #67c23a;
}

.view-card .hint {
  font-size: 0.82rem;
  color: #909399;
  margin: 0;
}

/* 公告板 */
.changelog-card {
  background: white;
  border-radius: 12px;
  padding: 2rem;
  box-shadow: 0 4px 12px rgba(0,0,0,0.05);
  border: 1px solid #ebeef5;
}

.version-tag {
  display: inline-block;
  background: var(--primary-color);
  color: white;
  padding: 4px 12px;
  border-radius: 4px;
  font-weight: bold;
  font-size: 0.85rem;
  margin-bottom: 0.8rem;
}

.changelog-card h3 {
  margin: 0 0 4px;
  font-size: 1.4rem;
  color: #303133;
}

.changelog-card .date {
  color: #909399;
  font-size: 0.82rem;
  margin: 0 0 1.5rem;
}

.changelog-list {
  padding-left: 20px;
  margin: 0;
}

.changelog-list li {
  margin-bottom: 0.8rem;
  color: #606266;
  line-height: 1.6;
  font-size: 0.92rem;
}
</style>
