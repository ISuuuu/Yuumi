<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, computed, inject, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { fetchConfig, updateConfig, lcuRequest, cleanError } from "../api/lcu";
import type { AppConfig } from "../api/lcu";
import { useLcuStore } from "../store/lcuStore";
import ChampionPicker from "../components/ChampionPicker.vue";
import SpellPicker from "../components/SpellPicker.vue";
import LcuImage from "../components/LcuImage.vue";
import { useDialog } from "naive-ui";
import { useToast } from "../composables/useToast";
import { useI18n } from 'vue-i18n';

const config = inject<Ref<AppConfig | null>>("appConfig") || ref<AppConfig | null>(null);
const store = useLcuStore();
const loading = ref(false);

const { showToast } = useToast();
const dialog = useDialog();
const { t } = useI18n();


// 自动选人/禁人/技能分路配置展示状态
const hoverActiveLane = ref<'default' | 'top' | 'jug' | 'mid' | 'bot' | 'sup'>('default');
const banActiveLane = ref<'default' | 'top' | 'jug' | 'mid' | 'bot' | 'sup'>('default');
const spellActiveLane = ref<'default' | 'top' | 'jug' | 'mid' | 'bot' | 'sup'>('default');
const LANE_OPTIONS = computed(() => [
  { value: 'default', label: t('tools.lane.default') },
  { value: 'top', label: t('tools.lane.top') },
  { value: 'jug', label: t('tools.lane.jug') },
  { value: 'mid', label: t('tools.lane.mid') },
  { value: 'bot', label: t('tools.lane.bot') },
  { value: 'sup', label: t('tools.lane.sup') }
] as const);

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

const SPOOF_QUEUE_LABELS = computed(() => ({
  RANKED_TFT: t('tools.spoofQueue.RANKED_TFT'),
  RANKED_SOLO_5x5: t('tools.spoofQueue.RANKED_SOLO_5x5'),
  RANKED_FLEX_SR: t('tools.spoofQueue.RANKED_FLEX_SR')
}));
const SPOOF_TIER_LABELS = computed<Record<string, string>>(() => ({
  UNRANKED: t('tools.spoofTier.UNRANKED'),
  CHALLENGER: t('tools.spoofTier.CHALLENGER'),
  GRANDMASTER: t('tools.spoofTier.GRANDMASTER'),
  MASTER: t('tools.spoofTier.MASTER'),
  DIAMOND: t('tools.spoofTier.DIAMOND'),
  EMERALD: t('tools.spoofTier.EMERALD'),
  PLATINUM: t('tools.spoofTier.PLATINUM'),
  GOLD: t('tools.spoofTier.GOLD'),
  SILVER: t('tools.spoofTier.SILVER'),
  BRONZE: t('tools.spoofTier.BRONZE'),
  IRON: t('tools.spoofTier.IRON'),
}));
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
      showToast(t('tools.background.noSkinData'), 'error');
    }
  } catch (e) {
    console.error("加载皮肤列表失败:", e);
    showToast(t('tools.background.skinLoadFailed'), 'error');
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
    showToast(t('tools.background.pickHeroFirst'), 'error');
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
const spectateMethod = ref<"LCU" | "CMD">("LCU");

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
  if (!config.value) {
    try {
      config.value = await fetchConfig();
    } catch (e) {
      console.error("加载其他功能配置失败:", e);
    }
  }
  await checkGameSettingsLock();
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
  // 同步自动选用开关和自动亮起开关，避免后台逻辑不触发
  config.value.Functions.EnableAutoSelectChampion = config.value.Functions.EnableAutoHoverChampion;
  try {
    await updateConfig(config.value);
  } catch (e) {
    console.error("自动保存设置失败:", e);
  }
}

