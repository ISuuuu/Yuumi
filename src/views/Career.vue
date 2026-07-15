<script setup lang="ts">
import { ref, inject, watch, provide, type Ref } from "vue";
import { useLcuStore } from "../store/lcuStore";
import MatchHistoryTab from "../components/career/MatchHistoryTab.vue";
import LootManagerTab from "../components/career/LootManagerTab.vue";
import LcuImage from "../components/LcuImage.vue";
import { useMatchHistory } from "../composables/useMatchHistory";

const store = useLcuStore();
const currentTab = ref("matches");

const {
  summoner, matches, recentMatches, rankedQueues, loading, copied,
  loadSummoner, loadMatches, loadRecentMatches, loadRankedStats,
  copyRiotId, clearCache, fetchMatchHistoryWithFallback,
} = useMatchHistory();

// 向子组件 provide 共享的 composable 状态（避免重复实例化）
provide("matchHistoryState", {
  summoner, matches, recentMatches, rankedQueues, loading,
  loadSummoner, loadMatches, loadRecentMatches, loadRankedStats,
  clearCache, fetchMatchHistoryWithFallback,
});

// 自动加载召唤师数据
watch(
  () => store.isConnected,
  (connected) => {
    if (connected) {
      loadSummoner();
    } else {
      summoner.value = null;
      clearCache();
    }
  },
  { immediate: true },
);

// 对局结束后自动刷新召唤师数据
watch(
  () => store.gamePhase,
  async (phase: string, oldPhase: string | undefined) => {
    if (!summoner.value?.puuid) return;
    const gamePhases = ["InProgress", "GameStart", "ChampSelect", "ReadyCheck", "PreEndOfGame"];
    const endPhases = ["EndOfGame", "Lobby", "None"];
    if (gamePhases.includes(oldPhase ?? "") && endPhases.includes(phase ?? "")) {
      await new Promise((r) => setTimeout(r, 2000));
      await loadSummoner(true);
    }
  },
);

// 从 App.vue 注入 → Search 跳转状态
const navigateSearchPayload = inject<
  Ref<{ name: string; gameId: number | null } | null>
>("navigateSearchPayload")!;
const navigateTo = inject<(page: string) => void>("navigateTo");

function goToHistory() {
  if (!summoner.value) return;
  const name = summoner.value.gameName || summoner.value.displayName;
  const fullName = summoner.value.tagLine
    ? `${name}#${summoner.value.tagLine}`
    : name;
  navigateSearchPayload.value = { name: fullName, gameId: -1 };
  navigateTo?.("search");
}

// 供 LootManagerTab 开启战利品后刷新召唤师数据
function refreshSummoner() {
  loadSummoner(true);
}
</script>

<template>
  <div class="career">
    <!-- 未连接 LCU 时显示提示 -->
    <div v-if="!store.isConnected" class="tip-container">
      <div class="offline-logo">🎮</div>
      <p class="tip">{{ $t("gameInfo.launchLolPrompt") }}</p>
    </div>

    <div v-else class="career-content">
      <!-- 召唤师信息卡片 -->
      <div v-if="summoner" class="summoner-header">
        <div class="profile-center">
          <div class="profile-icon-wrapper">
            <svg class="gauge-ring-svg" viewBox="0 0 100 100">
              <circle class="gauge-track" cx="50" cy="50" r="45" />
              <circle
                class="gauge-progress"
                cx="50" cy="50" r="45"
                :style="{ '--progress': summoner.percentCompleteForNextLevel }"
              />
            </svg>
            <div class="avatar-container">
              <LcuImage
                :src="summoner.profileIconUrl"
                class="profile-avatar"
                alt="avatar"
              />
            </div>
            <div class="level-badge">{{ summoner.summonerLevel }}</div>
          </div>

          <div class="summoner-info">
            <h1 class="display-name">
              {{ summoner.gameName || summoner.displayName }}
            </h1>
            <div class="copy-wrapper">
              <button
                class="copy-riot-id-btn"
                @click="copyRiotId"
                :title="`复制: ${summoner.gameName || summoner.displayName}#${summoner.tagLine}`"
              >
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="copy-icon">
                  <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                  <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                </svg>
              </button>
              <span v-if="copied" class="copied-text">✓ {{ $t("career.copied") }}</span>
            </div>
            <span class="tagline"># {{ summoner.tagLine }}</span>
          </div>
        </div>

        <div class="header-actions">
          <button class="action-btn" @click="loadSummoner(true)" :disabled="loading">
            {{ $t("career.refresh") }}
          </button>
          <button class="action-btn" @click="goToHistory" :disabled="loading">
            {{ $t("career.historyBtn") }}
          </button>
        </div>
      </div>

      <!-- Tab 导航栏 -->
      <div v-if="summoner" class="career-tabs">
        <div
          :class="['career-tab-item', { active: currentTab === 'matches' }]"
          @click="currentTab = 'matches'"
        >
          生涯战绩
        </div>
        <div
          :class="['career-tab-item', { active: currentTab === 'loot' }]"
          @click="currentTab = 'loot'"
        >
          战利品
        </div>
      </div>

      <!-- 视图内容：使用 v-show 保留切换状态 -->
      <template v-if="summoner">
        <MatchHistoryTab
          v-show="currentTab === 'matches'"
        />
        <LootManagerTab
          v-show="currentTab === 'loot'"
          :refresh-summoner="refreshSummoner"
        />
      </template>
    </div>
  </div>
