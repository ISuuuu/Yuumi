<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
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

// 自定义下拉状态
const showLogLevelDropdown = ref(false);
const showDpiDropdown = ref(false);
const showLangDropdown = ref(false);
function closeAllDropdowns() {
  showLogLevelDropdown.value = false;
  showDpiDropdown.value = false;
  showLangDropdown.value = false;
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
  document.addEventListener("click", closeAllDropdowns);
  try {
    config.value = await fetchConfig();
    if (config.value && config.value.Personalization && config.value.Personalization.ThemeColor) {
      updateThemeColor(config.value.Personalization.ThemeColor);
    }
  } catch (e) {
    console.error("加载系统配置失败:", e);
  }
});

onUnmounted(() => {
  document.removeEventListener("click", closeAllDropdowns);
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
      <div :class="['collapse-item', 'border-bottom', { 'has-dropdown-open': showDpiDropdown }]">
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
            <div class="dropdown-trigger" @click.stop="showDpiDropdown = !showDpiDropdown">
              <span>{{ config.Personalization.DpiScale === 'Auto' ? '跟随系统设置' : config.Personalization.DpiScale + '%' }}</span>
              <svg :class="['arrow-icon', { expanded: showDpiDropdown }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
              <div v-if="showDpiDropdown" class="dropdown-menu" @click.stop>
                <div :class="['dropdown-item', { active: config.Personalization.DpiScale === 'Auto' }]" @click="config.Personalization.DpiScale = 'Auto'; autoSave(); showDpiDropdown = false">跟随系统设置</div>
                <div :class="['dropdown-item', { active: config.Personalization.DpiScale === '100' }]" @click="config.Personalization.DpiScale = '100'; autoSave(); showDpiDropdown = false">100%</div>
                <div :class="['dropdown-item', { active: config.Personalization.DpiScale === '125' }]" @click="config.Personalization.DpiScale = '125'; autoSave(); showDpiDropdown = false">125%</div>
                <div :class="['dropdown-item', { active: config.Personalization.DpiScale === '150' }]" @click="config.Personalization.DpiScale = '150'; autoSave(); showDpiDropdown = false">150%</div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- 语言 -->
      <div :class="['collapse-item', { 'has-dropdown-open': showLangDropdown }]">
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
            <div class="dropdown-trigger" @click.stop="showLangDropdown = !showLangDropdown">
              <span>{{ config.Personalization.Language === 'Auto' ? '跟随系统设置' : config.Personalization.Language === 'zh_CN' ? '简体中文' : config.Personalization.Language === 'zh_TW' ? '繁體中文' : 'English' }}</span>
              <svg :class="['arrow-icon', { expanded: showLangDropdown }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
              <div v-if="showLangDropdown" class="dropdown-menu" @click.stop>
                <div :class="['dropdown-item', { active: config.Personalization.Language === 'Auto' }]" @click="config.Personalization.Language = 'Auto'; autoSave(); showLangDropdown = false">跟随系统设置</div>
                <div :class="['dropdown-item', { active: config.Personalization.Language === 'zh_CN' }]" @click="config.Personalization.Language = 'zh_CN'; autoSave(); showLangDropdown = false">简体中文</div>
                <div :class="['dropdown-item', { active: config.Personalization.Language === 'zh_TW' }]" @click="config.Personalization.Language = 'zh_TW'; autoSave(); showLangDropdown = false">繁體中文</div>
                <div :class="['dropdown-item', { active: config.Personalization.Language === 'en_US' }]" @click="config.Personalization.Language = 'en_US'; autoSave(); showLangDropdown = false">English</div>
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

      <div class="card-item">
        <div class="card-left">
          <h3 class="card-title">关于</h3>
          <span class="card-desc">当前版本 0.1.0</span>
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
  width: 36px; height: 36px; border: 3px solid rgba(0, 0, 0, 0.05);
  border-top-color: var(--primary-color); border-radius: 50%;
  animation: spin 0.8s linear infinite;
}
@keyframes spin { to { transform: rotate(360deg); } }

.action-btn, .github-btn {
  background: rgba(255, 255, 255, 0.5);
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
  background: rgba(255, 255, 255, 0.95);
  border-color: var(--primary-color);
  transform: translateY(-0.5px);
}
.action-btn:active, .github-btn:active {
  background: rgba(255, 255, 255, 0.7);
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
  color: rgba(255, 255, 255, 0.7);
  transform: translateY(0.5px);
}

.settings-container { max-width: 800px; margin: 0 auto; animation: fadeIn 0.3s ease-out; }

.page-title { font-size: 1.4rem; font-weight: 800; color: var(--text-color); margin: 0 0 1.5rem; letter-spacing: 0.5px; }

.group-header {
  font-size: 0.8rem; font-weight: 700; color: var(--text-muted);
  margin: 1.8rem 0 0.6rem 6px; text-transform: uppercase; letter-spacing: 0.5px;
}

.card-item, .collapse-item {
  background: var(--card-bg);
  padding: 16px 24px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border: 1px solid var(--border-color);
  border-radius: 12px;
  margin-bottom: 8px;
  box-shadow: var(--shadow-sm);
  transition: all 0.25s cubic-bezier(0.25, 0.8, 0.25, 1);
  position: relative;
}
.collapse-item.has-dropdown-open {
  z-index: 10;
}
.card-item:hover, .collapse-item:hover {
  box-shadow: var(--shadow-md);
  border-color: var(--primary-color-alpha-30);
  background-color: #ffffff;
}

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
  transition: background-color 0.25s; padding: 0 8px;
}
.toggle-switch.off { background-color: rgba(0, 0, 0, 0.06); justify-content: flex-end; }
.toggle-switch.on { background-color: var(--primary-color); justify-content: flex-start; }
.toggle-text { font-size: 0.75rem; font-weight: bold; color: white; }
.toggle-switch.off .toggle-text { color: var(--text-dimmed); }
.toggle-slider {
  width: 22px; height: 22px; background-color: white;
  border-radius: 50%; position: absolute; top: 3px;
  box-shadow: 0 1px 3px rgba(0,0,0,0.15);
  transition: left 0.25s, right 0.25s;
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
  background: rgba(255, 255, 255, 0.5);
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
  background: rgba(255, 255, 255, 0.8);
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
  padding: 8px 12px;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  font-size: 0.82rem;
  outline: none;
  background-color: rgba(255, 255, 255, 0.6);
  transition: all 0.2s ease;
  color: var(--text-color);
}
.text-input:hover {
  background-color: rgba(255, 255, 255, 0.95);
  border-color: var(--border-color-hover);
}
.text-input:focus {
  background-color: #fff;
  border-color: var(--primary-color);
  box-shadow: 0 0 8px var(--primary-color-alpha-15);
}

.dropdown-trigger {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background: rgba(255, 255, 255, 0.5);
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
  background: #ffffff;
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
  background: #ffffff;
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
  background: rgba(0, 0, 0, 0.02);
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
  background-color: rgba(255, 255, 255, 0.6);
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
  background-color: rgba(255, 255, 255, 0.85);
  border-color: var(--primary-color-alpha-40);
}
.number-input:focus {
  background-color: #fff;
  border-color: var(--primary-color);
  box-shadow: 0 0 0 3px var(--primary-color-alpha-15);
}

.color-picker-label { font-size: 0.82rem; color: var(--text-muted); }
.color-picker {
  border: 1px solid var(--border-color); background: rgba(255, 255, 255, 0.6); padding: 2px;
  width: 44px; height: 28px; cursor: pointer; border-radius: 4px;
}
.color-pickers-row { display: flex; gap: 16px; flex-wrap: wrap; }
.color-picker-item { display: flex; align-items: center; gap: 8px; font-size: 0.82rem; color: var(--text-muted); }
.color-picker-item input[type="color"] {
  border: 1px solid var(--border-color); background: rgba(255, 255, 255, 0.6); padding: 2px;
  width: 36px; height: 24px; cursor: pointer; border-radius: 4px;
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
</style>