// 观战启动
async function handleSpectate() {
  if (!spectateSummonerName.value.trim()) return;
  loading.value = true;
  try {
    const name = spectateSummonerName.value.trim().replace(/[⁦⁩]/g, '');

    if (spectateMethod.value === "CMD") {
      // CMD 方式：通过 SGP 获取凭据后直接启动 League of Legends.exe
      await invoke<string>("spectate_directly", {
        params: { summoner_name: name },
      });
      showToast(t('tools.spectate.startedCmd'));
    } else {
      // LCU API 方式：通过 LCU 接口进行好友/对局观战
      const summonerResp = await lcuRequest<any>("GET", `/lol-summoner/v1/summoners?name=${encodeURIComponent(name)}`);
      if (!summonerResp.success || !summonerResp.data) {
        showToast(t('tools.spectate.notFound'), 'error');
        return;
      }
      const puuid = summonerResp.data.puuid;

      // 1. 安全地从全量好友列表中匹配目标 puuid 提取对局 ID（使用可选链，并对无效的 0/null 值进行过滤）
      let gameIdFromFriend = "";
      const friendsResp = await lcuRequest<any[]>("GET", "/lol-chat/v1/friends");
      if (friendsResp.success && Array.isArray(friendsResp.data)) {
        const friend = friendsResp.data.find((f: any) => f.puuid === puuid || f.id === puuid);
        const rawGameId = friend?.lol?.gameId;
        if (rawGameId && String(rawGameId) !== "0" && String(rawGameId) !== "null") {
          gameIdFromFriend = String(rawGameId);
          console.log("从好友列表匹配到当前对局 ID:", gameIdFromFriend);
        }
      }

      // 2. 从 Lobby 大厅中匹配当前自定义房间的对局 ID（排除无效的 0/null 值）
      let gameIdFromLobby = "";
      const lobbyResp = await lcuRequest<any>("GET", "/lol-lobby/v2/lobby");
      if (lobbyResp.success && lobbyResp.data) {
        const config = lobbyResp.data.gameConfig;
        if (config && config.id && String(config.id) !== "0" && String(config.id) !== "null") {
          gameIdFromLobby = String(config.id);
          console.log("从 Lobby 大厅匹配到自定义对局 ID:", gameIdFromLobby);
        }
      }

      // 3. 自定义对局必须传对局 ID 才能加载观战，否则退化为名字匹配
      const targetGameId = gameIdFromFriend || gameIdFromLobby || name;

      // 4. 发送 LCU 观战启动请求
      const resp = await lcuRequest<any>("POST", "/lol-spectator/v1/spectate/launch", {
        allowObserveMode: "ALL",
        dropInSpectateGameId: targetGameId,
        gameQueueType: "",
        puuid: puuid
      });
      if (resp.success) {
        showToast(t('tools.spectate.success'), 'success');
      } else {
        // 5. 观战降级兜底：LCU 方式发生任何失败（如 missing key/gameflow），立刻尝试通过 CMD 方式静默唤醒启动以提高可靠性
        console.warn("LCU 观战失败，尝试通过 CMD 方式兜底拉起...", {
          targetGameId,
          puuid,
          error: resp.error
        });
        
        try {
          await invoke<string>("spectate_directly", {
            params: { summoner_name: name },
          });
          showToast(t('tools.spectate.fallbackCmd'), "success");
        } catch (cmdErr: any) {
          // 如果 CMD 方式也最终失败，向控制台记录详细信息，并报出最原始直观的友好 Toast 引导
          console.error("CMD 兜底观战亦告失败:", cmdErr);
          showToast(t('tools.spectate.failed', { error: cleanError(resp.error || '该召唤师当前可能无法被观战') + ' (自定义/新开局请先在官方客户端右键尝试观战以同步密钥)' }), 'error');
        }
      }
    }
  } catch (e: any) {
    showToast(t('tools.spectate.error', { error: cleanError(e) }), 'error');
  } finally {
    loading.value = false;
  }
}

// 修复客户端窗口
async function handleFixWindow() {
  loading.value = true;
  try {
    await invoke("fix_lcu_window");
    showToast(t('tools.fixWindow.success'));
  } catch (e: any) {
    showToast(t('tools.fixWindow.failed', { error: e.toString() }), 'error');
  } finally {
    loading.value = false;
  }
}

// 重启客户端
function handleRestartClient() {
  dialog.warning({
    title: t('tools.restartClient.dialogTitle'),
    content: t('tools.restartClient.dialogContent'),
    positiveText: t('tools.confirm'),
    negativeText: t('tools.cancel'),
    positiveButtonProps: { type: 'primary' },
    onPositiveClick: async () => {
      loading.value = true;
      try {
        const resp = await lcuRequest<any>("POST", "/riotclient/kill-and-restart-ux");
        if (resp.success) {
          showToast(t('tools.restartClient.success'));
        } else {
          showToast(t('tools.restartClient.failed', { error: resp.error }), 'error');
        }
      } catch (e: any) {
        showToast(t('tools.restartClient.error', { error: e.toString() }), 'error');
      } finally {
        loading.value = false;
      }
    }
  });
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
      showToast(t('tools.signature.success'));
      statusInput.value = "";
    } else {
      showToast(t('tools.signature.failed', { error: resp.error }), 'error');
    }
  } catch (e: any) {
    showToast(t('tools.signature.error', { error: e.toString() }), 'error');
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
      showToast(t('tools.background.success'));
    } else {
      showToast(t('tools.background.failed', { error: resp.error }), 'error');
    }
  } catch (e: any) {
    showToast(t('tools.background.error', { error: e.toString() }), 'error');
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
      showToast(t('tools.rankSpoof.success'));
    } else {
      showToast(t('tools.rankSpoof.failed', { error: resp.error }), 'error');
    }
  } catch (e: any) {
    showToast(t('tools.rankSpoof.error', { error: e.toString() }), 'error');
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
      const availText = avail === 'chat' ? t('tools.status.online') : avail === 'away' ? t('tools.status.away') : t('tools.status.invisible');
      showToast(t('tools.status.success', { status: availText }));
    } else {
      showToast(t('tools.status.failed', { error: resp.error }), 'error');
    }
  } catch (e: any) {
    showToast(t('tools.status.error', { error: e.toString() }), 'error');
  } finally {
    loading.value = false;
  }
}

// 卸载全部勋章
function handleClearBadges() {
  dialog.warning({
    title: t('tools.badges.title'),
    content: "🏅 " + t('tools.badges.confirmText'),
    positiveText: t('tools.confirm'),
    negativeText: t('tools.cancel'),
    positiveButtonProps: { type: 'primary' },
    onPositiveClick: async () => {
      loading.value = true;
      try {
        const meResp = await lcuRequest<any>("GET", "/lol-chat/v1/me");
        const banner = meResp.data?.lol?.bannerIdSelected || "";
        const resp = await lcuRequest<any>("POST", "/lol-challenges/v1/update-player-preferences/", {
          challengeIds: [],
          bannerAccent: banner
        });
        if (resp.success) {
          showToast(t('tools.badges.success'));
        } else {
          showToast(t('tools.badges.failed', { error: resp.error }), 'error');
        }
      } catch (e: any) {
        showToast(t('tools.badges.error', { error: e.toString() }), 'error');
      } finally {
        loading.value = false;
      }
    }
  });
}

