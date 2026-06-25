<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { fetchConfig, updateConfig, lcuRequest } from "../api/lcu";
import type { AppConfig } from "../api/lcu";
import ChampionPicker from "../components/ChampionPicker.vue";
import SpellPicker from "../components/SpellPicker.vue";
import LcuImage from "../components/LcuImage.vue";

const config = ref<AppConfig | null>(null);
const loading = ref(false);

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

// 折叠面板状态
const activeCollapse = ref<string | null>(null);

function toggleCollapse(panelName: string) {
  activeCollapse.value = activeCollapse.value === panelName ? null : panelName;
}

// 个人主页状态项
const statusInput = ref("");
const skinIdInput = ref<number | null>(null);
// 段位展示状态
const spoofQueue = ref("RANKED_SOLO_5x5");
const spoofTier = ref("CHALLENGER");
const spoofDivision = ref("I");

// 自定义下拉状态
const showGameModeDropdown = ref(false);
const showSpoofQueueDropdown = ref(false);
const showSpoofTierDropdown = ref(false);
const showSpoofDivisionDropdown = ref(false);
function closeAllDropdowns() {
  showGameModeDropdown.value = false;
  showSpoofQueueDropdown.value = false;
  showSpoofTierDropdown.value = false;
  showSpoofDivisionDropdown.value = false;
}
const currentGameModeName = computed(() => GAME_MODES.find(m => m.id === config.value?.Functions?.DefaultGameMode)?.name || '未知');
const SPOOF_QUEUE_LABELS: Record<string, string> = { RANKED_TFT: '云顶之弈', RANKED_SOLO_5x5: '单双排位', RANKED_FLEX_SR: '灵活排位' };
const SPOOF_TIER_LABELS: Record<string, string> = { UNRANKED: '未定级', CHALLENGER: '最强王者', GRANDMASTER: '傲世宗师', MASTER: '超凡大师', DIAMOND: '璀璨钻石', EMERALD: '流光翡翠', PLATINUM: '华贵铂金', GOLD: '荣耀黄金', SILVER: '不屈白银', BRONZE: '英勇黄铜', IRON: '坚韧黑铁' };
const bgChampion = ref<number[]>([]);

// 皮肤列表（选英雄后加载）
interface SkinInfo { id: number; name: string; splashPath: string; loadScreenPath: string }
const skinList = ref<SkinInfo[]>([]);
const selectedSkinId = ref<number | null>(null);
const skinLoading = ref(false);

// 皮肤弹窗状态
const showSkinModal = ref(false);
const activeSkinIndex = ref(0);

const currentSelectedSkin = computed(() => {
  return skinList.value.find(s => s.id === selectedSkinId.value) || null;
});

// 监听背景英雄点选，自动加载该英雄的皮肤列表
watch(bgChampion, async (newVal: number[]) => {
  skinList.value = [];
  selectedSkinId.value = null;
  skinIdInput.value = null;
  activeSkinIndex.value = 0;
  if (!newVal || newVal.length === 0) return;

  skinLoading.value = true;
  try {
    const skins = await invoke<Array<{id: number; name: string; load_screen_path: string}>>("get_champion_skins", {
      championId: newVal[0],
    });
    if (skins && skins.length > 0) {
      skinList.value = skins.map((s: {id: number; name: string; load_screen_path: string}) => ({
        id: s.id,
        name: s.name,
        splashPath: s.load_screen_path,
        loadScreenPath: s.load_screen_path,
      }));
      selectedSkinId.value = skinList.value[0].id;
      skinIdInput.value = skinList.value[0].id;
      activeSkinIndex.value = 0;
    } else {
      showToast('该英雄暂无皮肤数据', 'error');
    }
  } catch (e) {
    console.error("加载皮肤列表失败:", e);
    showToast('加载皮肤失败', 'error');
  } finally {
    skinLoading.value = false;
  }
});

// 键盘事件处理
function handleKeyDown(e: KeyboardEvent) {
  if (!showSkinModal.value) return;
  if (e.key === "ArrowLeft") {
    prevSkin();
  } else if (e.key === "ArrowRight") {
    nextSkin();
  } else if (e.key === "Enter") {
    confirmSkinSelection();
  } else if (e.key === "Escape") {
    showSkinModal.value = false;
  }
}

// 监听弹窗打开以注册/解绑键盘事件
watch(showSkinModal, (val) => {
  if (val) {
    window.addEventListener("keydown", handleKeyDown);
  } else {
    window.removeEventListener("keydown", handleKeyDown);
  }
});

function openSkinModal() {
  if (skinList.value.length === 0) {
    showToast('请先选择英雄以加载皮肤列表', 'error');
    return;
  }
  const idx = skinList.value.findIndex(s => s.id === selectedSkinId.value);
  if (idx !== -1) {
    activeSkinIndex.value = idx;
  } else {
    activeSkinIndex.value = 0;
  }
  showSkinModal.value = true;
}

function prevSkin() {
  if (skinList.value.length === 0) return;
  activeSkinIndex.value = (activeSkinIndex.value - 1 + skinList.value.length) % skinList.value.length;
}

function nextSkin() {
  if (skinList.value.length === 0) return;
  activeSkinIndex.value = (activeSkinIndex.value + 1) % skinList.value.length;
}

function selectSkin(index: number) {
  activeSkinIndex.value = index;
}

async function confirmSkinSelection() {
  const currentSkin = skinList.value[activeSkinIndex.value];
  if (currentSkin) {
    selectedSkinId.value = currentSkin.id;
    skinIdInput.value = currentSkin.id;
    showSkinModal.value = false;
    await handleApplyBackground();
  }
}

// 观战输入项
const spectateSummonerName = ref("");

// 锁定游戏设置状态
const isGameSettingsLocked = ref(false);
async function checkGameSettingsLock() {
  try {
    isGameSettingsLocked.value = await invoke<boolean>("get_game_settings_readonly");
  } catch (e) {
    console.error("获取游戏设置锁定状态失败:", e);
  }
}

const GAME_MODES: { id: number; name: string }[] = [
  { id: 2400, name: "海克斯大乱斗" },
  { id: 450, name: "极地大乱斗" },
  { id: 430, name: "匹配模式" },
  { id: 420, name: "单双排位" },
  { id: 440, name: "灵活排位" },
  { id: 900, name: "无限火力" },
  { id: 1020, name: "克隆模式" },
  { id: 1300, name: "极限闪击" },
  { id: 1700, name: "斗魂竞技场" },
];

onMounted(async () => {
  document.addEventListener("click", closeAllDropdowns);
  try {
    config.value = await fetchConfig();
    await checkGameSettingsLock();
  } catch (e) {
    console.error("加载其他功能配置失败:", e);
  }
});

onUnmounted(() => {
  document.removeEventListener("click", closeAllDropdowns);
});

// 英雄/技能选择变化时自动保存
function onPickerChange() {
  triggerAutoSave();
}

