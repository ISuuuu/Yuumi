<script setup lang="ts">
import { ref, onMounted, watch, computed, provide } from "vue";
import { useLcuStore, initLcuListeners } from "./store/lcuStore";
import { storeToRefs } from "pinia";
import { fetchCurrentSummoner, lcuRequest, fetchConfig } from "./api/lcu";
import { updateThemeColor } from "./utils/theme";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { SummonerDisplay } from "./api/lcu";
import Home from "./views/Home.vue";
import Career from "./views/Career.vue";
import Search from "./views/Search.vue";
import GameInfo from "./views/GameInfo.vue";
import TFT from "./views/TFT.vue";
import Settings from "./views/Settings.vue";
import Tools from "./views/Tools.vue";
import LcuImage from "./components/LcuImage.vue";
import opggIcon from "./assets/opgg.svg";

const store = useLcuStore();
const { gamePhase, connectionVersion } = storeToRefs(store);
const currentPage = ref("home");
const pageHistory: string[] = [];
const isSidebarExpanded = ref(false);
const summoner = ref<SummonerDisplay | null>(null);
const platformId = ref("");

// Toast 通知
const toast = ref<{ message: string; type: 'success' | 'error'; visible: boolean }>({
  message: '', type: 'success', visible: false
});
let toastTimer: ReturnType<typeof setTimeout> | null = null;
function showToast(message: string, type: 'success' | 'error' = 'success') {
  if (toastTimer) clearTimeout(toastTimer);
  toast.value = { message, type, visible: true };
  toastTimer = setTimeout(() => { toast.value.visible = false; }, 2500);
}

const PHASE_LABELS: Record<string, string> = {
  None: "空闲",
  Lobby: "房间中",
  Matchmaking: "匹配中",
  ReadyCheck: "确认对局",
  ChampSelect: "选择英雄",
  GameStart: "游戏加载",
  InProgress: "游戏中",
  EndOfGame: "对局结束",
  PreEndOfGame: "结算中",
  WaitingForStats: "等待数据",
  Reconnect: "重新连接",
};

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

onMounted(async () => {
  await initLcuListeners();
  // 直接触发一次 loadLcuState，不依赖 watcher 的时序
  // loadLcuState 内部已有 1 秒延迟 + 重试机制
  console.log("[App] onMounted 完成，直接调用 loadLcuState");
  loadLcuState();

  // 监听系统托盘菜单导航事件
  await listen<string>("tray-navigate", (event: { payload: string }) => {
    navigate(event.payload);
  });

  // 自动启动 LOL 客户端并按需显示主窗口
  try {
    const cfg = await fetchConfig();
    if (cfg.General?.EnableStartLolWithApp) {
      invoke("launch_lol_client").catch((e: any) => console.warn("自动启动 LOL 失败:", e));
    }
    // 如果没有开启“游戏开始最小化”（静默启动），则在组件挂载并完成配置获取后显示窗口
    if (!cfg.General?.EnableGameStartMinimize) {
      await getCurrentWindow().show();
    }
    // 无论是否静默启动，都在配置加载完后尝试应用一次云母效果，防止窗口创建时隐藏导致 DWM 特效应用失败
    if (cfg.Personalization?.MicaEnabled) {
      invoke("set_mica_effect", { enabled: true }).catch((e: any) => console.warn("应用云母效果失败:", e));
    }
  } catch (e) {
    console.warn("[App] 启动配置检查失败:", e);
    // 异常情况下兜底显示窗口，保证软件可用性
    await getCurrentWindow().show();
  }
});

function navigate(page: string) {
  if (currentPage.value !== page) {
    pageHistory.push(currentPage.value);
  }
  currentPage.value = page;
}

function goBack() {
  if (pageHistory.length > 0) {
    currentPage.value = pageHistory.pop()!;
  }
}

function toggleSidebar() {
  isSidebarExpanded.value = !isSidebarExpanded.value;
}

async function openOpggWindow() {
  const existing = await WebviewWindow.getByLabel("opgg");
  if (existing) {
    await existing.setFocus();
    return;
  }
  const mainPos = await getCurrentWindow().outerPosition();
  const mainSize = await getCurrentWindow().innerSize();
  new WebviewWindow("opgg", {
    url: "opgg.html",
    title: "OP.GG",
    width: 760,
    height: 820,
    x: mainPos.x + mainSize.width + 2,
    y: mainPos.y,
    decorations: true,
    resizable: true,
    center: false,
  });
}

