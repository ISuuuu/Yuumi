<script setup lang="ts">
import { ref, onMounted, onUnmounted, inject, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getVersion } from "@tauri-apps/api/app";
import { fetchConfig, updateConfig } from "../api/lcu";
import type { AppConfig } from "../api/lcu";
import { updateThemeColor, updateDeathColor, updateCardColors } from "../utils/theme";

import { useDialog } from "naive-ui";
import UpdateDialog, { type UpdateInfo } from "../components/UpdateDialog.vue";
import { useToast } from "../composables/useToast";
import ColorPickerWithAlpha from "../components/ColorPickerWithAlpha.vue";

const config = inject<Ref<AppConfig | null>>("appConfig") || ref<AppConfig | null>(null);
const dialog = useDialog();

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

const { showToast } = useToast();

// activeCollapse and toggleCollapse are no longer needed - replaced by n-collapse

// activeCollapse and toggleCollapse are no longer needed - replaced by n-collapse


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
  if (config.value && config.value.Personalization) {
    if (config.value.Personalization.ThemeColor) {
      updateThemeColor(config.value.Personalization.ThemeColor);
    }
    updateCardColors(
      config.value.Personalization.WinCardColor,
      config.value.Personalization.LoseCardColor,
      config.value.Personalization.RemakeCardColor
    );
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


async function handleEditPathDirect(index: number, val: string) {
  if (!config.value) return;
  const pathVal = val.trim();
  if (!pathVal) return;
  config.value.General.LolPath[index] = pathVal;
  await updateConfig(config.value);
}

// 清除缓存
function handleClearCache() {
  dialog.warning({
    title: "清除缓存",
    content: "确定要清除所有游戏资源缓存吗？清除后需要重新加载游戏资源。",
    positiveText: "确定",
    negativeText: "取消",
    positiveButtonProps: { type: 'primary' },
    onPositiveClick: async () => {
      try {
        const result = await invoke<string>("clear_game_cache");
        showToast(result);
      } catch (e: any) {
        showToast('清除缓存失败', 'error');
      }
    }
  });
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

function onCardColorChange(val: string, field: 'WinCardColor' | 'LoseCardColor' | 'RemakeCardColor') {
  if (!config.value?.Personalization) return;
  config.value.Personalization[field] = val;
  
  // 实时更新全局 CSS 变量
  updateCardColors(
    config.value.Personalization.WinCardColor,
    config.value.Personalization.LoseCardColor,
    config.value.Personalization.RemakeCardColor
  );
}



function onThemeColorSelect(color: string) {
  if (config.value?.Personalization) {
    config.value.Personalization.ThemeColor = color;
  }
  updateThemeColor(color);
  autoSave();
}



function onDeathColorSelect(color: string, field: 'LightDeathsNumberColor' | 'DarkDeathsNumberColor') {
  const color8 = toColor8(color);
  if (config.value?.Personalization) {
    config.value.Personalization[field] = color8;
  }
  // 实时更新 CSS 变量
  const light = field === 'LightDeathsNumberColor' ? color : toColor6(config.value?.Personalization?.LightDeathsNumberColor);
  const dark  = field === 'DarkDeathsNumberColor'  ? color : toColor6(config.value?.Personalization?.DarkDeathsNumberColor);
  updateDeathColor(light, dark);
  autoSave();
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
  updateCardColors(
    DEFAULT_COLORS.WinCardColor,
    DEFAULT_COLORS.LoseCardColor,
    DEFAULT_COLORS.RemakeCardColor
  );
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
    <div v-if="!config" class="tip-container">
      <div class="loading-spinner"></div>
      <p class="tip">加载配置数据中...</p>
    </div>

    <div v-else class="settings-container">
      <h1 class="page-title">设置</h1>

      <!-- 1. 功能组 -->
      <div class="group-header">功能</div>

      <!-- LCU API 并发数 -->
      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">LCU API 并发数</h3>
          <span class="card-desc">该值越大数据加载速度越快，但越可能引起客户端闪退</span>
        </div>
        <div class="card-right">
          <n-input-number
            v-model:value="config.Functions.ApiConcurrencyNumber"
            :min="1"
            :max="10"
            @update:value="autoSave"
            style="width: 140px;"
            size="small"
          />
        </div>
      </div>

      <!-- 默认对局数量 -->
      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">默认对局数量</h3>
          <span class="card-desc">调整在个人生涯界面中显示的最大对局数量</span>
        </div>
        <div class="card-right">
          <n-input-number
            v-model:value="config.Functions.CareerGamesNumber"
            :min="1"
            :max="100"
            :step="5"
            @update:value="autoSave"
            style="width: 140px;"
            size="small"
          />
        </div>
      </div>

      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">对局信息过滤</h3>
          <span class="card-desc">基于你当前游戏模式（地图/队列）筛选战绩，只显示相同模式的玩家历史数据</span>
        </div>
        <div class="card-right">
          <n-switch v-model:value="config.Functions.GameInfoFilter" @update:value="autoSave" />
        </div>
      </div>

      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">保留对局信息界面内容</h3>
          <span class="card-desc">保留上一局的对局信息内容直到下一次对局开始</span>
        </div>
        <div class="card-right">
          <n-switch v-model:value="config.Functions.EnableReserveGameinfo" @update:value="autoSave" />
        </div>
      </div>

      <div class="card-item">
        <div class="card-left">
          <h3 class="card-title">对局详情中显示段位</h3>
          <span class="card-desc">在搜索界面对局详情界面中显示段位，启动该选项将影响加载界面的速度</span>
        </div>
        <div class="card-right">
          <n-switch v-model:value="config.Functions.ShowTierInGameInfo" @update:value="autoSave" />
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
          <n-switch v-model:value="config.Functions.AutoShowOpgg" @update:value="autoSave" />
        </div>
      </div>

      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">置顶 OP.GG 窗口</h3>
          <span class="card-desc">在英雄选择时将 OP.GG 窗口置顶</span>
        </div>
        <div class="card-right">
          <n-switch v-model:value="config.Functions.EnableOpggOnTop" @update:value="autoSave" />
        </div>
      </div>

      <n-collapse arrow-placement="right" class="collapse-card">
        <n-collapse-item name="opggproxy">
          <template #header>
            <div class="collapse-header-wrapper">
              <div class="collapse-left-simple">
                <span class="card-title">OP.GG HTTP 代理</span>
                <span class="card-desc">连接 OP.GG 时启用 HTTP 代理</span>
              </div>
              <div class="collapse-right-status">
                <span class="status-preview">{{ config.General.EnableOpggProxy ? '已启用' : '未启用' }}</span>
              </div>
            </div>
          </template>
          <div class="setting-row">
            <n-switch v-model:value="config.General.EnableOpggProxy" @update:value="autoSave" />
            <n-input v-model:value="config.General.OpggProxyAddr" placeholder="127.0.0.1:10809" :disabled="!config.General.EnableOpggProxy" clearable @change="autoSave" style="max-width:300px" />
          </div>
        </n-collapse-item>
      </n-collapse>

      <!-- 3. 通用 -->
      <div class="group-header">通用</div>

      <n-collapse arrow-placement="right" class="collapse-card">
        <n-collapse-item name="lolpath">
          <template #header>
            <div class="collapse-header-wrapper">
              <div class="collapse-left-simple">
                <span class="card-title">客户端路径</span>
                <span class="card-desc">选择或自动检测 LOL 客户端的安装路径</span>
              </div>
              <div class="collapse-right-status">
                <span class="status-preview">{{ config?.General?.LolPath?.length ? `已设置 ${config.General.LolPath.length} 个路径` : '未设置' }}</span>
              </div>
            </div>
          </template>
          <!-- 已保存的路径列表 -->
          <div v-for="(path, index) in (config?.General?.LolPath || [])" :key="index" class="path-item">
            <n-input class="path-input" :value="path" @change="(val) => handleEditPathDirect(index, val)" placeholder="客户端安装路径" style="flex:1; margin-right:8px" />
            <n-button size="tiny" circle @click="handleRemovePath(index)" title="删除">✕</n-button>
          </div>
          <div v-if="!config?.General?.LolPath?.length" class="path-empty">暂无已保存的客户端路径</div>
          <!-- 操作按钮 -->
          <div class="path-actions">
            <n-button size="small" @click="handleDetectPath">自动检测</n-button>
            <n-button size="small" @click="handleBrowseFolder">添加目录</n-button>
          </div>
        </n-collapse-item>
      </n-collapse>

      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">自动启动游戏</h3>
          <span class="card-desc">启动 Yuumi 时自动启动 LOL 客户端</span>
        </div>
        <div class="card-right">
          <n-switch v-model:value="config.General.EnableStartLolWithApp" @update:value="autoSave" />
        </div>
      </div>

      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">清除缓存</h3>
          <span class="card-desc">删除所有游戏资源的缓存（建议在游戏资源有更新时使用）</span>
        </div>
        <div class="card-right">
          <n-button size="small" @click="handleClearCache">删除</n-button>
        </div>
      </div>

      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">最小化到任务栏托盘</h3>
          <span class="card-desc">点击右上角关闭时将程序最小化到托盘</span>
        </div>
        <div class="card-right">
          <n-switch :value="!!config?.General?.EnableCloseToTray" @update:value="(val) => { if (config) { config.General.EnableCloseToTray = val; autoSave(); } }" />
        </div>
      </div>

      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">静默启动</h3>
          <span class="card-desc">启动 Yuumi 后最小化窗口到任务栏托盘</span>
        </div>
        <div class="card-right">
          <n-switch v-model:value="config.General.EnableGameStartMinimize" @update:value="autoSave" />
        </div>
      </div>

      <!-- 云端服务 -->
      <!-- 云端服务 -->
      <n-collapse arrow-placement="right" class="collapse-card">
        <n-collapse-item name="upload_and_signalr">
          <template #header>
            <div class="collapse-header-wrapper">
              <div class="collapse-left-simple">
                <span class="card-title">云端服务</span>
                <span class="card-desc">LCU 实时数据上传及 SignalR 实时查询服务</span>
              </div>
              <div class="collapse-right-status">
                <span class="status-preview">
                  {{ config.General.UploadApiUrl ? '已配置上传' : '未配置' }}
                  <template v-if="config.Functions.LcuRealtimeEnabled">
                    / 
                    <span :class="['signalr-status-badge', signalrStatus]">
                      {{ signalrStatus === 'connected' ? '云端已连接' : signalrStatus === 'connecting' ? '云端连接中...' : signalrStatus === 'error' ? '云端连接失败' : '云端未连接' }}
                    </span>
                  </template>
                </span>
              </div>
            </div>
          </template>
          <div class="setting-input-row">
            <span class="setting-input-label">服务器 API 地址:</span>
            <n-input v-model:value="config.General.UploadApiUrl" placeholder="http://example.com" clearable @change="if (config.Functions.LcuRealtimeEnabled && config.General.UploadApiUrl) { signalrStatus = 'connecting'; }; autoSave()" style="max-width:300px" />
          </div>
          <div class="setting-input-row">
            <span class="setting-input-label">LCU 实时查询:</span>
            <n-switch v-model:value="config.Functions.LcuRealtimeEnabled" @update:value="if (config.Functions.LcuRealtimeEnabled && config.General.UploadApiUrl) { signalrStatus = 'connecting'; } else { signalrStatus = 'disconnected'; }; autoSave()" />
          </div>
          <div v-if="signalrStatus === 'error' && signalrError" class="setting-error-tip">
            连接异常: {{ signalrError }}
          </div>
          <div class="setting-input-row">
            <span class="setting-input-label">userid:</span>
            <n-input v-model:value="config.General.SignalrUserId" placeholder="留空默认使用 lcu_user_001" clearable @change="autoSave" style="max-width:300px" />
          </div>
        </n-collapse-item>
      </n-collapse>

      <!-- 隐藏云顶之弈 -->
      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">隐藏云顶之弈</h3>
          <span class="card-desc">在左侧菜单中隐藏 Teamfight Tactics 入口，关闭后立即生效</span>
        </div>
        <div class="card-right">
          <n-switch v-model:value="config.Functions.HideTft" @update:value="autoSave" />
        </div>
      </div>

      <!-- 4. 日志 -->
      <div class="group-header">日志</div>

      <!-- 日志等级 -->
      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">日志等级</h3>
          <span class="card-desc">日志写入文件记录的等级（Debug, Info, Error）</span>
        </div>
        <div class="card-right">
          <n-select
            v-model:value="config.General.LogLevel"
            :options="[
              { label: 'Debug', value: 0 },
              { label: 'Info', value: 1 },
              { label: 'Error', value: 2 }
            ]"
            @update:value="autoSave"
            style="width: 120px;"
            size="small"
          />
        </div>
      </div>

      <div class="card-item">
        <div class="card-left">
          <h3 class="card-title">日志文件</h3>
          <span class="card-desc">&lt;exe 目录&gt;/log/</span>
        </div>
        <div class="card-right">
          <n-button size="small" @click="handleOpenLogFolder">打开文件夹</n-button>
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
          <n-switch v-model:value="config.Personalization.MicaEnabled" @update:value="autoSave(); invoke('set_mica_effect', { enabled: config.Personalization.MicaEnabled })" />
        </div>
      </div>

      <!-- 应用主题 -->
      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">应用主题</h3>
          <span class="card-desc">选择 Yuumi 的显示主题</span>
        </div>
        <div class="card-right">
          <n-select
            v-model:value="config.Personalization.ThemeMode"
            :options="[
              { label: '浅色', value: 'Light' },
              { label: '深色', value: 'Dark' },
              { label: '跟随系统', value: 'Auto' }
            ]"
            @update:value="(val) => { applyThemeMode(val); autoSave(); }"
            style="width: 140px;"
            size="small"
          />
        </div>
      </div>

      <!-- 主题色 -->
      <n-collapse arrow-placement="right" class="collapse-card">
        <n-collapse-item name="themecolor">
          <template #header>
            <div class="collapse-header-wrapper">
              <div class="collapse-left-simple">
                <span class="card-title">主题色</span>
                <span class="card-desc">选择 Yuumi 的全局视觉主题色</span>
              </div>
              <div class="collapse-right-status">
                <span class="status-preview">#{{ toColor6(config.Personalization.ThemeColor)?.replace('#', '') }}</span>
              </div>
            </div>
          </template>
          <div class="setting-row">
            <span class="setting-label">选择调色盘颜色:</span>
            <n-color-picker
              :value="toColor6(config.Personalization.ThemeColor)"
              :show-alpha="false"
              @update:value="onThemeColorSelect"
              style="width: 100px; flex-shrink: 0;"
              size="small"
            />
          </div>
          <div class="reset-row">
            <n-button size="small" @click="resetThemeColor">恢复默认</n-button>
          </div>
        </n-collapse-item>
      </n-collapse>

      <!-- 对局卡片颜色 -->
      <n-collapse arrow-placement="right" class="collapse-card">
        <n-collapse-item name="cardcolors">
          <template #header>
            <div class="collapse-header-wrapper">
              <div class="collapse-left-simple">
                <span class="card-title">对局卡片颜色</span>
                <span class="card-desc">自定义胜利、失败与重开对局卡片的背景色</span>
              </div>
              <div class="collapse-right-status">
                <span class="status-preview">已设置自定义颜色</span>
              </div>
            </div>
          </template>
          <div class="setting-row">
            <span class="setting-label">胜利对局卡片:</span>
            <ColorPickerWithAlpha
              :value="config ? config.Personalization.WinCardColor : '#ffffffff'"
              @update:value="(val) => onCardColorChange(val, 'WinCardColor')"
              @save="autoSave"
              size="small"
              style="width: 120px; flex-shrink: 0;"
            />
          </div>
          <div class="setting-row">
            <span class="setting-label">失败对局卡片:</span>
            <ColorPickerWithAlpha
              :value="config ? config.Personalization.LoseCardColor : '#ffffffff'"
              @update:value="(val) => onCardColorChange(val, 'LoseCardColor')"
              @save="autoSave"
              size="small"
              style="width: 120px; flex-shrink: 0;"
            />
          </div>
          <div class="setting-row">
            <span class="setting-label">重开对局卡片:</span>
            <ColorPickerWithAlpha
              :value="config ? config.Personalization.RemakeCardColor : '#ffffffff'"
              @update:value="(val) => onCardColorChange(val, 'RemakeCardColor')"
              @save="autoSave"
              size="small"
              style="width: 120px; flex-shrink: 0;"
            />
          </div>
          <div class="reset-row">
            <n-button size="small" @click="resetCardColors">恢复默认</n-button>
          </div>
        </n-collapse-item>
      </n-collapse>

      <!-- 死亡数字体颜色 -->
      <n-collapse arrow-placement="right" class="collapse-card">
        <n-collapse-item name="deathfontcolor">
          <template #header>
            <div class="collapse-header-wrapper">
              <div class="collapse-left-simple">
                <span class="card-title">死亡数字体颜色</span>
                <span class="card-desc">针对亮色与暗色模式分别自定义死亡文字的着色</span>
              </div>
              <div class="collapse-right-status">
                <span class="status-preview">已设置自定义颜色</span>
              </div>
            </div>
          </template>
          <div class="setting-row">
            <span class="setting-label">浅色主题死亡数字:</span>
            <n-color-picker
              :value="config ? toColor6(config.Personalization.LightDeathsNumberColor) : ''"
              :show-alpha="false"
              @update:value="(val) => onDeathColorSelect(val, 'LightDeathsNumberColor')"
              style="width: 100px; flex-shrink: 0;"
              size="small"
            />
          </div>
          <div class="setting-row">
            <span class="setting-label">深色主题死亡数字:</span>
            <n-color-picker
              :value="config ? toColor6(config.Personalization.DarkDeathsNumberColor) : ''"
              :show-alpha="false"
              @update:value="(val) => onDeathColorSelect(val, 'DarkDeathsNumberColor')"
              style="width: 100px; flex-shrink: 0;"
              size="small"
            />
          </div>
          <div class="reset-row">
            <n-button size="small" @click="resetDeathColors">恢复默认</n-button>
          </div>
        </n-collapse-item>
      </n-collapse>

      <!-- 界面缩放 -->
      <!-- 界面缩放 -->
      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">界面缩放</h3>
          <span class="card-desc">支持调整客户端视图的 DPI 缩放大小比例</span>
        </div>
        <div class="card-right">
          <n-select
            v-model:value="config.Personalization.DpiScale"
            :options="[
              { label: '跟随系统', value: 'Auto' },
              { label: '100%', value: '100' },
              { label: '125%', value: '125' },
              { label: '150%', value: '150' }
            ]"
            @update:value="() => { skipAutoSaveToast = true; autoSave(); showToast('缩放已修改，重启后生效'); }"
            style="width: 140px;"
            size="small"
          />
        </div>
      </div>

      <!-- 语言 -->
      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">语言</h3>
          <span class="card-desc">选择软件界面语言选项</span>
        </div>
        <div class="card-right">
          <n-select
            v-model:value="config.Personalization.Language"
            :options="[
              { label: '跟随系统', value: 'Auto' },
              { label: '简体中文', value: 'zh_CN' },
              { label: '繁體中文', value: 'zh_TW' },
              { label: 'English', value: 'en_US' }
            ]"
            @update:value="() => { skipAutoSaveToast = true; autoSave(); showToast('语言已保存，重启后生效'); }"
            style="width: 140px;"
            size="small"
          />
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
          <n-button size="small" :disabled="checkingUpdate" @click="manualCheckUpdate" :title="checkingUpdate ? '检查中...' : '检查更新'">
            <template #icon>
              <svg v-if="!checkingUpdate" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" width="13" height="13">
                <path d="M21 2v6h-6"/>
                <path d="M3 12a9 9 0 0 1 15-6.7L21 8"/>
                <path d="M3 22v-6h6"/>
                <path d="M21 12a9 9 0 0 1-15 6.7L3 16"/>
              </svg>
              <svg v-else class="spin" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="13" height="13">
                <path d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83"/>
              </svg>
            </template>
            {{ checkingUpdate ? '检查中...' : '检查更新' }}
          </n-button>
          <n-switch v-model:value="config.General.EnableCheckUpdate" @update:value="autoSave" />
        </div>
      </div>

      <!-- 手动检查更新对话框 -->
      <UpdateDialog
        v-if="updateInfo"
        :update-info="updateInfo"
        @dismiss="updateInfo = null"
      />

      <n-collapse arrow-placement="right" class="collapse-card">
        <n-collapse-item name="githubproxy">
          <template #header>
            <div class="collapse-header-wrapper">
              <div class="collapse-left-simple">
                <span class="card-title">GitHub HTTP 代理</span>
                <span class="card-desc">软件检查与拉取版本更新时走 HTTP 代理通道</span>
              </div>
              <div class="collapse-right-status">
                <span class="status-preview">{{ config.General.EnableGithubProxy ? '已启用' : '未启用' }}</span>
              </div>
            </div>
          </template>
          <div class="setting-row">
            <n-input v-model:value="config.General.GithubProxyAddr" placeholder="127.0.0.1:7897" :disabled="!config.General.EnableGithubProxy" clearable @change="autoSave" style="max-width:300px" />
            <n-switch v-model:value="config.General.EnableGithubProxy" @update:value="autoSave" />
          </div>
        </n-collapse-item>
      </n-collapse>

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

