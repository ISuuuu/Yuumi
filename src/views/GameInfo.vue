<script setup lang="ts">
import { ref, watch, computed, onMounted, inject, type Ref } from "vue";
import { useLcuStore } from "../store/lcuStore";
import {
  getGameflowPhase, getChampSelectSession, fetchMatchHistory,
  fetchCurrentSummoner, lcuRequest, fetchConfig,
} from "../api/lcu";
import type { MatchDisplay, AppConfig } from "../api/lcu";
import LcuImage from "../components/LcuImage.vue";

const store = useLcuStore();
const loading = ref(false);
const error = ref("");
const activeTab = ref<"my" | "their">("my");

// 应用配置（用于获取对局卡片颜色）
const appConfig = inject<Ref<AppConfig | null>>("appConfig") || ref<AppConfig | null>(null);

const currentSummonerId = ref<number>(0);

// 当前游戏模式对应的队列 ID（用于过滤战绩）
const currentQueueId = ref<number | null>(null);

async function updateCurrentQueueId() {
  try {
    const resp = await lcuRequest<any>("GET", "/lol-gameflow/v1/session");
    if (resp.success && resp.data?.gameData?.queue?.id !== undefined) {
      currentQueueId.value = resp.data.gameData.queue.id;
      console.log("[GameInfo] Detected current game mode queueId:", currentQueueId.value);
    } else {
      currentQueueId.value = null;
    }
  } catch {
    currentQueueId.value = null;
  }
}

// 每个玩家的完整数据
interface PlayerData {
  info: any;
  matches: MatchDisplay[];
  ranked: { solo: any; flex: any };
  loading: boolean;
  avgKda?: number;
  winRate?: number;
  winCount?: number;
  lossesCount?: number;
}
const playerData = ref<Record<number, PlayerData>>({});

const TIER_MAP: Record<string, string> = {
  NONE: "", IRON: "黑铁", BRONZE: "黄铜", SILVER: "白银", GOLD: "黄金",
  PLATINUM: "铂金", EMERALD: "翡翠", DIAMOND: "钻石",
  MASTER: "大师", GRANDMASTER: "宗师", CHALLENGER: "王者",
};

const myTeam = computed(() => {
  if (gameflowMyTeam.value.length > 0) return gameflowMyTeam.value;
  return store.champSelectSession?.myTeam ?? [];
});
const theirTeam = computed(() => {
  if (gameflowTheirTeam.value.length > 0) return gameflowTheirTeam.value;
  return store.champSelectSession?.theirTeam ?? [];
});
const currentTeam = computed(() => activeTab.value === "my" ? myTeam.value : theirTeam.value);

// InProgress/GameStart 阶段的队伍数据（从 gameflow session 获取）
const gameflowMyTeam = ref<any[]>([]);
const gameflowTheirTeam = ref<any[]>([]);

/** 当前是否处于可展示战绩的阶段 */
const isGameActive = computed(() =>
  store.gamePhase === "ChampSelect" || store.gamePhase === "GameStart" || store.gamePhase === "InProgress",
);

/** 是否应该在界面中展示对局信息（如果开启了“保留对局信息界面内容”且有历史数据，则即使不在对局中也展示） */
const shouldShowContent = computed(() => {
  if (isGameActive.value) return true;
  if (appConfig.value?.Functions?.EnableReserveGameinfo) {
    return Object.keys(playerData.value).length > 0;
  }
  return false;
});

