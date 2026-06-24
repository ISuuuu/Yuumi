<script setup lang="ts">
import { ref, watch, computed, onMounted } from "vue";
import { useLcuStore } from "../store/lcuStore";
import { getGameflowPhase, getChampSelectSession, fetchMatchHistory, lcuRequest } from "../api/lcu";
import type { MatchDisplay } from "../api/lcu";
import LcuImage from "../components/LcuImage.vue";

const store = useLcuStore();
const loading = ref(false);
const error = ref("");

const activeTab = ref<"my" | "their">("my");

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
  MASTER: "大师", GRANDMASTER: "宗师", CHALLENGER: "王者"
};

const myTeam = computed(() => store.champSelectSession?.myTeam ?? []);
const theirTeam = computed(() => store.champSelectSession?.theirTeam ?? []);
const currentTeam = computed(() => activeTab.value === "my" ? myTeam.value : theirTeam.value);

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

async function loadAllPlayers() {
  const team = currentTeam.value;
  if (!team || team.length === 0) return;

  await Promise.all(team.map(async (p: any) => {
    const cellId = p.cellId;
    if (!p.summonerId || playerData.value[cellId]?.info) return;
    playerData.value[cellId] = { info: null, matches: [], ranked: { solo: null, flex: null }, loading: true };

    try {
      // 1. 召唤师信息
      const resp = await lcuRequest<any>("GET", `/lol-summoner/v1/summoners/${p.summonerId}`);
      if (!resp.success || !resp.data) {
        playerData.value[cellId] = { info: null, matches: [], ranked: { solo: null, flex: null }, loading: false };
        return;
      }
      const info = resp.data;

      // 2. 并行拉取战绩 + 排位数据
      const [matches, rankedResp] = await Promise.all([
        info.puuid ? fetchMatchHistory(info.puuid, 0, 10) : Promise.resolve([]),
        info.puuid ? lcuRequest<any>("GET", `/lol-ranked/v1/ranked-stats/${info.puuid}`) : Promise.resolve({ success: false } as any)
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
  }));
}

function getChampionIcon(id: number): string {
  return id > 0 ? `/lol-game-data/assets/v1/champion-icons/${id}.png` : "";
}

function getProfileIcon(id: number): string {
  return `/lol-game-data/assets/v1/profile-icons/${id ?? 29}.jpg`;
}

function formatRank(q: any): string {
  if (!q || !q.tier || q.tier === "NONE") return "";
  const tier = TIER_MAP[q.tier] || q.tier;
  const div = q.rank && q.rank !== "NA" ? q.rank : "";
  return `${tier}${div}`;
}

onMounted(() => refreshState());

watch(() => store.gamePhase, (phase: string) => {
  if (phase === "ChampSelect") refreshState();
});

watch(() => store.champSelectSession, (session: any) => {
  if (session) {
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
      <div class="header-right">
        <span :class="['phase-badge', store.gamePhase.toLowerCase()]">
          {{ store.gamePhase === 'ChampSelect' ? '选人阶段' :
             store.gamePhase === 'InProgress' ? '游戏中' :
             store.gamePhase === 'ReadyCheck' ? '准备确认' :
             store.gamePhase === 'Lobby' ? '房间大厅' :
             store.gamePhase === 'Matchmaking' ? '匹配中' :
             store.gamePhase === 'EndOfGame' ? '对局结束' : '等待中' }}
        </span>
        <button class="refresh-btn" @click="refreshState" :disabled="loading">
          {{ loading ? "加载中..." : "刷新数据" }}
        </button>
      </div>
    </div>

    <div v-if="!store.isConnected" class="tip-container">
      <div class="offline-logo">🎮</div>
      <p class="tip">请先启动英雄联盟客户端</p>
    </div>

    <div v-else-if="store.gamePhase !== 'ChampSelect'" class="tip-container">
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
              <div class="pc-avatar">
                <LcuImage
                  v-if="playerData[p.cellId]?.info?.profileIconId"
                  :src="getProfileIcon(playerData[p.cellId].info.profileIconId)"
                  alt="icon"
                />
                <div v-else class="pc-avatar-placeholder">?</div>
              </div>
              <!-- 选中的英雄小图标 -->
              <div v-if="p.championId" class="pc-champ-badge">
                <LcuImage :src="getChampionIcon(p.championId)" alt="champ" />
              </div>
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
  background-color: #fafbfc;
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
  color: #303133;
  margin: 0;
}
.header-right { display: flex; align-items: center; gap: 10px; }

.phase-badge {
  padding: 4px 12px; border-radius: 12px;
  font-size: 0.82rem; font-weight: 600; color: white;
}
.phase-badge.champselect { background: #1976d2; }
.phase-badge.inprogress { background: #388e3c; }
.phase-badge.readycheck { background: #f57c00; }
.phase-badge.lobby { background: #7b1fa2; }
.phase-badge.matchmaking { background: #0097a7; }
.phase-badge.endofgame { background: #616161; }
.phase-badge.none { background: #9e9e9e; }

.refresh-btn {
  padding: 6px 14px; border: 1px solid #dcdfe6; border-radius: 6px;
  background: white; color: #606266; font-size: 0.82rem; cursor: pointer;
}
.refresh-btn:hover { background: #f5f7fa; }
.refresh-btn:disabled { opacity: 0.5; cursor: not-allowed; }

.tip-container {
  display: flex; flex-direction: column; align-items: center; justify-content: center;
  padding: 6rem 2rem; color: #909399;
}
.offline-logo { font-size: 3rem; margin-bottom: 1rem; }
.tip { font-size: 1rem; color: #8c8c8c; margin: 0; }

/* 左右分栏 */
.game-layout {
  display: grid;
  grid-template-columns: 260px 1fr;
  gap: 16px;
  height: calc(100vh - 100px);
}

/* 左侧面板 */
.left-panel {
  background: white; border: 1px solid #ebeef5;
  border-radius: 12px; overflow: hidden;
  box-shadow: 0 2px 8px rgba(0,0,0,0.02);
  display: flex; flex-direction: column;
  height: 100%;
}
.team-tabs { display: flex; border-bottom: 1px solid #ebeef5; }
.tab-btn {
  flex: 1; padding: 12px; border: none; background: #fafbfc;
  font-size: 0.88rem; font-weight: 600; color: #606266; cursor: pointer;
  border-bottom: 2px solid transparent; transition: all 0.2s;
}
.tab-btn:hover { background: #f0f2f5; }
.tab-btn.active { color: #1976d2; background: white; border-bottom-color: #1976d2; }

.player-list {
  display: flex; flex-direction: column; flex: 1;
}

.player-card {
  display: flex; align-items: center; gap: 12px;
  padding: 14px 14px; border-bottom: 1px solid #f0f0f0;
  flex: 1; transition: background 0.15s;
}
.player-card:last-child { border-bottom: none; }
.player-card:hover { background: #f8f9fb; }

/* 左侧：头像区域 */
.pc-avatar-area {
  position: relative; flex-shrink: 0;
}
.pc-avatar {
  width: 48px; height: 48px; border-radius: 50%;
  overflow: hidden; border: 2px solid #dcdfe6;
  box-shadow: 0 1px 4px rgba(0,0,0,0.08);
}
.pc-avatar-placeholder {
  width: 100%; height: 100%; display: flex; align-items: center; justify-content: center;
  background: #e0e0e0; font-size: 1rem; color: #999;
}
/* 英雄小图标角标 */
.pc-champ-badge {
  position: absolute; bottom: -2px; right: -4px;
  width: 22px; height: 22px; border-radius: 50%;
  overflow: hidden; border: 2px solid #fff;
  box-shadow: 0 1px 3px rgba(0,0,0,0.15);
  background: #fff;
}
.pc-level {
  position: absolute; top: -4px; right: -4px;
  background: #2ecc71; color: white; font-size: 0.6rem; font-weight: 700;
  padding: 1px 6px; border-radius: 8px; white-space: nowrap;
  box-shadow: 0 1px 2px rgba(0,0,0,0.15);
}

.pc-info { display: flex; flex-direction: column; min-width: 0; flex: 1; gap: 4px; }
.pc-name {
  font-size: 0.9rem; font-weight: 600; color: #303133;
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
}
.pc-tag { font-size: 0.78rem; color: #909399; font-weight: 400; }
.pc-ranks { display: flex; gap: 8px; flex-wrap: wrap; }
.pc-rank-item {
  font-size: 0.75rem; color: #606266; background: #f5f7fa;
  padding: 2px 8px; border-radius: 4px; white-space: nowrap;
}

/* 右侧：5 列战绩 */
.right-panel {
  background: white; border: 1px solid #ebeef5;
  border-radius: 12px; box-shadow: 0 2px 8px rgba(0,0,0,0.02);
  overflow: hidden; height: 100%;
}
.columns-container {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  height: 100%;
}

.player-column {
  border-right: 1px solid #f0f0f0;
  display: flex; flex-direction: column;
  overflow-y: auto;
}
.player-column:last-child { border-right: none; }

.col-header {
  display: flex; align-items: center; gap: 8px;
  padding: 10px 10px; border-bottom: 1px solid #ebeef5;
  background: #fafbfc; flex-shrink: 0;
}
.col-champ-avatar {
  width: 30px; height: 30px; border-radius: 50%;
  overflow: hidden; border: 1px solid #dcdfe6; flex-shrink: 0;
}
.col-champ-placeholder {
  width: 100%; height: 100%; display: flex; align-items: center; justify-content: center;
  background: #e0e0e0; font-size: 0.7rem; color: #999;
}
.col-name {
  font-size: 0.82rem; font-weight: 600; color: #303133;
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
}

.col-loading { display: flex; align-items: center; justify-content: center; flex: 1; }
.mini-spinner {
  width: 20px; height: 20px; border: 2px solid #e2e5e9;
  border-top-color: #1976d2; border-radius: 50%;
  animation: spin 0.8s linear infinite;
}
@keyframes spin { to { transform: rotate(360deg); } }

.col-empty {
  text-align: center; padding: 2rem 0.5rem;
  font-size: 0.78rem; color: #c0c4cc;
}

.col-matches {
  display: flex; flex-direction: column; flex: 1;
}

.col-match {
  display: flex; align-items: center; gap: 8px;
  padding: 0 10px; border-bottom: 1px solid #f8f8f8;
  flex: 1;
}
.col-match:last-child { border-bottom: none; }

/* 胜/败背景色 */
.col-match.win { background: #f0f9eb; }
.col-match.lose { background: #fef0f0; }

/* 头像区域 */
.cm-champ {
  position: relative; width: 34px; height: 34px; flex-shrink: 0;
}
.cm-champ-img {
  width: 34px; height: 34px; border-radius: 50%;
  overflow: hidden; border: 1px solid #dcdfe6;
}
.cm-level {
  position: absolute; bottom: -2px; right: -2px;
  width: 13px; height: 13px; line-height: 11px;
  background: #202124; color: white; border-radius: 50%;
  font-size: 0.58rem; font-weight: bold; text-align: center;
  border: 1px solid #fff;
}

/* 右侧详情 */
.cm-detail {
  display: flex; flex-direction: column; min-width: 0; flex: 1; gap: 2px;
}
.cm-mode {
  font-size: 0.75rem; font-weight: 600; color: #303133;
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
}
.cm-bottom {
  display: flex; justify-content: space-between; align-items: center; gap: 4px;
}
.cm-kda { font-size: 0.75rem; font-weight: 600; color: #606266; }
.cm-date { font-size: 0.68rem; color: #bbb; white-space: nowrap; }
</style>
