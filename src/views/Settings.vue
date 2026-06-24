<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { openUrl, openPath } from "@tauri-apps/plugin-opener";
import { fetchConfig, updateConfig } from "../api/lcu";
import type { AppConfig } from "../api/lcu";
import { updateThemeColor } from "../utils/theme";

const config = ref<AppConfig | null>(null);

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

// ─── 自动保存（防抖 500ms）───
let saveDebounce: ReturnType<typeof setTimeout> | null = null;
function autoSave() {
  if (!config.value) return;
  if (saveDebounce) clearTimeout(saveDebounce);
  saveDebounce = setTimeout(async () => {
    try {
      await updateConfig(config.value!);
      showToast('设置已自动保存');
    } catch (e) {
      showToast('保存失败', 'error');
    }
  }, 500);
}

onMounted(async () => {
  try {
    config.value = await fetchConfig();
    if (config.value && config.value.Personalization && config.value.Personalization.ThemeColor) {
      updateThemeColor(config.value.Personalization.ThemeColor);
    }
  } catch (e) {
    console.error("加载系统配置失败:", e);
  }
});

// 打开客户端路径
async function handleSelectFolder() {
  try {
    const path = config.value?.General?.LolPath?.[0] || "";
    if (path) {
      await openPath(path);
    } else {
      showToast('请先设置 LOL 客户端路径', 'error');
    }
  } catch (e: any) {
    showToast('打开路径失败', 'error');
  }
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

async function handleProvideFeedback() {
  try { await openUrl("https://github.com/xzmkim/lolhelper/issues"); }
  catch { showToast('打开链接失败', 'error'); }
}

async function handleOpenGithub() {
  try { await openUrl("https://github.com/xzmkim/lolhelper"); }
  catch { showToast('打开链接失败', 'error'); }
}

function onThemeColorInput(e: Event) {
  const target = e.target as HTMLInputElement;
  updateThemeColor(target.value);
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
            <select v-model.number="config.Functions.ApiConcurrencyNumber" class="select-input" @change="autoSave">
              <option v-for="n in 10" :key="n" :value="n">{{ n }} (个并发)</option>
            </select>
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
            <select v-model.number="config.Functions.CareerGamesNumber" class="select-input" @change="autoSave">
              <option :value="10">10 场</option><option :value="20">20 场</option>
              <option :value="50">50 场</option><option :value="100">100 场</option>
            </select>
          </div>
        </div>
      </div>

      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('filter')">
          <div class="collapse-left">
            <h3 class="card-title">对局信息过滤</h3>
            <span class="card-desc">基于你所处的游戏模式筛选对局信息界面显示的战绩</span>
          </div>
          <div class="collapse-right">
            <span class="status-preview">{{ config.Functions.GameInfoFilter ? '已启用' : '未启用' }}</span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'filter' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'filter'" class="collapse-content">
          <div class="input-row align-center">
            <div :class="['toggle-switch', config.Functions.GameInfoFilter ? 'on' : 'off']" @click="config.Functions.GameInfoFilter = !config.Functions.GameInfoFilter; autoSave()">
              <span class="toggle-text">{{ config.Functions.GameInfoFilter ? '开' : '关' }}</span>
              <span class="toggle-slider"></span>
            </div>
            <span class="toggle-desc">只显示当前地图/队列的玩家数据以净化对局信息</span>
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

      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">客户端路径</h3>
          <span class="card-desc">设置客户端路径以及顺序</span>
        </div>
        <div class="card-right">
          <button class="action-btn" @click="handleSelectFolder">选择文件夹</button>
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

      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('apiaddr')">
          <div class="collapse-left">
            <h3 class="card-title">对局上传 API 地址</h3>
            <span class="card-desc">用于上传对局历史数据的 URL 端点，留空则不上传</span>
          </div>
          <div class="collapse-right">
            <span class="status-preview truncate">{{ config.General.UploadApiUrl || '未设置' }}</span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'apiaddr' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'apiaddr'" class="collapse-content">
          <div class="input-row">
            <input v-model="config.General.UploadApiUrl" placeholder="http://example.com" class="text-input" @change="autoSave" />
          </div>
        </div>
      </div>

      <div class="card-item">
        <div class="card-left">
          <h3 class="card-title">LCU 实时查询</h3>
          <span class="card-desc">通过 LCU 客户端实时查询对局历史</span>
        </div>
        <div class="card-right">
          <div :class="['toggle-switch', config.Functions.LcuRealtimeEnabled ? 'on' : 'off']" @click="config.Functions.LcuRealtimeEnabled = !config.Functions.LcuRealtimeEnabled; autoSave()">
            <span class="toggle-text">{{ config.Functions.LcuRealtimeEnabled ? '开' : '关' }}</span>
            <span class="toggle-slider"></span>
          </div>
        </div>
      </div>

      <!-- 4. 日志 -->
      <div class="group-header">日志</div>

      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('loglevel')">
          <div class="collapse-left">
            <h3 class="card-title">日志等级</h3>
            <span class="card-desc">修改 Yuumi 记录日志的等级（重启后生效）</span>
          </div>
          <div class="collapse-right">
            <span class="status-preview">{{ config.General.LogLevel === 0 ? 'Debug' : config.General.LogLevel === 1 ? 'Info' : 'Error' }}</span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'loglevel' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'loglevel'" class="collapse-content">
          <div class="input-row">
            <select v-model.number="config.General.LogLevel" class="select-input" @change="autoSave">
              <option :value="0">Debug</option>
              <option :value="1">Info</option>
              <option :value="2">Error</option>
            </select>
          </div>
        </div>
      </div>

      <div class="card-item">
        <div class="card-left">
          <h3 class="card-title">日志文件</h3>
          <span class="card-desc">打开日志文件夹</span>
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
          <div :class="['toggle-switch', config.Personalization.MicaEnabled ? 'on' : 'off']" @click="config.Personalization.MicaEnabled = !config.Personalization.MicaEnabled; autoSave()">
            <span class="toggle-text">{{ config.Personalization.MicaEnabled ? '开' : '关' }}</span>
            <span class="toggle-slider"></span>
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
            <input type="color" class="color-picker" v-model="config.Personalization.ThemeColor" @input="onThemeColorInput" @change="autoSave" />
          </div>
        </div>
      </div>

      <!-- 对局卡片颜色 -->
      <div class="collapse-item border-bottom">
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
              <input type="color" v-model="config.Personalization.WinCardColor" @change="autoSave" />
            </div>
            <div class="color-picker-item">
              <label>失败卡片:</label>
              <input type="color" v-model="config.Personalization.LoseCardColor" @change="autoSave" />
            </div>
            <div class="color-picker-item">
              <label>重开卡片:</label>
              <input type="color" v-model="config.Personalization.RemakeCardColor" @change="autoSave" />
            </div>
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
              <input type="color" v-model="config.Personalization.LightDeathsNumberColor" @change="autoSave" />
            </div>
            <div class="color-picker-item">
              <label>深色主题下颜色:</label>
              <input type="color" v-model="config.Personalization.DarkDeathsNumberColor" @change="autoSave" />
            </div>
          </div>
        </div>
      </div>

      <!-- 界面缩放 -->
      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('dpiscale')">
          <div class="collapse-left">
            <h3 class="card-title">界面缩放</h3>
            <span class="card-desc">调整部件和字体的大小</span>
          </div>
          <div class="collapse-right">
            <span class="status-preview">{{ config.Personalization.DpiScale === 'Auto' ? '跟随系统' : config.Personalization.DpiScale + '%' }}</span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'dpiscale' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'dpiscale'" class="collapse-content">
          <div class="input-row">
            <select v-model="config.Personalization.DpiScale" class="select-input" @change="autoSave">
              <option value="Auto">跟随系统设置</option>
              <option value="100">100%</option><option value="125">125%</option>
              <option value="150">150%</option>
            </select>
          </div>
        </div>
      </div>

      <!-- 语言 -->
      <div class="collapse-item">
        <div class="collapse-header" @click="toggleCollapse('lang')">
          <div class="collapse-left">
            <h3 class="card-title">语言</h3>
            <span class="card-desc">选择 Yuumi 所使用的语言</span>
          </div>
          <div class="collapse-right">
            <span class="status-preview">
              {{ config.Personalization.Language === 'Auto' ? '跟随系统' :
                 config.Personalization.Language === 'zh_CN' ? '简体中文' :
                 config.Personalization.Language === 'zh_TW' ? '繁體中文' : 'English' }}
            </span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'lang' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'lang'" class="collapse-content">
          <div class="input-row">
            <select v-model="config.Personalization.Language" class="select-input" @change="autoSave">
              <option value="Auto">跟随系统设置</option>
              <option value="zh_CN">简体中文</option>
              <option value="zh_TW">繁體中文</option>
              <option value="en_US">English</option>
            </select>
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
        <div class="card-right">
          <div :class="['toggle-switch', config.General.EnableCheckUpdate ? 'on' : 'off']" @click="config.General.EnableCheckUpdate = !config.General.EnableCheckUpdate; autoSave()">
            <span class="toggle-text">{{ config.General.EnableCheckUpdate ? '开' : '关' }}</span>
            <span class="toggle-slider"></span>
          </div>
        </div>
      </div>

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

      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">提供反馈</h3>
          <span class="card-desc">通过提供反馈帮助我们改善 Yuumi</span>
        </div>
        <div class="card-right">
          <button class="feedback-btn" @click="handleProvideFeedback">提供反馈</button>
        </div>
      </div>

      <div class="card-item">
        <div class="card-left">
          <h3 class="card-title">关于</h3>
          <span class="card-desc">版权所有 © 2026, xzmkim. 当前版本 1.5.42</span>
        </div>
        <div class="card-right">
          <button class="github-btn" @click="handleOpenGithub">
            <svg class="github-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 19c-5 1.5-5-2.5-7-3m14 6v-3.87a3.37 3.37 0 0 0-.94-2.61c3.14-.35 6.44-1.54 6.44-7A5.44 5.44 0 0 0 20 4.77 5.07 5.07 0 0 0 19.91 1S18.73.65 16 2.48a13.38 13.38 0 0 0-7 0C6.27.65 5.09 1 5.09 1A5.07 5.07 0 0 0 5 4.77a5.44 5.44 0 0 0-1.5 3.78c0 5.42 3.3 6.61 6.44 7A3.37 3.37 0 0 0 9 18.13V22"/></svg>
            查看 GitHub
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings-view {
  padding: 1.5rem 1.5rem 1.5rem 0.6rem;
  background-color: #fafbfc;
  min-height: 100%;
}

.tip-container {
  display: flex; flex-direction: column; align-items: center;
  justify-content: center; padding: 8rem 2rem; color: #909399;
}
.tip { font-size: 1rem; color: #8c8c8c; margin-top: 12px; }

.loading-spinner {
  width: 36px; height: 36px; border: 3px solid #e2e5e9;
  border-top-color: var(--primary-color); border-radius: 50%;
  animation: spin 0.8s linear infinite;
}
@keyframes spin { to { transform: rotate(360deg); } }

.action-btn, .github-btn {
  background: rgba(255, 255, 255, 0.7);
  border: 1px solid rgba(0, 0, 0, 0.08);
  border-bottom: 1px solid rgba(0, 0, 0, 0.183);
  color: #2c3e50;
  padding: 6px 16px;
  border-radius: 6px;
  font-size: 0.82rem;
  font-weight: bold;
  cursor: pointer;
  outline: none;
  transition: all 0.15s cubic-bezier(0.25, 0.8, 0.25, 1);
  display: inline-flex;
  align-items: center;
  gap: 6px;
  box-shadow: 0 1px 2px rgba(0,0,0,0.02);
}
.action-btn:hover, .github-btn:hover {
  background: rgba(249, 249, 249, 0.85);
  border-color: rgba(0, 0, 0, 0.12);
  border-bottom-color: rgba(0, 0, 0, 0.24);
  transform: translateY(-0.5px);
}
.action-btn:active, .github-btn:active {
  background: rgba(243, 243, 243, 0.6);
  border-bottom-color: rgba(0, 0, 0, 0.08);
  transform: translateY(0.5px);
}

.feedback-btn {
  background: #2ecc71;
  border: 1px solid #27ae60;
  border-bottom: 1px solid rgba(0, 0, 0, 0.2);
  color: white;
  padding: 6px 16px;
  border-radius: 6px;
  font-size: 0.82rem;
  font-weight: bold;
  cursor: pointer;
  transition: all 0.15s cubic-bezier(0.25, 0.8, 0.25, 1);
  box-shadow: 0 1px 2px rgba(46, 204, 113, 0.1);
}
.feedback-btn:hover {
  background: #2ecc71cc;
  border-color: #27ae60;
  transform: translateY(-0.5px);
}
.feedback-btn:active {
  color: rgba(255, 255, 255, 0.7);
  border-bottom-color: transparent;
  transform: translateY(0.5px);
}

.settings-container { max-width: 800px; margin: 0 auto; animation: fadeIn 0.3s ease-out; }

.page-title { font-size: 1.8rem; font-weight: 800; color: #2c3e50; margin: 0 0 1.5rem; }

.group-header {
  font-size: 0.85rem; font-weight: bold; color: #909399;
  margin: 1.8rem 0 0.6rem 6px; text-transform: uppercase; letter-spacing: 0.5px;
}

.card-item, .collapse-item {
  background: rgba(255, 255, 255, 0.85);
  backdrop-filter: blur(10px);
  -webkit-backdrop-filter: blur(10px);
  padding: 16px 24px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border: 1px solid rgba(235, 238, 245, 0.8);
  border-radius: 12px;
  margin-bottom: 8px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.015);
  transition: transform 0.25s cubic-bezier(0.25, 0.8, 0.25, 1),
              box-shadow 0.25s cubic-bezier(0.25, 0.8, 0.25, 1),
              border-color 0.25s ease,
              background-color 0.25s ease;
}
.card-item:hover, .collapse-item:hover {
  transform: translateY(-2px);
  box-shadow: 0 8px 24px var(--primary-color-alpha-15), 0 2px 6px rgba(0, 0, 0, 0.02);
  border-color: var(--primary-color-alpha-30);
  background-color: rgba(255, 255, 255, 0.95);
}

.card-left { display: flex; flex-direction: column; flex: 1; }
.card-title { font-size: 0.95rem; font-weight: bold; color: #303133; margin: 0; }
.card-desc { font-size: 0.78rem; color: #909399; margin-top: 4px; line-height: 1.4; }
.card-right { margin-left: 16px; display: flex; align-items: center; }
.status-preview { font-size: 0.82rem; color: #909399; margin-right: 10px; }
.status-preview.truncate {
  max-width: 180px; white-space: nowrap; overflow: hidden;
  text-overflow: ellipsis; display: inline-block; vertical-align: middle;
}



.github-icon { width: 16px; height: 16px; }

.toggle-switch {
  display: flex; align-items: center; width: 58px; height: 28px;
  border-radius: 14px; cursor: pointer; position: relative;
  transition: background-color 0.25s; padding: 0 8px;
}
.toggle-switch.off { background-color: #e4e7eb; justify-content: flex-end; }
.toggle-switch.on { background-color: #67c23a; justify-content: flex-start; }
.toggle-text { font-size: 0.75rem; font-weight: bold; color: white; }
.toggle-switch.off .toggle-text { color: #909399; }
.toggle-slider {
  width: 22px; height: 22px; background-color: white;
  border-radius: 50%; position: absolute; top: 3px;
  box-shadow: 0 1px 3px rgba(0,0,0,0.15);
  transition: left 0.25s, right 0.25s;
}
.toggle-switch.on .toggle-slider { right: 3px; }
.toggle-switch.off .toggle-slider { left: 3px; }
.toggle-desc { font-size: 0.82rem; color: #606266; margin-left: 10px; }

.collapse-item { flex-direction: column; align-items: stretch; padding: 0; }
.collapse-header {
  padding: 14px 20px; display: flex; align-items: center;
  justify-content: space-between; cursor: pointer;
}
.collapse-left { display: flex; flex-direction: column; flex: 1; }
.collapse-right { margin-left: 16px; color: #909399; display: flex; align-items: center; }
.arrow-icon { width: 18px; height: 18px; transition: transform 0.2s; }
.arrow-icon.expanded { transform: rotate(180deg); }
.collapse-content {
  padding: 0 20px 16px; border-top: 1px dashed #f0f2f5;
  padding-top: 14px; animation: slideDown 0.2s ease-out; width: 100%;
}

.input-row { display: flex; gap: 8px; width: 100%; }
.input-row.align-center { align-items: center; }
.text-input, .select-input {
  padding: 8px 12px;
  border: 1px solid rgba(0, 0, 0, 0.08);
  border-bottom: 1px solid rgba(0, 0, 0, 0.183);
  border-radius: 6px;
  font-size: 0.85rem;
  outline: none;
  background-color: rgba(255, 255, 255, 0.7);
  transition: all 0.2s ease;
  color: #303133;
}
.text-input:hover, .select-input:hover {
  background-color: rgba(255, 255, 255, 0.9);
  border-color: rgba(0, 0, 0, 0.12);
}
.text-input:focus, .select-input:focus {
  background-color: #ffffff;
  border-color: var(--primary-color);
  border-bottom: 2px solid var(--primary-color);
  padding-bottom: 7px;
  box-shadow: 0 4px 12px var(--primary-color-alpha-15);
}
.text-input {
  flex: 1;
}
.select-input {
  min-width: 140px;
}

.color-picker-label { font-size: 0.88rem; color: #606266; }
.color-picker {
  border: 1px solid #dcdfe6; background: white; padding: 2px;
  width: 44px; height: 28px; cursor: pointer; border-radius: 4px;
}
.color-pickers-row { display: flex; gap: 16px; flex-wrap: wrap; }
.color-picker-item { display: flex; align-items: center; gap: 8px; font-size: 0.85rem; color: #606266; }
.color-picker-item input[type="color"] {
  border: 1px solid #dcdfe6; background: white; padding: 2px;
  width: 36px; height: 24px; cursor: pointer; border-radius: 4px;
}

@keyframes fadeIn { from { opacity: 0; transform: translateY(6px); } to { opacity: 1; transform: translateY(0); } }
@keyframes slideDown { from { opacity: 0; transform: translateY(-4px); } to { opacity: 1; transform: translateY(0); } }

/* Toast */
.toast {
  position: fixed; top: 20px; left: 50%; transform: translateX(-50%);
  padding: 10px 24px; border-radius: 8px; font-size: 0.88rem;
  font-weight: 600; color: white; z-index: 9999;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15); pointer-events: none;
}
.toast-success { background-color: #67c23a; }
.toast-error { background-color: #f56c6c; }
.toast-enter-active { transition: all 0.25s ease-out; }
.toast-leave-active { transition: all 0.2s ease-in; }
.toast-enter-from { opacity: 0; transform: translateX(-50%) translateY(-12px); }
.toast-leave-to { opacity: 0; transform: translateX(-50%) translateY(-8px); }
</style>
