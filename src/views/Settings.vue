<script setup lang="ts">
import { ref, onMounted, onUnmounted, inject, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getVersion } from "@tauri-apps/api/app";
import { fetchConfig, updateConfig } from "../api/lcu";
import type { AppConfig } from "../api/lcu";
import { updateThemeColor, updateDeathColor } from "../utils/theme";
import ColorPicker from "../components/ColorPicker.vue";
import UpdateDialog, { type UpdateInfo } from "../components/UpdateDialog.vue";

const config = inject<Ref<AppConfig | null>>("appConfig") || ref<AppConfig | null>(null);

// 当前版本号
const appVersion = ref("");

// 手动检查更新状态
const checkingUpdate = ref(false);
const updateInfo = ref<UpdateInfo | null>(null);

async function manualCheckUpdate() {
  if (checkingUpdate.value) return;
  checkingUpdate.value = true;
  try {
    const result = await invoke<UpdateInfo | null>("check_update");
    if (result) {
      updateInfo.value = result;
    } else {
      showToast("已是最新版本！", "success");
    }
  } catch (e: any) {
    showToast("检查更新失败: " + String(e), "error");
  } finally {
    checkingUpdate.value = false;
  }
}

// Toast 通知
const toast = ref<{ message: string; type: 'success' | 'error'; visible: boolean }>({
  message: '', type: 'success', visible: false
});
let toastTimer: ReturnType<typeof setTimeout> | null = null;
function showToast(message: string, type: 'success' | 'error' = 'success') {
  if (toastTimer) clearTimeout(toastTimer);
  toast.value = { message, type, visible: true };
  toastTimer = setTimeout(() => { toast.value.visible = false; }, 2000);
}

// 手风琴折叠状态管理
const activeCollapse = ref<string | null>(null);
function toggleCollapse(panelName: string) {
  activeCollapse.value = activeCollapse.value === panelName ? null : panelName;
}

// 自定义下拉状态
const showLogLevelDropdown = ref(false);
const showDpiDropdown = ref(false);
const showLangDropdown = ref(false);
const showThemeModeDropdown = ref(false);
function closeAllDropdowns() {
  showLogLevelDropdown.value = false;
  showDpiDropdown.value = false;
  showLangDropdown.value = false;
  showThemeModeDropdown.value = false;
}

// ─── 自动保存（防抖 500ms）───
let saveDebounce: ReturnType<typeof setTimeout> | null = null;
let skipAutoSaveToast = false;
function autoSave() {
  if (!config.value) return;
  if (saveDebounce) clearTimeout(saveDebounce);
  saveDebounce = setTimeout(async () => {
    try {
      await updateConfig(config.value!);
      if (!skipAutoSaveToast) {
        showToast('设置已自动保存');
      }
      skipAutoSaveToast = false;
    } catch (e) {
      showToast('保存失败', 'error');
    }
  }, 500);
}

// SignalR 连接状态
const signalrStatus = ref<'disconnected' | 'connecting' | 'connected' | 'error'>('disconnected');
const signalrError = ref('');
const unlistenFns = ref<Array<() => void>>([]);

onMounted(async () => {
  document.addEventListener("click", closeAllDropdowns);

  // 获取当前应用版本号
  try {
    appVersion.value = await getVersion();
  } catch (e) {
    console.warn("获取版本号失败:", e);
  }

  if (!config.value) {
    try {
      config.value = await fetchConfig();
    } catch (e) {
      console.error("加载系统配置失败:", e);
    }
  }
  if (config.value && config.value.Personalization && config.value.Personalization.ThemeColor) {
    updateThemeColor(config.value.Personalization.ThemeColor);
  }

  // 获取初始 SignalR 状态
  try {
    signalrStatus.value = await invoke<any>("get_signalr_status");
  } catch (e) {
    console.error("获取 SignalR 状态失败:", e);
  }

  // 监听后端 SignalR 事件
  try {
    const unConnecting = await listen("signalr-connecting", () => {
      signalrStatus.value = "connecting";
      signalrError.value = "";
    });
    const unConnected = await listen("signalr-connected", () => {
      signalrStatus.value = "connected";
      signalrError.value = "";
    });
    const unDisconnected = await listen("signalr-disconnected", () => {
      signalrStatus.value = "disconnected";
    });
    const unError = await listen<string>("signalr-error", (event) => {
      signalrStatus.value = "error";
      signalrError.value = event.payload;
    });

    unlistenFns.value.push(unConnecting);
    unlistenFns.value.push(unConnected);
    unlistenFns.value.push(unDisconnected);
    unlistenFns.value.push(unError);
  } catch (e) {
    console.error("注册 SignalR 监听器失败:", e);
  }
});

onUnmounted(() => {
  document.removeEventListener("click", closeAllDropdowns);
  unlistenFns.value.forEach(fn => fn());
});

// 自动检测客户端路径（追加到列表）
async function handleDetectPath() {
  try {
    const path = await invoke<string | null>("detect_lol_path");
    if (path) {
      if (!config.value) return;
      const paths = config.value.General.LolPath || [];
      if (!paths.includes(path)) {
        paths.push(path);
        config.value.General.LolPath = paths;
        await updateConfig(config.value);
        showToast('已添加: ' + path);
      } else {
        showToast('该路径已存在');
      }
    } else {
      showToast('未检测到运行中的英雄联盟客户端', 'error');
    }
  } catch (e: any) {
    showToast('检测失败: ' + e.toString(), 'error');
  }
}

// 手动选择客户端目录（追加到列表）
async function handleBrowseFolder() {
  try {
    const path = await invoke<string | null>("select_lol_folder");
    if (path) {
      if (!config.value) return;
      const paths = config.value.General.LolPath || [];
      if (!paths.includes(path)) {
        paths.push(path);
        config.value.General.LolPath = paths;
        await updateConfig(config.value);
        showToast('已添加: ' + path);
      } else {
        showToast('该路径已存在');
      }
    }
  } catch (e: any) {
    showToast('选择失败: ' + e.toString(), 'error');
  }
}

// 删除指定路径
async function handleRemovePath(index: number) {
  if (!config.value) return;
  config.value.General.LolPath.splice(index, 1);
  await updateConfig(config.value);
}

// 修改指定路径
async function handleEditPath(index: number, e: Event) {
  if (!config.value) return;
  const val = (e.target as HTMLInputElement).value.trim();
  if (!val) return;
  config.value.General.LolPath[index] = val;
  await updateConfig(config.value);
}