// 卸载头像框
function handleClearBorder() {
  dialog.warning({
    title: t('tools.border.title'),
    content: "🖼️ " + t('tools.border.confirmText'),
    positiveText: t('tools.confirm'),
    negativeText: t('tools.cancel'),
    positiveButtonProps: { type: 'primary' },
    onPositiveClick: async () => {
      loading.value = true;
      try {
        const resp = await lcuRequest<any>("PUT", "/lol-regalia/v2/current-regalia", {
          preferredBorderType: "NONE"
        });
        if (resp.success) {
          showToast(t('tools.border.success'));
        } else {
          showToast(t('tools.border.failed', { error: resp.error }), 'error');
        }
      } catch (e: any) {
        showToast(t('tools.border.error', { error: e.toString() }), 'error');
      } finally {
        loading.value = false;
      }
    }
  });
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

    <div v-if="!config" class="tip-container">
      <div class="loading-spinner"></div>
      <p class="tip">{{ $t('tools.loading') }}</p>
    </div>

    <div v-else class="tools-container">
      <h1 class="page-title">{{ $t('tools.title') }}</h1>

      <!-- 未连接 LCU 覆盖层 -->
      <div v-if="!store.isConnected" class="offline-overlay"></div>

      <!-- 1. 英雄选择组 -->
      <div class="group-header">{{ $t('tools.groupChampSelect') }}</div>

      <!-- 自动接受对局 -->
      <n-collapse arrow-placement="right" class="collapse-card">
        <n-collapse-item name="autoaccept">
          <template #header>
            <div class="collapse-header-wrapper">
              <div class="collapse-left">
                <div class="icon-container">
                  <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path>
                    <polyline points="22 4 12 14.01 9 11.01"></polyline>
                  </svg>
                </div>
                <div class="title-container">
                  <h3 class="card-title">{{ $t('tools.autoAccept.title') }}</h3>
                  <span class="card-desc">{{ $t('tools.autoAccept.desc') }}</span>
                </div>
              </div>
              <div class="collapse-right-status">
                <span class="status-preview">
                  {{ config.Functions.EnableAutoAcceptMatching 
                    ? $t('tools.autoAccept.statusEnabled', { delay: config.Functions.AutoAcceptMatchingDelay }) 
                    : $t('tools.autoAccept.statusDisabled') 
                  }}
                </span>
              </div>
            </div>
          </template>
          <div class="setting-row">
            <span class="setting-label">{{ $t('tools.autoAccept.delayLabel') }}</span>
            <n-input-number v-model:value="config.Functions.AutoAcceptMatchingDelay" :min="0" :max="11" @update:value="triggerAutoSave" style="width:120px" size="small" />
          </div>
          <div class="setting-row justify-end">
            <n-switch v-model:value="config.Functions.EnableAutoAcceptMatching" @update:value="triggerAutoSave" />
          </div>
        </n-collapse-item>
      </n-collapse>

      <!-- 自动接受交换请求 -->
      <!-- 自动接受交换请求 -->
      <n-collapse arrow-placement="right" class="collapse-card">
        <n-collapse-item name="autoswap">
          <template #header>
            <div class="collapse-header-wrapper">
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
                  <h3 class="card-title">{{ $t('tools.autoSwap.title') }}</h3>
                  <span class="card-desc">{{ $t('tools.autoSwap.desc') }}</span>
                </div>
              </div>
              <div class="collapse-right-status">
                <span class="status-preview">
                  {{ (config.Functions.AutoAcceptCeilSwap || config.Functions.AutoAcceptChampTrade) ? $t('tools.autoSwap.statusEnabled') : $t('tools.autoSwap.statusDisabled') }}
                </span>
              </div>
            </div>
          </template>
          <div class="setting-row">
            <span class="setting-label">{{ $t('tools.autoSwap.floorLabel') }}</span>
            <n-switch v-model:value="config.Functions.AutoAcceptCeilSwap" @update:value="triggerAutoSave" />
          </div>
          <div class="setting-row">
            <span class="setting-label">{{ $t('tools.autoSwap.champLabel') }}</span>
            <n-switch v-model:value="config.Functions.AutoAcceptChampTrade" @update:value="triggerAutoSave" />
          </div>
        </n-collapse-item>
      </n-collapse>

      <!-- 自动亮起/选用英雄 -->
      <n-collapse arrow-placement="right" class="collapse-card">
        <n-collapse-item name="autohover">
          <template #header>
            <div class="collapse-header-wrapper">
              <div class="collapse-left">
                <div class="icon-container">
                  <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <polyline points="9 11 12 14 22 4"></polyline>
                    <path d="M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11"></path>
                  </svg>
                </div>
                <div class="title-container">
                  <h3 class="card-title">{{ $t('tools.autoHover.title') }}</h3>
                  <span class="card-desc">{{ $t('tools.autoHover.desc') }}</span>
                </div>
              </div>
              <div class="collapse-right-status">
                <span class="status-preview">{{ config.Functions.EnableAutoHoverChampion ? $t('tools.autoHover.statusEnabled') : $t('tools.autoHover.statusDisabled') }}</span>
              </div>
            </div>
          </template>
          <div class="setting-row">
            <span class="setting-label">{{ $t('tools.autoHover.enableHover') }}</span>
            <n-switch v-model:value="config.Functions.EnableAutoHoverChampion" @update:value="triggerAutoSave" />
          </div>
          <div class="setting-row">
            <span class="setting-label">{{ $t('tools.autoHover.confirmTimeout') }}</span>
            <n-switch v-model:value="config.Functions.AutoSelectConfirmOnTimeout" @update:value="triggerAutoSave" />
          </div>
          
          <!-- 分路选择选项卡 -->
          <div class="lane-tab-group">
            <button 
              v-for="lane in LANE_OPTIONS" 
              :key="lane.value" 
              :class="['lane-tab-btn', { active: hoverActiveLane === lane.value }]" 
              @click="hoverActiveLane = lane.value"
            >
              {{ lane.label }}
            </button>
          </div>

          <div class="setting-picker-row">
            <ChampionPicker v-if="hoverActiveLane === 'default'" v-model="config.Functions.AutoSelectChampion" :maxCount="1" @update:modelValue="onPickerChange" />
            <ChampionPicker v-else-if="hoverActiveLane === 'top'" v-model="config.Functions.AutoSelectChampionTop" :maxCount="1" @update:modelValue="onPickerChange" />
            <ChampionPicker v-else-if="hoverActiveLane === 'jug'" v-model="config.Functions.AutoSelectChampionJug" :maxCount="1" @update:modelValue="onPickerChange" />
            <ChampionPicker v-else-if="hoverActiveLane === 'mid'" v-model="config.Functions.AutoSelectChampionMid" :maxCount="1" @update:modelValue="onPickerChange" />
            <ChampionPicker v-else-if="hoverActiveLane === 'bot'" v-model="config.Functions.AutoSelectChampionBot" :maxCount="1" @update:modelValue="onPickerChange" />
            <ChampionPicker v-else-if="hoverActiveLane === 'sup'" v-model="config.Functions.AutoSelectChampionSup" :maxCount="1" @update:modelValue="onPickerChange" />
          </div>
        </n-collapse-item>
      </n-collapse>

      <!-- 自动禁用英雄 -->
      <n-collapse arrow-placement="right" class="collapse-card">
        <n-collapse-item name="autoban">
          <template #header>
            <div class="collapse-header-wrapper">
              <div class="collapse-left">
                <div class="icon-container">
                  <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
                    <line x1="9" y1="9" x2="15" y2="15"></line>
                    <line x1="15" y1="9" x2="9" y2="15"></line>
                  </svg>
                </div>
                <div class="title-container">
                  <h3 class="card-title">{{ $t('tools.autoBan.title') }}</h3>
                  <span class="card-desc">{{ $t('tools.autoBan.desc') }}</span>
                </div>
              </div>
              <div class="collapse-right-status">
                <span class="status-preview">{{ config.Functions.EnableAutoBanChampion ? $t('tools.autoBan.statusEnabled') : $t('tools.autoBan.statusDisabled') }}</span>
              </div>
            </div>
          </template>
          <div class="setting-row">
            <span class="setting-label">{{ $t('tools.autoBan.enableBan') }}</span>
            <n-switch v-model:value="config.Functions.EnableAutoBanChampion" @update:value="triggerAutoSave" />
          </div>
          <div class="setting-row">
            <span class="setting-label">{{ $t('tools.autoBan.pretendBan') }}</span>
            <n-switch v-model:value="config.Functions.PretendBan" @update:value="triggerAutoSave" />
          </div>
          <div class="setting-row">
            <span class="setting-label">{{ $t('tools.autoBan.banDelay') }}</span>
            <n-input-number v-model:value="config.Functions.AutoBanDelay" :min="0" :max="15" :step="0.5" @update:value="triggerAutoSave" style="width:120px" size="small" />
          </div>

          <!-- 分路选择选项卡 -->
          <div class="lane-tab-group">
            <button 
              v-for="lane in LANE_OPTIONS" 
              :key="lane.value" 
              :class="['lane-tab-btn', { active: banActiveLane === lane.value }]" 
              @click="banActiveLane = lane.value"
            >
              {{ lane.label }}
            </button>
          </div>

          <div class="setting-picker-row">
            <ChampionPicker v-if="banActiveLane === 'default'" v-model="config.Functions.AutoBanChampion" :maxCount="1" @update:modelValue="onPickerChange" />
            <ChampionPicker v-else-if="banActiveLane === 'top'" v-model="config.Functions.AutoBanChampionTop" :maxCount="1" @update:modelValue="onPickerChange" />
            <ChampionPicker v-else-if="banActiveLane === 'jug'" v-model="config.Functions.AutoBanChampionJug" :maxCount="1" @update:modelValue="onPickerChange" />
            <ChampionPicker v-else-if="banActiveLane === 'mid'" v-model="config.Functions.AutoBanChampionMid" :maxCount="1" @update:modelValue="onPickerChange" />
            <ChampionPicker v-else-if="banActiveLane === 'bot'" v-model="config.Functions.AutoBanChampionBot" :maxCount="1" @update:modelValue="onPickerChange" />
            <ChampionPicker v-else-if="banActiveLane === 'sup'" v-model="config.Functions.AutoBanChampionSup" :maxCount="1" @update:modelValue="onPickerChange" />
          </div>
        </n-collapse-item>
      </n-collapse>

      <!-- 自动设置召唤师技能 -->
      <n-collapse arrow-placement="right" class="collapse-card">
        <n-collapse-item name="autospells">
          <template #header>
            <div class="collapse-header-wrapper">
              <div class="collapse-left">
                <div class="icon-container">
                  <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <polygon points="12 2 2 7 12 12 22 7 12 2"></polygon>
                    <polyline points="2 17 12 22 22 17"></polyline>
                    <polyline points="2 12 12 17 22 12"></polyline>
                  </svg>
                </div>
                <div class="title-container">
                  <h3 class="card-title">{{ $t('tools.autoSpells.title') }}</h3>
                  <span class="card-desc">{{ $t('tools.autoSpells.desc') }}</span>
                </div>
              </div>
              <div class="collapse-right-status">
                <span class="status-preview">{{ config.Functions.EnableAutoSetSpells ? $t('tools.autoSpells.statusEnabled') : $t('tools.autoSpells.statusDisabled') }}</span>
              </div>
            </div>
          </template>
          <div class="setting-row">
            <span class="setting-label">{{ $t('tools.autoSpells.label') }}</span>
            <n-switch v-model:value="config.Functions.EnableAutoSetSpells" @update:value="triggerAutoSave" />
          </div>

          <!-- 分路选择选项卡 -->
          <div class="lane-tab-group">
            <button 
              v-for="lane in LANE_OPTIONS" 
              :key="lane.value" 
              :class="['lane-tab-btn', { active: spellActiveLane === lane.value }]" 
              @click="spellActiveLane = lane.value"
            >
              {{ lane.label }}
            </button>
          </div>

          <div class="setting-picker-row">
            <SpellPicker v-if="spellActiveLane === 'default'" v-model="config.Functions.AutoSetSummonerSpell" @update:modelValue="onPickerChange" />
            <SpellPicker v-else-if="spellActiveLane === 'top'" v-model="config.Functions.AutoSetSummonerSpellTop" @update:modelValue="onPickerChange" />
            <SpellPicker v-else-if="spellActiveLane === 'jug'" v-model="config.Functions.AutoSetSummonerSpellJug" @update:modelValue="onPickerChange" />
            <SpellPicker v-else-if="spellActiveLane === 'mid'" v-model="config.Functions.AutoSetSummonerSpellMid" @update:modelValue="onPickerChange" />
            <SpellPicker v-else-if="spellActiveLane === 'bot'" v-model="config.Functions.AutoSetSummonerSpellBot" @update:modelValue="onPickerChange" />
            <SpellPicker v-else-if="spellActiveLane === 'sup'" v-model="config.Functions.AutoSetSummonerSpellSup" @update:modelValue="onPickerChange" />
          </div>
        </n-collapse-item>
      </n-collapse>

      <!-- 大乱斗板凳席悬浮窗 -->
      <div class="card-item border-bottom">
        <div class="card-left">
          <div class="icon-container">
            <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <rect x="2" y="3" width="20" height="14" rx="2" ry="2"></rect>
              <line x1="8" y1="21" x2="16" y2="21"></line>
              <line x1="12" y1="17" x2="12" y2="21"></line>
            </svg>
          </div>
          <div class="title-container">
            <h3 class="card-title">{{ $t('tools.benchOverlay.title') }}</h3>
            <span class="card-desc">{{ $t('tools.benchOverlay.desc') }}</span>
          </div>
        </div>
        <div class="card-right">
          <n-switch v-model:value="config.Functions.EnableBenchOverlay" @update:value="triggerAutoSave" />
        </div>
      </div>

      <!-- 2. 游戏组 -->
      <div class="group-header">{{ $t('tools.groupGame') }}</div>

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
            <h3 class="card-title">{{ $t('tools.autoReconnect.title') }}</h3>
            <span class="card-desc">{{ $t('tools.autoReconnect.desc') }}</span>
          </div>
        </div>
        <div class="card-right">
          <n-switch v-model:value="config.Functions.EnableAutoReconnect" @update:value="triggerAutoSave" />
        </div>
      </div>

      <!-- 自动创建大厅 -->
      <!-- 自动创建大厅 -->
      <n-collapse arrow-placement="right" class="collapse-card">
        <n-collapse-item name="createlobby">
          <template #header>
            <div class="collapse-header-wrapper">
              <div class="collapse-left">
                <div class="icon-container">
                  <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"></path>
                    <polyline points="9 22 9 12 15 12 15 22"></polyline>
                  </svg>
                </div>
                <div class="title-container">
                  <h3 class="card-title">{{ $t('tools.autoCreateLobby.title') }}</h3>
                  <span class="card-desc">{{ $t('tools.autoCreateLobby.desc') }}</span>
                </div>
              </div>
              <div class="collapse-right-status">
                <span class="status-preview">
                  {{ config?.Functions.EnableAutoCreateLobby
                    ? $t('tools.autoCreateLobby.enabled', { mode: config?.Functions.DefaultGameMode ? $t('gameModes.' + config.Functions.DefaultGameMode) : $t('tools.autoCreateLobby.unknownMode') })
                    : $t('tools.autoCreateLobby.disabled')
                  }}
                </span>
              </div>
            </div>
          </template>
          <div class="setting-row">
            <span class="setting-label">{{ $t('tools.autoCreateLobby.label') }}</span>
            <n-switch v-model:value="config.Functions.EnableAutoCreateLobby" @update:value="triggerAutoSave" />
          </div>
          <div class="setting-row">
            <span class="setting-label">{{ $t('tools.autoCreateLobby.defaultMode') }}</span>
            <n-select
              v-model:value="config.Functions.DefaultGameMode"
              :options="GAME_MODES.map(m => ({ label: $t('gameModes.' + m.id), value: m.id }))"
              :disabled="!config.Functions.EnableAutoCreateLobby"
              @update:value="triggerAutoSave"
              style="width: 140px;"
              size="small"
            />
          </div>
        </n-collapse-item>
      </n-collapse>

      <!-- 观战 -->
      <n-collapse arrow-placement="right" class="collapse-card">
        <n-collapse-item name="spectate">
          <template #header>
            <div class="collapse-header-wrapper">
              <div class="collapse-left">
                <div class="icon-container">
                  <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"></path>
                    <circle cx="12" cy="12" r="3"></circle>
                  </svg>
                </div>
                <div class="title-container">
                  <h3 class="card-title">{{ $t('tools.spectate.title') }}</h3>
                  <span class="card-desc">{{ $t('tools.spectate.desc') }}</span>
                </div>
              </div>
              <div class="collapse-right-status">
                <span class="status-preview">{{ $t('tools.spectate.expand') }}</span>
              </div>
            </div>
          </template>
          <div class="setting-row">
            <span class="setting-label">{{ $t('tools.spectate.nameLabel') }}</span>
            <n-input
              v-model:value="spectateSummonerName"
              :placeholder="$t('tools.spectate.namePlaceholder')"
              clearable
              style="max-width: 300px;"
            >
              <template #suffix>
                <n-button
                  size="small"
                  type="primary"
                  :disabled="!spectateSummonerName.trim()"
                  @click="handleSpectate"
                >
                  {{ $t('tools.spectate.btn') }}
                </n-button>
              </template>
            </n-input>
          </div>
          <div class="setting-row">
            <span class="setting-label">{{ $t('tools.spectate.methodLabel') }}</span>
            <n-select
              v-model:value="spectateMethod"
              :options="[
                { label: 'LCU API', value: 'LCU' },
                { label: 'CMD', value: 'CMD' }
              ]"
              style="width: 120px;"
              size="small"
            />
          </div>
        </n-collapse-item>
      </n-collapse>

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
            <h3 class="card-title">{{ $t('tools.lockGameSettings.title') }}</h3>
            <span class="card-desc">{{ $t('tools.lockGameSettings.desc') }}</span>
          </div>
        </div>
        <div class="card-right">
          <n-switch :value="isGameSettingsLocked" @update:value="handleToggleLockGameSettings" />
        </div>
      </div>

      <!-- 3. 客户端组 -->
      <div class="group-header">{{ $t('tools_extra.clientGroupTitle') }}</div>

      <div class="card-item border-bottom">
        <div class="card-left">
          <div class="icon-container">
            <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"></path>
            </svg>
          </div>
          <div class="title-container">
            <h3 class="card-title">{{ $t('tools_extra.fixWindow') }}</h3>
            <span class="card-desc">{{ $t('tools_extra.fixWindowDesc') }}</span>
          </div>
        </div>
        <div class="card-right">
          <button class="action-btn" @click="handleFixWindow" :disabled="loading">{{ $t('tools_extra.fixBtn') }}</button>
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
            <h3 class="card-title">{{ $t('tools_extra.restartClient') }}</h3>
            <span class="card-desc">{{ $t('tools_extra.restartClientDesc') }}</span>
          </div>
        </div>
        <div class="card-right">
          <n-button class="action-btn" @click="handleRestartClient" :loading="loading">{{ $t('tools_extra.restartBtn') }}</n-button>
        </div>
      </div>

      <!-- 4. 个人主页组 -->
      <div class="group-header">个人主页</div>

      <!-- 个人签名 -->
      <n-collapse arrow-placement="right" class="collapse-card">
        <n-collapse-item name="signature">
          <template #header>
            <div class="collapse-header-wrapper">
              <div class="collapse-left">
                <div class="icon-container">
                  <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M12 20h9"></path>
                    <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"></path>
                  </svg>
                </div>
                <div class="title-container">
                  <h3 class="card-title">{{ $t('tools.signature.title') }}</h3>
                  <span class="card-desc">{{ $t('tools.signature.desc') }}</span>
                </div>
              </div>
              <div class="collapse-right-status">
                <span class="status-preview">{{ $t('tools.spectate.expand') }}</span>
              </div>
            </div>
          </template>
          <div class="setting-row">
            <span class="setting-label">{{ $t('tools.signature.delayLabel') || '输入新的个性化签名:' }}</span>
            <n-input
              v-model:value="statusInput"
              :placeholder="$t('tools.signature.placeholder')"
              clearable
              style="max-width: 300px;"
              size="small"
            >
              <template #suffix>
                <n-button
                  size="small"
                  type="primary"
                  :disabled="loading || !statusInput.trim()"
                  @click="handleApplyStatus"
                >
                  {{ $t('tools.signature.updateBtn') }}
                </n-button>
              </template>
            </n-input>
          </div>
        </n-collapse-item>
      </n-collapse>

      <!-- 个人主页背景 -->
      <n-collapse arrow-placement="right" class="collapse-card">
        <n-collapse-item name="profilebg">
          <template #header>
            <div class="collapse-header-wrapper">
              <div class="collapse-left">
                <div class="icon-container">
                  <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
                    <circle cx="8.5" cy="8.5" r="1.5"></circle>
                    <polyline points="21 15 16 10 5 21"></polyline>
                  </svg>
                </div>
                <div class="title-container">
                  <h3 class="card-title">{{ $t('tools.background.title') }}</h3>
                  <span class="card-desc">{{ $t('tools.background.desc') }}</span>
                </div>
              </div>
              <div class="collapse-right-status">
                <span class="status-preview">{{ $t('tools.spectate.expand') }}</span>
              </div>
            </div>
          </template>
          <div class="setting-row no-border">
            <span class="setting-label">{{ $t('tools.background.desc') }}</span>
          </div>
          <div class="setting-picker-row">
            <ChampionPicker v-model="bgChampion" :maxCount="1" />
          </div>
          
          <div v-if="skinLoading" class="skin-loading">
            <div class="loading-spinner"></div>
            <span>{{ $t('tools.loading') }}</span>
          </div>
          
          <!-- 已选择皮肤的预览信息，代替原来的平铺列表 -->
          <div v-else-if="skinList.length > 0" class="selected-skin-preview">
            <div class="preview-layout">
              <div class="preview-img-container">
                <LcuImage :src="currentSelectedSkin?.loadScreenPath" class="preview-img" />
              </div>
              <div class="preview-info-box">
                <span class="preview-title">{{ $t('tools.background.title') }}</span>
                <span class="preview-skin-name">{{ currentSelectedSkin?.name }}</span>
                <n-button size="small" type="primary" @click="openSkinModal">{{ $t('tools.background.selectSkinBtn') }}</n-button>
              </div>
            </div>
          </div>
        </n-collapse-item>
      </n-collapse>

      <!-- 段位展示 -->
      <n-collapse arrow-placement="right" class="collapse-card">
        <n-collapse-item name="rankdisplay">
          <template #header>
            <div class="collapse-header-wrapper">
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
                  <h3 class="card-title">{{ $t('tools.rankSpoof.title') }}</h3>
                  <span class="card-desc">{{ $t('tools.rankSpoof.desc') }}</span>
                </div>
              </div>
              <div class="collapse-right-status">
                <span class="status-preview">{{ $t('tools.spectate.expand') }}</span>
              </div>
            </div>
          </template>
          <div class="setting-row">
            <span class="setting-label">{{ $t('tools.rankSpoof.queueLabel') }}</span>
            <n-select
              v-model:value="spoofQueue"
              :options="Object.entries(SPOOF_QUEUE_LABELS).map(([k, v]) => ({ label: v, value: k }))"
              style="width: 140px;"
              size="small"
            />
          </div>
          <div class="setting-row">
            <span class="setting-label">{{ $t('tools.rankSpoof.tierLabel') }} {{ $t('tools.rankSpoof.divisionLabel') }}</span>
            <div class="rank-select-group">
              <n-select
                v-model:value="spoofTier"
                :options="['UNRANKED','CHALLENGER','GRANDMASTER','MASTER','DIAMOND','EMERALD','PLATINUM','GOLD','SILVER','BRONZE','IRON'].map(t => ({ label: SPOOF_TIER_LABELS[t], value: t }))"
                style="width: 130px;"
                size="small"
              />
              <n-select
                v-model:value="spoofDivision"
                :options="['I','II','III','IV'].map(d => ({ label: d, value: d }))"
                :disabled="['UNRANKED','MASTER','GRANDMASTER','CHALLENGER'].includes(spoofTier)"
                style="width: 80px;"
                size="small"
              />
              <n-button size="small" type="primary" @click="handleApplyRankSpoof" :disabled="loading">{{ $t('tools_extra.applySpoofBtn') }}</n-button>
            </div>
          </div>
        </n-collapse-item>
      </n-collapse>

      <!-- 在线状态 -->
      <n-collapse arrow-placement="right" class="collapse-card">
        <n-collapse-item name="onlinestate">
          <template #header>
            <div class="collapse-header-wrapper">
              <div class="collapse-left">
                <div class="icon-container">
                  <svg class="header-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M21 11.5a8.38 8.38 0 0 1-.9 3.8 8.5 8.5 0 0 1-7.6 4.7 8.38 8.38 0 0 1-3.8-.9L3 21l1.9-5.7a8.38 8.38 0 0 1-.9-3.8 8.5 8.5 0 0 1 4.7-7.6 8.38 8.38 0 0 1 3.8-.9h.5a8.48 8.48 0 0 1 8 8v.5z"></path>
                  </svg>
                </div>
                <div class="title-container">
                  <h3 class="card-title">{{ $t('tools.status.title') }}</h3>
                  <span class="card-desc">{{ $t('tools.status.title') }}</span>
                </div>
              </div>
              <div class="collapse-right-status">
                <span class="status-preview">{{ $t('tools.spectate.expand') }}</span>
              </div>
            </div>
          </template>
          <div class="setting-row">
            <span class="setting-label">{{ $t('tools.status.title') }}</span>
            <div class="btn-group">
              <n-button class="status-btn online" size="small" @click="handleApplyAvailability('chat')" :disabled="loading">{{ $t('tools.status.online') }}</n-button>
              <n-button class="status-btn away" size="small" @click="handleApplyAvailability('away')" :disabled="loading">{{ $t('tools.status.away') }}</n-button>
              <n-button class="status-btn offline" size="small" @click="handleApplyAvailability('offline')" :disabled="loading">{{ $t('tools.status.invisible') }}</n-button>
            </div>
          </div>
        </n-collapse-item>
      </n-collapse>

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
            <h3 class="card-title">{{ $t('tools.badges.title') }}</h3>
            <span class="card-desc">{{ $t('tools.badges.title') }}</span>
          </div>
        </div>
        <div class="card-right">
          <n-button class="action-btn text-danger" @click="handleClearBadges" :loading="loading">{{ $t('tools_extra.removeBtn') }}</n-button>
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
            <h3 class="card-title">{{ $t('tools.border.title') }}</h3>
            <span class="card-desc">{{ $t('tools.border.title') }}</span>
          </div>
        </div>
        <div class="card-right">
          <n-button class="action-btn text-danger" @click="handleClearBorder" :loading="loading">{{ $t('tools_extra.removeBtn') }}</n-button>
        </div>
      </div>

    </div>

    <!-- 皮肤选择轮播图弹窗 -->
    <Transition name="fade">
      <div v-if="showSkinModal" class="skin-modal-overlay" @click.self="showSkinModal = false">
        <div class="skin-modal-card">
          <!-- 弹窗头部 -->
          <div class="skin-modal-header">
            <h3>{{ $t('tools.background.titleModal') }}</h3>
            <button class="modal-close-btn" @click="showSkinModal = false">✕</button>
          </div>

          <!-- 轮播主图区 -->
          <div class="skin-carousel-container">
            <!-- 左箭头 -->
            <button class="carousel-nav-btn prev" @click="prevSkin" :title="$t('titlebar.back')">
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
            <button class="carousel-nav-btn next" @click="nextSkin" :title="$t('titlebar.back')">
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
              <button class="cancel-action-btn" @click="showSkinModal = false">{{ $t('tools.cancel') }}</button>
              <button class="confirm-action-btn" @click="confirmSkinSelection">{{ $t('tools.confirm') }}</button>
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

