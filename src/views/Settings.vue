<script setup lang="ts">
import { ref, onMounted } from "vue";
import { fetchConfig, updateConfig } from "../api/lcu";
import type { AppConfig } from "../api/lcu";

const config = ref<AppConfig | null>(null);
const saving = ref(false);

// 手风琴折叠状态管理
const activeCollapse = ref<string | null>(null);

function toggleCollapse(panelName: string) {
  activeCollapse.value = activeCollapse.value === panelName ? null : panelName;
}

onMounted(async () => {
  try {
    config.value = await fetchConfig();
  } catch (e) {
    console.error("加载系统配置失败:", e);
  }
});

async function saveSettings() {
  if (!config.value) return;
  saving.value = true;
  try {
    await updateConfig(config.value);
    alert("💾 系统设置已成功保存！");
  } catch (e: any) {
    alert("❌ 保存设置失败: " + e.toString());
  } finally {
    saving.value = false;
  }
}

// 模拟动作
function handleSelectFolder() {
  alert("📁 启动文件夹选择。已自动将当前 LOL 路径保存到路径配置中。");
}

function handleClearCache() {
  alert("🧹 所有缓存的 LCU 游戏资源（头像、装备、技能、符文等）已成功清除并重置，将在下次重新连接时自动加载。");
}

function handleOpenLogFolder() {
  alert("📂 正在打开应用日志目录：%APPDATA%/Yuumi/logs/");
}

function handleProvideFeedback() {
  alert("💌 感谢您的反馈！如有 Bug 或建议，欢迎访问 GitHub Issues 进行提交与交流。");
}

function handleOpenGithub() {
  alert("🔗 正在打开浏览器载入 GitHub 项目主页...");
}
</script>