</template>

<style scoped>
.career {
  padding: 1rem 1.5rem 1rem 0.6rem;
  background-color: transparent;
  flex: 1;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.career-content {
  max-width: 1000px;
  width: 100%;
  margin: 0 auto;
  animation: fadeIn 0.3s ease-out;
  flex: 1;
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

.career-tabs {
  display: flex;
  gap: 8px;
  margin: 12px 0 16px 0;
  border-bottom: 1px solid var(--border-color);
  padding-bottom: 8px;
}

.career-tab-item {
  font-size: 0.88rem;
  font-weight: 700;
  color: var(--text-muted);
  padding: 6px 16px;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s ease;
  background: transparent;
}

.career-tab-item:hover {
  color: var(--primary-color);
  background: var(--card-bg-hover);
}

.career-tab-item.active {
  color: var(--primary-color);
  background: var(--primary-color-alpha-10);
  box-shadow: inset 0 0 0 1px var(--primary-color-alpha-20);
}

/* 召唤师头部卡片 */
.summoner-header {
  display: grid;
  grid-template-columns: 1fr auto 1fr;
  align-items: center;
  padding: 1.5rem;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  margin-bottom: 1.5rem;
  box-shadow: var(--shadow-sm);
  transition: all 0.25s ease;
}

.profile-center {
  display: flex;
  align-items: center;
  gap: 1.2rem;
  justify-content: center;
  grid-column: 2;
}

.summoner-header:hover {
  border-color: var(--primary-color-alpha-30);
  box-shadow: var(--shadow-md);
  background-color: var(--card-bg-hover);
}

.profile-icon-wrapper {
  position: relative;
  width: 104px;
  height: 104px;
  flex-shrink: 0;
}

.gauge-ring-svg {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  overflow: visible;
}

.gauge-track,
.gauge-progress {
  fill: none;
  stroke-width: 5;
  stroke-linecap: round;
  stroke-dasharray: 235.62 282.74;
  transform: rotate(120deg);
  transform-origin: center;
}

.gauge-track {
  stroke: #d9d9d9;
}

.gauge-progress {
  stroke: var(--primary-color);
  stroke-dashoffset: calc(235.62px * (1 - var(--progress) / 100));
  transition: stroke-dashoffset 0.8s ease;
}

.avatar-container {
  position: absolute;
  inset: 13px;
  border-radius: 50%;
  overflow: hidden;
}

.profile-avatar {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.level-badge {
  position: absolute;
  bottom: 0;
  left: 50%;
  transform: translateX(-50%);
  color: var(--text-color);
  font-size: 0.72rem;
  font-weight: 700;
  z-index: 2;
  white-space: nowrap;
  line-height: 1;
}

[data-theme="dark"] .level-badge {
  color: var(--text-color);
}

.summoner-info {
  display: grid;
  grid-template-columns: 1fr auto;
  align-items: center;
  gap: 4px 8px;
  flex: 1;
}

.display-name {
  grid-column: 1;
  grid-row: 1;
  font-size: 1.6rem;
  font-weight: 800;
  color: var(--text-color);
  margin: 0;
  text-align: center;
}

.tagline {
  grid-column: 1;
  grid-row: 2;
  font-size: 0.85rem;
  color: var(--text-muted);
  text-align: center;
}

.copy-wrapper {
  grid-column: 2;
  grid-row: 1 / 3;
  align-self: center;
  justify-self: end;
  position: relative;
  display: flex;
  align-items: center;
}

.copy-riot-id-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  background: rgba(0, 0, 0, 0.02);
  cursor: pointer;
  color: var(--text-muted);
  transition: all 0.2s;
  padding: 0;
}

.copy-riot-id-btn:hover {
  background: rgba(0, 0, 0, 0.05);
  color: var(--text-color);
  border-color: var(--primary-color);
}

.copy-icon {
  width: 14px;
  height: 14px;
}

.copied-text {
  position: absolute;
  left: calc(100% + 6px);
  top: 50%;
  transform: translateY(-50%);
  font-size: 0.75rem;
  color: var(--win-color);
  font-weight: 600;
  white-space: nowrap;
}

.header-actions {
  display: flex;
  flex-direction: column;
  gap: 8px;
  grid-column: 3;
  justify-self: end;
}

.action-btn {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  color: var(--text-color);
  padding: 6px 16px;
  border-radius: 6px;
  font-size: 0.82rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  box-sizing: border-box;
}

.action-btn:hover {
  background: var(--card-bg-hover);
  color: var(--text-color);
  border-color: var(--primary-color);
}

.action-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
  pointer-events: none;
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

@keyframes fadeIn {
  from { opacity: 0; transform: translateY(6px); }
  to { opacity: 1; transform: translateY(0); }
}
</style>
