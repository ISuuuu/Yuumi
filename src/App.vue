<script setup lang="ts">
import { ref, onMounted, watch, computed, provide } from "vue";
import { useLcuStore, initLcuListeners } from "./store/lcuStore";
import { storeToRefs } from "pinia";
import { fetchCurrentSummoner, lcuRequest, fetchConfig } from "./api/lcu";
import { updateThemeColor, updateDeathColor, applyDpiScale, toHex6 } from "./utils/theme";
import { getCurrentWindow, currentMonitor } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { SummonerDisplay, AppConfig } from "./api/lcu";
import { darkTheme, type GlobalThemeOverrides } from "naive-ui";
import NaiveUIBridge from "./components/NaiveUIBridge.vue";
import Home from "./views/Home.vue";
import Career from "./views/Career.vue";
import Search from "./views/Search.vue";
import GameInfo from "./views/GameInfo.vue";
import TFT from "./views/TFT.vue";
import Settings from "./views/Settings.vue";
import Tools from "./views/Tools.vue";
import Notice from "./views/Notice.vue";
import BenchOverlay from "./views/BenchOverlay.vue";
import LcuImage from "./components/LcuImage.vue";
import UpdateDialog, { type UpdateInfo } from "./components/UpdateDialog.vue";
import opggIcon from "./assets/opgg.svg";

function applyThemeMode(mode: string) {
  const root = document.documentElement;
  if (mode === 'Auto') {
    root.removeAttribute('data-theme');
    localStorage.setItem("yuumi_theme", "Auto");
  } else if (mode === 'Light' || mode === 'Dark') {
    root.setAttribute('data-theme', mode.toLowerCase());
    localStorage.setItem("yuumi_theme", mode);
  }
}

// 水晶极光主题覆盖（动态响应系统主题色）
const themeOverrides = computed<GlobalThemeOverrides>(() => {
  const customColor = appConfig.value?.Personalization?.ThemeColor
    ? toHex6(appConfig.value.Personalization.ThemeColor)
    : "#a78bfa";
  
  return {
    common: {
      primaryColor: customColor,
      primaryColorHover: customColor + "d9", // 85% alpha hover
      primaryColorPressed: customColor + "a6", // 65% alpha pressed
      borderRadius: "10px",
    },
    Card: {
      color: "rgba(255, 255, 255, 0.15)",
      borderColor: "rgba(255, 255, 255, 0.2)",
    },
    Dialog: {
      color: "rgba(255, 255, 255, 0.25)",
      borderColor: "rgba(255, 255, 255, 0.2)",
    },
    Button: {
      textColorPrimary: "#ffffff",
    },
  };
});

const store = useLcuStore();
const { gamePhase } = storeToRefs(store);
const currentPage = ref("home");
const appConfig = ref<AppConfig | null>(null);
provide("appConfig", appConfig);
const pageHistory: string[] = [];
const isSidebarExpanded = ref(false);
const summoner = ref<SummonerDisplay | null>(null);
const platformId = ref("");
const mapSideLabel = ref(""); // 蓝色方/红色方

// 检测当前是否是悬浮窗窗口（bench-overlay）
const isOverlayWindow = ref(window.location.search.includes("window=bench-overlay"));

// 自动更新弹窗
const updateInfo = ref<UpdateInfo | null>(null);

// Toast 通知（通过 Naive UI Message API）
function showToast(message: string, type: 'success' | 'error' = 'success') {
  try {
    const msg = (window as any).$message;
    if (msg) {
      if (type === 'error') {
        msg.error(message);
      } else {
        msg.success(message);
      }
    }
  } catch (e) {
    console.warn("[Toast] Naive UI message not available yet:", e);
  }
}