// 监听游戏活跃状态的变化，当退出游戏或选人阶段时，若开启了保留上一局配置，则从本地存储中加载数据以在内存中还原对局选手和战绩
watch(isGameActive, (active) => {
  if (!active) {
    try {
      const savedMyTeam = localStorage.getItem("yuumi_last_gameflow_my_team");
      const savedTheirTeam = localStorage.getItem("yuumi_last_gameflow_their_team");
      const savedPlayerData = localStorage.getItem("yuumi_last_game_player_data");
      if (savedMyTeam) {
        const parsed = JSON.parse(savedMyTeam);
        if (Array.isArray(parsed) && parsed.length > 0) gameflowMyTeam.value = parsed;
      }
      if (savedTheirTeam) {
        const parsed = JSON.parse(savedTheirTeam);
        if (Array.isArray(parsed) && parsed.length > 0) gameflowTheirTeam.value = parsed;
      }
      if (savedPlayerData) {
        const parsed = JSON.parse(savedPlayerData);
        if (parsed && Object.keys(parsed).length > 0) playerData.value = parsed;
      }
    } catch { /* ignore */ }
  }
});

async function refreshState() {
  loading.value = true;
  try {
    const phaseResp = await getGameflowPhase();
    if (phaseResp.success && phaseResp.data) store.setGamePhase(phaseResp.data);
  } catch { /* ignore */ }
  try {
    const sessionResp = await getChampSelectSession();
    if (sessionResp.success && sessionResp.data) store.setChampSelectSession(sessionResp.data);
  } catch { /* ignore */ }
  loading.value = false;
}

/** 加载单个玩家的战绩/排位数据（通用，ChampSelect 和 InProgress 共用） */
async function loadPlayerData(cellId: number, summonerId: number) {
  if (!summonerId || playerData.value[cellId]?.info) return;
  playerData.value[cellId] = { info: null, matches: [], ranked: { solo: null, flex: null }, loading: true };

  try {
    const resp = await lcuRequest<any>("GET", `/lol-summoner/v1/summoners/${summonerId}`);
    if (!resp.success || !resp.data) {
      playerData.value[cellId] = { info: null, matches: [], ranked: { solo: null, flex: null }, loading: false };
      return;
    }
    const info = resp.data;

    const filterEnabled = appConfig.value?.Functions?.GameInfoFilter ?? false;
    const maxMatches = filterEnabled ? 50 : 10;

    const [historyMatches, rankedResp] = await Promise.all([
      info.puuid ? fetchMatchHistory(info.puuid, 0, maxMatches) : Promise.resolve([]),
      info.puuid ? lcuRequest<any>("GET", `/lol-ranked/v1/ranked-stats/${info.puuid}`) : Promise.resolve({ success: false } as any),
    ]);

    let matches = historyMatches;
    if (filterEnabled && currentQueueId.value !== null) {
      matches = historyMatches.filter((m: MatchDisplay) => m.queueId === currentQueueId.value).slice(0, 10);
    }

    let solo = null, flex = null;
    if (rankedResp.success && rankedResp.data?.queues) {
      solo = rankedResp.data.queues.find((q: any) => q.queueType === "RANKED_SOLO_5x5") || null;
      flex = rankedResp.data.queues.find((q: any) => q.queueType === "RANKED_FLEX_SR") || null;
    }

    let avgKda = 0;
    let winRate = 0;
    let winCount = 0;
    let lossesCount = 0;

    if (matches && matches.length > 0) {
      let totalKills = 0;
      let totalDeaths = 0;
      let totalAssists = 0;
      let remakeCount = 0;

      matches.forEach((m: MatchDisplay) => {
        if (m.remake) {
          remakeCount++;
        } else {
          totalKills += m.kills ?? 0;
          totalDeaths += m.deaths ?? 0;
          totalAssists += m.assists ?? 0;
          if (m.win) {
            winCount++;
          } else {
            lossesCount++;
          }
        }
      });

      const validMatches = matches.length - remakeCount;
      winRate = validMatches > 0 ? Math.round((winCount / validMatches) * 100) : 0;
      const deathsForCalc = totalDeaths === 0 ? 1 : totalDeaths;
      avgKda = (totalKills + totalAssists) / deathsForCalc;
    }

    playerData.value[cellId] = {
      info,
      matches,
      ranked: { solo, flex },
      loading: false,
      avgKda,
      winRate,
      winCount,
      lossesCount,
    };
    try {
      localStorage.setItem("yuumi_last_game_player_data", JSON.stringify(playerData.value));
      localStorage.setItem("yuumi_last_gameflow_my_team", JSON.stringify(myTeam.value));
      localStorage.setItem("yuumi_last_gameflow_their_team", JSON.stringify(theirTeam.value));
    } catch { /* ignore */ }
  } catch {
    playerData.value[cellId] = { info: null, matches: [], ranked: { solo: null, flex: null }, loading: false };
  }
}

