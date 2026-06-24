<script setup lang="ts">
import { ref, watch, computed } from "vue";
import { useLcuStore } from "../store/lcuStore";

const store = useLcuStore();
const loading = ref(false);
const error = ref("");
const players = ref<any[]>([]);

// 从 champSelectSession 中提取 myTeam + theirTeam
const myTeam = computed(() => store.champSelectSession?.myTeam ?? []);
const theirTeam = computed(() => store.champSelectSession?.theirTeam ?? []);

async function loadPlayerData() {
  if (!store.champSelectSession) {
    error.value = "当前不在选人阶段";
    return;
  }
  loading.value = true;
  error.value = "";
  try {
    // 占位：实际需要调用 get_game_player_summaries
    players.value = [...myTeam.value, ...theirTeam.value];
  } catch (e: any) {
    error.value = e.toString();
  } finally {
    loading.value = false;
  }
}

// 监听选人阶段自动加载
watch(() => store.gamePhase, (phase: string) => {
  if (phase === "ChampSelect") {
    loadPlayerData();
  }
});

</script>

<template>
  <div class="game-info">
    <h2>⚔️ 对局辅助</h2>

    <div v-if="!store.isConnected" class="tip">
      请先启动英雄联盟客户端
    </div>

    <div v-else>
      <!-- 状态栏 -->
      <div class="status-bar">
        <span :class="['phase-badge', store.gamePhase.toLowerCase()]">
          {{ store.gamePhase === 'ChampSelect' ? '选人阶段' :
             store.gamePhase === 'InProgress' ? '游戏中' :
             store.gamePhase === 'ReadyCheck' ? '准备确认' :
             store.gamePhase === 'Lobby' ? '房间大厅' :
             store.gamePhase === 'Matchmaking' ? '匹配中' :
             store.gamePhase === 'EndOfGame' ? '对局结束' : '等待中' }}
        </span>
        <button v-if="store.gamePhase === 'ChampSelect'" @click="loadPlayerData" :disabled="loading">
          {{ loading ? "加载中..." : "刷新数据" }}
        </button>
      </div>

      <div v-if="error" class="error">{{ error }}</div>

      <!-- 选人阶段：十人面板 -->
      <div v-if="store.gamePhase === 'ChampSelect'" class="teams">
        <!-- 己方 -->
        <div class="team ally">
          <h3>己方队伍</h3>
          <div v-for="(p, i) in myTeam" :key="i" class="player-card ally-card">
            <div class="player-champ">
              <div class="champ-placeholder">{{ p.championId || '?' }}</div>
            </div>
            <div class="player-info">
              <span class="player-name">召唤师 {{ i + 1 }}</span>
              <span class="player-detail">英雄ID: {{ p.championId || '未选择' }}</span>
            </div>
            <div class="player-stats">
              <div class="stat-item">
                <span class="stat-label">段位</span>
                <span class="stat-value rank-placeholder">—</span>
              </div>
              <div class="stat-item">
                <span class="stat-label">胜率</span>
                <span class="stat-value">—</span>
              </div>
              <div class="stat-item">
                <span class="stat-label">近期KDA</span>
                <span class="stat-value">—</span>
              </div>
            </div>
            <div class="fate-badge" title="上局关系">
              <span>—</span>
            </div>
          </div>
          <div v-if="myTeam.length === 0" class="tip">暂无队友数据</div>
        </div>

        <!-- 敌方 -->
        <div class="team enemy">
          <h3>敌方队伍</h3>
          <div v-for="(p, i) in theirTeam" :key="i" class="player-card enemy-card">
            <div class="player-champ">
              <div class="champ-placeholder">{{ p.championId || '?' }}</div>
            </div>
            <div class="player-info">
              <span class="player-name">对手 {{ i + 1 }}</span>
              <span class="player-detail">英雄ID: {{ p.championId || '未选择' }}</span>
            </div>
            <div class="player-stats">
              <div class="stat-item">
                <span class="stat-label">段位</span>
                <span class="stat-value rank-placeholder">—</span>
              </div>
              <div class="stat-item">
                <span class="stat-label">胜率</span>
                <span class="stat-value">—</span>
              </div>
            </div>
          </div>
          <div v-if="theirTeam.length === 0" class="tip">暂无对手数据</div>
        </div>
      </div>

      <!-- 非选人阶段 -->
      <div v-else class="waiting">
        <div class="waiting-icon">🎮</div>
        <p>进入选人阶段后将自动加载十人对局信息</p>
        <p class="sub">包含：段位、胜率、近期KDA、常用英雄、上局关系标记</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.game-info { padding: 1rem; }
.tip { color: #888; text-align: center; padding: 1rem; }
.error { color: #e74c3c; margin-bottom: 1rem; }

.status-bar {
  display: flex; align-items: center; gap: 12px;
  margin-bottom: 1rem; padding: 10px 14px;
  background: #f5f5f5; border-radius: 8px;
}
.phase-badge {
  padding: 4px 12px; border-radius: 12px;
  font-size: 0.85rem; font-weight: 600; color: white;
}
.phase-badge.champselect { background: #1976d2; }
.phase-badge.inprogress { background: #388e3c; }
.phase-badge.readycheck { background: #f57c00; }
.phase-badge.lobby { background: #7b1fa2; }
.phase-badge.matchmaking { background: #0097a7; }
.phase-badge.endofgame { background: #616161; }
.phase-badge.none { background: #9e9e9e; }

.teams { display: flex; gap: 16px; }
.team { flex: 1; }
.team h3 { margin: 0 0 8px; font-size: 1rem; }

.player-card {
  display: flex; align-items: center; gap: 10px;
  padding: 10px; margin-bottom: 6px; border-radius: 6px;
  border-left: 3px solid;
}
.ally-card { background: #e3f2fd; border-color: #1976d2; }
.enemy-card { background: #fce4ec; border-color: #d32f2f; }

.player-champ .champ-placeholder {
  width: 36px; height: 36px; border-radius: 4px;
  background: #e0e0e0; display: flex; align-items: center; justify-content: center;
  font-size: 0.7rem; color: #666;
}
.player-info { display: flex; flex-direction: column; }
.player-name { font-weight: 600; font-size: 0.85rem; }
.player-detail { font-size: 0.75rem; color: #888; }
.player-stats { display: flex; gap: 12px; margin-left: auto; }
.stat-item { text-align: center; }
.stat-label { display: block; font-size: 0.65rem; color: #999; }
.stat-value { font-size: 0.8rem; font-weight: 600; }
.fate-badge { margin-left: 8px; font-size: 0.85rem; }

.waiting {
  text-align: center; padding: 4rem 1rem; color: #666;
}
.waiting-icon { font-size: 3rem; margin-bottom: 1rem; }
.sub { color: #aaa; font-size: 0.85rem; }

button {
  padding: 6px 16px; border: none; border-radius: 6px;
  background: #1976d2; color: white; cursor: pointer;
}
button:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