// 清除缓存
async function handleClearCache() {
  if (!confirm("确定要清除所有游戏资源缓存吗？")) return;
  try {
    const result = await invoke<string>("clear_game_cache");
    showToast(result);
  } catch (e: any) {
    showToast('清除缓存失败', 'error');
  }
}

// 打开日志文件夹
async function handleOpenLogFolder() {
  try {
    await invoke("open_log_folder");
  } catch (e: any) {
    showToast('打开日志文件夹失败', 'error');
  }
}

function toColor6(color: string | undefined): string {
  if (!color) return "#000000";
  if (color.startsWith('#') && color.length === 9) {
    return '#' + color.slice(3);
  }
  return color;
}

function toColor8(color: string): string {
  if (color.startsWith('#') && color.length === 7) {
    return '#ff' + color.slice(1);
  }
  return color;
}

function onThemeColorInput(e: Event) {
  const target = e.target as HTMLInputElement;
  if (config.value?.Personalization) {
    config.value.Personalization.ThemeColor = target.value;
  }
  updateThemeColor(target.value);
}

function onDeathColorInput(e: Event, field: 'LightDeathsNumberColor' | 'DarkDeathsNumberColor') {
  const target = e.target as HTMLInputElement;
  const color8 = toColor8(target.value);
  if (config.value?.Personalization) {
    config.value.Personalization[field] = color8;
  }
  // 实时更新 CSS 变量
  const light = field === 'LightDeathsNumberColor' ? target.value : toColor6(config.value?.Personalization?.LightDeathsNumberColor);
  const dark  = field === 'DarkDeathsNumberColor'  ? target.value : toColor6(config.value?.Personalization?.DarkDeathsNumberColor);
  updateDeathColor(light, dark);
}

const DEFAULT_COLORS = {
  ThemeColor: '#00d2c4',
  WinCardColor: '#1510b981',
  LoseCardColor: '#12f43f5e',
  RemakeCardColor: '#1294a3b8',
  LightDeathsNumberColor: '#ffb60000',
  DarkDeathsNumberColor: '#ffff8d8d',
};

function resetThemeColor() {
  if (!config.value?.Personalization) return;
  config.value.Personalization.ThemeColor = DEFAULT_COLORS.ThemeColor;
  updateThemeColor(DEFAULT_COLORS.ThemeColor);
  autoSave();
}

function resetCardColors() {
  if (!config.value?.Personalization) return;
  config.value.Personalization.WinCardColor = DEFAULT_COLORS.WinCardColor;
  config.value.Personalization.LoseCardColor = DEFAULT_COLORS.LoseCardColor;
  config.value.Personalization.RemakeCardColor = DEFAULT_COLORS.RemakeCardColor;
  autoSave();
}

function resetDeathColors() {
  if (!config.value?.Personalization) return;
  config.value.Personalization.LightDeathsNumberColor = DEFAULT_COLORS.LightDeathsNumberColor;
  config.value.Personalization.DarkDeathsNumberColor = DEFAULT_COLORS.DarkDeathsNumberColor;
  updateDeathColor(toColor6(DEFAULT_COLORS.LightDeathsNumberColor), toColor6(DEFAULT_COLORS.DarkDeathsNumberColor));
  autoSave();
}

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

</script>