/** ChampSelect 阶段：加载当前 Tab 下的己方/敌方玩家 */
async function loadAllPlayers() {
  const team = currentTeam.value;
  if (!team || team.length === 0) return;
  await updateCurrentQueueId();
  await Promise.all(team.map((p: any) => loadPlayerData(p.cellId, p.summonerId)));
}

/**
 * InProgress / GameStart 阶段：通过 /lol-gameflow/v1/session 加载双方数据。
 * 该接口的 teamOne/teamTwo 包含完整 championId，可同时获取敌方信息。
 */
async function loadFromGameflowSession() {
  loading.value = true;
  error.value = "";
  await updateCurrentQueueId();

  // 保底获取当前玩家 summonerId（onMounted 可能还没拿到）
  if (!currentSummonerId.value) {
    try {
      const s = await fetchCurrentSummoner();
      if (s?.summonerId) currentSummonerId.value = s.summonerId;
    } catch { /* ignore */ }
  }

  try {
    const resp = await lcuRequest<any>("GET", "/lol-gameflow/v1/session");
    if (!resp.success || !resp.data?.gameData) {
      error.value = "无法获取对局 Session";
      loading.value = false;
      return;
    }

    const { teamOne, teamTwo } = resp.data.gameData;
    // 队伍数据缺失或为空时不覆盖已有数据（退出游戏过程中可能出现）
    if (!teamOne || !teamTwo || teamOne.length === 0 || teamTwo.length === 0) {
      loading.value = false;
      return;
    }

    // 用当前玩家 summonerId 判断哪队是己方
    console.log("[GameInfo] currentSummonerId:", currentSummonerId.value);
    const isTeamOne = teamOne.some((p: any) => p.summonerId === currentSummonerId.value);
    const allyTeam = isTeamOne ? teamOne : teamTwo;
    const enemyTeam = isTeamOne ? teamTwo : teamOne;

    // 将 gameflow session 的数据注入队伍列表
    // 使用 summonerId 作为 cellId（gameflow 无 cellId 字段）
    gameflowMyTeam.value = allyTeam.map((p: any) => ({
      ...p,
      cellId: p.summonerId,
      displayName: p.summonerName || p.displayName,
    }));
    gameflowTheirTeam.value = enemyTeam.map((p: any) => ({
      ...p,
      cellId: p.summonerId,
      displayName: p.summonerName || p.displayName,
    }));

    try {
      localStorage.setItem("yuumi_last_gameflow_my_team", JSON.stringify(gameflowMyTeam.value));
      localStorage.setItem("yuumi_last_gameflow_their_team", JSON.stringify(gameflowTheirTeam.value));
    } catch { /* ignore */ }

    // 并行加载双方全部玩家
    await Promise.all([
      ...allyTeam.map((p: any) => loadPlayerData(p.summonerId, p.summonerId)),
      ...enemyTeam.map((p: any) => loadPlayerData(p.summonerId, p.summonerId)),
    ]);
  } catch (e) {
    console.error("加载 gameflow session 失败:", e);
    error.value = "加载对局数据失败";
  }
  loading.value = false;
}

function getChampionIcon(id: number): string {
  return id > 0 ? `/lol-game-data/assets/v1/champion-icons/${id}.png` : "";
}

