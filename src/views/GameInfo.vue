<script setup lang="ts">
import { ref, computed, inject, type Ref } from "vue";
import { onMounted, watch } from "vue";
import { useLcuStore } from "../store/lcuStore";
import type { AppConfig, SavedPlayerMarker } from "../api/lcu";
import { querySavedPlayersMap } from "../api/lcu";
import {
  PREMADE_COLORS,
  getChampionIcon,
  type PlayerData,
  type PremadePlayerLike,
} from "../types/gameInfo";
import { usePremadeGroup } from "../composables/usePremadeGroup";
import { useGamePlayerData, isIdentityCompatible } from "../composables/useGamePlayerData";
import PlayerCard from "../components/gameinfo/PlayerCard.vue";
import LcuOfflineState from "../components/LcuOfflineState.vue";
import PlayerMatchColumn from "../components/gameinfo/PlayerMatchColumn.vue";
import LcuImage from "../components/LcuImage.vue";

const store = useLcuStore();
const activeTab = ref<"my" | "their">("my");
const viewMode = ref<"ten" | "five">("ten");

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
  currentSummonerPuuid,
} = useGamePlayerData(
  appConfig,
  premadeColorsMy,
  premadeColorsTheir,
  activeTab,
);

const {
  getPremadeIdx,
  getPremadeCardStyle,
  myPremadeGroups,
  theirPremadeGroups,
} = usePremadeGroup(
  myTeam,
  theirTeam,
  sessionAllyTeam,
  sessionEnemyTeam,
  playerData,
  premadeColorsMy,
  premadeColorsTheir,
);

// 选人阶段敌我 cellId 会碰撞：直接按 key 命中可能拿到异队旧数据，
// 必须先核验身份，不一致就跳过（宁可显示占位，也不显示别人的 ID 和战绩）
function isSameIdentity(p: PremadePlayerLike, d: PlayerData | undefined): boolean {
  if (!d) return false;
  if (!d.info) return true; // 加载中占位：保留原行为（显示 loading）
  // 匿名查询（选人敌方未公开）不得借用任何实名数据
  const dSidReal = d.info.summonerId && d.info.summonerId !== p.cellId ? d.info.summonerId : 0;
  if (!p.puuid && !p.summonerId && (d.info.puuid || dSidReal)) return false;
  return isIdentityCompatible(d, { puuid: p.puuid, summonerId: p.summonerId, cellId: p.cellId });
}

function getPlayerData(p: PremadePlayerLike, idx: number) {
  // 身份优先查找：puuid → summonerId → cellId（与数据层顺序一致，身份键优先命中同一玩家）
  if (p.puuid) {
    const byPuuid = playerData.value[p.puuid];
    if (byPuuid && isSameIdentity(p, byPuuid)) return byPuuid;
  }
  if (p.summonerId) {
    const bySid = playerData.value[p.summonerId];
    if (bySid && isSameIdentity(p, bySid)) return bySid;
  }
  if (p.cellId !== undefined) {
    const byCell = playerData.value[p.cellId];
    if (byCell && isSameIdentity(p, byCell)) return byCell;
  }
  // 仅在明确为我方队伍且未设置 cellId 时，才允许尝试索引兜底，防止 5 列敌方队伍（idx 0..4）误查我方槽位
  const isAlly = (myTeam.value as PremadePlayerLike[]).includes(p);
  if (isAlly && p.cellId === undefined) {
    const byIdx = playerData.value[idx];
    if (byIdx && isSameIdentity(p, byIdx)) return byIdx;
  }
  // 若按真实 stableCellId 或 idx 索引均未直接命中（如 5 列切 Tab 或 10 列合并展示时）：
  // 尝试在 playerData 中按队员已知身份（puuid / summonerId / displayName）遍历反查
  for (const key of Object.keys(playerData.value)) {
    const d = playerData.value[key];
    if (!d?.info) continue;
    if (p.puuid && d.info.puuid && d.info.puuid === p.puuid) return d;
    if (p.summonerId && d.info.summonerId && d.info.summonerId === p.summonerId) return d;
    const pName = p.displayName || p.gameName || p.summonerName;
    const dName = d.info.displayName || d.info.gameName;
    if (pName && dName && pName === dName) return d;
  }
  return undefined;
}