const PHASE_LABELS: Record<string, string> = {
  None: "空闲",
  Lobby: "房间组队中",
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

const isSystemDark = ref(window.matchMedia("(prefers-color-scheme: dark)").matches);
onMounted(() => {
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  const handler = (e: MediaQueryListEvent) => {
    isSystemDark.value = e.matches;
  };
  media.addEventListener("change", handler);
});

const isDarkTheme = computed(() => {
  const mode = appConfig.value?.Personalization?.ThemeMode || "Auto";
  if (mode === "Dark") return true;
  if (mode === "Light") return false;
  return isSystemDark.value;
});

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

  if (isOverlayWindow.value) {
    loadLcuState();
    return;
  }

  // 监听系统托盘菜单导航事件
  await listen<string>("tray-navigate", (event: { payload: string }) => {
    navigate(event.payload);
  });

  // 监听 Rust 后端推送的更新可用事件
  await listen<UpdateInfo>("updater://update-available", (event) => {
    updateInfo.value = event.payload;
  });

  // 自动启动 LOL 客户端并按需显示主窗口
  try {
    appConfig.value = await fetchConfig();

    // 检查配置加载时是否有错误（如配置文件损坏已自动恢复）
    const configErr = await invoke<null | string>("get_config_load_error");
    if (configErr) {
      alert("配置文件异常:\n" + configErr);
    }
    const cfg = appConfig.value;
    if (cfg?.General?.EnableStartLolWithApp) {
      invoke("launch_lol_client").catch((e: any) => console.warn("自动启动 LOL 失败:", e));
    }
    // 如果没有开启“游戏开始最小化”（静默启动），则在组件挂载并完成配置获取后显示窗口
    if (!cfg?.General?.EnableGameStartMinimize) {
      await getCurrentWindow().show();
    }
    // 无论是否静默启动，都在配置加载完后尝试应用一次云母效果，防止窗口创建时隐藏导致 DWM 特效应用失败
    if (cfg?.Personalization?.MicaEnabled) {
      invoke("set_mica_effect", { enabled: true }).catch((e: any) => console.warn("应用云母效果失败:", e));
    }
    // 应用主题色、死亡数字颜色 & 界面缩放
    if (cfg?.Personalization) {
      if (cfg.Personalization.ThemeColor) {
        updateThemeColor(cfg.Personalization.ThemeColor);
      }
      updateDeathColor(
        cfg.Personalization.LightDeathsNumberColor,
        cfg.Personalization.DarkDeathsNumberColor
      );
      applyDpiScale(cfg.Personalization.DpiScale);
      applyThemeMode(cfg.Personalization.ThemeMode);
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
  if (!store.isConnected) {
    showToast("LCU 未连接，请先启动英雄联盟客户端", "success");
    return;
  }
  const existing = await WebviewWindow.getByLabel("opgg");
  if (existing) {
    await existing.setFocus();
    return;
  }

  // 从配置中读取是否置顶窗口
  let alwaysOnTop = false;
  try {
    const cfg = appConfig.value || await fetchConfig();
    alwaysOnTop = cfg.Functions?.EnableOpggOnTop ?? false;
  } catch (e) {
    console.warn("加载置顶配置失败，使用默认值:", e);
  }

  // 根据当前主题决定原生标题栏颜色
  const savedTheme = localStorage.getItem("yuumi_theme");
  const isSystemDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  const nativeTheme: "dark" | "light" =
    savedTheme === "Dark" || (savedTheme !== "Light" && isSystemDark)
      ? "dark"
      : "light";

  // 获取当前窗口所在屏幕（显示器），将 OP.GG 窗口放置在屏幕右侧
  const monitor = await currentMonitor();
  if (monitor) {
    // Monitor 的 position/size 是物理像素，需要转为逻辑像素
    const pos = monitor.position.toLogical(monitor.scaleFactor);
    const size = monitor.size.toLogical(monitor.scaleFactor);
    new WebviewWindow("opgg", {
      url: "opgg.html",
      title: "OP.GG",
      width: 760,
      height: 820,
      x: pos.x + size.width - 760 - 2,
      y: pos.y + 2,
      decorations: true,
      resizable: true,
      center: false,
      alwaysOnTop,
      theme: nativeTheme,
    });
  } else {
    // 兜底：获取不到屏幕信息时，放在主窗口右侧
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
      alwaysOnTop,
      theme: nativeTheme,
    });
  }
}

async function loadLcuState() {
  console.log(`[loadLcuState] 开始, isConnected=${store.isConnected}`);
  if (!store.isConnected) return;

  // 等待 1 秒，让 LCU API 完全就绪
  await new Promise(r => setTimeout(r, 1000));

  // 每一步独立 try-catch，互不影响

  // 1. 获取当前召唤师数据
  try {
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
    const phaseResp = await lcuRequest<string>("GET", "/lol-gameflow/v1/gameflow-phase");
    if (phaseResp.success && phaseResp.data) {
      store.setGamePhase(phaseResp.data);
    }
  } catch (e) {
    console.warn("[loadLcuState] 获取游戏阶段失败:", e);
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
    const cfg = appConfig.value || await fetchConfig();
    if (cfg && cfg.Personalization && cfg.Personalization.ThemeColor) {
      updateThemeColor(cfg.Personalization.ThemeColor);
    }
    if (cfg && cfg.Personalization && cfg.Personalization.ThemeMode) {
      applyThemeMode(cfg.Personalization.ThemeMode);
    }
  } catch (e) {
    console.warn("[loadLcuState] 加载配置失败:", e);
  }

  console.log("[loadLcuState] 完成, gamePhase=", store.gamePhase, "summoner=", summoner.value?.displayName);
}