/** 获取对局卡片颜色样式 */
function getMatchCardStyle(m: MatchDisplay): Record<string, string> {
  if (!appConfig.value?.Personalization) return {};

  const colors = appConfig.value.Personalization;
  let color = '';

  if (m.remake) {
    color = colors.RemakeCardColor || '';
  } else if (m.win) {
    color = colors.WinCardColor || '';
  } else {
    color = colors.LoseCardColor || '';
  }

  if (color) {
    // 处理不同的颜色格式
    if (color.startsWith('#') && color.length === 9) {
      // #AARRGGBB 格式：转换为 rgba()
      const alpha = parseInt(color.slice(1, 3), 16) / 255;
      const r = parseInt(color.slice(3, 5), 16);
      const g = parseInt(color.slice(5, 7), 16);
      const b = parseInt(color.slice(7, 9), 16);
      return { background: `rgba(${r}, ${g}, ${b}, ${alpha.toFixed(2)})` };
    } else if (color.startsWith('#') && color.length === 7) {
      // #RRGGBB 格式：添加半透明背景
      return { background: `${color}1a` }; // 1a = 约 10% 透明度
    }
    // 其他格式（rgba 等）直接使用
    return { background: color };
  }
  return {};
}

function formatRank(q: any): string {
  if (!q || !q.tier || q.tier === "NONE") return "";
  const tier = TIER_MAP[q.tier] || q.tier;
  const div = q.rank && q.rank !== "NA" ? q.rank : "";
  const lp = q.leaguePoints !== undefined ? ` ${q.leaguePoints} LP` : "";
  return `${tier}${div}${lp}`;
}

onMounted(async () => {
  // 从 localStorage 加载上一局数据以支持持久化显示
  try {
    const savedMyTeam = localStorage.getItem("yuumi_last_gameflow_my_team");
    const savedTheirTeam = localStorage.getItem("yuumi_last_gameflow_their_team");
    const savedPlayerData = localStorage.getItem("yuumi_last_game_player_data");
    if (savedMyTeam) {
      const parsed = JSON.parse(savedMyTeam);
      if (Array.isArray(parsed) && parsed.length > 0) gameflowMyTeam.value = parsed;
    }
    if (savedTheirTeam) {
      const parsed = JSON.parse(savedTheirTeam);
      if (Array.isArray(parsed) && parsed.length > 0) gameflowTheirTeam.value = parsed;
    }
    if (savedPlayerData) {
      const parsed = JSON.parse(savedPlayerData);
      if (parsed && Object.keys(parsed).length > 0) playerData.value = parsed;
    }
  } catch { /* ignore */ }

  // 获取当前玩家 summonerId，用于 InProgress 阶段分离队伍
  try {
    const s = await fetchCurrentSummoner();
    if (s?.summonerId) currentSummonerId.value = s.summonerId;
  } catch { /* ignore */ }

  // 获取应用配置（用于对局卡片颜色）
  if (!appConfig.value) {
    try {
      appConfig.value = await fetchConfig();
    } catch { /* ignore */ }
  }

  refreshState();
});

// 监听阶段变化：ChampSelect → 加载己方；InProgress/GameStart → 加载双方
watch(() => store.gamePhase, (phase: string) => {
  if (phase === "ChampSelect") {
    // 清除 gameflow 数据，回退到 champSelectSession
    gameflowMyTeam.value = [];
    gameflowTheirTeam.value = [];
    playerData.value = {}; // 清除上一局的旧玩家数据缓存
    try {
      localStorage.removeItem("yuumi_last_gameflow_my_team");
      localStorage.removeItem("yuumi_last_gameflow_their_team");
      localStorage.removeItem("yuumi_last_game_player_data");
    } catch { /* ignore */ }
    refreshState();
  }
  if (phase === "InProgress" || phase === "GameStart") loadFromGameflowSession();
});

// ChampSelect session 更新时加载己方玩家
watch(() => store.champSelectSession, (session: any) => {
  if (session && store.gamePhase === "ChampSelect") {
    loading.value = false;
    error.value = "";
    loadAllPlayers();
  }
}, { deep: true });