<template>
  <div class="settings-view">
    <div v-if="!config" class="tip-container">
      <div class="loading-spinner"></div>
      <p class="tip">加载配置数据中...</p>
    </div>

    <div v-else class="settings-container">
      <div class="header-with-save">
        <h1 class="page-title">设置</h1>
        <button class="save-settings-btn" @click="saveSettings" :disabled="saving">
          {{ saving ? "保存中..." : "保存设置" }}
        </button>
      </div>

      <!-- 1. 功能组 -->
      <div class="group-header">功能</div>

      <!-- LCU API 并发数 -->
      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('concurrency')">
          <div class="collapse-left">
            <h3 class="card-title">LCU API 并发数</h3>
            <span class="card-desc">该值越大数据加载速度越快，但越可能引起客户端闪退</span>
          </div>
          <div class="collapse-right">
            <span class="status-preview">当前值: {{ config.Functions.ApiConcurrencyNumber }}</span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'concurrency' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'concurrency'" class="collapse-content">
          <div class="input-row">
            <select v-model.number="config.Functions.ApiConcurrencyNumber" class="select-input">
              <option v-for="n in 10" :key="n" :value="n">{{ n }} (个并发)</option>
            </select>
          </div>
        </div>
      </div>

      <!-- 默认对局数量 -->
      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('matchcount')">
          <div class="collapse-left">
            <h3 class="card-title">默认对局数量</h3>
            <span class="card-desc">调整在个人生涯界面中显示的最大对局数量</span>
          </div>
          <div class="collapse-right">
            <span class="status-preview">当前值: {{ config.Functions.CareerGamesNumber }}</span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'matchcount' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'matchcount'" class="collapse-content">
          <div class="input-row">
            <select v-model.number="config.Functions.CareerGamesNumber" class="select-input">
              <option :value="10">10 场</option>
              <option :value="20">20 场</option>
              <option :value="50">50 场</option>
              <option :value="100">100 场</option>
            </select>
          </div>
        </div>
      </div>

      <!-- 对局信息过滤 -->
      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('filter')">
          <div class="collapse-left">
            <h3 class="card-title">对局信息过滤</h3>
            <span class="card-desc">基于你所处的游戏模式筛选对局信息界面显示的战绩</span>
          </div>
          <div class="collapse-right">
            <span class="status-preview">{{ config.Functions.GameInfoFilter ? '已启用' : '未启用' }}</span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'filter' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'filter'" class="collapse-content">
          <div class="input-row align-center">
            <div :class="['toggle-switch', config.Functions.GameInfoFilter ? 'on' : 'off']" @click="config.Functions.GameInfoFilter = !config.Functions.GameInfoFilter">
              <span class="toggle-text">{{ config.Functions.GameInfoFilter ? '开' : '关' }}</span>
              <span class="toggle-slider"></span>
            </div>
            <span class="toggle-desc">只显示当前地图/队列的玩家数据以净化对局信息</span>
          </div>
        </div>
      </div>

      <!-- 保留对局信息界面内容 -->
      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">保留对局信息界面内容</h3>
          <span class="card-desc">保留上一局的对局信息内容直到下一次对局开始</span>
        </div>
        <div class="card-right">
          <div :class="['toggle-switch', config.General.EnableSignalrHub ? 'on' : 'off']" @click="config.General.EnableSignalrHub = !config.General.EnableSignalrHub">
            <span class="toggle-text">{{ config.General.EnableSignalrHub ? '开' : '关' }}</span>
            <span class="toggle-slider"></span>
          </div>
        </div>
      </div>

      <!-- 对局详情中显示段位 -->
      <div class="card-item">
        <div class="card-left">
          <h3 class="card-title">对局详情中显示段位</h3>
          <span class="card-desc">在搜索界面对局详情界面中显示段位，启动该选项将影响加载界面的速度</span>
        </div>
        <div class="card-right">
          <div :class="['toggle-switch', config.Functions.ShowTierInGameInfo ? 'on' : 'off']" @click="config.Functions.ShowTierInGameInfo = !config.Functions.ShowTierInGameInfo">
            <span class="toggle-text">{{ config.Functions.ShowTierInGameInfo ? '开' : '关' }}</span>
            <span class="toggle-slider"></span>
          </div>
        </div>
      </div>

      <!-- 2. OP.GG 组 -->
      <div class="group-header">OP.GG</div>

      <!-- 自动显示 OP.GG 窗口 -->
      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">自动显示 OP.GG 窗口</h3>
          <span class="card-desc">在英雄选择开始时自动显示 OP.GG 窗口</span>
        </div>
        <div class="card-right">
          <div :class="['toggle-switch', config.Functions.AutoShowOpgg ? 'on' : 'off']" @click="config.Functions.AutoShowOpgg = !config.Functions.AutoShowOpgg">
            <span class="toggle-text">{{ config.Functions.AutoShowOpgg ? '开' : '关' }}</span>
            <span class="toggle-slider"></span>
          </div>
        </div>
      </div>

      <!-- 置顶 OP.GG 窗口 -->
      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">置顶 OP.GG 窗口</h3>
          <span class="card-desc">在英雄选择时将 OP.GG 窗口置顶</span>
        </div>
        <div class="card-right">
          <div :class="['toggle-switch', config.Functions.EnableOpggOnTop ? 'on' : 'off']" @click="config.Functions.EnableOpggOnTop = !config.Functions.EnableOpggOnTop">
            <span class="toggle-text">{{ config.Functions.EnableOpggOnTop ? '开' : '关' }}</span>
            <span class="toggle-slider"></span>
          </div>
        </div>
      </div>

      <!-- OP.GG HTTP 代理 -->
      <div class="collapse-item">
        <div class="collapse-header" @click="toggleCollapse('opggproxy')">
          <div class="collapse-left">
            <h3 class="card-title">OP.GG HTTP 代理</h3>
            <span class="card-desc">连接 OP.GG 时启用 HTTP 代理</span>
          </div>
          <div class="collapse-right">
            <span class="status-preview">{{ config.General.EnableOpggProxy ? '已启用' : '未启用' }}</span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'opggproxy' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'opggproxy'" class="collapse-content">
          <div class="input-row align-center">
            <div :class="['toggle-switch', config.General.EnableOpggProxy ? 'on' : 'off']" @click="config.General.EnableOpggProxy = !config.General.EnableOpggProxy">
              <span class="toggle-text">{{ config.General.EnableOpggProxy ? '开' : '关' }}</span>
              <span class="toggle-slider"></span>
            </div>
            <input v-model="config.General.OpggProxyAddr" placeholder="127.0.0.1:10809" class="text-input" :disabled="!config.General.EnableOpggProxy" />
          </div>
        </div>
      </div>

      <!-- 3. 通用组 -->
      <div class="group-header">通用</div>

      <!-- 客户端路径 -->
      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">客户端路径</h3>
          <span class="card-desc">设置客户端路径以及顺序</span>
        </div>
        <div class="card-right">
          <button class="action-btn" @click="handleSelectFolder">选择文件夹</button>
        </div>
      </div>

      <!-- 自动启动游戏 -->
      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">自动启动游戏</h3>
          <span class="card-desc">启动 Yuumi 时自动启动 LOL 客户端</span>
        </div>
        <div class="card-right">
          <div :class="['toggle-switch', config.General.EnableStartLolWithApp ? 'on' : 'off']" @click="config.General.EnableStartLolWithApp = !config.General.EnableStartLolWithApp">
            <span class="toggle-text">{{ config.General.EnableStartLolWithApp ? '开' : '关' }}</span>
            <span class="toggle-slider"></span>
          </div>
        </div>
      </div>

      <!-- 清除缓存 -->
      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">清除缓存</h3>
          <span class="card-desc">删除所有游戏资源的缓存（建议在游戏资源有更新时使用）</span>
        </div>
        <div class="card-right">
          <button class="action-btn" @click="handleClearCache">删除</button>
        </div>
      </div>

      <!-- 最小化到任务栏托盘 -->
      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">最小化到任务栏托盘</h3>
          <span class="card-desc">点击右上角关闭时将程序最小化到托盘</span>
        </div>
        <div class="card-right">
          <div :class="['toggle-switch', config.General.EnableCloseToTray ? 'on' : 'off']" @click="config.General.EnableCloseToTray = !config.General.EnableCloseToTray">
            <span class="toggle-text">{{ config.General.EnableCloseToTray ? '开' : '关' }}</span>
            <span class="toggle-slider"></span>
          </div>
        </div>
      </div>

      <!-- 静默启动 -->
      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">静默启动</h3>
          <span class="card-desc">启动 Yuumi 后最小化窗口到任务栏托盘</span>
        </div>
        <div class="card-right">
          <div :class="['toggle-switch', config.General.EnableGameStartMinimize ? 'on' : 'off']" @click="config.General.EnableGameStartMinimize = !config.General.EnableGameStartMinimize">
            <span class="toggle-text">{{ config.General.EnableGameStartMinimize ? '开' : '关' }}</span>
            <span class="toggle-slider"></span>
          </div>
        </div>
      </div>

      <!-- 对局上传 API 地址 -->
      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('apiaddr')">
          <div class="collapse-left">
            <h3 class="card-title">对局上传 API 地址</h3>
            <span class="card-desc">用于上传对局历史数据的 URL 端点</span>
          </div>
          <div class="collapse-right">
            <span class="status-preview truncate">Current: {{ config.General.SignalrServerUrl || 'https://your-signalr-server.com/' }}</span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'apiaddr' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'apiaddr'" class="collapse-content">
          <div class="input-row">
            <input v-model="config.General.SignalrServerUrl" placeholder="https://your-signalr-server.com/" class="text-input" />
          </div>
        </div>
      </div>

      <!-- LCU 实时查询 -->
      <div class="card-item">
        <div class="card-left">
          <h3 class="card-title">LCU 实时查询</h3>
          <span class="card-desc">通过 LCU 客户端实时查询对局历史</span>
        </div>
        <div class="card-right">
          <div :class="['toggle-switch', config.General.EnableCheckUpdate ? 'on' : 'off']" @click="config.General.EnableCheckUpdate = !config.General.EnableCheckUpdate">
            <span class="toggle-text">{{ config.General.EnableCheckUpdate ? '开' : '关' }}</span>
            <span class="toggle-slider"></span>
          </div>
        </div>
      </div>

      <!-- 4. 日志组 -->
      <div class="group-header">日志</div>

      <!-- 日志等级 -->
      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('loglevel')">
          <div class="collapse-left">
            <h3 class="card-title">日志等级</h3>
            <span class="card-desc">修改 Yuumi 记录日志的等级（重启后生效）</span>
          </div>
          <div class="collapse-right">
            <span class="status-preview">{{ config.General.LogLevel === 0 ? 'Debug' : config.General.LogLevel === 1 ? 'Info' : 'Error' }}</span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'loglevel' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'loglevel'" class="collapse-content">
          <div class="input-row">
            <select v-model.number="config.General.LogLevel" class="select-input">
              <option :value="0">Debug</option>
              <option :value="1">Info</option>
              <option :value="2">Error</option>
            </select>
          </div>
        </div>
      </div>

      <!-- 日志文件 -->
      <div class="card-item">
        <div class="card-left">
          <h3 class="card-title">日志文件</h3>
          <span class="card-desc">打开日志文件夹</span>
        </div>
        <div class="card-right">
          <button class="action-btn" @click="handleOpenLogFolder">打开文件夹</button>
        </div>
      </div>

      <!-- 5. 个性化组 -->
      <div class="group-header">个性化</div>

      <!-- 云母效果 -->
      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">云母效果</h3>
          <span class="card-desc">窗口和表面显示半透明（仅在 Win11 上可用）</span>
        </div>
        <div class="card-right">
          <div :class="['toggle-switch', config.Personalization.MicaEnabled ? 'on' : 'off']" @click="config.Personalization.MicaEnabled = !config.Personalization.MicaEnabled">
            <span class="toggle-text">{{ config.Personalization.MicaEnabled ? '开' : '关' }}</span>
            <span class="toggle-slider"></span>
          </div>
        </div>
      </div>

      <!-- 应用主题 -->
      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('apptheme')">
          <div class="collapse-left">
            <h3 class="card-title">应用主题</h3>
            <span class="card-desc">调整 Yuumi 的外观主题</span>
          </div>
          <div class="collapse-right">
            <span class="status-preview">{{ config.Personalization.Language === 'Dark' ? '深色' : '浅色' }}</span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'apptheme' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'apptheme'" class="collapse-content">
          <div class="input-row">
            <select v-model="config.Personalization.Language" class="select-input">
              <option value="Light">浅色</option>
              <option value="Dark">深色</option>
            </select>
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
            <span class="status-preview">自定义颜色</span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'themecolor' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'themecolor'" class="collapse-content">
          <div class="input-row align-center">
            <label class="color-picker-label">调色盘:</label>
            <input type="color" class="color-picker" v-model="config.Personalization.WinCardColor" />
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
            <span class="status-preview">默认颜色</span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'cardcolors' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'cardcolors'" class="collapse-content">
          <div class="color-pickers-row">
            <div class="color-picker-item">
              <label>胜利卡片:</label>
              <input type="color" v-model="config.Personalization.WinCardColor" />
            </div>
            <div class="color-picker-item">
              <label>失败卡片:</label>
              <input type="color" v-model="config.Personalization.LoseCardColor" />
            </div>
            <div class="color-picker-item">
              <label>重开卡片:</label>
              <input type="color" v-model="config.Personalization.RemakeCardColor" />
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
            <span class="status-preview">默认颜色</span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'deathfontcolor' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'deathfontcolor'" class="collapse-content">
          <div class="color-pickers-row">
            <div class="color-picker-item">
              <label>浅色主题下颜色:</label>
              <input type="color" v-model="config.Personalization.LightDeathsNumberColor" />
            </div>
            <div class="color-picker-item">
              <label>深色主题下颜色:</label>
              <input type="color" v-model="config.Personalization.DarkDeathsNumberColor" />
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
            <span class="status-preview">
              {{ config.Personalization.DpiScale === 'Auto' ? '跟随系统设置' : config.Personalization.DpiScale + '%' }}
            </span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'dpiscale' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'dpiscale'" class="collapse-content">
          <div class="input-row">
            <select v-model="config.Personalization.DpiScale" class="select-input">
              <option value="Auto">跟随系统设置</option>
              <option value="100">100%</option>
              <option value="125">125%</option>
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
              {{ config.Personalization.Language === 'Auto' ? '跟随系统设置' :
                 config.Personalization.Language === 'zh_CN' ? '简体中文' :
                 config.Personalization.Language === 'zh_TW' ? '繁體中文' : 'English' }}
            </span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'lang' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'lang'" class="collapse-content">
          <div class="input-row">
            <select v-model="config.Personalization.Language" class="select-input">
              <option value="Auto">跟随系统设置</option>
              <option value="zh_CN">简体中文</option>
              <option value="zh_TW">繁體中文</option>
              <option value="en_US">English</option>
            </select>
          </div>
        </div>
      </div>

      <!-- 6. 软件更新组 -->
      <div class="group-header">软件更新</div>

      <!-- 检查更新 -->
      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">检查更新</h3>
          <span class="card-desc">在 Yuumi 启动时自动检查更新</span>
        </div>
        <div class="card-right">
          <div :class="['toggle-switch', config.General.EnableCheckUpdate ? 'on' : 'off']" @click="config.General.EnableCheckUpdate = !config.General.EnableCheckUpdate">
            <span class="toggle-text">{{ config.General.EnableCheckUpdate ? '开' : '关' }}</span>
            <span class="toggle-slider"></span>
          </div>
        </div>
      </div>

      <!-- GitHub HTTP 代理 -->
      <div class="collapse-item">
        <div class="collapse-header" @click="toggleCollapse('githubproxy')">
          <div class="collapse-left">
            <h3 class="card-title">GitHub HTTP 代理</h3>
            <span class="card-desc">连接 GitHub 时启用 HTTP 代理</span>
          </div>
          <div class="collapse-right">
            <span class="status-preview">{{ config.General.EnableGithubProxy ? `已启用，代理: ${config.General.GithubProxyAddr}` : '未启用' }}</span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'githubproxy' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'githubproxy'" class="collapse-content">
          <div class="input-row align-center">
            <div :class="['toggle-switch', config.General.EnableGithubProxy ? 'on' : 'off']" @click="config.General.EnableGithubProxy = !config.General.EnableGithubProxy">
              <span class="toggle-text">{{ config.General.EnableGithubProxy ? '开' : '关' }}</span>
              <span class="toggle-slider"></span>
            </div>
            <input v-model="config.General.GithubProxyAddr" placeholder="127.0.0.1:7897" class="text-input" :disabled="!config.General.EnableGithubProxy" />
          </div>
        </div>
      </div>

      <!-- 7. 关于组 -->
      <div class="group-header">关于</div>

      <!-- 提供反馈 -->
      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">提供反馈</h3>
          <span class="card-desc">通过提供反馈帮助我们改善 Yuumi</span>
        </div>
        <div class="card-right">
          <button class="feedback-btn" @click="handleProvideFeedback">提供反馈</button>
        </div>
      </div>

      <!-- 关于 -->
      <div class="card-item">
        <div class="card-left">
          <h3 class="card-title">关于</h3>
          <span class="card-desc">版权所有 © 2026, xzmkim. 当前版本 1.5.42</span>
        </div>
        <div class="card-right">
          <button class="github-btn" @click="handleOpenGithub">
            <svg class="github-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M9 19c-5 1.5-5-2.5-7-3m14 6v-3.87a3.37 3.37 0 0 0-.94-2.61c3.14-.35 6.44-1.54 6.44-7A5.44 5.44 0 0 0 20 4.77 5.07 5.07 0 0 0 19.91 1S18.73.65 16 2.48a13.38 13.38 0 0 0-7 0C6.27.65 5.09 1 5.09 1A5.07 5.07 0 0 0 5 4.77a5.44 5.44 0 0 0-1.5 3.78c0 5.42 3.3 6.61 6.44 7A3.37 3.37 0 0 0 9 18.13V22"/>
            </svg>
            查看 GitHub
          </button>
        </div>
      </div>

    </div>
  </div>
