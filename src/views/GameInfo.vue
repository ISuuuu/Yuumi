<script setup lang="ts">
import { ref, watch, computed, onMounted } from "vue";
import { useLcuStore } from "../store/lcuStore";
import {
  getGameflowPhase, getChampSelectSession, fetchMatchHistory,
  fetchCurrentSummoner, lcuRequest,
} from "../api/lcu";
import type { MatchDisplay } from "../api/lcu";
import LcuImage from "../components/LcuImage.vue";

const store = useLcuStore();
const loading = ref(false);
const error = ref("");
const activeTab = ref<"my" | "their">("my");

// 当前登录玩家的 summonerId，用于 InProgress 阶段分离己方/敌方
const currentSummonerId = ref<number>(0);

// 每个玩家的完整数据
interface PlayerData {
  info: any;
  matches: MatchDisplay[];
  ranked: { solo: any; flex: any };
  loading: boolean;
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
  store.gamePhase === "ChampSelect" || store.gamePhase === "InProgress",
);

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

    const [matches, rankedResp] = await Promise.all([
      info.puuid ? fetchMatchHistory(info.puuid, 0, 10) : Promise.resolve([]),
      info.puuid ? lcuRequest<any>("GET", `/lol-ranked/v1/ranked-stats/${info.puuid}`) : Promise.resolve({ success: false } as any),
    ]);

    let solo = null, flex = null;
    if (rankedResp.success && rankedResp.data?.queues) {
      solo = rankedResp.data.queues.find((q: any) => q.queueType === "RANKED_SOLO_5x5") || null;
      flex = rankedResp.data.queues.find((q: any) => q.queueType === "RANKED_FLEX_SR") || null;
    }

    playerData.value[cellId] = { info, matches, ranked: { solo, flex }, loading: false };
  } catch {
    playerData.value[cellId] = { info: null, matches: [], ranked: { solo: null, flex: null }, loading: false };
  }
}

/** ChampSelect 阶段：加载当前 Tab 下的己方/敌方玩家 */
async function loadAllPlayers() {
  const team = currentTeam.value;
  if (!team || team.length === 0) return;
  await Promise.all(team.map((p: any) => loadPlayerData(p.cellId, p.summonerId)));
}

/**
 * InProgress / GameStart 阶段：通过 /lol-gameflow/v1/session 加载双方数据。
 * 该接口的 teamOne/teamTwo 包含完整 championId，可同时获取敌方信息。
 */