async function loadLcuState(retryCount = 0) {
  console.log(`[loadLcuState] 开始 (retry=${retryCount}, isConnected=${store.isConnected})`);

  // 如果未连接，等待 1 秒后重试（后端可能还在初始化）
  if (!store.isConnected) {
    if (retryCount < 3) {
      console.log(`[loadLcuState] 未连接，1 秒后重试 (${retryCount + 1}/3)`);
      setTimeout(() => loadLcuState(retryCount + 1), 1000);
    } else {
      console.log("[loadLcuState] 多次重试仍未连接，放弃");
      summoner.value = null;
      platformId.value = "";
    }
    return;
  }

  // 首次调用时等待 1 秒，让 LCU API 就绪（尤其游戏中启动时）
  if (retryCount === 0) {
    await new Promise(r => setTimeout(r, 1000));
  }

  // 每一步独立 try-catch，互不影响

  // 1. 获取当前召唤师数据
  try {
    console.log("[loadLcuState] 获取召唤师数据...");
    summoner.value = await fetchCurrentSummoner();
    console.log("[loadLcuState] 召唤师:", summoner.value?.displayName);
  } catch (e) {
    console.warn("[loadLcuState] 获取召唤师失败:", e);
  }

  // 2. 获取大区平台
  try {
    const resp = await lcuRequest<any>("GET", "/lol-platform-config/v1/namespaces/LoginPlatformLocalization/platformId");
    if (resp.success && resp.data) {
      platformId.value = resp.data;
    }
  } catch (e) {
    console.warn("[loadLcuState] 获取大区失败:", e);
  }

  // 3. 同步拉取当前 LCU 对局状态（游戏中重启时关键，必须执行）
  try {
    console.log("[loadLcuState] 获取游戏阶段...");
    const phaseResp = await lcuRequest<string>("GET", "/lol-gameflow/v1/gameflow-phase");
    console.log("[loadLcuState] gameflow-phase resp:", JSON.stringify(phaseResp));
    if (phaseResp.success && phaseResp.data) {
      console.log("[loadLcuState] setGamePhase:", phaseResp.data);
      store.setGamePhase(phaseResp.data);
    }
  } catch (e) {
    console.error("[loadLcuState] 获取游戏阶段失败:", e);
  }

  // 4. 同步拉取对局 Session
  if (store.gamePhase === "ChampSelect") {
    try {
      const sessionResp = await lcuRequest<any>("GET", "/lol-champ-select/v1/session");
      if (sessionResp.success && sessionResp.data) {
        store.setChampSelectSession(sessionResp.data);
      }
    } catch (e) {
      console.warn("[loadLcuState] 获取选人 Session 失败:", e);
    }
  }

  // 5. 初始化加载主题色
  try {
    const cfg = await fetchConfig();
    if (cfg && cfg.Personalization && cfg.Personalization.ThemeColor) {
      updateThemeColor(cfg.Personalization.ThemeColor);
    }
  } catch (e) {
    console.warn("[loadLcuState] 加载配置失败:", e);
  }

  // 如果召唤师数据加载失败且是首次调用，触发重试
  if (!summoner.value && retryCount < 3) {
    console.log(`[loadLcuState] 召唤师数据为空，2 秒后重试 (${retryCount + 1}/3)`);
    setTimeout(() => loadLcuState(retryCount + 1), 2000);
  }

  console.log("[loadLcuState] 完成, gamePhase=", store.gamePhase, "summoner=", summoner.value?.displayName);
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

// lcu-client-started 事件触发时重新加载（游戏中重启等场景）
watch(connectionVersion, () => {
  console.log("[App] connectionVersion 变化，重新加载 LCU 状态");
  loadLcuState();
});

// 游戏阶段变化 → 更新窗口标题 + 自动跳转对局信息页
watch(gamePhase, (phase: string) => {
  console.log("[watch gamePhase] phase changed:", phase);

  // 更新窗口标题栏显示游戏状态
  const label = PHASE_LABELS[phase];
  const title = label ? `Yuumi · ${label}` : "Yuumi";
  getCurrentWindow().setTitle(title).catch(() => {});

  // 进入选人/游戏加载/游戏中时自动跳转到对局信息页
  if (phase === "ChampSelect" || phase === "GameStart" || phase === "InProgress") {
    console.log("[watch gamePhase] navigating to gameinfo");
    currentPage.value = "gameinfo";
  }
});

function handleReconnect() {
  initLcuListeners();
  // 先查询当前游戏阶段，按情况处理
  lcuRequest<string>("GET", "/lol-gameflow/v1/gameflow-phase").then(resp => {
    if (!resp.success) {
      showToast("LCU 未连接，请先启动英雄联盟客户端", "error");
      return;
    }
    const phase = resp.data;
    if (phase === "InProgress" || phase === "GameStart" || phase === "Reconnect") {
      // 游戏中 → 调用 reconnect API
      lcuRequest("POST", "/lol-gameflow/v1/reconnect").then(r => {
        if (r.success) {
          showToast("🔄 已触发游戏重连");
        } else {
          showToast("重连请求失败: " + (r.error || ""), "error");
        }
      });
    } else {
      showToast("LCU 监听服务已重置 (当前: " + (PHASE_LABELS[phase ?? ""] || phase) + ")");
    }
  }).catch(() => {
    showToast("LCU 监听服务已重置");
  });
}

async function handleClose() {
  try {
    const closeToTray = await invoke<boolean>("get_close_to_tray");
    const win = getCurrentWindow();
    if (closeToTray) {
      await win.hide();
    } else {
      await win.close();
    }
  } catch (e) {
    console.error("[handleClose] 失败，直接关闭窗口:", e);
    await getCurrentWindow().close();
  }
}
</script>

<template>
  <div class="app-layout">
    <!-- Toast -->
    <Transition name="toast">
      <div v-if="toast.visible" :class="['toast', `toast-${toast.type}`]">{{ toast.message }}</div>
    </Transition>

    <!-- 自定义标题栏 -->
    <div class="titlebar" data-tauri-drag-region>
      <div class="titlebar-left">
        <div v-if="pageHistory.length > 0" class="titlebar-btn" @click="goBack" title="返回">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="15 18 9 12 15 6"/>
          </svg>
        </div>
        <img src="/logo.png" class="titlebar-logo" alt="logo" />
        <span class="titlebar-title">
          Yummi
          <span v-if="store.isConnected && gamePhase !== 'None'" class="titlebar-phase">
            · {{ PHASE_LABELS[gamePhase] || gamePhase }}
          </span>
        </span>
      </div>
      <div class="titlebar-controls">
        <div class="titlebar-btn" @click="getCurrentWindow().minimize()" title="最小化">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="5" y1="12" x2="19" y2="12"/></svg>
        </div>
        <div class="titlebar-btn" @click="getCurrentWindow().toggleMaximize()" title="最大化">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="5" y="5" width="14" height="14" rx="2"/></svg>
        </div>
        <div class="titlebar-btn close-btn" @click="handleClose" title="关闭">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
        </div>
      </div>
    </div>

    <!-- 主体区域：侧边栏 + 内容 -->
    <div class="main-row">
    <aside :class="['sidebar', isSidebarExpanded ? 'expanded' : 'collapsed']">
      <!-- 顶部折叠按钮 -->
      <div class="sidebar-header">
        <div class="hamburger-btn" @click="toggleSidebar">
          <svg class="hamburger-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M3 12h18M3 6h18M3 18h18" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </div>
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
        <div class="nav-item" @click="openOpggWindow" title="OP.GG">
          <span class="nav-icon"><img :src="opggIcon" style="width:18px;height:18px;border-radius:3px" /></span>
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
      <!-- Search 和 GameInfo 用 v-show 保持状态，切页面不清空数据 -->
      <div v-show="currentPage === 'search'" style="height:100%;overflow-y:auto;">
        <Search />
      </div>
      <div v-show="currentPage === 'gameinfo'" style="height:100%;overflow-y:auto;">
        <GameInfo />
      </div>
      <template v-if="currentPage !== 'search' && currentPage !== 'gameinfo'">
        <Home v-if="currentPage === 'home'" @navigate="navigate" />
        <Career v-else-if="currentPage === 'career'" />
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
    </div>
  </div>
</template>

<style>
:root {
  --primary-color: #6c5ce7;
  --primary-color-hover: #5b4cc4;
  --primary-color-alpha-15: rgba(108, 92, 231, 0.15);
  --primary-color-alpha-30: rgba(108, 92, 231, 0.3);
  --primary-color-alpha-40: rgba(108, 92, 231, 0.4);

  /* 纯白水晶极光主题变量 */
  --bg-color-gradient: linear-gradient(135deg, #ffffff 0%, #fcfdfe 40%, #f3f6fc 100%);
  --bg-color: #f7fafc;
  --sidebar-bg: rgba(255, 255, 255, 0.85);
  --card-bg: rgba(255, 255, 255, 0.82);
  --card-bg-hover: rgba(255, 255, 255, 0.96);
  --border-color: rgba(0, 0, 0, 0.04);
  --border-color-hover: rgba(0, 0, 0, 0.08);
  
  --text-color: #0f172a;
  --text-muted: #334155;
  --text-dimmed: #64748b;

  --win-color: #10b981;
  --win-bg: rgba(16, 185, 129, 0.08);
  --win-border: rgba(16, 185, 129, 0.18);
  --win-glow: rgba(16, 185, 129, 0.08);

  --loss-color: #ef4444;
  --loss-bg: rgba(239, 68, 68, 0.07);
  --loss-border: rgba(239, 68, 68, 0.18);
  --loss-glow: rgba(239, 68, 68, 0.08);

  --font-sans: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", Roboto, sans-serif;
  --radius-sm: 4px;
  --radius-md: 8px;
  --radius-lg: 12px;
  --shadow-sm: 0 2px 12px rgba(0, 0, 0, 0.015);
  --shadow-md: 0 8px 30px rgba(0, 0, 0, 0.03);
  --shadow-lg: 0 16px 40px rgba(0, 0, 0, 0.06);
  --glass-filter: blur(16px);
}

* {
  box-sizing: border-box;
}

body {
  margin: 0;
  font-family: var(--font-sans);
  background-color: var(--bg-color);
  color: var(--text-color);
  overflow: hidden;
  user-select: none;
}

/* 美化全局滚动条 */
::-webkit-scrollbar {
  width: 6px;
  height: 6px;
}
::-webkit-scrollbar-track {
  background: transparent;
}
::-webkit-scrollbar-thumb {
  background: rgba(0, 0, 0, 0.06);
  border-radius: 4px;
  transition: background 0.2s;
}
::-webkit-scrollbar-thumb:hover {
  background: var(--primary-color);
}
</style>

<style scoped>
.app-layout {
  display: flex;
  flex-direction: column;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
  background: var(--bg-color-gradient);
}

/* 自定义标题栏 */
.titlebar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 38px;
  background: rgba(255, 255, 255, 0.88);
  border-bottom: 1px solid var(--border-color);
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
  flex-shrink: 0;
  user-select: none;
  padding: 0 8px;
  z-index: 1000;
}

.titlebar-left {
  display: flex;
  align-items: center;
  gap: 4px;
}

.titlebar-logo {
  width: 20px;
  height: 20px;
  object-fit: contain;
  margin-left: 4px;
}

.titlebar-title {
  font-size: 0.8rem;
  font-weight: 700;
  color: var(--text-color);
  letter-spacing: 0.5px;
  margin-left: 2px;
  text-transform: none;
}

.titlebar-phase {
  font-weight: 600;
  color: var(--primary-color);
  font-size: 0.78rem;
  background: var(--primary-color-alpha-15);
  padding: 1px 6px;
  border-radius: 4px;
  margin-left: 6px;
}

.titlebar-controls {
  display: flex;
  align-items: center;
  gap: 2px;
}

.titlebar-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 26px;
  border-radius: 4px;
  cursor: pointer;
  color: var(--text-muted);
  transition: all 0.2s ease;
}