// 自动保存设置函数
async function triggerAutoSave() {
  if (!config.value) return;
  try {
    await updateConfig(config.value);
  } catch (e) {
    console.error("自动保存设置失败:", e);
  }
}

// 模拟观战启动
async function handleSpectate() {
  if (!spectateSummonerName.value.trim()) return;
  loading.value = true;
  try {
    // 第一步：通过召唤师名称获取 puuid
    const name = spectateSummonerName.value.trim().replace(/[⁦⁩]/g, '');
    const summonerResp = await lcuRequest<any>("GET", `/lol-summoner/v1/summoners?name=${encodeURIComponent(name)}`);
    if (!summonerResp.success || !summonerResp.data) {
      showToast('未找到该召唤师，请检查名称后重试', 'error');
      return;
    }
    const puuid = summonerResp.data.puuid;

    // 第二步：通过 LCU API 发起观战
    const resp = await lcuRequest<any>("POST", "/lol-spectator/v1/spectate/launch", {
      allowObserveMode: "ALL",
      dropInSpectateGameId: name,
      gameQueueType: "",
      puuid: puuid
    });
    if (resp.success) {
      showToast('观战启动成功，请等待加载...');
    } else {
      // 空 body 表示成功，非空表示该玩家不在游戏中
      showToast('观战失败: ' + (resp.error || '该召唤师可能在游戏中'), 'error');
    }
  } catch (e: any) {
    showToast('观战异常: ' + e.toString(), 'error');
  } finally {
    loading.value = false;
  }
}

// 修复客户端窗口
async function handleFixWindow() {
  loading.value = true;
  try {
    await invoke("fix_lcu_window");
    showToast('客户端窗口已重设，未生效请用管理员模式启动');
  } catch (e: any) {
    showToast('修复失败: ' + e.toString(), 'error');
  } finally {
    loading.value = false;
  }
}

// 重启客户端
async function handleRestartClient() {
  if (!confirm("⚡ 您确定要重启 LOL 客户端吗？(无需重新登录或排队)")) return;
  loading.value = true;
  try {
    const resp = await lcuRequest<any>("POST", "/riotclient/kill-and-restart-ux");
    if (resp.success) {
      showToast('重启指令已发送，客户端正在重新引导...');
    } else {
      showToast('重启失败: ' + resp.error, 'error');
    }
  } catch (e: any) {
    showToast('重启异常: ' + e.toString(), 'error');
  } finally {
    loading.value = false;
  }
}

// 更换状态签名
async function handleApplyStatus() {
  if (!statusInput.value.trim()) return;
  loading.value = true;
  try {
    const resp = await lcuRequest<any>("PUT", "/lol-chat/v1/me", {
      statusMessage: statusInput.value.trim()
    });
    if (resp.success) {
      showToast('签名已更新');
      statusInput.value = "";
    } else {
      showToast('修改失败: ' + resp.error, 'error');
    }
  } catch (e: any) {
    showToast('修改异常: ' + e.toString(), 'error');
  } finally {
    loading.value = false;
  }
}

// 更换生涯背景
async function handleApplyBackground() {
  if (skinIdInput.value === null) return;
  loading.value = true;
  try {
    // Python 使用 POST /lol-summoner/v1/current-summoner/summoner-profile
    const resp = await lcuRequest<any>("POST", "/lol-summoner/v1/current-summoner/summoner-profile", {
      key: "backgroundSkinId",
      value: skinIdInput.value
    });
    if (resp.success) {
      showToast('背景皮肤更换成功');
    } else {
      showToast('更换失败: ' + resp.error, 'error');
    }
  } catch (e: any) {
    showToast('更换异常: ' + e.toString(), 'error');
  } finally {
    loading.value = false;
  }
}

// 伪装段位展示
async function handleApplyRankSpoof() {
  loading.value = true;
  try {
    const resp = await lcuRequest<any>("PUT", "/lol-chat/v1/me", {
      lol: {
        rankedLeagueQueue: spoofQueue.value,
        rankedLeagueTier: spoofTier.value,
        rankedLeagueDivision: spoofDivision.value
      }
    });
    if (resp.success) {
      showToast('段位伪装已应用');
    } else {
      showToast('段位伪装失败: ' + resp.error, 'error');
    }
  } catch (e: any) {
    showToast('伪装异常: ' + e.toString(), 'error');
  } finally {
    loading.value = false;
  }
}

// 在线状态更改
async function handleApplyAvailability(avail: string) {
  loading.value = true;
  try {
    const resp = await lcuRequest<any>("PUT", "/lol-chat/v1/me", {
      availability: avail
    });
    if (resp.success) {
      const availText = avail === 'chat' ? '在线' : avail === 'away' ? '离开' : '隐身';
      showToast('在线状态已切换: ' + availText);
    } else {
      showToast('状态切换失败: ' + resp.error, 'error');
    }
  } catch (e: any) {
    showToast('状态切换异常: ' + e.toString(), 'error');
  } finally {
    loading.value = false;
  }
}

// 卸载全部勋章
async function handleClearBadges() {
  if (!confirm("🏅 确定要清除个人主页展示的所有挑战勋章吗？")) return;
  loading.value = true;
  try {
    // Python: POST /lol-challenges/v1/update-player-preferences/ with challengeIds: []
    // 先获取当前 banner 信息
    const meResp = await lcuRequest<any>("GET", "/lol-chat/v1/me");
    const banner = meResp.data?.lol?.bannerIdSelected || "";
    const resp = await lcuRequest<any>("POST", "/lol-challenges/v1/update-player-preferences/", {
      challengeIds: [],
      bannerAccent: banner
    });
    if (resp.success) {
      showToast('勋章已全部卸下');
    } else {
      showToast('勋章卸下失败: ' + resp.error, 'error');
    }
  } catch (e: any) {
    showToast('勋章卸下异常: ' + e.toString(), 'error');
  } finally {
    loading.value = false;
  }
}

// 卸载头像框
async function handleClearBorder() {
  if (!confirm("🖼️ 确定要清除你的召唤师头像框吗？")) return;
  loading.value = true;
  try {
    const resp = await lcuRequest<any>("PUT", "/lol-regalia/v2/current-regalia", {
      preferredBorderType: "NONE"
    });
    if (resp.success) {
      showToast('头像框已卸下');
    } else {
      showToast('头像框卸下失败: ' + resp.error, 'error');
    }
  } catch (e: any) {
    showToast('卸下头像框异常: ' + e.toString(), 'error');
  } finally {
    loading.value = false;
  }
}

// 切换锁定游戏设置
async function handleToggleLockGameSettings() {
  try {
    const nextState = !isGameSettingsLocked.value;
    const msg = await invoke<string>("set_game_settings_readonly", { readonly: nextState });
    isGameSettingsLocked.value = nextState;
    showToast(msg);
  } catch (e: any) {
    showToast(e.toString(), 'error');
  }
}
</script>