async function loadFromGameflowSession() {
  loading.value = true;
  error.value = "";

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
    if (!teamOne || !teamTwo) return;

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

function formatRank(q: any): string {
  if (!q || !q.tier || q.tier === "NONE") return "";
  const tier = TIER_MAP[q.tier] || q.tier;
  const div = q.rank && q.rank !== "NA" ? q.rank : "";
  return `${tier}${div}`;
}

onMounted(async () => {
  // 获取当前玩家 summonerId，用于 InProgress 阶段分离队伍
  try {
    const s = await fetchCurrentSummoner();
    if (s?.summonerId) currentSummonerId.value = s.summonerId;
  } catch { /* ignore */ }
  refreshState();
});

// 监听阶段变化：ChampSelect → 加载己方；InProgress/GameStart → 加载双方
watch(() => store.gamePhase, (phase: string) => {
  if (phase === "ChampSelect") {
    // 清除 gameflow 数据，回退到 champSelectSession
    gameflowMyTeam.value = [];
    gameflowTheirTeam.value = [];
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

    <div v-else-if="!isGameActive" class="tip-container">
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
              <!-- 选人阶段：选了英雄才显示英雄头像；未选则只显示等级 -->
              <template v-if="p.championId">
                <div class="pc-avatar">
                  <LcuImage :src="getChampionIcon(p.championId)" alt="champ" />
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
              <span class="pc-name">
                {{ playerData[p.cellId]?.info?.gameName || playerData[p.cellId]?.info?.displayName || p.displayName || '未知' }}
                <span v-if="playerData[p.cellId]?.info?.tagLine" class="pc-tag">#{{ playerData[p.cellId].info.tagLine }}</span>
              </span>
              <div class="pc-ranks">
                <span class="pc-rank-item" :title="'单/双排'">
                  🏆 {{ formatRank(playerData[p.cellId]?.ranked?.solo) || '无段位' }}
                </span>
                <span class="pc-rank-item" :title="'灵活排位'">
                  🎯 {{ formatRank(playerData[p.cellId]?.ranked?.flex) || '无段位' }}
                </span>
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
              <div class="col-champ-avatar">
                <LcuImage v-if="p.championId" :src="getChampionIcon(p.championId)" alt="champ" />
                <div v-else class="col-champ-placeholder">?</div>
              </div>
              <span class="col-name">{{ playerData[p.cellId]?.info?.gameName || p.displayName || `玩家${i+1}` }}</span>
            </div>

            <template v-if="playerData[p.cellId]?.loading">
              <div class="col-loading">
                <div class="mini-spinner"></div>
              </div>
            </template>
            <template v-else-if="playerData[p.cellId]?.matches?.length">
              <div
                v-for="m in playerData[p.cellId].matches"
                :key="m.gameId"
                :class="['col-match', m.win ? 'win' : 'lose']"
              >
                <div class="cm-champ">
                  <LcuImage :src="m.championIconUrl" class="cm-champ-img" alt="champ" />
                  <span class="cm-level">{{ m.champLevel }}</span>
                </div>
                <div class="cm-detail">
                  <span class="cm-mode">{{ m.name }}</span>
                  <div class="cm-bottom">
                    <span class="cm-kda">{{ m.kills }}/{{ m.deaths }}/{{ m.assists }}</span>
                    <span class="cm-date">{{ m.shortTime }}</span>
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
  padding: 1.5rem 1.5rem 1.5rem 0.6rem;
  background-color: transparent;
  min-height: 100%;
}

.page-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 1.2rem;
}
.page-title {
  font-size: 1.4rem;
  font-weight: 800;
  color: var(--text-color);
  margin: 0;
  letter-spacing: 0.5px;
}
.title-phase {
  font-size: 0.85rem;
  font-weight: 500;
  color: var(--text-dimmed);
  margin-left: 2px;
}
.header-right { display: flex; align-items: center; gap: 10px; }

.phase-badge {
  padding: 4px 12px; border-radius: 12px;
  font-size: 0.78rem; font-weight: 600; color: white;
  box-shadow: var(--shadow-sm);
}
.phase-badge.champselect { background: var(--primary-color); box-shadow: 0 4px 10px var(--primary-color-alpha-30); }
.phase-badge.inprogress { background: var(--win-color); box-shadow: 0 4px 10px var(--win-glow); }
.phase-badge.gamestart { background: var(--win-color); box-shadow: 0 4px 10px var(--win-glow); }
.phase-badge.readycheck { background: #fbbf24; color: #000; box-shadow: 0 4px 10px rgba(251, 191, 36, 0.2); }
.phase-badge.lobby { background: #a855f7; box-shadow: 0 4px 10px rgba(168, 85, 247, 0.2); }
.phase-badge.matchmaking { background: #06b6d4; box-shadow: 0 4px 10px rgba(6, 182, 212, 0.2); }
.phase-badge.endofgame { background: var(--text-dimmed); }
.phase-badge.none { background: rgba(0, 0, 0, 0.05); color: var(--text-muted); }


.tip-container {
  display: flex; flex-direction: column; align-items: center; justify-content: center;
  padding: 6rem 2rem; color: var(--text-muted);
}
.offline-logo { font-size: 3rem; margin-bottom: 1rem; }
.tip { font-size: 0.95rem; color: var(--text-dimmed); margin: 0; }

/* 左右分栏 */
.game-layout {
  display: grid;
  grid-template-columns: 260px 1fr;
  gap: 16px;
  height: calc(100vh - 100px);
}

/* 左侧面板 */
.left-panel {
  background: var(--card-bg); border: 1px solid var(--border-color);
  border-radius: var(--radius-lg); overflow: hidden;
  box-shadow: var(--shadow-sm);
  display: flex; flex-direction: column;
  height: 100%;
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
}
.team-tabs { display: flex; border-bottom: 1px solid var(--border-color); }
.tab-btn {
  flex: 1; padding: 12px; border: none; background: rgba(0, 0, 0, 0.01);
  font-size: 0.82rem; font-weight: 600; color: var(--text-muted); cursor: pointer;
  border-bottom: 2px solid transparent; transition: all 0.2s;
}
.tab-btn:hover { background: rgba(0, 0, 0, 0.03); }
.tab-btn.active { color: var(--primary-color); background: transparent; border-bottom-color: var(--primary-color); }

.player-list {
  display: flex; flex-direction: column; flex: 1;
}

.player-card {
  display: flex; align-items: center; gap: 12px;
  padding: 14px 14px; border-bottom: 1px solid var(--border-color);
  flex: 1; transition: background 0.15s;
}
.player-card:last-child { border-bottom: none; }
.player-card:hover { background: rgba(0, 0, 0, 0.02); }

/* 左侧：头像区域 */
.pc-avatar-area {
  position: relative; flex-shrink: 0;
}
.pc-avatar {
  width: 48px; height: 48px; border-radius: 50%;
  overflow: hidden; border: 2px solid var(--border-color);
  box-shadow: var(--shadow-sm);
}
.pc-avatar-empty { border-color: var(--border-color); }
.pc-avatar-placeholder {
  width: 100%; height: 100%; display: flex; align-items: center; justify-content: center;
  background: rgba(0, 0, 0, 0.04); font-size: 1rem; color: var(--text-dimmed);
}
.pc-level {
  position: absolute; top: -4px; right: -4px;
  background: var(--primary-color); color: white; font-size: 0.6rem; font-weight: 700;
  padding: 1px 6px; border-radius: 8px; white-space: nowrap;
  box-shadow: 0 2px 6px var(--primary-color-alpha-40);
}

.pc-info { display: flex; flex-direction: column; min-width: 0; flex: 1; gap: 4px; }
.pc-name {
  font-size: 0.85rem; font-weight: 600; color: var(--text-color);
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
}
.pc-tag { font-size: 0.75rem; color: var(--text-muted); font-weight: 400; }
.pc-ranks { display: flex; gap: 8px; flex-wrap: wrap; }
.pc-rank-item {
  font-size: 0.72rem; color: var(--text-muted); background: rgba(0, 0, 0, 0.02);
  padding: 2px 8px; border-radius: 4px; white-space: nowrap; border: 1px solid var(--border-color);
}

/* 右侧：5 列战绩 */
.right-panel {
  background: var(--card-bg); border: 1px solid var(--border-color);
  border-radius: var(--radius-lg); box-shadow: var(--shadow-sm);
  overflow: hidden; height: 100%;
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
  display: flex; flex-direction: column;
  overflow-y: auto;
}
.player-column:last-child { border-right: none; }

.col-header {
  display: flex; align-items: center; gap: 8px;
  padding: 10px 10px; border-bottom: 1px solid var(--border-color);
  background: rgba(0, 0, 0, 0.01); flex-shrink: 0;
}
.col-champ-avatar {
  width: 30px; height: 30px; border-radius: 50%;
  overflow: hidden; border: 1px solid var(--border-color); flex-shrink: 0;
}
.col-champ-placeholder {
  width: 100%; height: 100%; display: flex; align-items: center; justify-content: center;
  background: rgba(0, 0, 0, 0.04); font-size: 0.7rem; color: var(--text-dimmed);
}
.col-name {
  font-size: 0.78rem; font-weight: 600; color: var(--text-color);
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
}

.col-loading { display: flex; align-items: center; justify-content: center; flex: 1; }
.mini-spinner {
  width: 20px; height: 20px; border: 2px solid rgba(0, 0, 0, 0.06);
  border-top-color: var(--primary-color); border-radius: 50%;
  animation: spin 0.8s linear infinite;
}
@keyframes spin { to { transform: rotate(360deg); } }

.col-empty {
  text-align: center; padding: 2rem 0.5rem;
  font-size: 0.75rem; color: var(--text-dimmed);
}

.col-matches {
  display: flex; flex-direction: column; flex: 1;
}

.col-match {
  display: flex; align-items: center; gap: 8px;
  padding: 0 10px; border-bottom: 1px solid var(--border-color);
  flex: 1;
}
.col-match:last-child { border-bottom: none; }

/* 胜/败背景色 */
.col-match.win { background: var(--win-bg); }
.col-match.lose { background: var(--loss-bg); }

/* 头像区域 */
.cm-champ {
  position: relative; width: 34px; height: 34px; flex-shrink: 0;
}
.cm-champ-img {
  width: 34px; height: 34px; border-radius: 50%;
  overflow: hidden; border: 1px solid var(--border-color);
}
.cm-level {
  position: absolute; bottom: -2px; right: -2px;
  width: 13px; height: 13px; line-height: 11px;
  background: rgba(255, 255, 255, 0.9); color: var(--text-color); border-radius: 50%;
  font-size: 0.58rem; font-weight: bold; text-align: center;
  border: 1px solid var(--border-color);
}

/* 右侧详情 */
.cm-detail {
  display: flex; flex-direction: column; min-width: 0; flex: 1; gap: 2px;
}
.cm-mode {
  font-size: 0.72rem; font-weight: 600; color: var(--text-color);
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
}
.cm-bottom {
  display: flex; justify-content: space-between; align-items: center; gap: 4px;
}
.cm-kda { font-size: 0.72rem; font-weight: 600; color: var(--text-muted); }
.cm-date { font-size: 0.65rem; color: var(--text-dimmed); white-space: nowrap; }
</style>