function getKdaClass(kda: number | undefined): string {
  if (kda === undefined) return "kda-gray";
  if (kda >= 5.0) return "kda-orange";
  if (kda >= 4.0) return "kda-blue";
  if (kda >= 3.0) return "kda-green";
  return "kda-gray";
}

function getWinRateClass(rate: number | undefined): string {
  if (rate === undefined) return "wr-low";
  if (rate >= 70) return "wr-high";
  if (rate >= 50) return "wr-medium";
  return "wr-low";
}

watch(activeTab, () => loadAllPlayers());
</script>

<template>
  <div class="game-info">
    <div class="page-header">
      <h2 class="page-title">🎮 对局信息</h2>
    </div>

    <div v-if="!store.isConnected" class="tip-container">
      <div class="offline-logo">🎮</div>
      <p class="tip">请先启动英雄联盟客户端</p>
    </div>

    <div v-else-if="!shouldShowContent" class="tip-container">
      <div class="offline-logo">⏳</div>
      <p class="tip">进入选人阶段后将自动加载对局信息</p>
    </div>

    <div v-else class="game-layout">
      <!-- 左侧：队伍切换 + 玩家列表 -->
      <div class="left-panel">
        <div class="team-tabs">
          <button :class="['tab-btn', { active: activeTab === 'my' }]" @click="activeTab = 'my'">
            己方 ({{ myTeam.length }})
          </button>
          <button :class="['tab-btn', { active: activeTab === 'their' }]" @click="activeTab = 'their'">
            敌方 ({{ theirTeam.length }})
          </button>
        </div>

        <div class="player-list">
          <div
            v-for="(p, i) in currentTeam"
            :key="p.cellId ?? i"
            class="player-card"
          >
            <div class="pc-avatar-area">
              <!-- 选人阶段：选了英雄或预选了英雄才显示英雄头像 -->
              <template v-if="p.championId || p.championPickIntent">
                <div class="pc-avatar">
                  <LcuImage :src="getChampionIcon(p.championId || p.championPickIntent)" alt="champ" />
                </div>
              </template>
              <template v-else>
                <div class="pc-avatar pc-avatar-empty">
                  <div class="pc-avatar-placeholder">?</div>
                </div>
              </template>
              <div v-if="playerData[p.cellId]?.info?.summonerLevel" class="pc-level">
                {{ playerData[p.cellId].info.summonerLevel }}
              </div>
            </div>
            <div class="pc-info">
              <div class="pc-name-row">
                <span class="pc-name">
                  {{ playerData[p.cellId]?.info?.gameName || playerData[p.cellId]?.info?.displayName || p.displayName || '未知' }}
                  <span v-if="playerData[p.cellId]?.info?.tagLine" class="pc-tag">#{{ playerData[p.cellId].info.tagLine }}</span>
                </span>
                <span v-if="playerData[p.cellId]?.winRate !== undefined" :class="['pc-winrate-badge', getWinRateClass(playerData[p.cellId].winRate)]">
                  {{ playerData[p.cellId].winRate }}% 胜率
                </span>
              </div>
              
              <div class="pc-meta-row">
                <span v-if="playerData[p.cellId]?.avgKda !== undefined" :class="['pc-kda-badge', getKdaClass(playerData[p.cellId].avgKda)]">
                  KDA: {{ playerData[p.cellId]?.avgKda?.toFixed(2) ?? '0.00' }}
                </span>
              </div>

              <div class="pc-ranks">
                <div class="pc-rank-line" :title="'单/双排'">
                  <span class="rank-icon-wrapper">🏆</span>
                  <span class="rank-text">{{ formatRank(playerData[p.cellId]?.ranked?.solo) || '单双排: 无段位' }}</span>
                </div>
                <div class="pc-rank-line" :title="'灵活排位'">
                  <span class="rank-icon-wrapper">🎯</span>
                  <span class="rank-text">{{ formatRank(playerData[p.cellId]?.ranked?.flex) || '灵活排: 无段位' }}</span>
                </div>
              </div>
            </div>
          </div>
          <div v-if="currentTeam.length === 0" class="tip">暂无队伍数据</div>
        </div>
      </div>

      <!-- 右侧：5 列战绩 -->
      <div class="right-panel">
        <div class="columns-container">
          <div
            v-for="(p, i) in currentTeam"
            :key="p.cellId ?? i"
            class="player-column"
          >
            <div class="col-header">
              <div class="col-header-top">
                <div class="col-header-info">
                  <span class="col-name">{{ playerData[p.cellId]?.info?.gameName || p.displayName || `玩家${i+1}` }}</span>
                  <span v-if="playerData[p.cellId]?.winRate !== undefined" class="col-summary">
                    近10局: <span class="summary-wins">{{ playerData[p.cellId].winCount }}胜</span>
                    <span class="summary-losses">{{ playerData[p.cellId].lossesCount }}负</span>
                  </span>
                </div>
              </div>
            </div>

            <template v-if="playerData[p.cellId]?.loading">
              <div class="col-loading">
                <div class="mini-spinner"></div>
              </div>
            </template>
            <template v-else-if="playerData[p.cellId]?.matches?.length">
              <div class="col-matches-list">
                <div
                  v-for="m in playerData[p.cellId].matches"
                  :key="m.gameId"
                  :class="['col-match', m.remake ? 'remake' : (m.win ? 'win' : 'lose')]"
                  :style="getMatchCardStyle(m)"
                >
                  <div class="cm-champ">
                    <LcuImage :src="m.championIconUrl" class="cm-champ-img" alt="champ" />
                    <span class="cm-level">{{ m.champLevel }}</span>
                  </div>
                  <div class="cm-detail">
                    <div class="cm-top-row">
                      <span class="cm-mode">{{ m.name.replace("排位赛 ", "") }}</span>
                    </div>
                    <div class="cm-bottom">
                      <span class="cm-kda">
                        <span class="k">{{ m.kills }}</span>/
                        <span class="d">{{ m.deaths }}</span>/
                        <span class="a">{{ m.assists }}</span>
                      </span>
                      <span class="cm-date">{{ m.shortTime.split(' ')[0] }}</span>
                    </div>
                  </div>
                </div>
              </div>
            </template>
            <template v-else>
              <div class="col-empty">暂无</div>
            </template>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.game-info {
  padding: 1rem 1.5rem 1rem 0.6rem;
  background-color: transparent;
  height: 100%;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.page-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 1rem;
  flex-shrink: 0;
}
.page-title {
  font-size: 1.4rem;
  font-weight: 800;
  color: var(--text-color);
  margin: 0;
  letter-spacing: 0.5px;
}