.titlebar-btn:hover {
  background-color: rgba(0, 0, 0, 0.05);
  color: var(--text-color);
}

.titlebar-btn svg {
  width: 14px;
  height: 14px;
}

.close-btn:hover {
  background-color: var(--loss-color) !important;
  color: white !important;
}

.main-row {
  display: flex;
  flex: 1;
  min-height: 0;
}

/* 侧边栏样式 */
.sidebar {
  display: flex;
  flex-direction: column;
  height: 100%;
  background-color: var(--sidebar-bg);
  border-right: 1px solid var(--border-color);
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
  transition: width 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
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
  height: 52px;
  padding: 0 10px;
  gap: 4px;
}

.back-btn, .hamburger-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border-radius: 8px;
  cursor: pointer;
  color: var(--text-muted);
  transition: all 0.2s cubic-bezier(0.25, 0.8, 0.25, 1);
  flex-shrink: 0;
}

.back-btn:hover, .hamburger-btn:hover {
  background-color: rgba(0, 0, 0, 0.03);
  color: var(--text-color);
}

.back-btn svg {
  width: 16px;
  height: 16px;
}

.hamburger-icon {
  width: 20px;
  height: 20px;
  flex-shrink: 0;
}

/* 导航链接 */
.sidebar-nav {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 6px;
  flex: 1;
}