</template>

<style scoped>
.settings-view {
  padding: 1.5rem;
  background-color: #fafbfc;
  min-height: 100%;
}

.tip-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 8rem 2rem;
  color: #909399;
}

.tip {
  font-size: 1rem;
  color: #8c8c8c;
  margin-top: 12px;
}

.loading-spinner {
  width: 36px;
  height: 36px;
  border: 3px solid #e2e5e9;
  border-top-color: #6c5ce7;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.settings-container {
  max-width: 800px;
  margin: 0 auto;
  animation: fadeIn 0.3s ease-out;
}

.header-with-save {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 1.5rem;
}

.page-title {
  font-size: 1.8rem;
  font-weight: 800;
  color: #2c3e50;
  margin: 0;
}

.save-settings-btn {
  background-color: #6c5ce7;
  color: white;
  border: none;
  padding: 8px 24px;
  border-radius: 6px;
  font-size: 0.88rem;
  font-weight: bold;
  cursor: pointer;
  box-shadow: 0 2px 6px rgba(108, 92, 231, 0.2);
  transition: background-color 0.2s;
}

.save-settings-btn:hover {
  background-color: #5a4bd1;
}

.save-settings-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.group-header {
  font-size: 0.85rem;
  font-weight: bold;
  color: #909399;
  margin: 1.8rem 0 0.6rem 6px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

/* 卡片 Item 与折叠块 */
.card-item, .collapse-item {
  background: white;
  padding: 14px 20px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border: 1px solid #ebeef5;
  border-radius: 8px;
  margin-bottom: 4px;
  box-shadow: 0 2px 4px rgba(0,0,0,0.01);
}

.card-item.border-bottom, .collapse-item.border-bottom {
  margin-bottom: 0px;
  border-bottom: none;
  border-bottom-left-radius: 0;
  border-bottom-right-radius: 0;
}

.card-item + .card-item:not(.border-bottom), 
.collapse-item + .card-item:not(.border-bottom),
.card-item + .collapse-item:not(.border-bottom) {
  border-top-left-radius: 0;
  border-top-right-radius: 0;
}

.card-left {
  display: flex;
  flex-direction: column;
  flex: 1;
}

.card-title {
  font-size: 0.95rem;
  font-weight: bold;
  color: #303133;
  margin: 0;
}

.card-desc {
  font-size: 0.78rem;
  color: #909399;
  margin-top: 4px;
  line-height: 1.4;
}

.card-right {
  margin-left: 16px;
  display: flex;
  align-items: center;
}

.status-preview {
  font-size: 0.82rem;
  color: #909399;
  margin-right: 10px;
}

.status-preview.truncate {
  max-width: 180px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  display: inline-block;
  vertical-align: middle;
}

/* 动作按钮 */
.action-btn {
  background: white;
  border: 1px solid #dcdfe6;
  color: #606266;
  padding: 6px 20px;
  border-radius: 6px;
  font-size: 0.82rem;
  font-weight: bold;
  cursor: pointer;
  transition: all 0.2s;
}

.action-btn:hover {
  background: #f5f7fa;
  border-color: #c0c4cc;
}

/* 绿色反馈按钮 */
.feedback-btn {
  background-color: #2ecc71;
  color: white;
  border: none;
  padding: 6px 20px;
  border-radius: 6px;
  font-size: 0.82rem;
  font-weight: bold;
  cursor: pointer;
  transition: background-color 0.2s;
}

.feedback-btn:hover {
  background-color: #27ae60;
}

/* GitHub 查看按钮 */
.github-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  background: white;
  border: 1px solid #dcdfe6;
  color: #606266;
  padding: 6px 16px;
  border-radius: 6px;
  font-size: 0.82rem;
  font-weight: bold;
  cursor: pointer;
  transition: all 0.2s;
}