// 保存玩家映射：puuid → { tag, encounterCount }，用于玩家卡片旁标记"曾同局"
const savedPlayerMap = ref<Record<string, SavedPlayerMarker>>({});

// 敌方队伍是否已公开可用（选人阶段敌方通常不可见；自定义/人机模式公开可见）
const isTheirTeamRevealed = computed(() => {
  if (theirTeam.value.length === 0) return false;
  if (store.gamePhase === "ChampSelect") {
    return theirTeam.value.some(
      (p: PremadePlayerLike) =>
        Boolean(
          p.summonerId ||
          p.puuid ||
          p.championId ||
          p.botChampionId ||
          p.bot ||
          p.isBot ||
          p.displayName ||
          p.summonerName,
        ),
    );
  }
  return true;
});

// 10 列并排模式：两队列合并渲染（我方在前，敌方在后）
// 选人阶段敌方信息尚未公开时，10 列模式也不展示敌方占位，只展示我方
const allPlayers = computed(() => {
  if (!isTheirTeamRevealed.value) {
    return myTeam.value;
  }
  return [...myTeam.value, ...theirTeam.value];
});

// in-flight 去重：挂载/连接/选人等多触发源同时到达时只发一次查询
let savedPlayerMapInflight: Promise<void> | null = null;

async function loadSavedPlayerMap() {
  const puuid = currentSummonerPuuid.value;
  if (!puuid || !store.isConnected) return;
  if (savedPlayerMapInflight) return;
  savedPlayerMapInflight = (async () => {
    try {
      savedPlayerMap.value = await querySavedPlayersMap(puuid);
    } catch (e) {
      console.error("[GameInfo] 保存玩家映射加载失败:", e);
    } finally {
      savedPlayerMapInflight = null;
    }
  })();
}

watch(() => store.gamePhase, (phase) => {
  if (phase === "ChampSelect") loadSavedPlayerMap();
});
watch(() => store.isConnected, () => loadSavedPlayerMap());
watch(currentSummonerPuuid, () => loadSavedPlayerMap());
onMounted(() => {
  loadSavedPlayerMap();
});
</script>