.offline-logo {
  font-size: 3rem;
  margin-bottom: 1rem;
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
  position: relative;
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

/* 卡片 Item 通用样式 */
.card-item, .collapse-item {
  background: var(--settings-card-bg);
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
.collapse-item.has-dropdown-open { z-index: 10; }

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
.card-item.border-bottom:last-of-type { border-radius: 0 0 12px 12px; border-bottom: 1px solid var(--settings-separator); }
.collapse-item.border-bottom:last-of-type { border-radius: 0 0 12px 12px; border-bottom: 1px solid var(--settings-separator); }
.collapse-item:not(.border-bottom) { border-bottom: none; }

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
.action-btn,
.action-btn.n-button {
  background: var(--settings-card-bg);
  border: 1px solid var(--settings-card-border);
  color: var(--text-color);
  padding: 6px 20px;
  height: auto;
  border-radius: 6px;
  font-size: 0.82rem;
  font-weight: 600;
  cursor: pointer;
  outline: none;
  transition: all 0.2s cubic-bezier(0.25, 0.8, 0.25, 1);
  box-shadow: var(--shadow-sm);
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.action-btn:hover,
.action-btn.n-button:hover {
  border-color: var(--primary-color);
  background-color: var(--settings-card-bg-hover);
  box-shadow: 0 0 0 1px rgba(0, 159, 170, 0.3);
  transform: translateY(-0.5px);
}

.action-btn:active,
.action-btn.n-button:active {
  background: var(--settings-card-bg);
  transform: translateY(0.5px);
}

.action-btn:disabled,
.action-btn.n-button:disabled {
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

/* 兼容 n-button 内部的 loading 和字体 */
.action-btn.n-button .n-button__content {
  color: inherit !important;
}
.action-btn.n-button .n-base-loading {
  color: inherit !important;
}

/* 统一 Toggle 开关样式 */
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
.toggle-text { font-size: 0.82rem; color: white; }
.toggle-switch.off .toggle-text { color: var(--text-dimmed); }
.toggle-slider {
  width: 22px; height: 22px; background-color: var(--toggle-slider);
  border-radius: 50%; position: absolute; top: 3px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3), 0 0 0 1px rgba(255,255,255,0.08);
  transition: left 0.3s cubic-bezier(0.4, 0, 0.2, 1), right 0.3s cubic-bezier(0.4, 0, 0.2, 1);
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
  border-top: 1px solid var(--settings-separator);
  padding: 14px 36px 16px;
  animation: slideDown 0.2s ease-out;
  background-color: rgba(0, 0, 0, 0.02);
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

/* 未连接 LCU 覆盖层 */
.offline-overlay {
  position: absolute;
  inset: 0;
  z-index: 10;
}

.setting-row.justify-end {
  justify-content: flex-end;
}

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
  width: 100%;
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

.text-input:hover, .number-input:hover {
  background-color: var(--card-bg);
  border-color: var(--border-color-hover);
}

.text-input:focus, .number-input:focus {
  background-color: var(--card-bg-hover);
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
  background: var(--card-bg);
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
  height: 24px;
}
.number-input:hover {
  background-color: var(--card-bg);
  border-color: var(--primary-color-alpha-40);
}
.number-input:focus {
  background-color: var(--card-bg-hover);
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
  color: var(--text-muted);
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
  background: var(--card-bg);
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
  background: var(--settings-card-bg);
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
  background: var(--card-bg);
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
  border: 2px solid var(--card-bg);
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
  background: var(--text-dimmed);
  border-radius: 2px;
  opacity: 0.3;
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
  background: var(--card-bg);
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
  background: var(--card-bg);
}

[data-theme="dark"] .cancel-action-btn:hover {
  background: rgba(30, 41, 59, 0.9);
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

.lane-tab-group {
  display: flex;
  background: rgba(0, 0, 0, 0.03);
  padding: 4px;
  border-radius: 8px;
  border: 1px solid var(--border-color);
  margin: 14px 0 8px;
  gap: 4px;
  width: 100%;
}

.lane-tab-btn {
  flex: 1;
  background: transparent;
  border: none;
  color: var(--text-muted);
  padding: 6px 10px;
  border-radius: 6px;
  font-size: 0.78rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease;
  white-space: nowrap;
  text-align: center;
}

.lane-tab-btn:hover {
  color: var(--text-color);
  background: var(--card-bg);
}

[data-theme="dark"] .lane-tab-btn:hover {
  color: var(--text-color);
  background: var(--card-bg);
}

.lane-tab-btn.active {
  color: var(--primary-color);
  background: var(--card-bg-hover);
  box-shadow: var(--shadow-sm);
}
</style>