.github-btn:hover {
  background: #f5f7fa;
  border-color: #c0c4cc;
}

.github-icon {
  width: 16px;
  height: 16px;
}

/* Switch 开关 */
.toggle-switch {
  display: flex;
  align-items: center;
  width: 58px;
  height: 28px;
  border-radius: 14px;
  cursor: pointer;
  position: relative;
  transition: background-color 0.25s;
  padding: 0 8px;
}

.toggle-switch.off {
  background-color: #e4e7eb;
  justify-content: flex-end;
}

.toggle-switch.on {
  background-color: #67c23a;
  justify-content: flex-start;
}

.toggle-text {
  font-size: 0.75rem;
  font-weight: bold;
  color: white;
}

.toggle-switch.off .toggle-text {
  color: #909399;
}

.toggle-slider {
  width: 22px;
  height: 22px;
  background-color: white;
  border-radius: 50%;
  position: absolute;
  top: 3px;
  box-shadow: 0 1px 3px rgba(0,0,0,0.15);
  transition: left 0.25s, right 0.25s;
}

.toggle-switch.on .toggle-slider {
  right: 3px;
}

.toggle-switch.off .toggle-slider {
  left: 3px;
}

.toggle-desc {
  font-size: 0.82rem;
  color: #606266;
  margin-left: 10px;
}