<template>
  <div class="settings-view">
    <!-- Toast -->
    <Transition name="toast">
      <div v-if="toast.visible" :class="['toast', `toast-${toast.type}`]">{{ toast.message }}</div>
    </Transition>

    <div v-if="!config" class="tip-container">
      <div class="loading-spinner"></div>
      <p class="tip">加载配置数据中...</p>
    </div>

    <div v-else class="settings-container">
      <h1 class="page-title">设置</h1>

      <!-- 1. 功能组 -->
      <div class="group-header">功能</div>

      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('concurrency')">
          <div class="collapse-left">
            <h3 class="card-title">LCU API 并发数</h3>
            <span class="card-desc">该值越大数据加载速度越快，但越可能引起客户端闪退</span>
          </div>
          <div class="collapse-right">
            <span class="status-preview">当前值: {{ config.Functions.ApiConcurrencyNumber }}</span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'concurrency' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'concurrency'" class="collapse-content">
          <div class="input-row">
            <input type="number" v-model.number="config.Functions.ApiConcurrencyNumber" class="number-input" min="1" max="10" @change="autoSave" />
          </div>
        </div>
      </div>

      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('matchcount')">
          <div class="collapse-left">
            <h3 class="card-title">默认对局数量</h3>
            <span class="card-desc">调整在个人生涯界面中显示的最大对局数量</span>
          </div>
          <div class="collapse-right">
            <span class="status-preview">当前值: {{ config.Functions.CareerGamesNumber }}</span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'matchcount' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'matchcount'" class="collapse-content">
          <div class="input-row">
            <input type="number" v-model.number="config.Functions.CareerGamesNumber" class="number-input" min="1" max="100" step="5" @change="autoSave" />
          </div>
        </div>
      </div>

      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">对局信息过滤</h3>
          <span class="card-desc">基于你当前游戏模式（地图/队列）筛选战绩，只显示相同模式的玩家历史数据</span>
        </div>
        <div class="card-right">
          <div :class="['toggle-switch', config.Functions.GameInfoFilter ? 'on' : 'off']" @click="config.Functions.GameInfoFilter = !config.Functions.GameInfoFilter; autoSave()">
            <span class="toggle-text">{{ config.Functions.GameInfoFilter ? '开' : '关' }}</span>
            <span class="toggle-slider"></span>
          </div>
        </div>
      </div>

      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">保留对局信息界面内容</h3>
          <span class="card-desc">保留上一局的对局信息内容直到下一次对局开始</span>
        </div>
        <div class="card-right">
          <div :class="['toggle-switch', config.Functions.EnableReserveGameinfo ? 'on' : 'off']" @click="config.Functions.EnableReserveGameinfo = !config.Functions.EnableReserveGameinfo; autoSave()">
            <span class="toggle-text">{{ config.Functions.EnableReserveGameinfo ? '开' : '关' }}</span>
            <span class="toggle-slider"></span>
          </div>
        </div>
      </div>

      <div class="card-item">
        <div class="card-left">
          <h3 class="card-title">对局详情中显示段位</h3>
          <span class="card-desc">在搜索界面对局详情界面中显示段位，启动该选项将影响加载界面的速度</span>
        </div>
        <div class="card-right">
          <div :class="['toggle-switch', config.Functions.ShowTierInGameInfo ? 'on' : 'off']" @click="config.Functions.ShowTierInGameInfo = !config.Functions.ShowTierInGameInfo; autoSave()">
            <span class="toggle-text">{{ config.Functions.ShowTierInGameInfo ? '开' : '关' }}</span>
            <span class="toggle-slider"></span>
          </div>
        </div>
      </div>

      <!-- 2. OP.GG -->
      <div class="group-header">OP.GG</div>

      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">自动显示 OP.GG 窗口</h3>
          <span class="card-desc">在英雄选择开始时自动显示 OP.GG 窗口</span>
        </div>
        <div class="card-right">
          <div :class="['toggle-switch', config.Functions.AutoShowOpgg ? 'on' : 'off']" @click="config.Functions.AutoShowOpgg = !config.Functions.AutoShowOpgg; autoSave()">
            <span class="toggle-text">{{ config.Functions.AutoShowOpgg ? '开' : '关' }}</span>
            <span class="toggle-slider"></span>
          </div>
        </div>
      </div>

      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">置顶 OP.GG 窗口</h3>
          <span class="card-desc">在英雄选择时将 OP.GG 窗口置顶</span>
        </div>
        <div class="card-right">
          <div :class="['toggle-switch', config.Functions.EnableOpggOnTop ? 'on' : 'off']" @click="config.Functions.EnableOpggOnTop = !config.Functions.EnableOpggOnTop; autoSave()">
            <span class="toggle-text">{{ config.Functions.EnableOpggOnTop ? '开' : '关' }}</span>
            <span class="toggle-slider"></span>
          </div>
        </div>
      </div>

      <div class="collapse-item">
        <div class="collapse-header" @click="toggleCollapse('opggproxy')">
          <div class="collapse-left">
            <h3 class="card-title">OP.GG HTTP 代理</h3>
            <span class="card-desc">连接 OP.GG 时启用 HTTP 代理</span>
          </div>
          <div class="collapse-right">
            <span class="status-preview">{{ config.General.EnableOpggProxy ? '已启用' : '未启用' }}</span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'opggproxy' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'opggproxy'" class="collapse-content">
          <div class="input-row align-center">
            <div :class="['toggle-switch', config.General.EnableOpggProxy ? 'on' : 'off']" @click="config.General.EnableOpggProxy = !config.General.EnableOpggProxy; autoSave()">
              <span class="toggle-text">{{ config.General.EnableOpggProxy ? '开' : '关' }}</span>
              <span class="toggle-slider"></span>
            </div>
            <input v-model="config.General.OpggProxyAddr" placeholder="127.0.0.1:10809" class="text-input" :disabled="!config.General.EnableOpggProxy" @change="autoSave" />
          </div>
        </div>
      </div>

      <!-- 3. 通用 -->
      <div class="group-header">通用</div>

      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('lolpath')">
          <div class="collapse-left">
            <h3 class="card-title">客户端路径</h3>
            <span class="card-desc">{{ config?.General?.LolPath?.length ? `已设置 ${config.General.LolPath.length} 个路径` : '未设置' }}</span>
          </div>
          <div class="collapse-right">
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'lolpath' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'lolpath'" class="collapse-content">
          <!-- 已保存的路径列表 -->
          <div v-for="(path, index) in (config?.General?.LolPath || [])" :key="index" class="path-item">
            <input class="path-input" :value="path" @change="handleEditPath(index, $event)" placeholder="客户端安装路径" />
            <button class="path-remove-btn" @click="handleRemovePath(index)" title="删除">✕</button>
          </div>
          <div v-if="!config?.General?.LolPath?.length" class="path-empty">暂无已保存的客户端路径</div>
          <!-- 操作按钮 -->
          <div class="path-actions">
            <button class="action-btn" @click="handleDetectPath">自动检测</button>
            <button class="action-btn" @click="handleBrowseFolder">添加目录</button>
          </div>
        </div>
      </div>

      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">自动启动游戏</h3>
          <span class="card-desc">启动 Yuumi 时自动启动 LOL 客户端</span>
        </div>
        <div class="card-right">
          <div :class="['toggle-switch', config.General.EnableStartLolWithApp ? 'on' : 'off']" @click="config.General.EnableStartLolWithApp = !config.General.EnableStartLolWithApp; autoSave()">
            <span class="toggle-text">{{ config.General.EnableStartLolWithApp ? '开' : '关' }}</span>
            <span class="toggle-slider"></span>
          </div>
        </div>
      </div>

      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">清除缓存</h3>
          <span class="card-desc">删除所有游戏资源的缓存（建议在游戏资源有更新时使用）</span>
        </div>
        <div class="card-right">
          <button class="action-btn" @click="handleClearCache">删除</button>
        </div>
      </div>

      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">最小化到任务栏托盘</h3>
          <span class="card-desc">点击右上角关闭时将程序最小化到托盘</span>
        </div>
        <div class="card-right">
          <div :class="['toggle-switch', config.General.EnableCloseToTray ? 'on' : 'off']" @click="config.General.EnableCloseToTray = !config.General.EnableCloseToTray; autoSave()">
            <span class="toggle-text">{{ config.General.EnableCloseToTray ? '开' : '关' }}</span>
            <span class="toggle-slider"></span>
          </div>
        </div>
      </div>

      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">静默启动</h3>
          <span class="card-desc">启动 Yuumi 后最小化窗口到任务栏托盘</span>
        </div>
        <div class="card-right">
          <div :class="['toggle-switch', config.General.EnableGameStartMinimize ? 'on' : 'off']" @click="config.General.EnableGameStartMinimize = !config.General.EnableGameStartMinimize; autoSave()">
            <span class="toggle-text">{{ config.General.EnableGameStartMinimize ? '开' : '关' }}</span>
            <span class="toggle-slider"></span>
          </div>
        </div>
      </div>

      <!-- 云端服务 -->
      <div class="collapse-item">
        <div class="collapse-header" @click="toggleCollapse('upload_and_signalr')">
          <div class="collapse-left">
            <h3 class="card-title">云端服务</h3>
            <span class="card-desc">配置战绩自动上报、LCU 实时数据同步与查询服务</span>
          </div>
          <div class="collapse-right">
            <span class="status-preview">
              {{ config.General.UploadApiUrl ? '已配置上传' : '未配置' }}
              <template v-if="config.Functions.LcuRealtimeEnabled">
                / 
                <span :class="['signalr-status-badge', signalrStatus]">
                  {{ signalrStatus === 'connected' ? '云端已连接' : signalrStatus === 'connecting' ? '云端连接中...' : signalrStatus === 'error' ? '云端连接失败' : '云端未连接' }}
                </span>
              </template>
            </span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'upload_and_signalr' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'upload_and_signalr'" class="collapse-content">
          <div class="setting-input-row">
            <span class="setting-input-label">服务器 API 地址:</span>
            <input v-model="config.General.UploadApiUrl" placeholder="http://example.com" class="text-input" @change="if (config.Functions.LcuRealtimeEnabled && config.General.UploadApiUrl) { signalrStatus = 'connecting'; }; autoSave()" />
          </div>
          <div class="setting-input-row">
            <span class="setting-input-label">LCU 实时查询:</span>
            <div :class="['toggle-switch', config.Functions.LcuRealtimeEnabled ? 'on' : 'off']" @click="config.Functions.LcuRealtimeEnabled = !config.Functions.LcuRealtimeEnabled; if (config.Functions.LcuRealtimeEnabled && config.General.UploadApiUrl) { signalrStatus = 'connecting'; } else { signalrStatus = 'disconnected'; }; autoSave()">
              <span class="toggle-text">{{ config.Functions.LcuRealtimeEnabled ? '开' : '关' }}</span>
              <span class="toggle-slider"></span>
            </div>
          </div>
          <div v-if="signalrStatus === 'error' && signalrError" class="setting-error-tip">
            连接异常: {{ signalrError }}
          </div>
          <div class="setting-input-row">
            <span class="setting-input-label">userid:</span>
            <input v-model="config.General.SignalrUserId" placeholder="留空默认使用 lcu_user_001" class="text-input" @change="autoSave" />
          </div>
        </div>
      </div>

      <!-- 4. 日志 -->
      <div class="group-header">日志</div>

      <div :class="['collapse-item', 'border-bottom', { 'has-dropdown-open': showLogLevelDropdown }]">
        <div class="collapse-header" @click="toggleCollapse('loglevel')">
          <div class="collapse-left">
            <h3 class="card-title">日志等级</h3>
            <span class="card-desc">修改 Yuumi 记录日志的等级（重启后生效）</span>
          </div>
          <div class="collapse-right">
            <div class="dropdown-trigger" @click.stop="showLogLevelDropdown = !showLogLevelDropdown">
              <span>{{ config.General.LogLevel === 0 ? 'Debug' : config.General.LogLevel === 1 ? 'Info' : 'Error' }}</span>
              <svg :class="['arrow-icon', { expanded: showLogLevelDropdown }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
              <div v-if="showLogLevelDropdown" class="dropdown-menu" @click.stop>
                <div :class="['dropdown-item', { active: config.General.LogLevel === 0 }]" @click="config.General.LogLevel = 0; autoSave(); showLogLevelDropdown = false">Debug</div>
                <div :class="['dropdown-item', { active: config.General.LogLevel === 1 }]" @click="config.General.LogLevel = 1; autoSave(); showLogLevelDropdown = false">Info</div>
                <div :class="['dropdown-item', { active: config.General.LogLevel === 2 }]" @click="config.General.LogLevel = 2; autoSave(); showLogLevelDropdown = false">Error</div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div class="card-item">
        <div class="card-left">
          <h3 class="card-title">日志文件</h3>
          <span class="card-desc">&lt;exe 目录&gt;/log/</span>
        </div>
        <div class="card-right">
          <button class="action-btn" @click="handleOpenLogFolder">打开文件夹</button>
        </div>
      </div>

      <!-- 5. 个性化 -->
      <div class="group-header">个性化</div>

      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">云母效果</h3>
          <span class="card-desc">窗口和表面显示半透明（仅在 Win11 上可用）</span>
        </div>
        <div class="card-right">
          <div :class="['toggle-switch', config.Personalization.MicaEnabled ? 'on' : 'off']" @click="config.Personalization.MicaEnabled = !config.Personalization.MicaEnabled; autoSave(); invoke('set_mica_effect', { enabled: config.Personalization.MicaEnabled })">
            <span class="toggle-text">{{ config.Personalization.MicaEnabled ? '开' : '关' }}</span>
            <span class="toggle-slider"></span>
          </div>
        </div>
      </div>

      <!-- 应用主题 -->
      <div :class="['card-item', 'border-bottom', { 'has-dropdown-open': showThemeModeDropdown }]">
        <div class="card-left">
          <h3 class="card-title">应用主题</h3>
          <span class="card-desc">选择 Yuumi 的显示主题</span>
        </div>
        <div class="card-right">
          <div class="dropdown-trigger" @click.stop="showThemeModeDropdown = !showThemeModeDropdown">
            <span>
              {{ config.Personalization.ThemeMode === 'Light' ? '浅色' :
                 config.Personalization.ThemeMode === 'Dark' ? '深色' : '跟随系统' }}
            </span>
            <svg :class="['arrow-icon', { expanded: showThemeModeDropdown }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
            <div v-if="showThemeModeDropdown" class="dropdown-menu" @click.stop>
              <div :class="['dropdown-item', { active: config.Personalization.ThemeMode === 'Light' }]" @click="if(config.Personalization.ThemeMode !== 'Light') { config.Personalization.ThemeMode = 'Light'; applyThemeMode('Light'); autoSave(); }; showThemeModeDropdown = false">浅色</div>
              <div :class="['dropdown-item', { active: config.Personalization.ThemeMode === 'Dark' }]" @click="if(config.Personalization.ThemeMode !== 'Dark') { config.Personalization.ThemeMode = 'Dark'; applyThemeMode('Dark'); autoSave(); }; showThemeModeDropdown = false">深色</div>
              <div :class="['dropdown-item', { active: config.Personalization.ThemeMode === 'Auto' }]" @click="if(config.Personalization.ThemeMode !== 'Auto') { config.Personalization.ThemeMode = 'Auto'; applyThemeMode('Auto'); autoSave(); }; showThemeModeDropdown = false">跟随系统</div>
            </div>
          </div>
        </div>
      </div>

      <!-- 主题色 -->
      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('themecolor')">
          <div class="collapse-left">
            <h3 class="card-title">主题色</h3>
            <span class="card-desc">调整 Yuumi 的主题色</span>
          </div>
          <div class="collapse-right">
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'themecolor' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'themecolor'" class="collapse-content">
          <div class="input-row align-center">
            <label class="color-picker-label">调色盘:</label>
            <input type="color" class="color-picker" :value="toColor6(config.Personalization.ThemeColor)" @input="onThemeColorInput" @change="autoSave" />
          </div>
          <div class="reset-row">
            <button class="action-btn" @click="resetThemeColor">恢复默认</button>
          </div>
        </div>
      </div>

      <!-- 对局卡片颜色 -->
      <div :class="['collapse-item', 'border-bottom', { 'has-dropdown-open': activeCollapse === 'cardcolors' }]">
        <div class="collapse-header" @click="toggleCollapse('cardcolors')">
          <div class="collapse-left">
            <h3 class="card-title">对局卡片颜色</h3>
            <span class="card-desc">改变对局卡片提示胜利 / 失败的颜色</span>
          </div>
          <div class="collapse-right">
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'cardcolors' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'cardcolors'" class="collapse-content">
          <div class="color-pickers-row">
            <div class="color-picker-item">
              <label>胜利卡片:</label>
              <ColorPicker v-model="config.Personalization.WinCardColor" @change="autoSave" />
            </div>
            <div class="color-picker-item">
              <label>失败卡片:</label>
              <ColorPicker v-model="config.Personalization.LoseCardColor" @change="autoSave" />
            </div>
            <div class="color-picker-item">
              <label>重开卡片:</label>
              <ColorPicker v-model="config.Personalization.RemakeCardColor" @change="autoSave" />
            </div>
          </div>
          <div class="reset-row">
            <button class="action-btn" @click="resetCardColors">恢复默认</button>
          </div>
        </div>
      </div>

      <!-- 死亡数字体颜色 -->
      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('deathfontcolor')">
          <div class="collapse-left">
            <h3 class="card-title">死亡数字体颜色</h3>
            <span class="card-desc">改变 KDA 标签中死亡数字的字体颜色</span>
          </div>
          <div class="collapse-right">
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'deathfontcolor' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'deathfontcolor'" class="collapse-content">
          <div class="color-pickers-row">
            <div class="color-picker-item">
              <label>浅色主题下颜色:</label>
              <input type="color" :value="toColor6(config.Personalization.LightDeathsNumberColor)" @input="onDeathColorInput($event, 'LightDeathsNumberColor')" @change="autoSave" />
            </div>
            <div class="color-picker-item">
              <label>深色主题下颜色:</label>
              <input type="color" :value="toColor6(config.Personalization.DarkDeathsNumberColor)" @input="onDeathColorInput($event, 'DarkDeathsNumberColor')" @change="autoSave" />
            </div>
          </div>
          <div class="reset-row">
            <button class="action-btn" @click="resetDeathColors">恢复默认</button>
          </div>
        </div>
      </div>

      <!-- 界面缩放 -->
      <div :class="['collapse-item', 'border-bottom', { 'has-dropdown-open': showDpiDropdown }]">
        <div class="collapse-header" @click.stop>
          <div class="collapse-left">
            <h3 class="card-title">界面缩放</h3>
            <span class="card-desc">调整部件和字体的大小（重启后生效）</span>
          </div>
          <div class="collapse-right">
            <div class="dropdown-trigger" @click.stop="showDpiDropdown = !showDpiDropdown">
              <span>{{ config.Personalization.DpiScale === 'Auto' ? '跟随系统' : config.Personalization.DpiScale + '%' }}</span>
              <svg :class="['arrow-icon', { expanded: showDpiDropdown }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
              <div v-if="showDpiDropdown" class="dropdown-menu" @click.stop>
                <div :class="['dropdown-item', { active: config.Personalization.DpiScale === 'Auto' }]" @click="if(config.Personalization.DpiScale !== 'Auto') { config.Personalization.DpiScale = 'Auto'; skipAutoSaveToast = true; autoSave(); showToast('缩放已修改，重启后生效'); }; showDpiDropdown = false">跟随系统</div>
                <div :class="['dropdown-item', { active: config.Personalization.DpiScale === '100' }]" @click="if(config.Personalization.DpiScale !== '100') { config.Personalization.DpiScale = '100'; skipAutoSaveToast = true; autoSave(); showToast('缩放已修改，重启后生效'); }; showDpiDropdown = false">100%</div>
                <div :class="['dropdown-item', { active: config.Personalization.DpiScale === '125' }]" @click="if(config.Personalization.DpiScale !== '125') { config.Personalization.DpiScale = '125'; skipAutoSaveToast = true; autoSave(); showToast('缩放已修改，重启后生效'); }; showDpiDropdown = false">125%</div>
                <div :class="['dropdown-item', { active: config.Personalization.DpiScale === '150' }]" @click="if(config.Personalization.DpiScale !== '150') { config.Personalization.DpiScale = '150'; skipAutoSaveToast = true; autoSave(); showToast('缩放已修改，重启后生效'); }; showDpiDropdown = false">150%</div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- 语言 -->
      <div :class="['collapse-item', 'border-bottom', { 'has-dropdown-open': showLangDropdown }]">
        <div class="collapse-header" @click.stop>
          <div class="collapse-left">
            <h3 class="card-title">语言</h3>
            <span class="card-desc">选择 Yuumi 所使用的语言（重启后生效）</span>
          </div>
          <div class="collapse-right">
            <div class="dropdown-trigger" @click.stop="showLangDropdown = !showLangDropdown">
              <span>
                {{ config.Personalization.Language === 'Auto' ? '跟随系统' :
                   config.Personalization.Language === 'zh_CN' ? '简体中文' :
                   config.Personalization.Language === 'zh_TW' ? '繁體中文' : 'English' }}
              </span>
              <svg :class="['arrow-icon', { expanded: showLangDropdown }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
              <div v-if="showLangDropdown" class="dropdown-menu" @click.stop>
                <div :class="['dropdown-item', { active: config.Personalization.Language === 'Auto' }]" @click="if(config.Personalization.Language !== 'Auto') { config.Personalization.Language = 'Auto'; skipAutoSaveToast = true; autoSave(); showToast('语言已保存，重启后生效'); }; showLangDropdown = false">跟随系统</div>
                <div :class="['dropdown-item', { active: config.Personalization.Language === 'zh_CN' }]" @click="if(config.Personalization.Language !== 'zh_CN') { config.Personalization.Language = 'zh_CN'; skipAutoSaveToast = true; autoSave(); showToast('语言已保存，重启后生效'); }; showLangDropdown = false">简体中文</div>
                <div :class="['dropdown-item', { active: config.Personalization.Language === 'zh_TW' }]" @click="if(config.Personalization.Language !== 'zh_TW') { config.Personalization.Language = 'zh_TW'; skipAutoSaveToast = true; autoSave(); showToast('语言已保存，重启后生效'); }; showLangDropdown = false">繁體中文</div>
                <div :class="['dropdown-item', { active: config.Personalization.Language === 'en_US' }]" @click="if(config.Personalization.Language !== 'en_US') { config.Personalization.Language = 'en_US'; skipAutoSaveToast = true; autoSave(); showToast('语言已保存，重启后生效'); }; showLangDropdown = false">English</div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- 6. 软件更新 -->
      <div class="group-header">软件更新</div>

      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">检查更新</h3>
          <span class="card-desc">在 Yuumi 启动时自动检查更新</span>
        </div>
        <div class="card-right" style="flex-shrink:0; gap:10px">
          <button
            class="check-update-btn"
            :disabled="checkingUpdate"
            @click="manualCheckUpdate"
            :title="checkingUpdate ? '检查中...' : '检查更新'"
          >
            <svg v-if="!checkingUpdate" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="13" height="13">
              <path d="M21 2v6h-6"/>
              <path d="M3 12a9 9 0 0 1 15-6.7L21 8"/>
              <path d="M3 22v-6h6"/>
              <path d="M21 12a9 9 0 0 1-15 6.7L3 16"/>
            </svg>
            <svg v-else class="spin" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="13" height="13">
              <path d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83"/>
            </svg>
            {{ checkingUpdate ? '检查中...' : '检查更新' }}
          </button>
          <div :class="['toggle-switch', config.General.EnableCheckUpdate ? 'on' : 'off']" @click="config.General.EnableCheckUpdate = !config.General.EnableCheckUpdate; autoSave()">
            <span class="toggle-text">{{ config.General.EnableCheckUpdate ? '开' : '关' }}</span>
            <span class="toggle-slider"></span>
          </div>
        </div>
      </div>

      <!-- 手动检查更新对话框 -->
      <UpdateDialog
        v-if="updateInfo"
        :update-info="updateInfo"
        @dismiss="updateInfo = null"
      />

      <div class="collapse-item">
        <div class="collapse-header" @click="toggleCollapse('githubproxy')">
          <div class="collapse-left">
            <h3 class="card-title">GitHub HTTP 代理</h3>
            <span class="card-desc">连接 GitHub 时启用 HTTP 代理</span>
          </div>
          <div class="collapse-right">
            <span class="status-preview">{{ config.General.EnableGithubProxy ? '已启用' : '未启用' }}</span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'githubproxy' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'githubproxy'" class="collapse-content">
          <div class="input-row align-center">
            <div :class="['toggle-switch', config.General.EnableGithubProxy ? 'on' : 'off']" @click="config.General.EnableGithubProxy = !config.General.EnableGithubProxy; autoSave()">
              <span class="toggle-text">{{ config.General.EnableGithubProxy ? '开' : '关' }}</span>
              <span class="toggle-slider"></span>
            </div>
            <input v-model="config.General.GithubProxyAddr" placeholder="127.0.0.1:7897" class="text-input" :disabled="!config.General.EnableGithubProxy" @change="autoSave" />
          </div>
        </div>
      </div>

      <!-- 7. 关于 -->
      <div class="group-header">关于</div>

      <div class="card-item">
        <div class="card-left">
          <h3 class="card-title">关于</h3>
          <span class="card-desc">当前版本 {{ appVersion ? `v${appVersion}` : '加载中...' }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings-view {
  padding: 1.5rem 1.5rem 1.5rem 0.6rem;
  background-color: transparent;
  min-height: 100%;
  color: var(--text-color);
}

.tip-container {
  display: flex; flex-direction: column; align-items: center;
  justify-content: center; padding: 8rem 2rem; color: var(--text-muted);
}
.tip { font-size: 0.95rem; color: var(--text-dimmed); margin-top: 12px; }

.loading-spinner {
  width: 36px; height: 36px; border: 3px solid var(--hover-bg);
  border-top-color: var(--primary-color); border-radius: 50%;
  animation: spin 0.8s linear infinite;
}
@keyframes spin { to { transform: rotate(360deg); } }

.action-btn, .github-btn {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  color: var(--text-color);
  padding: 6px 16px;
  border-radius: 6px;
  font-size: 0.82rem;
  font-weight: 600;
  cursor: pointer;
  outline: none;
  transition: all 0.2s cubic-bezier(0.25, 0.8, 0.25, 1);
  display: inline-flex;
  align-items: center;
  gap: 6px;
  box-shadow: var(--shadow-sm);
}
.action-btn:hover, .github-btn:hover {
  background: var(--hover-bg-strong);
  border-color: var(--primary-color);
  transform: translateY(-0.5px);
}
.action-btn:active, .github-btn:active {
  background: var(--hover-bg);
  transform: translateY(0.5px);
}

.feedback-btn {
  background: var(--win-color);
  border: 1px solid rgba(16, 185, 129, 0.2);
  color: white;
  padding: 6px 16px;
  border-radius: 6px;
  font-size: 0.82rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s cubic-bezier(0.25, 0.8, 0.25, 1);
  box-shadow: 0 2px 8px rgba(16, 185, 129, 0.2);
}
.feedback-btn:hover {
  background: rgba(16, 185, 129, 0.85);
  transform: translateY(-0.5px);
}
.feedback-btn:active {
  color: var(--text-muted);
  transform: translateY(0.5px);
}

.settings-container { max-width: 800px; margin: 0 auto; animation: fadeIn 0.3s ease-out; }

.page-title { font-size: 1.4rem; font-weight: 800; color: var(--text-color); margin: 0 0 1.5rem; letter-spacing: 0.5px; }

.group-header {
  font-size: 0.8rem; font-weight: 700; color: var(--text-muted);
  margin: 1.8rem 0 0.6rem 6px; text-transform: uppercase; letter-spacing: 0.5px;
}

.card-item, .collapse-item {
  background: var(--settings-card-bg);
  padding: 16px 24px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border: 1px solid var(--settings-card-border);
  border-radius: 12px;
  margin-bottom: 8px;
  box-shadow: var(--shadow-sm);
  transition: box-shadow 0.25s cubic-bezier(0.25, 0.8, 0.25, 1),
              border-color 0.25s,
              background-color 0.25s,
              transform 0.2s;
  position: relative;
}
.collapse-item.has-dropdown-open,
.card-item.has-dropdown-open { z-index: 20; }
.card-item:hover, .collapse-item:hover {
  border-color: var(--settings-card-border-hover);
  background-color: var(--settings-card-bg-hover);
  box-shadow: var(--card-glow-hover);
  transform: translateY(-1px);
}
/* 底部分隔线 — 卡片组内用细线分隔 */
.card-item.border-bottom {
  border-radius: 12px 12px 0 0;
  border-bottom: 1px solid var(--settings-separator);
  margin-bottom: 0;
}
.card-item.border-bottom + .card-item { border-radius: 0; margin-top: 0; }
.card-item.border-bottom + .card-item:last-child { border-radius: 0 0 12px 12px; }
.collapse-item.border-bottom {
  border-radius: 12px 12px 0 0;
  border-bottom: 1px solid var(--settings-separator);
  margin-bottom: 0;
}
.collapse-item.border-bottom + .card-item { border-radius: 0; margin-top: 0; }
.collapse-item.border-bottom + .collapse-item { border-radius: 0; margin-top: 0; }
.collapse-item.border-bottom + .card-item:last-child { border-radius: 0 0 12px 12px; }
.collapse-item.border-bottom + .collapse-item:last-child { border-radius: 0 0 12px 12px; }
/* 折叠项内部内容面板 */
.collapse-item .collapse-content {
  border-top: 1px solid var(--settings-separator);
}
/* 分组最后的卡片恢复圆角 */
.card-item.border-bottom:last-of-type { border-radius: 0 0 12px 12px; border-bottom: 1px solid var(--settings-separator); }
.collapse-item.border-bottom:last-of-type { border-radius: 0 0 12px 12px; border-bottom: 1px solid var(--settings-separator); }
/* 整体折叠项（带内容）底部有圆角 */
.collapse-item:not(.border-bottom) { border-bottom: none; }

.card-left { display: flex; flex-direction: column; flex: 1; }
.card-title { font-size: 0.88rem; font-weight: bold; color: var(--text-color); margin: 0; }
.card-desc { font-size: 0.78rem; color: var(--text-muted); margin-top: 4px; line-height: 1.4; }
.card-right { margin-left: auto; display: flex; align-items: center; }
.status-preview { font-size: 0.78rem; color: var(--text-dimmed); margin-right: 10px; }
.status-preview.truncate {
  max-width: 180px; white-space: nowrap; overflow: hidden;
  text-overflow: ellipsis; display: inline-block; vertical-align: middle;
}

.github-icon { width: 16px; height: 16px; }

.toggle-switch {
  display: flex; align-items: center; width: 58px; height: 28px;
  border-radius: 14px; cursor: pointer; position: relative;
  transition: background-color 0.3s cubic-bezier(0.4, 0, 0.2, 1), box-shadow 0.3s; padding: 0 8px;
  flex-shrink: 0;
}
.toggle-switch.off { background-color: var(--toggle-track-off); justify-content: flex-end; }
.toggle-switch.on {
  background-color: var(--primary-color);
  justify-content: flex-start;
  box-shadow: var(--toggle-glow);
}
.toggle-text { font-size: 0.75rem; font-weight: bold; color: white; }
.toggle-switch.off .toggle-text { color: var(--text-dimmed); }
.toggle-slider {
  width: 22px; height: 22px; background-color: var(--toggle-slider);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.18), 0 1px 2px rgba(0, 0, 0, 0.12);
  border-radius: 50%; position: absolute; top: 3px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3), 0 0 0 1px rgba(255,255,255,0.08);
  transition: left 0.3s cubic-bezier(0.4, 0, 0.2, 1), right 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}