.tip-container {
  display: flex; flex-direction: column; align-items: center; justify-content: center;
  padding: 6rem 2rem; color: var(--text-muted);
  flex: 1;
}
.offline-logo { font-size: 3rem; margin-bottom: 1rem; }
.tip { font-size: 0.95rem; color: var(--text-dimmed); margin: 0; }

/* 左右分栏 */
.game-layout {
  display: grid;
  grid-template-columns: 280px 1fr;
  gap: 16px;
  flex: 1;
  min-height: 0;
}

/* 左侧面板 */
.left-panel {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  overflow: hidden;
  box-shadow: var(--shadow-sm);
  display: flex;
  flex-direction: column;
  height: 100%;
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
}
.team-tabs {
  display: flex;
  border-bottom: 1px solid var(--border-color);
  background: rgba(0, 0, 0, 0.02);
}
.tab-btn {
  flex: 1;
  padding: 14px;
  border: none;
  background: transparent;
  font-size: 0.85rem;
  font-weight: 700;
  color: var(--text-muted);
  cursor: pointer;
  border-bottom: 2px solid transparent;
  transition: all 0.2s ease-in-out;
}
.tab-btn:hover {
  background: rgba(0, 0, 0, 0.03);
  color: var(--text-color);
}
.tab-btn.active {
  color: var(--primary-color);
  border-bottom-color: var(--primary-color);
  background: var(--card-bg);
}