<template>
  <div class="game-info">
    <LcuOfflineState v-if="!store.isConnected" />

    <div v-else-if="isTftMode" class="tip-container">
      <div class="offline-logo">♟️</div>
      <p class="tip">云顶之弈对局中，对局信息页面不显示数据</p>
    </div>

    <div v-else-if="!shouldShowContent" class="tip-container">
      <div class="offline-logo">⏳</div>
      <p class="tip">{{ $t("gameInfo.awaitingLoad") }}</p>
    </div>

    <div v-else class="game-layout" :class="{ 'layout-ten': viewMode === 'ten' }">
      <!-- 左侧：队伍切换 + 玩家列表（10 列模式隐藏） -->
      <div v-if="viewMode === 'five'" class="left-panel">
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
            {{ $t("gameInfo.theirTeam", { count: isTheirTeamRevealed ? theirTeam.length : 0 }) }}
          </button>
        </div>

        <div class="player-list">
          <PlayerCard
            v-for="(p, i) in currentTeam"
            :key="p.cellId ?? i"
            :player="p"
            :player-data="getPlayerData(p, i)"
            :premade-idx="getPremadeIdx(p, activeTab)"
            :active-tab="activeTab"
            :premade-card-style="getPremadeCardStyle(p, activeTab)"
            :saved-map="savedPlayerMap"
            :self-puuid="currentSummonerPuuid"
            :index="i"
          />
          <div v-if="currentTeam.length === 0" class="tip">
            {{ $t("gameInfo.noTeamData") }}
          </div>
        </div>
      </div>

      <!-- 右侧：5 列/10 列战绩 -->
      <div class="right-panel">
        <div class="view-toolbar">
          <!-- 10 列视图且双方队伍均有数据且敌方已公开：左右精准 50% 对称阵营与组队信息 -->
          <template v-if="viewMode === 'ten' && isTheirTeamRevealed">
            <!-- 我方区域（占左侧 50%） -->
            <div class="ten-toolbar-left">
              <span class="side-pill ally-pill">
                <span class="pill-dot"></span>
                {{ $t("gameInfo.myTeam", { count: myTeam.length }) }}
              </span>
              <!-- 我方组队芯片 -->
              <div v-if="myPremadeGroups.length > 0" class="premade-chips-wrapper">
                <div
                  v-for="group in myPremadeGroups"
                  :key="group.colorIdx"
                  class="premade-group-chip"
                  :style="{
                    borderColor: PREMADE_COLORS[group.colorIdx % PREMADE_COLORS.length].border,
                    backgroundColor: PREMADE_COLORS[group.colorIdx % PREMADE_COLORS.length].bg,
                  }"
                  :title="$t('gameInfo.premadeIdx', { idx: group.colorIdx + 1 })"
                >
                  <span
                    class="legend-dot"
                    :style="{
                      background: PREMADE_COLORS[group.colorIdx % PREMADE_COLORS.length].dot,
                    }"
                  ></span>
                  <div class="premade-avatars">
                    <template v-for="m in group.members" :key="m.summonerId">
                      <LcuImage
                        v-if="m.championId > 0"
                        :src="getChampionIcon(m.championId)"
                        class="premade-avatar"
                        :title="m.displayName"
                      />
                      <div v-else class="premade-avatar premade-avatar-empty" :title="m.displayName">
                        {{ m.displayName ? m.displayName.slice(0, 1) : '?' }}
                      </div>
                    </template>
                  </div>
                </div>
              </div>
            </div>

            <!-- 中间精准绝对居中 50% 轴线的 VS 分隔指示 -->
            <div class="toolbar-center-vs">
              <span class="side-vs">VS</span>
            </div>

            <!-- 敌方区域（占右侧 50%） -->
            <div class="ten-toolbar-right">
              <!-- 敌方组队芯片 -->
              <div v-if="theirPremadeGroups.length > 0" class="premade-chips-wrapper">
                <div
                  v-for="group in theirPremadeGroups"
                  :key="group.colorIdx"
                  class="premade-group-chip"
                  :style="{
                    borderColor: PREMADE_COLORS[group.colorIdx % PREMADE_COLORS.length].border,
                    backgroundColor: PREMADE_COLORS[group.colorIdx % PREMADE_COLORS.length].bg,
                  }"
                  :title="$t('gameInfo.premadeIdx', { idx: group.colorIdx + 1 })"
                >
                  <span
                    class="legend-dot"
                    :style="{
                      background: PREMADE_COLORS[group.colorIdx % PREMADE_COLORS.length].dot,
                    }"
                  ></span>
                  <div class="premade-avatars">
                    <template v-for="m in group.members" :key="m.summonerId">
                      <LcuImage
                        v-if="m.championId > 0"
                        :src="getChampionIcon(m.championId)"
                        class="premade-avatar"
                        :title="m.displayName"
                      />
                      <div v-else class="premade-avatar premade-avatar-empty" :title="m.displayName">
                        {{ m.displayName ? m.displayName.slice(0, 1) : '?' }}
                      </div>
                    </template>
                  </div>
                </div>
              </div>

              <!-- 敌方阵营指示与切换按钮容器 -->
              <div class="ten-right-controls">
                <span class="side-pill enemy-pill">
                  <span class="pill-dot"></span>
                  {{ $t("gameInfo.theirTeam", { count: theirTeam.length }) }}
                </span>

                <!-- 视图模式分段切换器 (10 列模式) -->
                <div class="view-segmented-control">
                  <button
                    class="segment-btn"
                    :class="{ active: false }"
                    @click="viewMode = 'five'"
                  >
                    <svg class="segment-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2" />
                      <circle cx="9" cy="7" r="4" />
                      <path d="M23 21v-2a4 4 0 0 0-3-3.87" />
                      <path d="M16 3.13a4 4 0 0 1 0 7.75" />
                    </svg>
                    <span>{{ $t("gameInfo.showTeam") }}</span>
                  </button>
                  <button
                    class="segment-btn"
                    :class="{ active: true }"
                    @click="viewMode = 'ten'"
                  >
                    <svg class="segment-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <rect x="3" y="3" width="7" height="7" rx="1.5" />
                      <rect x="14" y="3" width="7" height="7" rx="1.5" />
                      <rect x="14" y="14" width="7" height="7" rx="1.5" />
                      <rect x="3" y="14" width="7" height="7" rx="1.5" />
                    </svg>
                    <span>{{ $t("gameInfo.showAll") }}</span>
                  </button>
                </div>
              </div>
            </div>
          </template>

          <!-- 5 列视图 或 选人阶段敌方为空时的单方 5 列/10 列视图 -->
          <template v-else>
            <div class="five-toolbar-content">
              <!-- 我方组队区 -->
              <div class="five-team-premade">
                <span
                  class="side-pill ally-pill"
                  :class="{ 'clickable-pill': viewMode === 'five', 'active-pill': viewMode === 'five' && activeTab === 'my' }"
                  :title="$t('gameInfo.myTeam', { count: myTeam.length })"
                  @click="viewMode === 'five' ? (activeTab = 'my') : null"
                >
                  <span class="pill-dot"></span>
                  {{ $t("gameInfo.myTeam", { count: myTeam.length }) }}
                </span>
                <div v-if="myPremadeGroups.length > 0" class="premade-chips-wrapper">
                  <div
                    v-for="group in myPremadeGroups"
                    :key="group.colorIdx"
                    class="premade-group-chip"
                    :style="{
                      borderColor: PREMADE_COLORS[group.colorIdx % PREMADE_COLORS.length].border,
                      backgroundColor: PREMADE_COLORS[group.colorIdx % PREMADE_COLORS.length].bg,
                    }"
                    :title="$t('gameInfo.premadeIdx', { idx: group.colorIdx + 1 })"
                  >
                    <span
                      class="legend-dot"
                      :style="{
                        background: PREMADE_COLORS[group.colorIdx % PREMADE_COLORS.length].dot,
                      }"
                    ></span>
                    <div class="premade-avatars">
                      <template v-for="m in group.members" :key="m.summonerId">
                        <LcuImage
                          v-if="m.championId > 0"
                          :src="getChampionIcon(m.championId)"
                          class="premade-avatar"
                          :title="m.displayName"
                        />
                        <div v-else class="premade-avatar premade-avatar-empty" :title="m.displayName">
                          {{ m.displayName ? m.displayName.slice(0, 1) : '?' }}
                        </div>
                      </template>
                    </div>
                  </div>
                </div>
              </div>

              <!-- VS 分隔指示与敌方组队区（仅当敌方有数据且已公开时显示） -->
              <template v-if="isTheirTeamRevealed">
                <!-- VS 分隔指示 -->
                <span class="side-vs">VS</span>

                <!-- 敌方组队区 -->
                <div class="five-team-premade">
                  <div v-if="theirPremadeGroups.length > 0" class="premade-chips-wrapper">
                    <div
                      v-for="group in theirPremadeGroups"
                      :key="group.colorIdx"
                      class="premade-group-chip"
                      :style="{
                        borderColor: PREMADE_COLORS[group.colorIdx % PREMADE_COLORS.length].border,
                        backgroundColor: PREMADE_COLORS[group.colorIdx % PREMADE_COLORS.length].bg,
                      }"
                      :title="$t('gameInfo.premadeIdx', { idx: group.colorIdx + 1 })"
                    >
                      <span
                        class="legend-dot"
                        :style="{
                          background: PREMADE_COLORS[group.colorIdx % PREMADE_COLORS.length].dot,
                        }"
                      ></span>
                      <div class="premade-avatars">
                        <template v-for="m in group.members" :key="m.summonerId">
                          <LcuImage
                            v-if="m.championId > 0"
                            :src="getChampionIcon(m.championId)"
                            class="premade-avatar"
                            :title="m.displayName"
                          />
                          <div v-else class="premade-avatar premade-avatar-empty" :title="m.displayName">
                            {{ m.displayName ? m.displayName.slice(0, 1) : '?' }}
                          </div>
                        </template>
                      </div>
                    </div>
                  </div>
                  <span
                    class="side-pill enemy-pill clickable-pill"
                    :class="{ 'active-pill': activeTab === 'their' }"
                    :title="$t('gameInfo.theirTeam', { count: isTheirTeamRevealed ? theirTeam.length : 0 })"
                    @click="activeTab = 'their'"
                  >
                    <span class="pill-dot"></span>
                    {{ $t("gameInfo.theirTeam", { count: isTheirTeamRevealed ? theirTeam.length : 0 }) }}
                  </span>
                </div>
              </template>
            </div>

            <!-- 视图模式分段切换器 (5 列模式或单方 10 列模式) -->
            <div class="view-segmented-control">
              <button
                class="segment-btn"
                :class="{ active: viewMode === 'five' }"
                @click="viewMode = 'five'"
              >
                <svg class="segment-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2" />
                  <circle cx="9" cy="7" r="4" />
                  <path d="M23 21v-2a4 4 0 0 0-3-3.87" />
                  <path d="M16 3.13a4 4 0 0 1 0 7.75" />
                </svg>
                <span>{{ $t("gameInfo.showTeam") }}</span>
              </button>
              <button
                class="segment-btn"
                :class="{ active: viewMode === 'ten' }"
                @click="viewMode = 'ten'"
              >
                <svg class="segment-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <rect x="3" y="3" width="7" height="7" rx="1.5" />
                  <rect x="14" y="3" width="7" height="7" rx="1.5" />
                  <rect x="14" y="14" width="7" height="7" rx="1.5" />
                  <rect x="3" y="14" width="7" height="7" rx="1.5" />
                </svg>
                <span>{{ $t("gameInfo.showAll") }}</span>
              </button>
            </div>
          </template>
        </div>

        <div
          class="columns-container"
          :class="{ 'columns-ten': viewMode === 'ten' }"
        >
          <PlayerMatchColumn
            v-for="(p, i) in (viewMode === 'ten' ? allPlayers : currentTeam)"
            :key="p.cellId ?? i"
            :player="p"
            :player-data="getPlayerData(p, i)"
            :index="i"
            :app-config="appConfig"
            :compact="viewMode === 'ten'"
            :side="viewMode === 'ten' ? (i < myTeam.length ? 'ally' : 'enemy') : (activeTab === 'my' ? 'ally' : 'enemy')"
            :premade-idx="getPremadeIdx(p, viewMode === 'ten' ? (i < myTeam.length ? 'my' : 'their') : activeTab)"
            :class="{ 'team-separator': viewMode === 'ten' && isTheirTeamRevealed && i === myTeam.length }"
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
  grid-template-columns: 260px 1fr;
  gap: 16px;
  flex: 1;
  min-height: 0;
}