<template>
  <div class="tools-view">
    <!-- Toast 通知 -->
    <Transition name="toast">
      <div v-if="toast.visible" :class="['toast', `toast-${toast.type}`]">
        {{ toast.message }}
      </div>
    </Transition>

    <div v-if="!config" class="tip-container">
      <div class="loading-spinner"></div>
      <p class="tip">加载功能模块中...</p>
    </div>

    <div v-else class="tools-container">
      <h1 class="page-title">其他功能</h1>

      <!-- 1. 英雄选择组 -->
      <div class="group-header">英雄选择</div>

      <!-- 自动接受对局 -->
      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('autoaccept')">
          <div class="collapse-left">
            <div class="icon-container">
              <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path>
                <polyline points="22 4 12 14.01 9 11.01"></polyline>
              </svg>
            </div>
            <div class="title-container">
              <h3 class="card-title">自动接受对局</h3>
              <span class="card-desc">在你设置的秒数之后自动接受对局匹配</span>
            </div>
          </div>
          <div class="collapse-right">
            <span class="status-preview">
              {{ config.Functions.EnableAutoAcceptMatching 
                ? `已启用，延迟: ${config.Functions.AutoAcceptMatchingDelay} 秒` 
                : '未启用' 
              }}
            </span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'autoaccept' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'autoaccept'" class="collapse-content">
          <div class="setting-row">
            <span class="setting-label">在对局找到后接受对局前延迟的秒数:</span>
            <input type="number" v-model.number="config.Functions.AutoAcceptMatchingDelay" class="number-input" min="0" max="11" @change="triggerAutoSave" />
          </div>
          <div class="setting-row justify-end">
            <div :class="['toggle-switch', config.Functions.EnableAutoAcceptMatching ? 'on' : 'off']" @click="config.Functions.EnableAutoAcceptMatching = !config.Functions.EnableAutoAcceptMatching; triggerAutoSave()">
              <span class="toggle-text">{{ config.Functions.EnableAutoAcceptMatching ? '开' : '关' }}</span>
              <span class="toggle-slider"></span>
            </div>
          </div>
        </div>
      </div>

      <!-- 自动接受交换请求 -->
      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('autoswap')">
          <div class="collapse-left">
            <div class="icon-container">
              <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <line x1="8" y1="6" x2="21" y2="6"></line>
                <line x1="8" y1="12" x2="21" y2="12"></line>
                <line x1="8" y1="18" x2="21" y2="18"></line>
                <line x1="3" y1="6" x2="3.01" y2="6"></line>
                <line x1="3" y1="12" x2="3.01" y2="12"></line>
                <line x1="3" y1="18" x2="3.01" y2="18"></line>
              </svg>
            </div>
            <div class="title-container">
              <h3 class="card-title">自动接受交换请求</h3>
              <span class="card-desc">自动接受队友的交换楼层或英雄的请求</span>
            </div>
          </div>
          <div class="collapse-right">
            <span class="status-preview">
              {{ (config.Functions.AutoAcceptCeilSwap || config.Functions.AutoAcceptChampTrade) ? '已启用' : '未启用' }}
            </span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'autoswap' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'autoswap'" class="collapse-content">
          <div class="setting-row">
            <span class="setting-label">自动接受楼层交换请求:</span>
            <div :class="['toggle-switch', config.Functions.AutoAcceptCeilSwap ? 'on' : 'off']" @click="config.Functions.AutoAcceptCeilSwap = !config.Functions.AutoAcceptCeilSwap; triggerAutoSave()">
              <span class="toggle-text">{{ config.Functions.AutoAcceptCeilSwap ? '开' : '关' }}</span>
              <span class="toggle-slider"></span>
            </div>
          </div>
          <div class="setting-row">
            <span class="setting-label">自动接受英雄交换请求:</span>
            <div :class="['toggle-switch', config.Functions.AutoAcceptChampTrade ? 'on' : 'off']" @click="config.Functions.AutoAcceptChampTrade = !config.Functions.AutoAcceptChampTrade; triggerAutoSave()">
              <span class="toggle-text">{{ config.Functions.AutoAcceptChampTrade ? '开' : '关' }}</span>
              <span class="toggle-slider"></span>
            </div>
          </div>
        </div>
      </div>

      <!-- 自动亮起英雄 -->
      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('autohover')">
          <div class="collapse-left">
            <div class="icon-container">
              <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="9 11 12 14 22 4"></polyline>
                <path d="M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11"></path>
              </svg>
            </div>
            <div class="title-container">
              <h3 class="card-title">自动亮起英雄</h3>
              <span class="card-desc">在你进入英雄选择时自动亮起/预选英雄</span>
            </div>
          </div>
          <div class="collapse-right">
            <span class="status-preview">{{ config.Functions.EnableAutoHoverChampion ? '已启用' : '未启用' }}</span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'autohover' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'autohover'" class="collapse-content">
          <div class="setting-row">
            <span class="setting-label">启用自动亮起:</span>
            <div :class="['toggle-switch', config.Functions.EnableAutoHoverChampion ? 'on' : 'off']" @click="config.Functions.EnableAutoHoverChampion = !config.Functions.EnableAutoHoverChampion; triggerAutoSave()">
              <span class="toggle-text">{{ config.Functions.EnableAutoHoverChampion ? '开' : '关' }}</span>
              <span class="toggle-slider"></span>
            </div>
          </div>
          <div class="setting-row">
            <span class="setting-label">在结束时确认选择:</span>
            <div :class="['toggle-switch', config.Functions.AutoSelectConfirmOnTimeout ? 'on' : 'off']" @click="config.Functions.AutoSelectConfirmOnTimeout = !config.Functions.AutoSelectConfirmOnTimeout; triggerAutoSave()">
              <span class="toggle-text">{{ config.Functions.AutoSelectConfirmOnTimeout ? '开' : '关' }}</span>
              <span class="toggle-slider"></span>
            </div>
          </div>
          <div class="setting-picker-row">
            <ChampionPicker v-model="config.Functions.AutoSelectChampion" :maxCount="1" @update:modelValue="onPickerChange" />
          </div>
        </div>
      </div>

      <!-- 自动禁用英雄 -->
      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('autoban')">
          <div class="collapse-left">
            <div class="icon-container">
              <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
                <line x1="9" y1="9" x2="15" y2="15"></line>
                <line x1="15" y1="9" x2="9" y2="15"></line>
              </svg>
            </div>
            <div class="title-container">
              <h3 class="card-title">自动禁用英雄</h3>
              <span class="card-desc">在你的禁用环节开始时自动禁用英雄</span>
            </div>
          </div>
          <div class="collapse-right">
            <span class="status-preview">{{ config.Functions.EnableAutoBanChampion ? '已启用' : '未启用' }}</span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'autoban' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'autoban'" class="collapse-content">
          <div class="setting-row">
            <span class="setting-label">禁用环节自动禁用设定英雄:</span>
            <div :class="['toggle-switch', config.Functions.EnableAutoBanChampion ? 'on' : 'off']" @click="config.Functions.EnableAutoBanChampion = !config.Functions.EnableAutoBanChampion; triggerAutoSave()">
              <span class="toggle-text">{{ config.Functions.EnableAutoBanChampion ? '开' : '关' }}</span>
              <span class="toggle-slider"></span>
            </div>
          </div>
          <div class="setting-picker-row">
            <ChampionPicker v-model="config.Functions.AutoBanChampion" :maxCount="1" @update:modelValue="onPickerChange" />
          </div>
        </div>
      </div>

      <!-- 自动设置召唤师技能 -->
      <div class="collapse-item">
        <div class="collapse-header" @click="toggleCollapse('autospells')">
          <div class="collapse-left">
            <div class="icon-container">
              <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <polygon points="12 2 2 7 12 12 22 7 12 2"></polygon>
                <polyline points="2 17 12 22 22 17"></polyline>
                <polyline points="2 12 12 17 22 12"></polyline>
              </svg>
            </div>
            <div class="title-container">
              <h3 class="card-title">自动设置召唤师技能</h3>
              <span class="card-desc">当你的英雄选择开始时自动设置召唤师技能</span>
            </div>
          </div>
          <div class="collapse-right">
            <span class="status-preview">{{ config.Functions.EnableAutoSetSpells ? '已启用' : '未启用' }}</span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'autospells' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'autospells'" class="collapse-content">
          <div class="setting-row">
            <span class="setting-label">锁定英雄后自动写入配置好的技能组:</span>
            <div :class="['toggle-switch', config.Functions.EnableAutoSetSpells ? 'on' : 'off']" @click="config.Functions.EnableAutoSetSpells = !config.Functions.EnableAutoSetSpells; triggerAutoSave()">
              <span class="toggle-text">{{ config.Functions.EnableAutoSetSpells ? '开' : '关' }}</span>
              <span class="toggle-slider"></span>
            </div>
          </div>
          <div class="setting-picker-row">
            <SpellPicker v-model="config.Functions.AutoSetSummonerSpell" :maxCount="2" @update:modelValue="onPickerChange" />
          </div>
        </div>
      </div>

      <!-- 2. 游戏组 -->
      <div class="group-header">游戏</div>

      <!-- 自动重连 -->
      <div class="card-item border-bottom">
        <div class="card-left">
          <div class="icon-container">
            <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"></path>
              <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"></path>
            </svg>
          </div>
          <div class="title-container">
            <h3 class="card-title">自动重连</h3>
            <span class="card-desc">当你掉线退出游戏时自动重新连接</span>
          </div>
        </div>
        <div class="card-right">
          <div :class="['toggle-switch', config.Functions.EnableAutoReconnect ? 'on' : 'off']" @click="config.Functions.EnableAutoReconnect = !config.Functions.EnableAutoReconnect; triggerAutoSave()">
            <span class="toggle-text">{{ config.Functions.EnableAutoReconnect ? '开' : '关' }}</span>
            <span class="toggle-slider"></span>
          </div>
        </div>
      </div>

      <!-- 自动创建大厅 -->
      <div :class="['collapse-item', 'border-bottom', { 'has-dropdown-open': showGameModeDropdown }]">
        <div class="collapse-header" @click="toggleCollapse('createlobby')">
          <div class="collapse-left">
            <div class="icon-container">
              <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"></path>
                <polyline points="9 22 9 12 15 12 15 22"></polyline>
              </svg>
            </div>
            <div class="title-container">
              <h3 class="card-title">自动创建大厅</h3>
              <span class="card-desc">启动 LOL 客户端后自动创建默认模式的大厅</span>
            </div>
          </div>
          <div class="collapse-right">
            <span class="status-preview">
              {{ config?.Functions.EnableAutoCreateLobby
                ? `已启用: ${GAME_MODES.find(m => m.id === config?.Functions.DefaultGameMode)?.name || '未知模式'}`
                : '未启用'
              }}
            </span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'createlobby' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'createlobby'" class="collapse-content">
          <div class="setting-row">
            <span class="setting-label">客户端引导就绪后自动拉入指定大厅房间:</span>
            <div :class="['toggle-switch', config.Functions.EnableAutoCreateLobby ? 'on' : 'off']" @click="config.Functions.EnableAutoCreateLobby = !config.Functions.EnableAutoCreateLobby; triggerAutoSave()">
              <span class="toggle-text">{{ config.Functions.EnableAutoCreateLobby ? '开' : '关' }}</span>
              <span class="toggle-slider"></span>
            </div>
          </div>
          <div class="setting-row">
            <span class="setting-label">默认游戏模式:</span>
            <div class="dropdown-trigger" :class="{ disabled: !config.Functions.EnableAutoCreateLobby }" @click.stop="config.Functions.EnableAutoCreateLobby && (showGameModeDropdown = !showGameModeDropdown)">
              <span>{{ currentGameModeName }}</span>
              <svg :class="['arrow-icon', { expanded: showGameModeDropdown }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
              <div v-if="showGameModeDropdown" class="dropdown-menu" @click.stop>
                <div v-for="mode in GAME_MODES" :key="mode.id" :class="['dropdown-item', { active: config.Functions.DefaultGameMode === mode.id }]" @click="config.Functions.DefaultGameMode = mode.id; triggerAutoSave(); showGameModeDropdown = false">{{ mode.name }}</div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- 观战 -->
      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('spectate')">
          <div class="collapse-left">
            <div class="icon-container">
              <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"></path>
                <circle cx="12" cy="12" r="3"></circle>
              </svg>
            </div>
            <div class="title-container">
              <h3 class="card-title">观战</h3>
              <span class="card-desc">观战同大区玩家正在进行的实时游戏</span>
            </div>
          </div>
          <div class="collapse-right">
            <span class="status-preview">点击展开</span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'spectate' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'spectate'" class="collapse-content">
          <div class="setting-row">
            <span class="setting-label">观战召唤师名称:</span>
            <div class="input-with-button">
              <input v-model="spectateSummonerName" placeholder="输入要观战的召唤师名称..." class="text-input" />
              <button class="apply-btn" @click="handleSpectate" :disabled="!spectateSummonerName.trim()">观战</button>
            </div>
          </div>
        </div>
      </div>

      <!-- 锁定游戏设置 -->
      <div class="card-item">
        <div class="card-left">
          <div class="icon-container">
            <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <rect x="3" y="11" width="18" height="11" rx="2" ry="2"></rect>
              <path d="M7 11V7a5 5 0 0 1 10 0v4"></path>
            </svg>
          </div>
          <div class="title-container">
            <h3 class="card-title">锁定游戏设置</h3>
            <span class="card-desc">让你的游戏设置不会因为切换账号而改变</span>
          </div>
        </div>
        <div class="card-right">
          <div :class="['toggle-switch', isGameSettingsLocked ? 'on' : 'off']" @click="handleToggleLockGameSettings">
            <span class="toggle-text">{{ isGameSettingsLocked ? '开' : '关' }}</span>
            <span class="toggle-slider"></span>
          </div>
        </div>
      </div>

      <!-- 3. 客户端组 -->
      <div class="group-header">客户端</div>

      <div class="card-item border-bottom">
        <div class="card-left">
          <div class="icon-container">
            <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"></path>
            </svg>
          </div>
          <div class="title-container">
            <h3 class="card-title">修复客户端窗口</h3>
            <span class="card-desc">修复客户端错误的窗口大小（需要管理员权限）</span>
          </div>
        </div>
        <div class="card-right">
          <button class="action-btn" @click="handleFixWindow" :disabled="loading">修复</button>
        </div>
      </div>

      <div class="card-item">
        <div class="card-left">
          <div class="icon-container">
            <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67"></path>
            </svg>
          </div>
          <div class="title-container">
            <h3 class="card-title">重启客户端</h3>
            <span class="card-desc">重启客户端而不需要重新排队</span>
          </div>
        </div>
        <div class="card-right">
          <button class="action-btn" @click="handleRestartClient" :disabled="loading">重启</button>
        </div>
      </div>

      <!-- 4. 个人主页组 -->
      <div class="group-header">个人主页</div>

      <!-- 个人签名 -->
      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('signature')">
          <div class="collapse-left">
            <div class="icon-container">
              <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M12 20h9"></path>
                <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"></path>
              </svg>
            </div>
            <div class="title-container">
              <h3 class="card-title">个人签名</h3>
              <span class="card-desc">修改你个人卡片的签名</span>
            </div>
          </div>
          <div class="collapse-right">
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'signature' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'signature'" class="collapse-content">
          <div class="setting-row">
            <span class="setting-label">输入新的个性化签名:</span>
            <div class="input-with-button">
              <input v-model="statusInput" placeholder="输入新的个性化签名..." class="text-input" />
              <button class="apply-btn" @click="handleApplyStatus" :disabled="loading || !statusInput.trim()">应用</button>
            </div>
          </div>
        </div>
      </div>

      <!-- 个人主页背景 -->
      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('profilebg')">
          <div class="collapse-left">
            <div class="icon-container">
              <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
                <circle cx="8.5" cy="8.5" r="1.5"></circle>
                <polyline points="21 15 16 10 5 21"></polyline>
              </svg>
            </div>
            <div class="title-container">
              <h3 class="card-title">个人主页背景</h3>
              <span class="card-desc">修改你个人主页背景皮肤图片</span>
            </div>
          </div>
          <div class="collapse-right">
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'profilebg' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'profilebg'" class="collapse-content">
          <div class="setting-row no-border">
            <span class="setting-label">选择英雄并更换皮肤背景:</span>
          </div>
          <div class="setting-picker-row">
            <ChampionPicker v-model="bgChampion" :maxCount="1" />
          </div>
          
          <div v-if="skinLoading" class="skin-loading">
            <div class="loading-spinner"></div>
            <span>加载皮肤中...</span>
          </div>
          
          <!-- 已选择皮肤的预览信息，代替原来的平铺列表 -->
          <div v-else-if="skinList.length > 0" class="selected-skin-preview">
            <div class="preview-layout">
              <div class="preview-img-container">
                <LcuImage :src="currentSelectedSkin?.loadScreenPath" class="preview-img" />
              </div>
              <div class="preview-info-box">
                <span class="preview-title">已选背景皮肤</span>
                <span class="preview-skin-name">{{ currentSelectedSkin?.name }}</span>
                <button class="select-skin-btn" @click="openSkinModal">更换背景皮肤</button>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- 段位展示 -->
      <div :class="['collapse-item', 'border-bottom', { 'has-dropdown-open': showSpoofQueueDropdown || showSpoofTierDropdown || showSpoofDivisionDropdown }]">
        <div class="collapse-header" @click="toggleCollapse('rankdisplay')">
          <div class="collapse-left">
            <div class="icon-container">
              <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M6 9H4.5a2.5 2.5 0 0 1 0-5H6"></path>
                <path d="M18 9h1.5a2.5 2.5 0 0 0 0-5H18"></path>
                <path d="M4 22h16"></path>
                <path d="M10 14.66V17c0 .55-.45 1-1 1H4v2h16v-2h-5c-.55 0-1-.45-1-1v-2.34"></path>
                <path d="M12 2a7 7 0 0 0-7 7h14a7 7 0 0 0-7-7z"></path>
              </svg>
            </div>
            <div class="title-container">
              <h3 class="card-title">段位展示</h3>
              <span class="card-desc">修改你个人卡片显示的段位</span>
            </div>
          </div>
          <div class="collapse-right">
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'rankdisplay' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'rankdisplay'" class="collapse-content">
          <div class="setting-row">
            <span class="setting-label">选择排位队列模式:</span>
            <div class="dropdown-trigger" @click.stop="showSpoofQueueDropdown = !showSpoofQueueDropdown">
              <span>{{ SPOOF_QUEUE_LABELS[spoofQueue] || spoofQueue }}</span>
              <svg :class="['arrow-icon', { expanded: showSpoofQueueDropdown }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
              <div v-if="showSpoofQueueDropdown" class="dropdown-menu" @click.stop>
                <div :class="['dropdown-item', { active: spoofQueue === 'RANKED_TFT' }]" @click="spoofQueue = 'RANKED_TFT'; showSpoofQueueDropdown = false">云顶之弈</div>
                <div :class="['dropdown-item', { active: spoofQueue === 'RANKED_SOLO_5x5' }]" @click="spoofQueue = 'RANKED_SOLO_5x5'; showSpoofQueueDropdown = false">单双排位</div>
                <div :class="['dropdown-item', { active: spoofQueue === 'RANKED_FLEX_SR' }]" @click="spoofQueue = 'RANKED_FLEX_SR'; showSpoofQueueDropdown = false">灵活排位</div>
              </div>
            </div>
          </div>
          <div class="setting-row">
            <span class="setting-label">段位与级数等级:</span>
            <div class="rank-select-group">
              <div class="dropdown-trigger" @click.stop="showSpoofTierDropdown = !showSpoofTierDropdown">
                <span>{{ SPOOF_TIER_LABELS[spoofTier] || spoofTier }}</span>
                <svg :class="['arrow-icon', { expanded: showSpoofTierDropdown }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
                <div v-if="showSpoofTierDropdown" class="dropdown-menu" @click.stop>
                  <div v-for="t in ['UNRANKED','CHALLENGER','GRANDMASTER','MASTER','DIAMOND','EMERALD','PLATINUM','GOLD','SILVER','BRONZE','IRON']" :key="t" :class="['dropdown-item', { active: spoofTier === t }]" @click="spoofTier = t; showSpoofTierDropdown = false">{{ SPOOF_TIER_LABELS[t] }}</div>
                </div>
              </div>
              <div class="dropdown-trigger" :class="{ disabled: ['UNRANKED','MASTER','GRANDMASTER','CHALLENGER'].includes(spoofTier) }" @click.stop="!['UNRANKED','MASTER','GRANDMASTER','CHALLENGER'].includes(spoofTier) && (showSpoofDivisionDropdown = !showSpoofDivisionDropdown)">
                <span>{{ ['UNRANKED','MASTER','GRANDMASTER','CHALLENGER'].includes(spoofTier) ? '-' : spoofDivision }}</span>
                <svg :class="['arrow-icon', { expanded: showSpoofDivisionDropdown }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
                <div v-if="showSpoofDivisionDropdown" class="dropdown-menu" @click.stop>
                  <div v-for="d in ['I','II','III','IV']" :key="d" :class="['dropdown-item', { active: spoofDivision === d }]" @click="spoofDivision = d; showSpoofDivisionDropdown = false">{{ d }}</div>
                </div>
              </div>
              <button class="apply-btn" @click="handleApplyRankSpoof" :disabled="loading">应用</button>
            </div>
          </div>
        </div>
      </div>

      <!-- 在线状态 -->
      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('onlinestate')">
          <div class="collapse-left">
            <div class="icon-container">
              <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M21 11.5a8.38 8.38 0 0 1-.9 3.8 8.5 8.5 0 0 1-7.6 4.7 8.38 8.38 0 0 1-3.8-.9L3 21l1.9-5.7a8.38 8.38 0 0 1-.9-3.8 8.5 8.5 0 0 1 4.7-7.6 8.38 8.38 0 0 1 3.8-.9h.5a8.48 8.48 0 0 1 8 8v.5z"></path>
              </svg>
            </div>
            <div class="title-container">
              <h3 class="card-title">在线状态</h3>
              <span class="card-desc">修改你的在线状态</span>
            </div>
          </div>
          <div class="collapse-right">
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'onlinestate' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'onlinestate'" class="collapse-content">
          <div class="setting-row">
            <span class="setting-label">选择当前呈报的状态:</span>
            <div class="btn-group">
              <button class="status-btn online" @click="handleApplyAvailability('chat')" :disabled="loading">在线</button>
              <button class="status-btn away" @click="handleApplyAvailability('away')" :disabled="loading">离开</button>
              <button class="status-btn offline" @click="handleApplyAvailability('offline')" :disabled="loading">隐身</button>
            </div>
          </div>
        </div>
      </div>

      <!-- 卸下勋章 -->
      <div class="card-item border-bottom">
        <div class="card-left">
          <div class="icon-container">
            <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <circle cx="12" cy="8" r="7"></circle>
              <polyline points="8.21 13.89 7 23 12 20 17 23 15.79 13.88"></polyline>
            </svg>
          </div>
          <div class="title-container">
            <h3 class="card-title">卸下勋章</h3>
            <span class="card-desc">卸下你个人卡片中的所有勋章</span>
          </div>
        </div>
        <div class="card-right">
          <button class="action-btn text-danger" @click="handleClearBadges" :disabled="loading">卸下</button>
        </div>
      </div>

      <!-- 卸下头像框 -->
      <div class="card-item">
        <div class="card-left">
          <div class="icon-container">
            <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <circle cx="12" cy="12" r="10"></circle>
              <circle cx="12" cy="12" r="6"></circle>
            </svg>
          </div>
          <div class="title-container">
            <h3 class="card-title">卸下头像框</h3>
            <span class="card-desc">卸下你的召唤师头像框（需要召唤师等级大于等于 525）</span>
          </div>
        </div>
        <div class="card-right">
          <button class="action-btn text-danger" @click="handleClearBorder" :disabled="loading">卸下</button>
        </div>
      </div>

    </div>

    <!-- 皮肤选择轮播图弹窗 -->
    <Transition name="fade">
      <div v-if="showSkinModal" class="skin-modal-overlay" @click.self="showSkinModal = false">
        <div class="skin-modal-card">
          <!-- 弹窗头部 -->
          <div class="skin-modal-header">
            <h3>选择背景皮肤</h3>
            <button class="modal-close-btn" @click="showSkinModal = false">✕</button>
          </div>

          <!-- 轮播主图区 -->
          <div class="skin-carousel-container">
            <!-- 左箭头 -->
            <button class="carousel-nav-btn prev" @click="prevSkin" title="上一张 (←)">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="15 18 9 12 15 6"/></svg>
            </button>

            <!-- 皮肤加载图展示 -->
            <div class="skin-carousel-slide">
              <div class="slide-img-wrapper">
                <LcuImage :src="skinList[activeSkinIndex]?.loadScreenPath" class="carousel-img" />
              </div>
              <div class="carousel-skin-name">{{ skinList[activeSkinIndex]?.name }}</div>
            </div>

            <!-- 右箭头 -->
            <button class="carousel-nav-btn next" @click="nextSkin" title="下一张 (→)">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="9 18 15 12 9 6"/></svg>
            </button>
          </div>

          <!-- 底部小缩略图滑轨 -->
          <div class="thumbnail-slider-wrapper">
            <div class="thumbnail-slider">
              <div
                v-for="(skin, index) in skinList"
                :key="skin.id"
                :class="['thumbnail-dot', { active: activeSkinIndex === index }]"
                @click="selectSkin(index)"
                :title="skin.name"
              >
                <LcuImage :src="skin.loadScreenPath" class="thumbnail-img" />
              </div>
            </div>
          </div>

          <!-- 底部控制按钮 -->
          <div class="skin-modal-footer">
            <span class="carousel-counter">{{ activeSkinIndex + 1 }} / {{ skinList.length }}</span>
            <div class="footer-actions">
              <button class="cancel-action-btn" @click="showSkinModal = false">取消</button>
              <button class="confirm-action-btn" @click="confirmSkinSelection">确 定</button>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.tools-view {
  padding: 1.5rem 1.5rem 1.5rem 0.6rem;
  background-color: transparent;
  min-height: 100%;
}