.toggle-switch.on .toggle-slider { right: 3px; }
.toggle-switch.off .toggle-slider { left: 3px; }
.toggle-desc { font-size: 0.8rem; color: var(--text-muted); margin-left: 10px; }

.collapse-item { flex-direction: column; align-items: stretch; padding: 0; }
.collapse-header {
  padding: 14px 20px; display: flex; align-items: center;
  justify-content: space-between; cursor: pointer;
}
.collapse-left { display: flex; flex-direction: column; flex: 1; }
.collapse-right { margin-left: auto; color: var(--text-dimmed); display: flex; align-items: center; }
.arrow-icon { width: 18px; height: 18px; transition: transform 0.2s; }
.arrow-icon.expanded { transform: rotate(180deg); }
.collapse-content {
  padding: 0 20px 16px; border-top: 1px dashed var(--border-color);
  padding-top: 14px; animation: slideDown 0.2s ease-out; width: 100%;
}

.input-row { display: flex; gap: 8px; width: 100%; }
.input-row.align-center { align-items: center; }

/* 客户端路径列表 */
.path-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  margin-bottom: 6px;
  background: var(--card-bg);
  transition: border-color 0.2s, box-shadow 0.2s;
}
.path-item:hover {
  border-color: rgba(0, 159, 170, 0.3);
  box-shadow: 0 0 0 1px rgba(0, 159, 170, 0.15);
}
.path-input {
  font-size: 0.82rem;
  color: var(--text-color);
  flex: 1;
  margin-right: 8px;
  border: 1px solid transparent;
  background: transparent;
  padding: 4px 8px;
  border-radius: 4px;
  outline: none;
  transition: all 0.2s;
}
.path-input:focus {
  border-color: var(--primary-color);
  background: var(--card-bg);
}
.path-remove-btn {
  background: transparent;
  border: none;
  color: var(--text-dimmed);
  cursor: pointer;
  font-size: 0.75rem;
  padding: 2px 6px;
  border-radius: 4px;
  flex-shrink: 0;
  transition: all 0.15s;
}
.path-remove-btn:hover {
  color: var(--loss-color);
  background: var(--loss-bg);
}
.path-empty {
  font-size: 0.8rem;
  color: var(--text-dimmed);
  text-align: center;
  padding: 12px 0;
}
.path-actions {
  display: flex;
  gap: 8px;
  margin-top: 8px;
}
.text-input {
  flex: 1;
  padding: 10px 12px 6px;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  font-size: 0.82rem;
  line-height: 1;
  outline: none;
  background-color: var(--card-bg);
  transition: all 0.2s ease;
  color: var(--text-color);
}
.text-input:hover {
  background-color: var(--card-bg-hover);
  border-color: var(--border-color-hover);
}
.text-input:focus {
  background-color: var(--card-bg-hover);
  border-color: var(--primary-color);
  box-shadow: 0 0 8px var(--primary-color-alpha-15);
}