/* 10 列并排模式：隐藏左侧面板，右侧占满整行 */
.game-layout.layout-ten {
  grid-template-columns: 1fr;
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
  display: flex;
  flex-direction: column;
}
.view-toolbar {
  position: relative;
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 6px 12px;
  border-bottom: 1px solid var(--border-color);
  background: rgba(0, 0, 0, 0.02);
  flex-shrink: 0;
  min-height: 44px;
  box-sizing: border-box;
}

/* 10 列视图阵营指示与组队芯片 */
.ten-toolbar-left {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 50%;
  padding-right: 28px;
  box-sizing: border-box;
  min-width: 0;
}

.toolbar-center-vs {
  position: absolute;
  left: 50%;
  top: 50%;
  transform: translate(-50%, -50%);
  z-index: 2;
  pointer-events: none;
}

.ten-toolbar-right {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  width: 50%;
  padding-left: 28px;
  box-sizing: border-box;
  min-width: 0;
}

.ten-right-controls {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
  margin-left: auto;
}

.five-toolbar-content {
  display: flex;
  align-items: center;
  gap: 12px;
  flex: 1;
  min-width: 0;
}

.five-team-premade {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.premade-chips-wrapper {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: nowrap;
  overflow: hidden;
}

.premade-group-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 2px 7px;
  border-width: 1px;
  border-style: solid;
  border-radius: 999px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.06);
  flex-shrink: 0;
  transition: all 0.2s ease;
  backdrop-filter: blur(4px);
  -webkit-backdrop-filter: blur(4px);
}
.premade-group-chip:hover {
  filter: brightness(1.1);
  transform: translateY(-1px);
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.12);
}