.nav-item {
  display: flex;
  align-items: center;
  height: 40px;
  padding: 0 12px;
  border-radius: 8px;
  cursor: pointer;
  color: var(--text-muted);
  position: relative;
  transition: all 0.2s cubic-bezier(0.25, 0.8, 0.25, 1);
}

.sidebar.collapsed .nav-item {
  justify-content: center;
  padding: 0;
}

.nav-item:hover {
  background-color: rgba(0, 0, 0, 0.02);
  color: var(--text-color);
  transform: translateX(2px);
}

.sidebar.collapsed .nav-item:hover {
  transform: translateX(0);
}

.nav-item.active {
  background-color: var(--primary-color-alpha-15);
  color: var(--primary-color);
  font-weight: 600;
  box-shadow: inset 0 0 0 1px var(--primary-color-alpha-15);
}

/* 左侧指示发光条 */
.nav-item.active::before {
  content: "";
  position: absolute;
  left: 0;
  top: 8px;
  bottom: 8px;
  width: 3px;
  background-color: var(--primary-color);
  border-radius: 0 4px 4px 0;
  box-shadow: 0 0 6px var(--primary-color-alpha-40);
}

.sidebar.collapsed .nav-item.active::before {
  left: 3px;
}

.nav-icon {
  width: 18px;
  height: 18px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  margin-right: 12px;
  color: inherit;
  transition: transform 0.2s;
}

