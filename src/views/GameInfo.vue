<script setup lang="ts">
import { ref, inject, type Ref } from "vue";
import { useLcuStore } from "../store/lcuStore";
import type { AppConfig } from "../api/lcu";
import { usePremadeGroup } from "../composables/usePremadeGroup";
import { useGamePlayerData } from "../composables/useGamePlayerData";
import PlayerCard from "../components/gameinfo/PlayerCard.vue";
import PremadeLegend from "../components/gameinfo/PremadeLegend.vue";
import PlayerMatchColumn from "../components/gameinfo/PlayerMatchColumn.vue";

const store = useLcuStore();
const activeTab = ref<"my" | "their">("my");

// 应用配置（用于获取对局卡片颜色）
const appConfig =
  inject<Ref<AppConfig | null>>("appConfig") || ref<AppConfig | null>(null);

const premadeColorsMy = ref<Record<number, number>>({});
const premadeColorsTheir = ref<Record<number, number>>({});

const {
  playerData,
  sessionAllyTeam,
  sessionEnemyTeam,
  isTftMode,
  myTeam,
  theirTeam,
  currentTeam,
  shouldShowContent,
} = useGamePlayerData(
  appConfig,
  premadeColorsMy,
  premadeColorsTheir,
  activeTab,
);

const {
  getPremadeIdx,
  getPremadeCardStyle,
  hasAnyPremadeInfo,
  premadeRows,
} = usePremadeGroup(
  myTeam,
  theirTeam,
  sessionAllyTeam,
  sessionEnemyTeam,
  playerData,
  premadeColorsMy,
  premadeColorsTheir,
);
</script>

<template>
  <div class="game-info">
    <div v-if="!store.isConnected" class="tip-container">
      <div class="offline-logo">🎮</div>
      <p class="tip">{{ $t("gameInfo.launchLolPrompt") }}</p>
    </div>

    <div v-else-if="isTftMode" class="tip-container">
      <div class="offline-logo">♟️</div>
      <p class="tip">云顶之弈对局中，对局信息页面不显示数据</p>
    </div>

    <div v-else-if="!shouldShowContent" class="tip-container">
      <div class="offline-logo">⏳</div>
      <p class="tip">{{ $t("gameInfo.awaitingLoad") }}</p>
    </div>

    <div v-else class="game-layout">
      <!-- 左侧：队伍切换 + 玩家列表 -->
      <div class="left-panel">
        <div class="team-tabs">
          <button
            :class="['tab-btn', { active: activeTab === 'my' }]"
            @click="activeTab = 'my'"
          >
            {{ $t("gameInfo.myTeam", { count: myTeam.length }) }}
          </button>
          <button
            :class="['tab-btn', { active: activeTab === 'their' }]"
            @click="activeTab = 'their'"
          >
            {{ $t("gameInfo.theirTeam", { count: theirTeam.length }) }}
          </button>
        </div>

        <div class="player-list">
          <PlayerCard
            v-for="(p, i) in currentTeam"
            :key="p.cellId ?? i"
            :player="p"
            :player-data="playerData[p.cellId]"
            :premade-idx="getPremadeIdx(p.summonerId, activeTab)"
            :active-tab="activeTab"
            :premade-card-style="getPremadeCardStyle(p.summonerId, activeTab)"
          />
          <div v-if="currentTeam.length === 0" class="tip">
            {{ $t("gameInfo.noTeamData") }}
          </div>
        </div>

        <!-- 组队图例 (按行交错配对) -->
        <PremadeLegend
          v-if="hasAnyPremadeInfo"
          :premade-rows="premadeRows"
        />
      </div>

      <!-- 右侧：5 列战绩 -->
      <div class="right-panel">
        <div class="columns-container">
          <PlayerMatchColumn
            v-for="(p, i) in currentTeam"
            :key="p.cellId ?? i"
            :player="p"
            :player-data="playerData[p.cellId]"
            :index="i"
            :app-config="appConfig"
          />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.game-info {
  padding: 1rem 1.5rem 1rem 0.6rem;
  background-color: transparent;
  flex: 1;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.tip-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 6rem 2rem;
  color: var(--text-muted);
  flex: 1;
}
.offline-logo {
  font-size: 3rem;
  margin-bottom: 1rem;
}
.tip {
  font-size: 0.95rem;
  color: var(--text-dimmed);
  margin: 0;
}

/* 左右分栏 */
.game-layout {
  display: grid;
  grid-template-columns: 240px 1fr;
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
</style>