.legend-dot {
  display: inline-block;
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex-shrink: 0;
  box-shadow: 0 0 5px currentColor;
}

.premade-avatars {
  display: flex;
  align-items: center;
}

.premade-avatar {
  width: 20px;
  height: 20px;
  border-radius: 50%;
  object-fit: cover;
  border: 1.5px solid rgba(255, 255, 255, 0.8);
  box-sizing: border-box;
  margin-left: -4px;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.2);
}
.premade-avatar:first-child {
  margin-left: 0;
}

.premade-avatar-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.2);
  color: var(--text-dimmed);
  font-size: 0.6rem;
  font-weight: 800;
  border: 1.5px solid rgba(255, 255, 255, 0.8);
  box-sizing: border-box;
  margin-left: -4px;
}
.premade-avatar-empty:first-child {
  margin-left: 0;
}

.side-pill {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 0.7rem;
  font-weight: 700;
  padding: 3px 9px;
  border-radius: 999px;
  letter-spacing: 0.2px;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.04);
}
.side-pill.ally-pill {
  background: rgba(59, 130, 246, 0.12);
  color: #3b82f6;
  border: 1px solid rgba(59, 130, 246, 0.28);
}
.side-pill.enemy-pill {
  background: rgba(244, 63, 94, 0.12);
  color: #f43f5e;
  border: 1px solid rgba(244, 63, 94, 0.28);
}
.side-pill.clickable-pill {
  cursor: pointer;
  opacity: 0.65;
  transition: all 0.2s ease-in-out;
  user-select: none;
}
.side-pill.clickable-pill:hover {
  opacity: 0.9;
  transform: translateY(-1px);
}
.side-pill.clickable-pill.active-pill {
  opacity: 1;
  box-shadow: 0 0 8px currentColor;
  transform: translateY(-1px);
  font-weight: 800;
}