watch(() => store.isConnected, (connected) => {
  if (connected) {
    loadLcuState();
    // 客户端连接成功后自动跳转到生涯页面
    currentPage.value = "career";
  } else {
    summoner.value = null;
    platformId.value = "";
    mapSideLabel.value = ""; // 断开连接时清空队伍阵营信息
    // 断开连接时回到首页
    currentPage.value = "home";
  }
}, { immediate: true });

// 监听 Career → Search 跳转
watch(navigateSearchPayload, (payload) => {
  if (payload && payload.gameId !== null) {
    currentPage.value = "search";
  }
});

async function showBenchOverlay(show: boolean = true) {
  console.log("[bench-debug] showBenchOverlay 被调用: show =", show, "EnableBenchOverlay 配置值 =", appConfig.value?.Functions?.EnableBenchOverlay);
  // 检查配置开关
  if (show && appConfig.value?.Functions?.EnableBenchOverlay === false) {
    console.log("[bench-debug] 悬浮窗已被配置开关禁用，不执行开启");
    return;
  }
  try {
    console.log("[bench-debug] 正在通过 invoke 向 Rust 发送窗口控制命令: show =", show);
    await invoke("show_bench_overlay_window", { show });
  } catch (err) {
    console.error("[bench] 控制悬浮窗失败:", err);
  }
}

// lcu-client-started 事件触发时重新加载（游戏中重启等场景）
// isConnected watcher 已覆盖此场景，无需额外监听

// 游戏阶段变化 → 更新窗口标题 + 自动跳转对局信息页
watch(gamePhase, (phase: string) => {
  if (isOverlayWindow.value) return;
  console.log("[watch gamePhase] phase changed:", phase);

  // 更新窗口标题栏显示游戏状态
  const label = PHASE_LABELS[phase];
  const title = label ? `Yuumi · ${label}` : "Yuumi";
  const setTitle = (t: string) => getCurrentWindow().setTitle(t).catch(() => {});

  if (phase === "ChampSelect") {
    // 异步获取队伍信息（蓝色方/红色方）追加到标题
    (async () => {
      try {
        const side = await invoke<string | null>("get_map_side");
        console.log("[watch gamePhase] get_map_side result:", side);
        if (side) {
          const sideLabel = side === "blue" ? "蓝色方" : "红色方";
          mapSideLabel.value = sideLabel;
          setTitle(`Yuumi · ${label} - ${sideLabel}`);
          return;
        }
      } catch (e) {
        console.warn("[watch gamePhase] get_map_side failed:", e);
      }
      setTitle(`Yuumi · ${label}`);
    })();
  } else if (phase === "GameStart" || phase === "InProgress") {
    // 游戏加载或进行中时，如果已经存了红蓝方标识，则标题保持带红蓝方的格式，否则尝试再异步拉取一次（如中途重启）
    if (mapSideLabel.value) {
      setTitle(`Yuumi · ${label} - ${mapSideLabel.value}`);
    } else {
      (async () => {
        try {
          const side = await invoke<string | null>("get_map_side");
          if (side) {
            const sideLabel = side === "blue" ? "蓝色方" : "红色方";
            mapSideLabel.value = sideLabel;
            setTitle(`Yuumi · ${label} - ${sideLabel}`);
            return;
          }
        } catch { /* ignore */ }
        setTitle(`Yuumi · ${label}`);
      })();
    }
  } else {
    // 离开活跃对局（如 Lobby, None, EndOfGame 等）时清除阵营数据并复原标题
    mapSideLabel.value = "";
    setTitle(title);
  }

  // 进入选人/游戏加载/游戏中时自动跳转到对局信息页
  if (phase === "ChampSelect" || phase === "GameStart" || phase === "InProgress") {
    console.log("[watch gamePhase] navigating to gameinfo");
    currentPage.value = "gameinfo";

    if (phase === "ChampSelect") {
      const runAutoShow = async () => {
        try {
          const cfg = appConfig.value || await fetchConfig();
          if (cfg?.Functions?.AutoShowOpgg) {
            openOpggWindow();
          }
        } catch (e) {
          console.warn("读取配置用于自动弹出 OP.GG 失败:", e);
        }
      };
      runAutoShow();
    }
  }

  // ─── 大乱斗板凳席悬浮窗生命周期控制 ───
  if (phase === "ChampSelect") {
    console.log("[bench-debug] 进入选人阶段，等待 1.5 秒后检测大乱斗板凳席状态...");
    setTimeout(async () => {
      const session = store.champSelectSession;
      console.log("[bench-debug] 1.5秒检测结果: session 存在 =", !!session, "benchEnabled =", session?.benchEnabled);
      if (session && session.benchEnabled) {
        console.log("[bench-debug] 满足创建悬浮窗条件，准备开启...");
        await showBenchOverlay();
      } else {
        console.log("[bench-debug] 不满足创建悬浮窗条件: session 为空或 benchEnabled 为 false");
      }
    }, 1500);
  } else {
    // 离开选人阶段，关闭悬浮窗
    showBenchOverlay(false);
  }
});