.player-list {
  display: flex;
  flex-direction: column;
  flex: 1;
  padding: 12px;
  gap: 10px;
  overflow: hidden;
}

/* 玩家卡片卡片化升级 */
.player-card {
  flex: 1;
  min-height: 0;
  max-height: 140px;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px;
  background: rgba(0, 0, 0, 0.02);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  transition: all 0.2s ease-in-out;
  box-sizing: border-box;
}
.player-card:hover {
  background: var(--hover-bg);
  transform: translateY(-2px);
  border-color: rgba(var(--primary-color-rgb, 59, 130, 246), 0.3);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08);
}

/* 左侧：头像区域 */
.pc-avatar-area {
  position: relative;
  flex-shrink: 0;
}
.pc-avatar {
  width: 50px;
  height: 50px;
  border-radius: 50%;
  overflow: hidden;
  border: 2px solid var(--border-color);
  box-shadow: var(--shadow-sm);
}
.pc-avatar-empty {
  border-color: var(--border-color);
}
.pc-avatar-placeholder {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.04);
  font-size: 1rem;
  color: var(--text-dimmed);
}
.pc-level {
  position: absolute;
  bottom: -2px;
  right: -2px;
  background: var(--primary-color);
  color: white;
  font-size: 0.6rem;
  font-weight: 700;
  padding: 1px 5px;
  border-radius: 8px;
  white-space: nowrap;
  box-shadow: 0 2px 6px var(--primary-color-alpha-40);
  border: 1px solid var(--border-color);
}