.nav-item:hover .nav-icon {
  transform: scale(1.05);
}

.nav-icon svg {
  width: 18px;
  height: 18px;
}

.sidebar.collapsed .nav-icon {
  margin-right: 0;
}

.text-icon {
  font-size: 0.7rem;
  font-weight: 900;
  border: 1.5px solid currentColor;
  border-radius: 4px;
  width: 18px;
  height: 18px;
  line-height: 15px;
  text-align: center;
}

.nav-label {
  font-size: 0.85rem;
  white-space: nowrap;
  opacity: 1;
  transition: opacity 0.2s;
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
  gap: 4px;
  padding: 6px;
  border-top: 1px solid var(--border-color);
}

/* 召唤师信息卡片 */
.user-card {
  display: flex;
  align-items: center;
  height: 48px;
  padding: 0 8px;
  margin: 6px 0;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s ease;
  overflow: hidden;
  border: 1px solid transparent;
}

.sidebar.collapsed .user-card {
  justify-content: center;
  padding: 0;
}

.user-card:hover {
  background-color: rgba(0, 0, 0, 0.03);
  border-color: var(--border-color);
}

.user-avatar {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  overflow: hidden;
  flex-shrink: 0;
  border: 1.5px solid var(--border-color);
  transition: border-color 0.2s;
}

.user-card:hover .user-avatar {
  border-color: var(--primary-color);
}

.user-info {
  display: flex;
  flex-direction: column;
  margin-left: 8px;
  min-width: 0;
}

.sidebar.collapsed .user-info {
  display: none;
}