.pill-dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
}
.ally-pill .pill-dot {
  background: #3b82f6;
  box-shadow: 0 0 5px #3b82f6;
}
.enemy-pill .pill-dot {
  background: #f43f5e;
  box-shadow: 0 0 5px #f43f5e;
}
.side-vs {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 0.62rem;
  font-weight: 800;
  color: var(--text-dimmed);
  padding: 2px 7px;
  border-radius: 999px;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.08);
  letter-spacing: 0.5px;
}

/* 视图分段控制器 */
.view-segmented-control {
  display: inline-flex;
  align-items: center;
  background: var(--hover-bg);
  padding: 2px;
  border-radius: 8px;
  border: 1px solid var(--border-color);
  gap: 2px;
}
.segment-btn {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 4px 10px;
  border-radius: 6px;
  border: none;
  background: transparent;
  color: var(--text-muted);
  font-size: 0.72rem;
  font-weight: 700;
  cursor: pointer;
  transition: all 0.18s cubic-bezier(0.4, 0, 0.2, 1);
  white-space: nowrap;
}
.segment-btn:hover {
  color: var(--text-color);
}
.segment-btn.active {
  background: var(--card-bg);
  color: var(--primary-color);
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.08);
}
.segment-icon {
  width: 12px;
  height: 12px;
  flex-shrink: 0;
}

.columns-container {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  flex: 1;
  min-height: 0;
}
.columns-ten {
  grid-template-columns: repeat(10, 1fr);
}
.columns-ten :deep(.team-separator) {
  border-left: 2px solid var(--border-color);
  position: relative;
}
.columns-ten :deep(.team-separator)::before {
  content: "";
  position: absolute;
  top: 0;
  left: -2px;
  bottom: 0;
  width: 2px;
  background: linear-gradient(
    180deg,
    rgba(244, 63, 94, 0.6) 0%,
    var(--border-color) 40%,
    var(--border-color) 60%,
    rgba(59, 130, 246, 0.6) 100%
  );
}
</style>
