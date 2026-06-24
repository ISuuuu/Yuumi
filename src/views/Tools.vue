<script setup lang="ts">
import { ref, onMounted, watch } from "vue";
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
const bgChampion = ref<number[]>([]);

// 皮肤列表（选英雄后加载）
interface SkinInfo { id: number; name: string; splashPath: string; loadScreenPath: string }
const skinList = ref<SkinInfo[]>([]);
const selectedSkinId = ref<number | null>(null);
const skinLoading = ref(false);

// 监听背景英雄点选，自动加载该英雄的皮肤列表
watch(bgChampion, async (newVal: number[]) => {
  skinList.value = [];
  selectedSkinId.value = null;
  skinIdInput.value = null;
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

// 选择皮肤
function selectSkin(skinId: number) {
  selectedSkinId.value = skinId;
  skinIdInput.value = skinId;
}

// 观战输入项
const spectateSummonerName = ref("");

const GAME_MODES: Record<number, string> = {
  2400: "海克斯大乱斗", 450: "极地大乱斗", 430: "匹配模式",
  420: "单双排位", 440: "灵活排位", 900: "无限火力",
  1020: "克隆模式", 1300: "极限闪击", 1700: "斗魂竞技场",
};

onMounted(async () => {
  try {
    config.value = await fetchConfig();
  } catch (e) {
    console.error("加载其他功能配置失败:", e);
  }
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
            <h3 class="card-title">自动接受对局</h3>
            <span class="card-desc">在你设置的秒数之后自动接受对局匹配</span>
          </div>
          <div class="collapse-right">
            <span class="status-preview">
              {{ config.Functions.EnableAutoAcceptMatching 
                ? `已启用，延迟: ${config.Functions.AutoAcceptMatchingDelay} 秒` 
                : '未启用' 
              }}
            </span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'autoaccept' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'autoaccept'" class="collapse-content">
          <div class="input-row align-center">
            <div :class="['toggle-switch', config.Functions.EnableAutoAcceptMatching ? 'on' : 'off']" @click="config.Functions.EnableAutoAcceptMatching = !config.Functions.EnableAutoAcceptMatching; triggerAutoSave()">
              <span class="toggle-text">{{ config.Functions.EnableAutoAcceptMatching ? '开' : '关' }}</span>
              <span class="toggle-slider"></span>
            </div>
            <label class="delay-label">延迟 (秒):</label>
            <input v-model.number="config.Functions.AutoAcceptMatchingDelay" type="number" min="0" max="11" class="number-input" @change="triggerAutoSave" />
          </div>
        </div>
      </div>

      <!-- 自动接受交换请求 -->
      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('autoswap')">
          <div class="collapse-left">
            <h3 class="card-title">自动接受交换请求</h3>
            <span class="card-desc">自动接受队友的交换楼层或英雄的请求</span>
          </div>
          <div class="collapse-right">
            <span class="status-preview">
              {{ (config.Functions.AutoAcceptCeilSwap || config.Functions.AutoAcceptChampTrade) ? '已启用' : '未启用' }}
            </span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'autoswap' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'autoswap'" class="collapse-content">
          <div class="checkbox-group">
            <label class="checkbox-row">
              <input type="checkbox" v-model="config.Functions.AutoAcceptCeilSwap" @change="triggerAutoSave" />
              <span>自动接受楼层交换请求 (选人顺序)</span>
            </label>
            <label class="checkbox-row">
              <input type="checkbox" v-model="config.Functions.AutoAcceptChampTrade" @change="triggerAutoSave" />
              <span>自动接受英雄交换请求 (大乱斗等)</span>
            </label>
          </div>
        </div>
      </div>

      <!-- 自动亮起英雄 -->
      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('autohover')">
          <div class="collapse-left">
            <h3 class="card-title">自动亮起英雄</h3>
            <span class="card-desc">在你进入英雄选择时自动亮起/预选英雄</span>
          </div>
          <div class="collapse-right">
            <span class="status-preview">{{ config.Functions.EnableAutoSelectChampion ? '已启用' : '未启用' }}</span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'autohover' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'autohover'" class="collapse-content">
          <div class="input-row align-center margin-bottom">
            <div :class="['toggle-switch', config.Functions.EnableAutoSelectChampion ? 'on' : 'off']" @click="config.Functions.EnableAutoSelectChampion = !config.Functions.EnableAutoSelectChampion; triggerAutoSave()">
              <span class="toggle-text">{{ config.Functions.EnableAutoSelectChampion ? '开' : '关' }}</span>
              <span class="toggle-slider"></span>
            </div>
            <span class="toggle-desc">预选环节自动点亮设定英雄</span>
          </div>
          <ChampionPicker v-model="config.Functions.AutoSelectChampion" :maxCount="1" @update:modelValue="onPickerChange" />
        </div>
      </div>

      <!-- 自动禁用英雄 -->
      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('autoban')">
          <div class="collapse-left">
            <h3 class="card-title">自动禁用英雄</h3>
            <span class="card-desc">在你的禁用环节开始时自动禁用英雄</span>
          </div>
          <div class="collapse-right">
            <span class="status-preview">{{ config.Functions.EnableAutoBanChampion ? '已启用' : '未启用' }}</span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'autoban' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'autoban'" class="collapse-content">
          <div class="input-row align-center margin-bottom">
            <div :class="['toggle-switch', config.Functions.EnableAutoBanChampion ? 'on' : 'off']" @click="config.Functions.EnableAutoBanChampion = !config.Functions.EnableAutoBanChampion; triggerAutoSave()">
              <span class="toggle-text">{{ config.Functions.EnableAutoBanChampion ? '开' : '关' }}</span>
              <span class="toggle-slider"></span>
            </div>
            <span class="toggle-desc">禁用环节自动禁用设定英雄</span>
          </div>
          <ChampionPicker v-model="config.Functions.AutoBanChampion" :maxCount="1" @update:modelValue="onPickerChange" />
        </div>
      </div>

      <!-- 自动设置召唤师技能 -->
      <div class="collapse-item">
        <div class="collapse-header" @click="toggleCollapse('autospells')">
          <div class="collapse-left">
            <h3 class="card-title">自动设置召唤师技能</h3>
            <span class="card-desc">当你的英雄选择开始时自动设置召唤师技能</span>
          </div>
          <div class="collapse-right">
            <span class="status-preview">{{ config.Functions.EnableAutoSetSpells ? '已启用' : '未启用' }}</span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'autospells' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'autospells'" class="collapse-content">
          <div class="input-row align-center margin-bottom">
            <div :class="['toggle-switch', config.Functions.EnableAutoSetSpells ? 'on' : 'off']" @click="config.Functions.EnableAutoSetSpells = !config.Functions.EnableAutoSetSpells; triggerAutoSave()">
              <span class="toggle-text">{{ config.Functions.EnableAutoSetSpells ? '开' : '关' }}</span>
              <span class="toggle-slider"></span>
            </div>
            <span class="toggle-desc">锁定英雄后自动写入配置好的技能组</span>
          </div>
          <SpellPicker v-model="config.Functions.AutoSetSummonerSpell" :maxCount="2" @update:modelValue="onPickerChange" />
        </div>
      </div>

      <!-- 2. 游戏组 -->
      <div class="group-header">游戏</div>

      <!-- 自动重连 -->
      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">自动重连</h3>
          <span class="card-desc">当你掉线退出游戏时自动重新连接</span>
        </div>
        <div class="card-right">
          <div :class="['toggle-switch', config.Functions.EnableAutoReconnect ? 'on' : 'off']" @click="config.Functions.EnableAutoReconnect = !config.Functions.EnableAutoReconnect; triggerAutoSave()">
            <span class="toggle-text">{{ config.Functions.EnableAutoReconnect ? '开' : '关' }}</span>
            <span class="toggle-slider"></span>
          </div>
        </div>
      </div>

      <!-- 自动创建大厅 -->
      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('createlobby')">
          <div class="collapse-left">
            <h3 class="card-title">自动创建大厅</h3>
            <span class="card-desc">启动 LOL 客户端后自动创建默认模式的大厅</span>
          </div>
          <div class="collapse-right">
            <span class="status-preview">
              {{ config.Functions.EnableAutoCreateLobby 
                ? `已启用: ${GAME_MODES[config.Functions.DefaultGameMode] || '未知模式'}` 
                : '未启用' 
              }}
            </span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'createlobby' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'createlobby'" class="collapse-content">
          <div class="input-row align-center margin-bottom">
            <div :class="['toggle-switch', config.Functions.EnableAutoCreateLobby ? 'on' : 'off']" @click="config.Functions.EnableAutoCreateLobby = !config.Functions.EnableAutoCreateLobby; triggerAutoSave()">
              <span class="toggle-text">{{ config.Functions.EnableAutoCreateLobby ? '开' : '关' }}</span>
              <span class="toggle-slider"></span>
            </div>
            <span class="toggle-desc">客户端引导就绪后自动拉入指定大厅房间</span>
          </div>
          <div class="input-row">
            <select v-model.number="config.Functions.DefaultGameMode" class="select-input" @change="triggerAutoSave" :disabled="!config.Functions.EnableAutoCreateLobby">
              <option v-for="(name, id) in GAME_MODES" :key="id" :value="Number(id)">{{ name }}</option>
            </select>
          </div>
        </div>
      </div>

      <!-- 观战 -->
      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('spectate')">
          <div class="collapse-left">
            <h3 class="card-title">观战</h3>
            <span class="card-desc">观战同大区玩家正在进行的实时游戏</span>
          </div>
          <div class="collapse-right">
            <span class="status-preview">点击展开</span>
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'spectate' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'spectate'" class="collapse-content">
          <div class="input-row">
            <input v-model="spectateSummonerName" placeholder="输入要观战的召唤师名称..." class="text-input" />
            <button class="apply-btn" @click="handleSpectate" :disabled="!spectateSummonerName.trim()">观战</button>
          </div>
        </div>
      </div>

      <!-- 锁定游戏设置 -->
      <div class="card-item">
        <div class="card-left">
          <h3 class="card-title">锁定游戏设置</h3>
          <span class="card-desc">让你的游戏设置不会因为切换账号而改变</span>
        </div>
        <div class="card-right">
          <div :class="['toggle-switch', config.Functions.GameInfoFilter ? 'on' : 'off']" @click="config.Functions.GameInfoFilter = !config.Functions.GameInfoFilter; triggerAutoSave()">
            <span class="toggle-text">{{ config.Functions.GameInfoFilter ? '开' : '关' }}</span>
            <span class="toggle-slider"></span>
          </div>
        </div>
      </div>

      <!-- 3. 客户端组 -->
      <div class="group-header">客户端</div>

      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">修复客户端窗口</h3>
          <span class="card-desc">修复客户端错误的窗口大小（需要管理员权限）</span>
        </div>
        <div class="card-right">
          <button class="action-btn" @click="handleFixWindow" :disabled="loading">修复</button>
        </div>
      </div>

      <div class="card-item">
        <div class="card-left">
          <h3 class="card-title">重启客户端</h3>
          <span class="card-desc">重启客户端而不需要重新排队</span>
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
            <h3 class="card-title">个人签名</h3>
            <span class="card-desc">修改你个人卡片的签名</span>
          </div>
          <div class="collapse-right">
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'signature' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'signature'" class="collapse-content">
          <div class="input-row">
            <input v-model="statusInput" placeholder="输入新的个性化签名..." class="text-input" />
            <button class="apply-btn" @click="handleApplyStatus" :disabled="loading || !statusInput.trim()">应用</button>
          </div>
        </div>
      </div>

      <!-- 个人主页背景 -->
      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('profilebg')">
          <div class="collapse-left">
            <h3 class="card-title">个人主页背景</h3>
            <span class="card-desc">修改你个人主页背景皮肤图片</span>
          </div>
          <div class="collapse-right">
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'profilebg' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'profilebg'" class="collapse-content">
          <div class="input-row align-center margin-bottom">
            <span class="toggle-desc">选择英雄后显示对应皮肤：</span>
          </div>
          <ChampionPicker v-model="bgChampion" :maxCount="1" />
          <div v-if="skinLoading" class="skin-loading">加载皮肤中...</div>
          <div v-else-if="skinList.length > 0" class="skin-grid">
            <div
              v-for="skin in skinList"
              :key="skin.id"
              :class="['skin-card', { active: selectedSkinId === skin.id }]"
              @click="selectSkin(skin.id)"
            >
              <LcuImage :src="skin.loadScreenPath" class="skin-img" />
              <div class="skin-name">{{ skin.name }}</div>
            </div>
          </div>
          <div class="input-row margin-top">
            <label class="delay-label">皮肤 ID:</label>
            <input v-model.number="skinIdInput" type="number" placeholder="手动输入皮肤 ID..." class="text-input" />
            <button class="apply-btn" @click="handleApplyBackground" :disabled="loading || skinIdInput === null">应用</button>
          </div>
        </div>
      </div>

      <!-- 段位展示 -->
      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('rankdisplay')">
          <div class="collapse-left">
            <h3 class="card-title">段位展示</h3>
            <span class="card-desc">修改你个人卡片显示的段位</span>
          </div>
          <div class="collapse-right">
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'rankdisplay' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'rankdisplay'" class="collapse-content">
          <div class="input-row" style="margin-bottom: 8px;">
            <select v-model="spoofQueue" class="select-input">
              <option value="RANKED_TFT">云顶之弈</option>
              <option value="RANKED_SOLO_5x5">单双排位</option>
              <option value="RANKED_FLEX_SR">灵活排位</option>
            </select>
          </div>
          <div class="input-row">
            <select v-model="spoofTier" class="select-input">
              <option value="UNRANKED">未定级</option>
              <option value="CHALLENGER">最强王者</option>
              <option value="GRANDMASTER">傲世宗师</option>
              <option value="MASTER">超凡大师</option>
              <option value="DIAMOND">璀璨钻石</option>
              <option value="EMERALD">流光翡翠</option>
              <option value="PLATINUM">华贵铂金</option>
              <option value="GOLD">荣耀黄金</option>
              <option value="SILVER">不屈白银</option>
              <option value="BRONZE">英勇黄铜</option>
              <option value="IRON">坚韧黑铁</option>
            </select>
            <select v-model="spoofDivision" class="select-input" :disabled="['UNRANKED','MASTER','GRANDMASTER','CHALLENGER'].includes(spoofTier)">
              <option v-if="['UNRANKED','MASTER','GRANDMASTER','CHALLENGER'].includes(spoofTier)" value="NA">-</option>
              <option value="I">I</option>
              <option value="II">II</option>
              <option value="III">III</option>
              <option value="IV">IV</option>
            </select>
            <button class="apply-btn" @click="handleApplyRankSpoof" :disabled="loading">应用</button>
          </div>
        </div>
      </div>

      <!-- 在线状态 -->
      <div class="collapse-item border-bottom">
        <div class="collapse-header" @click="toggleCollapse('onlinestate')">
          <div class="collapse-left">
            <h3 class="card-title">在线状态</h3>
            <span class="card-desc">修改你的在线状态</span>
          </div>
          <div class="collapse-right">
            <svg :class="['arrow-icon', { expanded: activeCollapse === 'onlinestate' }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
        <div v-show="activeCollapse === 'onlinestate'" class="collapse-content">
          <div class="btn-group">
            <button class="status-btn online" @click="handleApplyAvailability('chat')" :disabled="loading">在线</button>
            <button class="status-btn away" @click="handleApplyAvailability('away')" :disabled="loading">离开</button>
            <button class="status-btn offline" @click="handleApplyAvailability('offline')" :disabled="loading">隐身</button>
          </div>
        </div>
      </div>

      <!-- 卸下勋章 -->
      <div class="card-item border-bottom">
        <div class="card-left">
          <h3 class="card-title">卸下勋章</h3>
          <span class="card-desc">卸下你个人卡片中的所有勋章</span>
        </div>
        <div class="card-right">
          <button class="action-btn text-danger" @click="handleClearBadges" :disabled="loading">卸下</button>
        </div>
      </div>

      <!-- 卸下头像框 -->
      <div class="card-item">
        <div class="card-left">
          <h3 class="card-title">卸下头像框</h3>
          <span class="card-desc">卸下你的召唤师头像框（需要召唤师等级大于等于 525）</span>
        </div>
        <div class="card-right">
          <button class="action-btn text-danger" @click="handleClearBorder" :disabled="loading">卸下</button>
        </div>
      </div>

    </div>
  </div>
</template>

<style scoped>
.tools-view {
  padding: 1.5rem 1.5rem 1.5rem 0.6rem;
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

.tools-container {
  max-width: 800px;
  margin: 0 auto;
  animation: fadeIn 0.3s ease-out;
}

.page-title {
  font-size: 1.8rem;
  font-weight: 800;
  color: #2c3e50;
  margin: 0 0 1.5rem;
}

.group-header {
  font-size: 0.85rem;
  font-weight: bold;
  color: #909399;
  margin: 1.8rem 0 0.6rem 6px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

/* 卡片 Item 通用样式 */
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

/* 按钮样式 */
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

.action-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.text-danger {
  color: #f56c6c !important;
}

.text-danger:hover {
  background-color: #fef0f0;
  border-color: #fde2e2;
}

/* Switch 开关样式 */
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
  background-color: #6c5ce7;
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

.delay-label {
  font-size: 0.85rem;
  color: #606266;
  margin-left: 16px;
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
}

.input-row {
  display: flex;
  gap: 8px;
  width: 100%;
}

.input-row.align-center {
  align-items: center;
}

.margin-bottom {
  margin-bottom: 12px;
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

.number-input {
  width: 70px;
  padding: 6px 10px;
  border: 1px solid #dcdfe6;
  border-radius: 6px;
  font-size: 0.85rem;
  outline: none;
}

.number-input:focus {
  border-color: #6c5ce7;
}

.select-input {
  padding: 8px 12px;
  border: 1px solid #dcdfe6;
  border-radius: 6px;
  font-size: 0.85rem;
  background-color: white;
  outline: none;
}

.select-input:focus {
  border-color: #6c5ce7;
}

.apply-btn {
  background-color: #6c5ce7;
  color: white;
  border: none;
  padding: 8px 20px;
  border-radius: 6px;
  font-size: 0.85rem;
  font-weight: bold;
  cursor: pointer;
  transition: background-color 0.2s;
}

.apply-btn:hover {
  background-color: #5a4bd1;
}

.apply-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-group {
  display: flex;
  gap: 8px;
}

.status-btn {
  border: 1px solid #dcdfe6;
  background: white;
  padding: 6px 20px;
  border-radius: 6px;
  font-size: 0.82rem;
  font-weight: bold;
  cursor: pointer;
  transition: all 0.2s;
}

.status-btn.online {
  color: #67c23a;
  border-color: #e1f3d8;
  background-color: #f0f9eb;
}
.status-btn.online:hover {
  background-color: #67c23a;
  color: white;
}

.status-btn.away {
  color: #e6a23c;
  border-color: #fdf6ec;
  background-color: #fdf6ec;
}
.status-btn.away:hover {
  background-color: #e6a23c;
  color: white;
}

.status-btn.offline {
  color: #909399;
  border-color: #f4f4f5;
  background-color: #f4f4f5;
}
.status-btn.offline:hover {
  background-color: #909399;
  color: white;
}

.checkbox-group {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.checkbox-row {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 0.85rem;
  color: #606266;
  cursor: pointer;
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

/* 皮肤选择网格 */
.skin-loading {
  font-size: 0.82rem;
  color: #909399;
  text-align: center;
  padding: 12px 0;
}

.skin-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
  margin: 8px 0 12px;
  max-height: 320px;
  overflow-y: auto;
  padding-right: 4px;
}

.skin-grid::-webkit-scrollbar {
  width: 4px;
}

.skin-grid::-webkit-scrollbar-thumb {
  background: #dcdfe6;
  border-radius: 2px;
}

.skin-card {
  border: 2px solid transparent;
  border-radius: 8px;
  overflow: hidden;
  cursor: pointer;
  transition: border-color 0.2s, transform 0.15s;
  background: #f5f7fa;
}

.skin-card:hover {
  border-color: #c0c4cc;
  transform: translateY(-1px);
}

.skin-card.active {
  border-color: #6c5ce7;
  box-shadow: 0 0 0 1px #6c5ce7;
}

.skin-img {
  width: 100%;
  aspect-ratio: 16 / 9;
  object-fit: cover;
  display: block;
  border-radius: 6px 6px 0 0;
}

.skin-name {
  font-size: 0.72rem;
  color: #606266;
  padding: 4px 6px;
  text-align: center;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* Toast 通知 */
.toast {
  position: fixed;
  top: 20px;
  left: 50%;
  transform: translateX(-50%);
  padding: 10px 24px;
  border-radius: 8px;
  font-size: 0.88rem;
  font-weight: 600;
  color: white;
  z-index: 9999;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  pointer-events: none;
}

.toast-success {
  background-color: #67c23a;
}

.toast-error {
  background-color: #f56c6c;
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