.user-name {
  font-size: 0.8rem;
  font-weight: 700;
  color: var(--text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.user-region {
  font-size: 0.65rem;
  color: var(--text-muted);
}

/* 右侧内容区域 */
.content-wrapper {
  flex: 1;
  height: 100%;
  overflow-y: auto;
  background-color: transparent;
}

/* 占位页面样式 */
.placeholder-view {
  padding: 2.5rem 1.5rem;
  max-width: 800px;
  margin: 0 auto;
}

.view-header {
  margin-bottom: 2rem;
  border-bottom: 1px solid var(--border-color);
  padding-bottom: 1rem;
}

.view-header h2 {
  font-size: 1.6rem;
  margin: 0;
  font-weight: 800;
  color: var(--text-color);
  letter-spacing: 0.5px;
}

.view-card {
  background: var(--card-bg);
  border-radius: var(--radius-lg);
  padding: 3rem 2rem;
  text-align: center;
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
  border: 1px solid var(--border-color);
  box-shadow: var(--shadow-sm);
  transition: all 0.3s ease;
}

.view-card:hover {
  border-color: var(--primary-color-alpha-30);
  box-shadow: var(--shadow-md);
  transform: translateY(-2px);
}

.avatar-circle.op-icon {
  width: 54px;
  height: 54px;
  line-height: 50px;
  border: 2px solid var(--primary-color);
  color: var(--primary-color);
  font-size: 1.3rem;
  font-weight: 900;
  border-radius: 50%;
  margin: 0 auto 1.5rem;
  text-align: center;
  box-shadow: 0 4px 10px var(--primary-color-alpha-15);
}

.view-card h3 {
  font-size: 1.25rem;
  margin: 0 0 0.8rem;
  font-weight: 700;
  color: var(--text-color);
}

.view-card p {
  color: var(--text-muted);
  font-size: 0.88rem;
  line-height: 1.6;
  max-width: 480px;
  margin: 0 auto 1.5rem;
}

.status-box {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  background: var(--win-bg);
  color: var(--win-color);
  padding: 6px 14px;
  border-radius: 20px;
  font-size: 0.8rem;
  font-weight: 600;
  margin-bottom: 1.5rem;
  border: 1px solid var(--win-border);
}

.dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
}

.dot.online {
  background: var(--win-color);
  box-shadow: 0 0 6px var(--win-color);
}

.view-card .hint {
  font-size: 0.78rem;
  color: var(--text-dimmed);
  margin: 0;
}

/* 公告板 */
.changelog-card {
  background: var(--card-bg);
  border-radius: var(--radius-lg);
  padding: 2rem;
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
  border: 1px solid var(--border-color);
  box-shadow: var(--shadow-sm);
  transition: all 0.3s ease;
}

.changelog-card:hover {
  border-color: var(--primary-color-alpha-30);
  box-shadow: var(--shadow-md);
}

.version-tag {
  display: inline-block;
  background: var(--primary-color);
  color: white;
  padding: 3px 10px;
  border-radius: 4px;
  font-weight: 700;
  font-size: 0.75rem;
  margin-bottom: 0.8rem;
  box-shadow: 0 4px 10px var(--primary-color-alpha-30);
}

.changelog-card h3 {
  margin: 0 0 4px;
  font-size: 1.25rem;
  font-weight: 700;
  color: var(--text-color);
}

.changelog-card .date {
  color: var(--text-dimmed);
  font-size: 0.78rem;
  margin: 0 0 1.5rem;
}

.changelog-list {
  padding-left: 18px;
  margin: 0;
}

.changelog-list li {
  margin-bottom: 0.8rem;
  color: var(--text-muted);
  line-height: 1.6;
  font-size: 0.85rem;
}

.changelog-list strong {
  color: var(--text-color);
}

/* Toast */
.toast {
  position: fixed; top: 20px; left: 50%; transform: translateX(-50%);
  padding: 10px 24px; border-radius: 8px; font-size: 0.82rem;
  font-weight: 600; color: white; z-index: 9999;
  box-shadow: var(--shadow-md); pointer-events: none;
}
.toast-success { background-color: var(--win-color); }
.toast-error { background-color: var(--loss-color); }
.toast-enter-active { transition: all 0.25s ease-out; }
.toast-leave-active { transition: all 0.2s ease-in; }
.toast-enter-from { opacity: 0; transform: translateX(-50%) translateY(-12px); }
.toast-leave-to { opacity: 0; transform: translateX(-50%) translateY(-8px); }
</style>