// 动态监听选人会话变化，双重保证在大乱斗模式（板凳席开启）时自动拉起悬浮窗
watch(() => store.champSelectSession, async (session) => {
  if (isOverlayWindow.value) return;
  console.log("[bench-debug] watch(session) 触发：session 存在 =", !!session, "benchEnabled =", session?.benchEnabled);
  if (store.gamePhase === "ChampSelect" && session && session.benchEnabled) {
    console.log("[bench-debug] watch(session) 条件满足，准备开启悬浮窗...");
    await showBenchOverlay();
  }
}, { deep: true });

function handleReconnect() {
  initLcuListeners();
  // 先查询当前游戏阶段，按情况处理
  lcuRequest<string>("GET", "/lol-gameflow/v1/gameflow-phase").then(resp => {
    if (!resp.success) {
      showToast("LCU 未连接，请先启动英雄联盟客户端", "success");
      return;
    }
    const phase = resp.data;
    if (phase === "InProgress" || phase === "GameStart" || phase === "Reconnect") {
      // 游戏中 → 调用 reconnect API
      lcuRequest("POST", "/lol-gameflow/v1/reconnect").then(r => {
        if (r.success) {
          showToast("🔄 已触发游戏重连");
        } else {
          showToast("重连请求失败: " + (r.error || ""), "success");
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
  <n-config-provider :theme-overrides="themeOverrides" :theme="isDarkTheme ? darkTheme : null">
    <n-message-provider>
      <n-dialog-provider>
        <NaiveUIBridge />

  <!-- 如果是悬浮窗窗口，仅渲染悬浮窗组件 -->
  <div v-if="isOverlayWindow" class="overlay-container">
    <BenchOverlay />
  </div>

  <!-- 否则渲染常规的主程序界面 -->
  <div v-else class="app-layout">

    <!-- 自动更新弹窗 -->
    <UpdateDialog
      v-if="updateInfo"
      :update-info="updateInfo"
      @dismiss="updateInfo = null"
    />

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
            · {{ PHASE_LABELS[gamePhase] || gamePhase }}<span v-if="mapSideLabel"> - {{ mapSideLabel }}</span>
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

        <div v-if="!appConfig?.Functions?.HideTft" :class="['nav-item', { active: currentPage === 'tft' }]" @click="navigate('tft')" title="Teamfight Tactics">
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

        <div :class="['nav-item', { active: currentPage === 'notice' }]" @click="navigate('notice')" title="更新日志">
          <span class="nav-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9M13.73 21a2 2 0 0 1-3.46 0"/>
            </svg>
          </span>
          <span class="nav-label">更新日志</span>
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
      <!-- GameInfo 用 v-show 保持状态，切页面不清空数据 -->
      <div v-show="currentPage === 'gameinfo'" style="flex:1;display:flex;flex-direction:column;overflow-y:auto;min-height:0;">
        <GameInfo />
      </div>
      <template v-if="currentPage !== 'gameinfo'">
        <Search v-if="currentPage === 'search'" />
        <Home v-else-if="currentPage === 'home'" @navigate="navigate" />
        <Career v-else-if="currentPage === 'career'" />
        <TFT v-else-if="currentPage === 'tft' && !appConfig?.Functions?.HideTft" />
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
              <span>OP.GG 代理地址: 127.0.0.1:7897</span>
            </div>
            <p class="hint">您可以在系统设置中开启"对局自动上传战绩"，该功能将静默安全地同步数据。</p>
          </div>
        </div>

        <!-- 更新日志页 -->
        <Notice v-else-if="currentPage === 'notice'" />
      </template>
    </main>
    </div>
  </div>
      </n-dialog-provider>
    </n-message-provider>
  </n-config-provider>
</template>

<style>
/* 全局 Naive UI 折叠面板卡片化定制与完美右对齐 */
.collapse-card {
  padding: 0 !important;
  border: 1px solid var(--settings-card-border, var(--border-color)) !important;
  border-radius: 12px !important;
  overflow: hidden !important;
  background: var(--settings-card-bg, var(--card-bg)) !important;
  margin-bottom: 12px !important; /* 增加卡片底部间距 */
  box-shadow: var(--shadow-sm) !important;
}
.collapse-card .n-collapse-item__header {
  padding: 16px 24px !important;
  font-size: 0.88rem !important;
  font-weight: bold !important;
  color: var(--text-color) !important;
  transition: background-color 0.25s !important;
  display: flex !important;
  width: 100% !important;
  box-sizing: border-box !important;
  justify-content: space-between !important;
  align-items: center !important;
}
.collapse-card .n-collapse-item__header:hover {
  background-color: var(--settings-card-bg-hover, var(--card-bg-hover)) !important;
}
.collapse-card .n-collapse-item__header-main {
  flex: 1 !important;
  display: flex !important;
  align-items: center !important;
  width: 100% !important;
  min-width: 0 !important;
}
.collapse-card .n-collapse-item__header-extra {
  margin-left: auto !important;
  margin-right: 12px !important;
  display: none !important; /* 完全隐藏自带的以使手写插槽对齐生效 */
}
.collapse-card .n-collapse-item__content {
  padding: 20px 48px !important;
  border-top: 1px dashed var(--border-color) !important;
  background-color: rgba(0, 0, 0, 0.015) !important;
  width: 100% !important;
  box-sizing: border-box !important;
  display: flex !important;
  flex-direction: column !important;
  align-items: stretch !important;
  gap: 8px !important;
}
.collapse-card .n-collapse-item__content-inner {
  width: 100% !important;
  box-sizing: border-box !important;
  display: flex !important;
  flex-direction: column !important;
  align-items: stretch !important;
  gap: 8px !important;
}

.setting-label {
  flex-shrink: 0 !important;
  white-space: nowrap !important;
}

.collapse-card .n-color-picker {
  width: 100px !important;
  flex-shrink: 0 !important;
}

/* 统一的手写头部 flex 排版布局，确保 100% 撑开并右对齐状态文本 */
.collapse-header-wrapper {
  display: flex !important;
  justify-content: space-between !important;
  align-items: center !important;
  width: 100% !important;
  flex: 1 !important;
  padding-right: 8px !important;
}
.collapse-left {
  display: flex !important;
  align-items: center !important;
  gap: 16px !important;
}
.collapse-left-simple {
  display: flex !important;
  flex-direction: column !important;
  flex: 1 !important;
}
.collapse-right-status {
  flex-shrink: 0 !important;
  margin-left: 12px !important;
  color: var(--primary-color) !important;
  font-weight: 600 !important;
}

/* 设置与工具页面中的输入/按钮行右对齐重写 */
.input-row {
  display: flex !important;
  width: 100% !important;
  justify-content: flex-end !important;
  align-items: center !important;
  gap: 8px !important;
}
/* 1. 外观装饰统一归并 (卡片化底色与内缩边距) */
.setting-input-row,
.setting-row,
.path-actions,
.color-pickers-row,
.reset-row,
.setting-picker-row {
  background: var(--settings-row-bg) !important;
  border: 1px solid var(--settings-row-border) !important;
  padding: 12px 20px !important;
  border-radius: 8px !important;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.01) !important;
  box-sizing: border-box !important;
  margin-bottom: 0 !important;
  transition: all 0.2s cubic-bezier(0.25, 0.8, 0.25, 1) !important;
}
.setting-input-row:hover,
.setting-row:hover,
.path-actions:hover,
.color-pickers-row:hover,
.reset-row:hover,
.setting-picker-row:hover {
  background: var(--settings-row-bg-hover) !important;
  border-color: var(--settings-row-border-hover) !important;
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.02) !important;
  transform: translateY(-0.5px);
}

/* 2. 布局特性各自保留 */
.setting-input-row,
.setting-row {
  display: flex !important;
  width: 100% !important;
  justify-content: space-between !important;
  align-items: center !important;
  gap: 16px !important;
}
.setting-row.justify-end {
  justify-content: flex-end !important;
}
.path-actions {
  display: flex !important;
  width: 100% !important;
  justify-content: flex-end !important;
  align-items: center !important;
  gap: 12px !important;
}
.color-pickers-row {
  display: flex !important;
  width: 100% !important;
  justify-content: flex-end !important;
  align-items: center !important;
  gap: 16px !important;
  flex-wrap: wrap !important;
}
.reset-row {
  display: flex !important;
  width: 100% !important;
  justify-content: flex-end !important;
  align-items: center !important;
}
.setting-picker-row {
  display: flex !important;
  width: 100% !important;
  justify-content: flex-end !important;
  align-items: center !important;
}
.setting-picker-row .champion-picker {
  width: 100% !important;
}
.setting-picker-row .picker-trigger {
  justify-content: flex-end !important;
}
.setting-picker-row .selected-chips {
  flex: none !important;
  display: flex !important;
  justify-content: flex-end !important;
}

:root {
  --primary-color: #00d2c4;
  --primary-color-hover: #00b3a7;
  --primary-color-alpha-15: rgba(0, 210, 196, 0.15);
  --primary-color-alpha-30: rgba(0, 210, 196, 0.3);
  --primary-color-alpha-40: rgba(0, 210, 196, 0.4);

  /* 纯白水晶极光主题变量 */
  --bg-color-gradient: linear-gradient(135deg, #f8fafc 0%, #f1f5f9 50%, #e2e8f0 100%);
  --bg-color: #f8fafc;
  --sidebar-bg: rgba(255, 255, 255, 0.75);
  --card-bg: rgba(255, 255, 255, 0.7);
  --card-bg-hover: rgba(255, 255, 255, 0.9);
  --border-color: rgba(0, 0, 0, 0.05);
  --border-color-hover: rgba(0, 210, 196, 0.25);
  --hover-bg: rgba(0, 0, 0, 0.03);
  --hover-bg-strong: rgba(0, 0, 0, 0.06);
  --titlebar-bg: rgba(255, 255, 255, 0.8);
  
  --text-color: #0f172a;
  --text-muted: #475569;
  --text-dimmed: #64748b;

  --win-color: #10b981;
  --win-bg: rgba(16, 185, 129, 0.08);
  --win-border: rgba(16, 185, 129, 0.2);
  --win-glow: rgba(16, 185, 129, 0.06);

  --loss-color: #f43f5e;
  --loss-bg: rgba(244, 63, 94, 0.08);
  --loss-border: rgba(244, 63, 94, 0.2);
  --loss-glow: rgba(244, 63, 94, 0.06);
  --death-color: #f43f5e;
  --accent-color: #f59e0b;
  --accent-bg: rgba(245, 158, 11, 0.08);
  --tier-blue: #3b82f6;
  --tier-blue-bg: rgba(59, 130, 246, 0.08);
  --tier-blue-border: rgba(59, 130, 246, 0.15);

  --font-sans: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
  --radius-sm: 6px;
  --radius-md: 10px;
  --radius-lg: 16px;
  --shadow-sm: 0 1px 3px rgba(0,0,0,0.05), 0 1px 2px rgba(0,0,0,0.02);
  --shadow-md: 0 4px 20px -2px rgba(0, 0, 0, 0.05), 0 2px 8px -1px rgba(0, 0, 0, 0.03);
  --shadow-lg: 0 20px 25px -5px rgba(0,0,0,0.08), 0 10px 10px -5px rgba(0,0,0,0.04);
  --glass-filter: blur(20px) saturate(190%);

  /* Settings UI 亮色子行卡片变量 */
  --settings-row-bg: rgba(255, 255, 255, 0.45);
  --settings-row-bg-hover: rgba(255, 255, 255, 0.75);
  --settings-row-border: rgba(0, 0, 0, 0.04);
  --settings-row-border-hover: rgba(0, 0, 0, 0.08);
}

[data-theme="dark"] {
  /* 暗黑海克斯水晶主题变量 */
  --bg-color-gradient: linear-gradient(135deg, #0b0f19 0%, #111827 50%, #172033 100%);
  --bg-color: #0b0f19;
  --sidebar-bg: rgba(17, 24, 39, 0.75);
  --card-bg: rgba(30, 41, 59, 0.55);
  --card-bg-hover: rgba(30, 41, 59, 0.75);
  --border-color: rgba(255, 255, 255, 0.06);
  --border-color-hover: rgba(0, 210, 196, 0.35);
  --hover-bg: rgba(255, 255, 255, 0.04);
  --hover-bg-strong: rgba(255, 255, 255, 0.08);
  --titlebar-bg: rgba(11, 15, 25, 0.8);
  
  --text-color: #f8fafc;
  --text-muted: #cbd5e1;
  --text-dimmed: #94a3b8;

  --win-color: #34d399;
  --win-bg: rgba(52, 211, 153, 0.12);
  --win-border: rgba(52, 211, 153, 0.25);
  --win-glow: rgba(52, 211, 153, 0.1);

  --loss-color: #fb7185;
  --loss-bg: rgba(251, 113, 133, 0.12);
  --loss-border: rgba(251, 113, 133, 0.25);
  --loss-glow: rgba(251, 113, 133, 0.1);
  --death-color: #fb7185;
  --accent-color: #fbbf24;
  --accent-bg: rgba(251, 191, 36, 0.12);
  --tier-blue: #60a5fa;
  --tier-blue-bg: rgba(96, 165, 250, 0.12);
  --tier-blue-border: rgba(96, 165, 250, 0.25);

  --shadow-sm: 0 1px 3px rgba(0,0,0,0.3);
  --shadow-md: 0 10px 25px -5px rgba(0,0,0,0.4), 0 8px 10px -6px rgba(0,0,0,0.4);
  --shadow-lg: 0 20px 25px -5px rgba(0,0,0,0.5), 0 10px 10px -5px rgba(0,0,0,0.5);
  --glass-filter: blur(25px) saturate(200%);

  /* Settings UI 暗色专用 */
  --toggle-track-off: rgba(255, 255, 255, 0.08);
  --toggle-slider: #ffffff;
  --toggle-glow: 0 0 14px rgba(0, 210, 196, 0.4);
  --segmented-bg: rgba(255, 255, 255, 0.05);
  --card-glow-hover: 0 0 0 1px rgba(0, 210, 196, 0.35), 0 8px 24px rgba(0, 0, 0, 0.4);
  --settings-card-bg: rgba(24, 34, 54, 0.7);
  --settings-card-bg-hover: rgba(30, 41, 64, 0.85);
  --settings-card-border: rgba(255, 255, 255, 0.06);
  --settings-card-border-hover: rgba(0, 210, 196, 0.4);
  --settings-collapse-bg: rgba(17, 24, 39, 0.8);
  --settings-separator: rgba(255, 255, 255, 0.04);

  /* Settings UI 暗色子行卡片变量 */
  --settings-row-bg: rgba(255, 255, 255, 0.04);
  --settings-row-bg-hover: rgba(255, 255, 255, 0.08);
  --settings-row-border: rgba(255, 255, 255, 0.03);
  --settings-row-border-hover: rgba(255, 255, 255, 0.08);
}

[data-theme="light"] {
  /* Settings UI 亮色专用 */
  --toggle-track-off: rgba(0, 0, 0, 0.1);
  --toggle-slider: #ffffff;
  --toggle-glow: 0 0 10px rgba(0, 210, 196, 0.35);
  --segmented-bg: rgba(0, 0, 0, 0.04);
  --card-glow-hover: 0 0 0 1px rgba(0, 210, 196, 0.3), 0 4px 12px rgba(0, 0, 0, 0.05);
  --settings-card-bg: rgba(255, 255, 255, 0.8);
  --settings-card-bg-hover: rgba(255, 255, 255, 0.95);
  --settings-card-border: rgba(0, 0, 0, 0.06);
  --settings-card-border-hover: rgba(0, 210, 196, 0.5);
  --settings-collapse-bg: rgba(243, 244, 246, 0.8);
  --settings-separator: rgba(0, 0, 0, 0.04);

  /* Settings UI 亮色子行卡片变量 */
  --settings-row-bg: rgba(255, 255, 255, 0.45);
  --settings-row-bg-hover: rgba(255, 255, 255, 0.75);
  --settings-row-border: rgba(0, 0, 0, 0.04);
  --settings-row-border-hover: rgba(0, 0, 0, 0.08);
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

/* 悬浮窗容器：全透明背景，无滚动 */
.overlay-container {
  width: 100vw;
  height: 100vh;
  background: transparent !important;
  overflow: hidden;
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
  background: var(--border-color);
  border-radius: 4px;
  transition: background 0.2s;
}
::-webkit-scrollbar-thumb:hover {
  background: var(--border-color-hover);
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
  background: var(--titlebar-bg);
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
  background-color: var(--hover-bg);
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
  background-color: var(--hover-bg);
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
  gap: 6px;
  padding: 8px;
  flex: 1;
}

.nav-item {
  display: flex;
  align-items: center;
  height: 42px;
  padding: 0 14px;
  border-radius: var(--radius-md);
  cursor: pointer;
  color: var(--text-muted);
  position: relative;
  border: 1px solid transparent;
  transition: all 0.25s cubic-bezier(0.25, 0.8, 0.25, 1);
}

.sidebar.collapsed .nav-item {
  justify-content: center;
  padding: 0;
}

.nav-item:hover {
  background-color: var(--hover-bg);
  color: var(--text-color);
  border-color: var(--border-color);
  transform: translateY(-1px);
}

.sidebar.collapsed .nav-item:hover {
  transform: none;
}

.nav-item.active {
  background-color: var(--primary-color-alpha-15);
  color: var(--text-color);
  font-weight: 600;
  border-color: var(--primary-color-alpha-30);
  box-shadow: 0 4px 12px var(--primary-color-alpha-15);
}

/* 左侧指示发光条 */
.nav-item.active::before {
  content: "";
  position: absolute;
  left: 0;
  top: 10px;
  bottom: 10px;
  width: 4px;
  background-color: var(--primary-color);
  border-radius: 0 4px 4px 0;
  box-shadow: 0 0 10px var(--primary-color);
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
  transition: transform 0.25s ease;
}

.nav-item:hover .nav-icon {
  transform: scale(1.1) rotate(2deg);
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
  font-size: 0.88rem;
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
  gap: 6px;
  padding: 8px;
  border-top: 1px solid var(--border-color);
}

/* 召唤师信息卡片 */
.user-card {
  display: flex;
  align-items: center;
  height: 52px;
  padding: 0 10px;
  margin: 8px 0;
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: all 0.25s ease;
  overflow: hidden;
  border: 1px solid transparent;
  background: rgba(255, 255, 255, 0.02);
}

.sidebar.collapsed .user-card {
  justify-content: center;
  padding: 0;
}

.user-card:hover {
  background-color: var(--hover-bg-strong);
  border-color: var(--border-color-hover);
  box-shadow: var(--shadow-sm);
}

.user-avatar {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  overflow: hidden;
  flex-shrink: 0;
  border: 1.5px solid var(--border-color);
  transition: all 0.25s ease;
}

.user-card:hover .user-avatar {
  border-color: var(--primary-color);
  box-shadow: 0 0 8px var(--primary-color-alpha-40);
  transform: scale(1.05);
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
  font-weight: 700;
  color: var(--text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.user-region {
  font-size: 0.68rem;
  color: var(--text-muted);
}

/* 右侧内容区域 */
.content-wrapper {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  background-color: transparent;
}

/* 占位页面样式 */
.placeholder-view {
  padding: 3rem 2rem;
  max-width: 840px;
  margin: 0 auto;
}

.view-header {
  margin-bottom: 2rem;
  border-bottom: 1px solid var(--border-color);
  padding-bottom: 1.2rem;
}

.view-header h2 {
  font-size: 1.75rem;
  margin: 0;
  font-weight: 800;
  color: var(--text-color);
  letter-spacing: 0.5px;
}

.view-card {
  background: var(--card-bg);
  border-radius: var(--radius-lg);
  padding: 3.5rem 2.5rem;
  text-align: center;
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
  border: 1px solid var(--border-color);
  box-shadow: var(--shadow-md);
  transition: all 0.35s cubic-bezier(0.25, 0.8, 0.25, 1);
}

.view-card:hover {
  border-color: var(--primary-color-alpha-40);
  box-shadow: 0 12px 30px -10px var(--primary-color-alpha-15), var(--shadow-lg);
  transform: translateY(-4px);
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

</style>