/* 分段控制组件（扁平化按钮组） */
.segmented-control {
  display: inline-flex;
  background: var(--segmented-bg);
  padding: 3px;
  border-radius: 8px;
  border: 1px solid var(--border-color);
}
.segmented-item {
  border: none;
  background: transparent;
  padding: 6px 14px;
  font-size: 0.78rem;
  font-weight: 600;
  color: var(--text-muted);
  cursor: pointer;
  border-radius: 6px;
  transition: all 0.2s ease;
  outline: none;
}
.segmented-item:hover { color: var(--text-color); background: var(--hover-bg); }
.segmented-item.active {
  background: var(--card-bg-hover);
  color: var(--primary-color);
  box-shadow: var(--shadow-sm), 0 0 8px rgba(0, 159, 170, 0.2);
}


.dropdown-trigger {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  padding: 8px 12px;
  border-radius: 6px;
  font-size: 0.82rem;
  color: var(--text-color);
  cursor: pointer;
  position: relative;
  transition: all 0.2s;
  min-width: 140px;
}
.dropdown-trigger:hover {
  background: var(--card-bg-hover);
  border-color: var(--primary-color);
}
.dropdown-trigger .arrow-icon {
  width: 12px;
  height: 12px;
  margin-left: auto;
  transition: transform 0.2s;
}
.dropdown-trigger .arrow-icon.expanded {
  transform: rotate(180deg);
}
.dropdown-menu {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  box-shadow: var(--shadow-lg);
  z-index: 100;
  min-width: 100%;
  padding: 4px 0;
}
.dropdown-item {
  padding: 6px 14px;
  font-size: 0.78rem;
  color: var(--text-muted);
  cursor: pointer;
  transition: all 0.2s;
}
.dropdown-item:hover {
  background: var(--hover-bg);
  color: var(--text-color);
}
.dropdown-item.active {
  color: var(--primary-color);
  font-weight: 600;
  background: var(--primary-color-alpha-15);
}