.pc-info {
  display: flex;
  flex-direction: column;
  min-width: 0;
  flex: 1;
  gap: 4px;
}
.pc-name-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 4px;
}
.pc-name {
  font-size: 0.85rem;
  font-weight: 700;
  color: var(--text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.pc-tag {
  font-size: 0.72rem;
  color: var(--text-muted);
  font-weight: 400;
}

/* 胜率徽章 */
.pc-winrate-badge {
  font-size: 0.68rem;
  font-weight: 700;
  padding: 2px 6px;
  border-radius: 4px;
  white-space: nowrap;
}
.pc-winrate-badge.wr-high {
  color: var(--win-color);
  background: var(--win-bg);
}
.pc-winrate-badge.wr-medium {
  color: var(--tier-blue);
  background: var(--tier-blue-bg);
}
.pc-winrate-badge.wr-low {
  color: var(--text-dimmed);
  background: var(--hover-bg);
}

.pc-meta-row {
  display: flex;
  align-items: center;
}

/* KDA 均值徽章 */
.pc-kda-badge {
  font-size: 0.7rem;
  font-weight: 600;
  padding: 2px 8px;
  border-radius: 4px;
}
.pc-kda-badge.kda-orange {
  color: var(--accent-color);
  background: var(--accent-bg);
  border: 1px solid rgba(230, 162, 60, 0.2);
  font-weight: 700;
}
.pc-kda-badge.kda-blue {
  color: var(--tier-blue);
  background: var(--tier-blue-bg);
  border: 1px solid var(--tier-blue-border);
}
.pc-kda-badge.kda-green {
  color: var(--win-color);
  background: var(--win-bg);
  border: 1px solid var(--win-border);
}
.pc-kda-badge.kda-gray {
  color: var(--text-dimmed);
  background: var(--hover-bg);
}

/* 排位段位格栅化 */
.pc-ranks {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-top: 4px;
}
.pc-rank-line {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 0.72rem;
  color: var(--text-muted);
}
.rank-icon-wrapper {
  font-size: 0.75rem;
}
.rank-text {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* 右侧：5 列战绩 */
.right-panel {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-sm);
  overflow: hidden;
  height: 100%;
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
}
.columns-container {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  height: 100%;
}

.player-column {
  border-right: 1px solid var(--border-color);
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}
.player-column:last-child {
  border-right: none;
}

.col-header {
  padding: 12px;
  border-bottom: 1px solid var(--border-color);
  background: rgba(0, 0, 0, 0.02);
  flex-shrink: 0;
}
.col-header-top {
  display: flex;
  align-items: center;
  gap: 10px;
}
.col-header-info {
  display: flex;
  flex-direction: column;
  min-width: 0;
  flex: 1;
}
.col-name {
  font-size: 0.8rem;
  font-weight: 700;
  color: var(--text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.col-summary {
  font-size: 0.68rem;
  color: var(--text-muted);
  margin-top: 2px;
}
.summary-wins {
  color: var(--win-color);
  font-weight: bold;
}
.summary-losses {
  color: var(--loss-color);
  font-weight: bold;
}

.col-loading {
  display: flex;
  align-items: center;
  justify-content: center;
  flex: 1;
}
.mini-spinner {
  width: 22px;
  height: 22px;
  border: 2px solid rgba(0, 0, 0, 0.06);
  border-top-color: var(--primary-color);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}
@keyframes spin { to { transform: rotate(360deg); } }

.col-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  flex: 1;
  font-size: 0.75rem;
  color: var(--text-dimmed);
}

.col-matches-list {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  gap: 6px;
  padding: 6px 6px;
  box-sizing: border-box;
}

.col-match {
  flex: 1;
  min-height: 0;
  max-height: 72px;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 8px;
  margin: 0;
  border-radius: 8px;
  border: 1px solid transparent;
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.02);
  transition: all 0.2s ease-in-out;
  box-sizing: border-box;
}

/* 胜/败/重赛 玻璃渐变色背景 */
.col-match.win {
  border-color: var(--win-border);
}
.col-match.win:hover {
  border-color: var(--win-color);
  box-shadow: var(--win-glow);
  transform: translateY(-1px);
}

.col-match.lose {
  border-color: var(--loss-border);
}
.col-match.lose:hover {
  border-color: var(--loss-color);
  box-shadow: var(--loss-glow);
  transform: translateY(-1px);
}

.col-match.remake {
  border-color: var(--border-color);
}

/* 头像区域 */
.cm-champ {
  position: relative;
  width: 34px;
  height: 34px;
  flex-shrink: 0;
}
.cm-champ-img {
  width: 34px;
  height: 34px;
  border-radius: 50%;
  overflow: hidden;
  border: 1px solid var(--border-color);
}
.cm-level {
  position: absolute;
  bottom: -2px;
  right: -2px;
  width: 13px;
  height: 13px;
  line-height: 11px;
  background: var(--card-bg);
  color: var(--text-color);
  border-radius: 50%;
  font-size: 0.55rem;
  font-weight: bold;
  text-align: center;
  border: 1px solid var(--border-color);
  box-shadow: var(--shadow-sm);
}

/* 右侧详情 */
.cm-detail {
  display: flex;
  flex-direction: column;
  min-width: 0;
  flex: 1;
  gap: 2px;
}
.cm-top-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.cm-mode {
  font-size: 0.72rem;
  font-weight: 700;
  color: var(--text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.cm-bottom {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-top: 1px;
  white-space: nowrap;
  overflow: hidden;
}
.cm-kda {
  font-size: 0.7rem;
  font-weight: 700;
  color: var(--text-muted);
  letter-spacing: 0.1px;
  white-space: nowrap;
}
.cm-kda .d {
  color: var(--death-color, #ef4444); /* 强调死亡数 */
}
.cm-date {
  font-size: 0.65rem;
  color: var(--text-dimmed);
  white-space: nowrap;
}
</style>