.tip-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 8rem 2rem;
  color: var(--text-muted);
}

.tip {
  font-size: 0.95rem;
  color: var(--text-dimmed);
  margin-top: 12px;
}

.loading-spinner {
  width: 36px;
  height: 36px;
  border: 3px solid rgba(0, 0, 0, 0.05);
  border-top-color: var(--primary-color);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.tools-container {
  max-width: 800px;
  margin: 0 auto;
  animation: fadeIn 0.3s ease-out;
}

.page-title {
  font-size: 1.4rem;
  font-weight: 800;
  color: var(--text-color);
  margin: 0 0 1.5rem;
  letter-spacing: 0.5px;
}

.group-header {
  font-size: 0.8rem;
  font-weight: 700;
  color: var(--text-muted);
  margin: 1.8rem 0 0.6rem 6px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

/* 卡片 Item 通用样式 (Seraphine 风格) */
.card-item, .collapse-item {
  background: var(--card-bg);
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

.card-item {
  padding: 16px 24px;
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.collapse-item {
  display: flex;
  flex-direction: column;
  align-items: stretch;
}

.card-item:hover, .collapse-item:hover {
  box-shadow: var(--shadow-md);
  border-color: var(--primary-color-alpha-30);
  background-color: #ffffff;
}

.card-left, .collapse-left {
  display: flex;
  align-items: center;
  flex: 1;
  gap: 14px;
}

.icon-container {
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-muted);
  flex-shrink: 0;
}

.header-icon {
  width: 18px;
  height: 18px;
  stroke-width: 2px;
}

.title-container {
  display: flex;
  flex-direction: column;
}

.card-title {
  font-size: 0.88rem;
  font-weight: bold;
  color: var(--text-color);
  margin: 0;
}

.card-desc {
  font-size: 0.78rem;
  color: var(--text-muted);
  margin-top: 4px;
  line-height: 1.4;
}

.card-right, .collapse-right {
  margin-left: auto;
  display: flex;
  align-items: center;
}

.status-preview {
  font-size: 0.78rem;
  color: var(--text-dimmed);
  margin-right: 10px;
}

/* 按钮样式 */
.action-btn {
  background: rgba(255, 255, 255, 0.5);
  border: 1px solid var(--border-color);
  color: var(--text-color);
  padding: 6px 20px;
  border-radius: 6px;
  font-size: 0.82rem;
  font-weight: 600;
  cursor: pointer;
  outline: none;
  transition: all 0.2s cubic-bezier(0.25, 0.8, 0.25, 1);
  box-shadow: var(--shadow-sm);
}

.action-btn:hover {
  background: var(--primary-color-alpha-15);
  border-color: var(--primary-color);
  transform: translateY(-0.5px);
}

.action-btn:active {
  background: var(--primary-color-alpha-30);
  transform: translateY(0.5px);
}

.action-btn:disabled {
  opacity: 0.3;
  cursor: not-allowed;
  transform: none !important;
  box-shadow: none !important;
}

.text-danger {
  color: var(--loss-color) !important;
}

.text-danger:hover {
  background-color: var(--loss-bg);
  border-color: var(--loss-border);
}

/* 统一 Toggle 开关样式 (与设置页一致) */
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

/* 手风琴折叠样式 */
.collapse-header {
  padding: 14px 20px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  cursor: pointer;
  background-color: transparent;
  border-radius: 12px 12px 0 0;
}

.arrow-icon {
  width: 18px;
  height: 18px;
  transition: transform 0.2s;
  color: var(--text-dimmed);
}

.arrow-icon.expanded {
  transform: rotate(180deg);
}

.collapse-content {
  border-top: 1px dashed var(--border-color);
  padding: 14px 20px 16px 56px;
  animation: slideDown 0.2s ease-out;
  background-color: rgba(0, 0, 0, 0.01);
  border-bottom-left-radius: 12px;
  border-bottom-right-radius: 12px;
}

/* 新增设置行级样式 */
.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 0;
  border-bottom: 1px dashed var(--border-color);
}

.setting-row:last-child {
  border-bottom: none;
}

.setting-row.no-border {
  border-bottom: none;
  padding-bottom: 6px;
}

.setting-row.justify-end {
  justify-content: flex-end;
}

.setting-label {
  font-size: 0.82rem;
  color: var(--text-muted);
}

.setting-picker-row {
  padding-top: 6px;
  padding-bottom: 10px;
}

.input-with-button {
  display: flex;
  gap: 8px;
  width: 320px;
}

.input-with-button .text-input {
  flex: 1;
}

.rank-select-group {
  display: flex;
  gap: 8px;
  align-items: center;
}

.text-input, .number-input {
  padding: 8px 12px;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  font-size: 0.82rem;
  outline: none;
  background-color: rgba(255, 255, 255, 0.6);
  transition: all 0.2s ease;
  color: var(--text-color);
}

.text-input:hover, .number-input:hover {
  background-color: rgba(255, 255, 255, 0.95);
  border-color: var(--border-color-hover);
}

.text-input:focus, .number-input:focus {
  background-color: #fff;
  border-color: var(--primary-color);
  box-shadow: 0 0 8px var(--primary-color-alpha-15);
}

.text-input {
  flex: 1;
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
.dropdown-trigger.disabled {
  opacity: 0.6;
  cursor: not-allowed;
  background-color: rgba(240, 240, 240, 0.4);
}
.dropdown-trigger.disabled:hover {
  background: rgba(240, 240, 240, 0.4);
  border-color: var(--border-color);
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
  white-space: nowrap;
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
  width: 70px;
  padding: 6px 10px;
  border: 1px solid var(--border-color);
  border-radius: 6px;
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
  height: 24px;
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

.apply-btn {
  background: var(--primary-color);
  border: none;
  color: white;
  padding: 8px 20px;
  border-radius: 6px;
  font-size: 0.82rem;
  font-weight: bold;
  cursor: pointer;
  transition: all 0.2s ease;
  box-shadow: 0 4px 10px var(--primary-color-alpha-30);
}

.apply-btn:hover {
  background: var(--primary-color-hover);
  transform: translateY(-0.5px);
}

.apply-btn:active {
  color: rgba(255, 255, 255, 0.7);
  transform: translateY(0.5px);
}

.apply-btn:disabled {
  opacity: 0.3;
  cursor: not-allowed;
}

.btn-group {
  display: flex;
  gap: 8px;
}

.status-btn {
  border: 1px solid var(--border-color);
  background: rgba(255, 255, 255, 0.5);
  padding: 6px 20px;
  border-radius: 6px;
  font-size: 0.78rem;
  font-weight: bold;
  cursor: pointer;
  color: var(--text-color);
  transition: all 0.2s;
}

.status-btn.online {
  color: var(--win-color);
  border-color: var(--win-border);
  background-color: var(--win-bg);
}

.status-btn.online:hover {
  background-color: var(--win-color);
  color: white;
}

.status-btn.away {
  color: #e6a23c;
  border-color: rgba(230, 162, 60, 0.2);
  background-color: rgba(230, 162, 60, 0.08);
}

.status-btn.away:hover {
  background-color: #e6a23c;
  color: white;
}

.status-btn.offline {
  color: var(--text-muted);
  border-color: var(--border-color);
  background-color: rgba(0, 0, 0, 0.02);
}

.status-btn.offline:hover {
  background-color: var(--text-dimmed);
  color: white;
}

@keyframes fadeIn {
  from { opacity: 0; transform: translateY(6px); }
  to { opacity: 1; transform: translateY(0); }
}

@keyframes slideDown {
  from { opacity: 0; transform: translateY(-4px); }
  to { opacity: 1; transform: translateY(0); }
}

.margin-top {
  margin-top: 12px;
}

/* 皮肤加载 */
.skin-loading {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  color: var(--text-dimmed);
  font-size: 0.82rem;
  padding: 1.5rem 0;
}

/* 已选择皮肤的横向精致预览框 */
.selected-skin-preview {
  margin: 12px 0 16px;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: 12px;
  padding: 16px;
  box-shadow: var(--shadow-sm);
}

.preview-layout {
  display: flex;
  align-items: center;
  gap: 18px;
}

.preview-img-container {
  width: 130px;
  aspect-ratio: 16 / 9;
  border-radius: 8px;
  overflow: hidden;
  box-shadow: var(--shadow-sm);
  border: 1px solid var(--border-color);
  flex-shrink: 0;
}

.preview-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.preview-info-box {
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex: 1;
}

.preview-title {
  font-size: 0.72rem;
  color: var(--text-dimmed);
  font-weight: bold;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.preview-skin-name {
  font-size: 0.88rem;
  font-weight: 800;
  color: var(--text-color);
}

.select-skin-btn {
  align-self: flex-start;
  background: var(--primary-color-alpha-15);
  color: var(--primary-color);
  border: 1px solid var(--primary-color-alpha-30);
  padding: 6px 16px;
  border-radius: 8px;
  font-size: 0.78rem;
  font-weight: bold;
  cursor: pointer;
  transition: all 0.2s ease;
  margin-top: 4px;
}

.select-skin-btn:hover {
  background: var(--primary-color);
  color: white;
  border-color: var(--primary-color);
  transform: translateY(-1px);
  box-shadow: 0 4px 12px var(--primary-color-alpha-30);
}

/* 轮播图皮肤选择模态弹窗 */
.skin-modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  background-color: rgba(0, 0, 0, 0.45);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
}

.skin-modal-card {
  width: 480px;
  background: #fff;
  border: 1px solid var(--border-color);
  border-radius: 16px;
  box-shadow: var(--shadow-lg);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  animation: modalScaleIn 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
}

@keyframes modalScaleIn {
  from { opacity: 0; transform: scale(0.95) translateY(10px); }
  to { opacity: 1; transform: scale(1) translateY(0); }
}

.skin-modal-header {
  padding: 16px 20px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--border-color);
  background: rgba(0, 0, 0, 0.01);
}

.skin-modal-header h3 {
  font-size: 1rem;
  font-weight: 800;
  color: var(--text-color);
  margin: 0;
}

.modal-close-btn {
  background: none;
  border: none;
  font-size: 1.1rem;
  color: var(--text-muted);
  cursor: pointer;
  width: 28px;
  height: 28px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s;
}

.modal-close-btn:hover {
  background: rgba(0, 0, 0, 0.03);
  color: var(--text-color);
}

/* 轮播主体区 */
.skin-carousel-container {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 24px 16px;
  position: relative;
  background: radial-gradient(circle at center, rgba(0,0,0,0.01) 0%, rgba(0,0,0,0.05) 100%);
}

.carousel-nav-btn {
  width: 40px;
  height: 40px;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.85);
  border: 1px solid var(--border-color);
  color: var(--text-color);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: all 0.2s cubic-bezier(0.25, 0.8, 0.25, 1);
  box-shadow: var(--shadow-sm);
  z-index: 2;
}

.carousel-nav-btn svg {
  width: 20px;
  height: 20px;
}

.carousel-nav-btn:hover {
  background: var(--primary-color);
  border-color: var(--primary-color);
  color: white;
  transform: scale(1.1);
  box-shadow: 0 4px 12px var(--primary-color-alpha-30);
}

.skin-carousel-slide {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
  max-width: 320px;
}

.slide-img-wrapper {
  width: 200px;
  height: 330px;
  border-radius: 12px;
  overflow: hidden;
  box-shadow: var(--shadow-md);
  border: 2px solid white;
  transition: transform 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
}

.slide-img-wrapper:hover {
  transform: scale(1.02);
}

.carousel-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.carousel-skin-name {
  font-size: 0.88rem;
  font-weight: 800;
  color: var(--text-color);
  text-align: center;
  min-height: 24px;
}

/* 缩略图横排滑轨 */
.thumbnail-slider-wrapper {
  padding: 0 24px 16px;
  overflow-x: auto;
}

.thumbnail-slider-wrapper::-webkit-scrollbar {
  height: 4px;
}

.thumbnail-slider-wrapper::-webkit-scrollbar-thumb {
  background: rgba(0, 0, 0, 0.1);
  border-radius: 2px;
}

.thumbnail-slider {
  display: flex;
  gap: 6px;
  padding-bottom: 4px;
}

.thumbnail-dot {
  width: 44px;
  height: 26px;
  border-radius: 4px;
  overflow: hidden;
  cursor: pointer;
  opacity: 0.5;
  border: 1px solid var(--border-color);
  transition: all 0.2s ease;
  flex-shrink: 0;
}

.thumbnail-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.thumbnail-dot:hover {
  opacity: 0.85;
  transform: scale(1.05);
}

.thumbnail-dot.active {
  opacity: 1;
  border-color: var(--primary-color);
  box-shadow: 0 0 0 2px var(--primary-color-alpha-30);
  transform: scale(1.08);
}

/* 弹窗底部 */
.skin-modal-footer {
  padding: 14px 20px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-top: 1px solid var(--border-color);
  background: rgba(0, 0, 0, 0.01);
}

.carousel-counter {
  font-size: 0.82rem;
  color: var(--text-dimmed);
  font-weight: bold;
}

.footer-actions {
  display: flex;
  gap: 8px;
}

.cancel-action-btn {
  background: rgba(255, 255, 255, 0.5);
  border: 1px solid var(--border-color);
  color: var(--text-color);
  padding: 8px 16px;
  border-radius: 8px;
  font-size: 0.82rem;
  font-weight: bold;
  cursor: pointer;
  transition: all 0.2s;
}

.cancel-action-btn:hover {
  background: rgba(255, 255, 255, 0.95);
}

.confirm-action-btn {
  background: var(--primary-color);
  color: white;
  border: none;
  padding: 8px 20px;
  border-radius: 8px;
  font-size: 0.82rem;
  font-weight: bold;
  cursor: pointer;
  transition: all 0.2s;
  box-shadow: 0 4px 12px var(--primary-color-alpha-30);
}

.confirm-action-btn:hover {
  background: var(--primary-color-hover);
  box-shadow: 0 6px 16px var(--primary-color-alpha-40);
}

/* Toast 通知 */
.toast {
  position: fixed;
  top: 20px;
  left: 50%;
  transform: translateX(-50%);
  padding: 10px 24px;
  border-radius: 8px;
  font-size: 0.82rem;
  font-weight: 600;
  color: white;
  z-index: 9999;
  box-shadow: var(--shadow-md);
  pointer-events: none;
}

.toast-success {
  background-color: var(--primary-color);
}

.toast-error {
  background-color: var(--loss-color);
}

.toast-enter-active {
  transition: all 0.25s ease-out;
}

.toast-leave-active {
  transition: all 0.2s ease-in;
}

.toast-enter-from {
  opacity: 0;
  transform: translateX(-50%) translateY(-12px);
}

.toast-leave-to {
  opacity: 0;
  transform: translateX(-50%) translateY(-8px);
}
</style>