.number-input {
  width: 100px;
  padding: 8px 12px;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  font-size: 0.82rem;
  outline: none;
  background-color: var(--card-bg);
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
  color: var(--text-color);
  text-align: center;
  box-shadow: var(--shadow-sm);
  appearance: textfield;
  -moz-appearance: textfield;
}
.number-input::-webkit-inner-spin-button,
.number-input::-webkit-outer-spin-button {
  opacity: 1;
  height: 28px;
}
.number-input:hover {
  background-color: var(--card-bg-hover);
  border-color: var(--primary-color-alpha-40);
}
.number-input:focus {
  background-color: var(--card-bg-hover);
  border-color: var(--primary-color);
  box-shadow: 0 0 0 3px var(--primary-color-alpha-15);
}

.color-picker-label { font-size: 0.82rem; color: var(--text-muted); }
.color-picker {
  border: 1px solid var(--border-color); background: var(--card-bg); padding: 2px;
  width: 44px; height: 28px; cursor: pointer; border-radius: 4px;
}
.color-pickers-row { display: flex; gap: 16px; flex-wrap: wrap; }
.color-picker-item { display: flex; align-items: center; gap: 8px; font-size: 0.82rem; color: var(--text-muted); }
.reset-row {
  display: flex; justify-content: flex-end; margin-top: 8px;
}