.card-item {
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
.card-item.has-dropdown-open { z-index: 20; }
.card-item:hover {
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
/* removed */
.card-item.border-bottom + .card-item:last-child { border-radius: 0 0 12px 12px; }
/* removed */
/* 分组最后的卡片恢复圆角 */
.card-item.border-bottom:last-of-type { border-radius: 0 0 12px 12px; border-bottom: 1px solid var(--settings-separator); }
/* removed */

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


.collapse-left { display: flex; flex-direction: column; flex: 1; }
.collapse-right { margin-left: auto; color: var(--text-dimmed); display: flex; align-items: center; }
.arrow-icon { width: 18px; height: 18px; transition: transform 0.2s; }
.arrow-icon.expanded { transform: rotate(180deg); }

.input-row { display: flex; gap: 8px; width: 100%; justify-content: flex-end; }
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
  justify-content: flex-end;
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

.color-picker-label { font-size: 0.82rem; color: var(--text-muted); }
.color-picker {
  border: 1px solid var(--border-color); background: var(--card-bg); padding: 2px;
  width: 44px; height: 28px; cursor: pointer; border-radius: 4px;
}
.color-pickers-row { display: flex; gap: 16px; flex-wrap: wrap; justify-content: flex-end; }
.color-picker-item { display: flex; align-items: center; gap: 8px; font-size: 0.82rem; color: var(--text-muted); }
.reset-row {
  display: flex; justify-content: flex-end; margin-top: 8px;
}

@keyframes fadeIn { from { opacity: 0; transform: translateY(6px); } to { opacity: 1; transform: translateY(0); } }

.setting-input-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
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