/* 手风琴折叠样式 */
.collapse-item {
  flex-direction: column;
  align-items: stretch;
  padding: 0;
}

.collapse-header {
  padding: 14px 20px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  cursor: pointer;
}

.collapse-left {
  display: flex;
  flex-direction: column;
  flex: 1;
}

.collapse-right {
  margin-left: 16px;
  color: #909399;
  display: flex;
  align-items: center;
}

.arrow-icon {
  width: 18px;
  height: 18px;
  transition: transform 0.2s;
}

.arrow-icon.expanded {
  transform: rotate(180deg);
}

.collapse-content {
  padding: 0 20px 16px;
  border-top: 1px dashed #f0f2f5;
  padding-top: 14px;
  animation: slideDown 0.2s ease-out;
  width: 100%;
}

.input-row {
  display: flex;
  gap: 8px;
  width: 100%;
}

.input-row.align-center {
  align-items: center;
}

.text-input {
  flex: 1;
  padding: 8px 12px;
  border: 1px solid #dcdfe6;
  border-radius: 6px;
  font-size: 0.85rem;
  outline: none;
}

.text-input:focus {
  border-color: #6c5ce7;
}

.select-input {
  padding: 8px 12px;
  border: 1px solid #dcdfe6;
  border-radius: 6px;
  font-size: 0.85rem;
  background-color: white;
  outline: none;
  min-width: 140px;
}

.select-input:focus {
  border-color: #6c5ce7;
}

/* 颜色选择器专有样式 */
.color-picker-label {
  font-size: 0.88rem;
  color: #606266;
}

.color-picker {
  border: 1px solid #dcdfe6;
  background: white;
  padding: 2px;
  width: 44px;
  height: 28px;
  cursor: pointer;
  border-radius: 4px;
}

.color-pickers-row {
  display: flex;
  gap: 16px;
  flex-wrap: wrap;
}

.color-picker-item {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 0.85rem;
  color: #606266;
}

.color-picker-item input[type="color"] {
  border: 1px solid #dcdfe6;
  background: white;
  padding: 2px;
  width: 36px;
  height: 24px;
  cursor: pointer;
  border-radius: 4px;
}

@keyframes fadeIn {
  from { opacity: 0; transform: translateY(6px); }
  to { opacity: 1; transform: translateY(0); }
}

@keyframes slideDown {
  from { opacity: 0; transform: translateY(-4px); }
  to { opacity: 1; transform: translateY(0); }
}
</style>