@keyframes fadeIn { from { opacity: 0; transform: translateY(6px); } to { opacity: 1; transform: translateY(0); } }
@keyframes slideDown { from { opacity: 0; transform: translateY(-4px); } to { opacity: 1; transform: translateY(0); } }

/* Toast */
.toast {
  position: fixed; top: 20px; left: 50%; transform: translateX(-50%);
  padding: 10px 24px; border-radius: 8px; font-size: 0.82rem;
  font-weight: 600; color: white; z-index: 9999;
  box-shadow: var(--shadow-md); pointer-events: none;
}
.toast-success { background-color: var(--primary-color); }
.toast-error { background-color: var(--loss-color); }
.toast-enter-active { transition: all 0.25s ease-out; }
.toast-leave-active { transition: all 0.2s ease-in; }
.toast-enter-from { opacity: 0; transform: translateX(-50%) translateY(-12px); }
.toast-leave-to { opacity: 0; transform: translateX(-50%) translateY(-8px); }

.setting-input-row {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  margin-bottom: 12px;
}

.setting-input-row:last-child {
  margin-bottom: 0;
}

.setting-input-label {
  font-size: 0.82rem;
  font-weight: 600;
  color: var(--text-muted);
  width: 130px;
  flex-shrink: 0;
}
.signalr-status-badge {
  font-weight: 600;
  transition: color 0.2s ease;
}
.signalr-status-badge.connected {
  color: var(--win-color);
}
.signalr-status-badge.connecting {
  color: #f59e0b;
}
.signalr-status-badge.error {
  color: var(--loss-color);
}
.signalr-status-badge.disconnected {
  color: var(--text-dimmed);
}
.setting-error-tip {
  font-size: 0.76rem;
  color: var(--loss-color);
  margin-top: -6px;
  margin-bottom: 10px;
  padding-left: 142px;
  text-align: left;
}

/* ── 检查更新按钮 ── */
.check-update-btn {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 5px 12px;
  border-radius: 7px;
  border: 1px solid var(--border-color, rgba(255,255,255,0.12));
  background: var(--bg-secondary, rgba(255,255,255,0.04));
  color: var(--text-secondary);
  font-size: 12px;
  cursor: pointer;
  transition: background 0.15s, color 0.15s, border-color 0.15s;
  white-space: nowrap;
}

.check-update-btn:hover:not(:disabled) {
  background: var(--bg-hover, rgba(255,255,255,0.08));
  color: var(--text-primary);
  border-color: var(--theme-color, #009faa);
}

.check-update-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to   { transform: rotate(360deg); }
}

.spin {
  animation: spin 0.9s linear infinite;
}
</style>
