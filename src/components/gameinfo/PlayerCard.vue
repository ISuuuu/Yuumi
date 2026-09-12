<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  getChampionIcon,
  resolvePlayerChampionId,
  getPlayerMasteryDetail,
  type PlayerData,
  type PremadePlayerLike,
} from "../../types/gameInfo";
import { useLcuStore } from "../../store/lcuStore";
import { usePlayerSearch } from "../../composables/usePlayerSearch";
import type { SavedPlayerMarker } from "../../api/lcu";
import LcuImage from "../LcuImage.vue";

const props = defineProps<{
  player: PremadePlayerLike;
  playerData?: PlayerData;
  premadeIdx: number;
  activeTab: "my" | "their";
  premadeCardStyle: Record<string, string>;
  savedMap?: Record<string, SavedPlayerMarker>;
  selfPuuid?: string;
  index?: number;
}>();

const store = useLcuStore();
const { t } = useI18n();
const { getPlayerSearchName, handleCareerClick } = usePlayerSearch();

const resolvedChampId = computed(() => {
  const fromResolver = resolvePlayerChampionId(props.player, store.champSelectSession);
  if (fromResolver > 0) return fromResolver;
  if (props.playerData?.championId && props.playerData.championId > 0) {
    return props.playerData.championId;
  }
  return 0;
});

const masteryDetail = computed(() => {
  const detail = getPlayerMasteryDetail(props.playerData, resolvedChampId.value);
  if (props.playerData?.info?.puuid) {
    console.log(`[PlayerCard] 玩家 ${props.playerData?.info?.gameName} 熟练度详情:`, {
      resolvedChampId: resolvedChampId.value,
      masteryCount: props.playerData?.masteries?.length ?? 0,
      detail,
    });
  }
  return detail;
});

// 该玩家是否为"保存的玩家"（曾同局/打了标签），取 tag 与相遇次数；
// 当前登录召唤师自身不展示曾同局徽章
const savedInfo = computed(() => {
  const puuid = props.playerData?.info?.puuid;
  if (!puuid || !props.savedMap) return undefined;
  if (props.selfPuuid && puuid === props.selfPuuid) return undefined;
  return props.savedMap[puuid];
});

function savedBadgeTitle(info: SavedPlayerMarker): string {
  return info.tag
    ? t("gameInfo.savedPlayerTip", {
        count: info.encounterCount,
        tag: info.tag,
      })
    : t("gameInfo.savedPlayerTipNoTag", { count: info.encounterCount });
}

const TIER_MAP = computed<Record<string, string>>(() => ({
  NONE: "",
  IRON: t("tools.spoofTier.IRON"),
  BRONZE: t("tools.spoofTier.BRONZE"),
  SILVER: t("tools.spoofTier.SILVER"),
  GOLD: t("tools.spoofTier.GOLD"),
  PLATINUM: t("tools.spoofTier.PLATINUM"),
  EMERALD: t("tools.spoofTier.EMERALD"),
  DIAMOND: t("tools.spoofTier.DIAMOND"),
  MASTER: t("tools.spoofTier.MASTER"),
  GRANDMASTER: t("tools.spoofTier.GRANDMASTER"),
  CHALLENGER: t("tools.spoofTier.CHALLENGER"),
}));

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

const soloQueue = computed(() => props.playerData?.ranked?.solo);
const soloTier = computed(() => soloQueue.value?.tier);
const soloTierName = computed(() => {
  if (!soloTier.value || soloTier.value === "NONE") return t("gameInfo.noRank");
  return TIER_MAP.value[soloTier.value] || soloTier.value;
});
const soloDivision = computed(() => {
  const q = soloQueue.value;
  if (!q || !q.rank || q.rank === "NA" || q.tier === "NONE") return "";
  return q.rank;
});
const soloLp = computed(() => {
  const q = soloQueue.value;
  if (!q || q.leaguePoints === undefined || !q.tier || q.tier === "NONE") return "";
  return `${q.leaguePoints} LP`;
});
const soloStats = computed(() => {
  const q = soloQueue.value;
  if (!q || !q.tier || q.tier === "NONE") return null;
  const wins = q.wins || 0;
  if (wins <= 0) return null;
  return { wins };
});
</script>

<template>
  <div
    class="player-card"
    :class="{
      'premade-card': premadeIdx >= 0,
    }"
    :style="premadeCardStyle"
  >
    <!-- 第一层：上部主体（左侧头像 + 右侧召唤师与段位信息） -->
    <div class="pc-top-section">
      <div class="pc-avatar-area">
        <div
          class="profile-icon-wrapper-mini"
          :class="{ 'top-champ-ring': masteryDetail?.isTopChampion }"
        >
          <!-- 等级进度环形条 -->
          <svg class="gauge-ring-svg-mini" viewBox="0 0 100 100">
            <circle class="gauge-track-mini" cx="50" cy="50" r="45" />
            <circle
              class="gauge-progress-mini"
              cx="50"
              cy="50"
              r="45"
              :style="{
                '--progress':
                  playerData?.info?.percentCompleteForNextLevel || 0,
              }"
            />
          </svg>
          <div class="avatar-container-mini">
            <!-- 选人阶段/对局中：选了英雄或预选了英雄优先显示英雄头像，否则兜底显示召唤师头像 -->
            <template v-if="resolvedChampId > 0">
              <LcuImage
                :src="getChampionIcon(resolvedChampId)"
                class="profile-avatar-mini"
                alt="champ"
              />
            </template>
            <template v-else-if="playerData?.info?.profileIconUrl">
              <LcuImage
                :src="playerData.info.profileIconUrl"
                class="profile-avatar-mini"
                alt="summoner"
              />
            </template>
            <template v-else-if="player.profileIconId">
              <LcuImage
                :src="`/lol-game-data/assets/v1/profile-icons/${player.profileIconId}.jpg`"
                class="profile-avatar-mini"
                alt="summoner"
              />
            </template>
            <template v-else>
              <div class="profile-avatar-mini profile-avatar-empty-mini">
                ?
              </div>
            </template>
          </div>
          <!-- 等级数字 -->
          <div
            v-if="playerData?.info?.summonerLevel"
            class="level-badge-mini"
          >
            {{ playerData.info.summonerLevel }}
          </div>
        </div>
      </div>

      <div class="pc-info">
        <!-- 第 1 行：召唤师名称 + 宿命/标记 -->
        <div class="pc-row pc-name-row">
          <span
            class="name-text"
            :title="getPlayerSearchName(player, playerData) ? `${$t('nav.career')} ${getPlayerSearchName(player, playerData)}` : undefined"
            @click="(e) => handleCareerClick(e, player, playerData)"
          >{{
            playerData?.info?.gameName ||
            playerData?.info?.displayName ||
            player.displayName ||
            player.gameName ||
            player.summonerName ||
            $t("gameInfo.playerIndex", { index: (index ?? 0) + 1 })
          }}</span>

          <!-- 战绩隐藏锁图标 -->
          <span
            v-if="playerData?.matchHistoryHidden"
            class="pc-name-lock"
            :title="$t('gameInfo.matchHistoryHidden')"
          >
            <svg
              class="pc-name-lock-svg"
              viewBox="0 0 24 24"
              fill="currentColor"
            >
              <path
                fill-rule="evenodd"
                clip-rule="evenodd"
                d="M12 2a5 5 0 0 0-5 5v3H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-8a2 2 0 0 0-2-2h-1V7a5 5 0 0 0-5-5zm-3 5a3 3 0 0 1 6 0v3H9V7zm3 6a1.5 1.5 0 0 0-1 2.618V18a1 1 0 1 0 2 0v-2.382A1.5 1.5 0 0 0 12 13z"
              />
            </svg>
          </span>

          <div class="name-tags-group">
            <!-- 宿命徽标 -->
            <span
              v-if="playerData?.fateFlag"
              :class="['fate-badge', playerData.fateFlag]"
              :title="
                playerData.recentlyChampionName
                  ? (playerData.fateFlag === 'ally'
                    ? `${$t('gameInfo.fateAllyTitle')} (使用: ${playerData.recentlyChampionName})`
                    : `${$t('gameInfo.fateEnemyTitle')} (使用: ${playerData.recentlyChampionName})`)
                  : (playerData.fateFlag === 'ally'
                    ? $t('gameInfo.fateAllyTitle')
                    : $t('gameInfo.fateEnemyTitle'))
              "
            >{{
                playerData.fateFlag === "ally"
                  ? $t("gameInfo.fateAllyText")
                  : $t("gameInfo.fateEnemyText")
              }}</span
            >

            <!-- 标记玩家 -->
            <span
              v-if="savedInfo"
              :class="['saved-badge', { 'met-only': !savedInfo.tag }]"
              :title="savedBadgeTitle(savedInfo)"
            >{{ savedInfo.tag || $t("gameInfo.savedPlayerMark") }}</span>
          </div>
        </div>

        <!-- 第 2 行：单双排位段位（包含品质色高亮 + LP + 全赛季战绩） -->
        <div class="pc-row pc-rank-row">
          <template v-if="soloTier && soloTier !== 'NONE'">
            <span :class="['tier-text', soloTier]">{{ soloTierName }}{{ soloDivision ? ' ' + soloDivision : '' }}</span>
            <span v-if="soloLp" class="tier-lp">{{ soloLp }}</span>
            <span v-if="soloStats" class="rank-winrate-stats">
              <span class="stats-divider">·</span>
              <span class="stats-num">{{ soloStats.wins }}{{ $t("career.win") }}</span>
            </span>
          </template>
          <template v-else>
            <span class="tier-unranked">{{ $t("gameInfo.noRank") }}</span>
          </template>
        </div>

        <!-- 第 3 行：近 10 场胜率与 KDA（战绩隐藏时不展示） -->
        <div
          class="pc-row pc-stats-row"
          v-if="!playerData?.matchHistoryHidden && (playerData?.winRate !== undefined || playerData?.avgKda !== undefined)"
        >
          <span
            v-if="playerData?.winRate !== undefined"
            :class="[
              'pc-winrate-text',
              getWinRateClass(playerData.winRate),
            ]"
          >
            {{ playerData.winRate }}%
          </span>
          <span
            v-if="playerData?.winRate !== undefined && playerData?.avgKda !== undefined"
            class="stats-divider"
          >·</span>
          <span
            v-if="playerData?.avgKda !== undefined"
            :class="[
              'pc-kda-text',
              getKdaClass(playerData.avgKda),
            ]"
          >
            KDA {{ playerData?.avgKda?.toFixed(2) ?? "0.00" }}
          </span>
        </div>
      </div>
    </div>

    <!-- 第二层：下部整行（横跨头像+右侧整张卡片的全宽，优雅展示熟练度、招牌/练英雄与连胜连败） -->
    <div
      v-if="masteryDetail || (playerData?.streak && playerData.streak.count >= 2)"
      class="pc-bottom-bar"
    >
      <div class="pc-bottom-left">
        <!-- 熟练度等级与点数 -->
        <div
          v-if="masteryDetail"
          class="mastery-pill"
          :class="{ 'is-top': masteryDetail.isTopChampion }"
          :title="$t('gameInfo.masteryTooltip', { level: masteryDetail.level, points: masteryDetail.points.toLocaleString() })"
        >
          <span class="mastery-lvl">Lv.{{ masteryDetail.level }}</span>
          <span class="mastery-pts">{{ masteryDetail.formattedPoints }}</span>
        </div>

        <!-- 招牌 / 练英雄微标 -->
        <span
          v-if="masteryDetail?.isTopChampion"
          class="mastery-role-badge top"
          :title="$t('gameInfo.topChampionTitle')"
        >
          <svg class="role-svg star" viewBox="0 0 24 24" fill="currentColor">
            <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
          </svg>
          {{ $t('gameInfo.topChampion') }}
        </span>
        <span
          v-else-if="masteryDetail?.isPracticing"
          class="mastery-role-badge practice"
          :title="$t('gameInfo.practicingTitle')"
        >
          {{ $t('gameInfo.practicing') }}
        </span>
      </div>

      <!-- 连胜 / 连败微标 -->
      <div
        v-if="playerData?.streak && playerData.streak.count >= 2"
        class="pc-bottom-right"
      >
        <span
          :class="['streak-badge', playerData.streak.type]"
          :title="
            playerData.streak.type === 'win'
              ? $t('gameInfo.winStreak', { count: playerData.streak.count })
              : $t('gameInfo.lossStreak', { count: playerData.streak.count })
          "
        >
          <svg
            v-if="playerData.streak.type === 'win'"
            class="streak-svg flame"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2.3"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <path d="M8.5 14.5A2.5 2.5 0 0 0 11 12c0-1.38-.5-2-1-3-1.072-2.143-.224-4.054 2-6 .5 2.5 2 4.9 4 6.5 2 1.6 3 3.5 3 5.5a7 7 0 1 1-14 0c0-1.153.433-2.294 1-3a2.5 2.5 0 0 0 2.5 2.5z" />
          </svg>
          <svg
            v-else
            class="streak-svg snow"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2.3"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <line x1="12" y1="2" x2="12" y2="22" />
            <line x1="2" y1="12" x2="22" y2="12" />
            <line x1="4.93" y1="4.93" x2="19.07" y2="19.07" />
            <line x1="19.07" y1="4.93" x2="4.93" y2="19.07" />
          </svg>
          <span class="streak-count">{{ playerData.streak.count }}{{ playerData.streak.type === 'win' ? $t('career.win') : $t('career.lose') }}</span>
        </span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.player-card {
  position: relative;
  overflow: hidden;
  flex: 1;
  min-height: 0;
  max-height: 142px;
  display: flex;
  flex-direction: column;
  justify-content: center;
  padding: 8px 12px;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  transition: all 0.25s cubic-bezier(0.25, 0.8, 0.25, 1);
  box-sizing: border-box;
  box-shadow: var(--shadow-sm);
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
  gap: 6px;
}
.player-card:hover {
  background: var(--card-bg-hover);
  transform: translateY(-2px) scale(1.01);
  border-color: var(--primary-color-alpha-40);
  box-shadow:
    var(--shadow-md),
    0 4px 16px var(--primary-color-alpha-15);
}
.player-card.premade-card:hover {
  filter: brightness(1.08);
  transform: translateY(-2px) scale(1.01);
}

/* ─── 第一层：上部主体（头像 + 信息） ─── */
.pc-top-section {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  min-width: 0;
}

/* ─── 头像区 ─── */
.pc-avatar-area {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}
.profile-icon-wrapper-mini {
  position: relative;
  width: 50px;
  height: 50px;
  flex-shrink: 0;
  margin: 0 auto;
}
.profile-icon-wrapper-mini.top-champ-ring .avatar-container-mini {
  box-shadow: 0 0 0 1.5px rgba(245, 158, 11, 0.65), 0 0 8px rgba(245, 158, 11, 0.35);
}
.gauge-ring-svg-mini {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  overflow: visible;
}
.gauge-track-mini,
.gauge-progress-mini {
  fill: none;
  stroke-width: 4.5;
  stroke-linecap: round;
  stroke-dasharray: 235.62 282.74;
  transform: rotate(120deg);
  transform-origin: center;
}
.gauge-track-mini {
  stroke: var(--border-color);
}
.gauge-progress-mini {
  stroke: var(--primary-color);
  stroke-dashoffset: calc(235.62px * (1 - var(--progress) / 100));
  transition: stroke-dashoffset 0.8s ease;
}
.avatar-container-mini {
  position: absolute;
  inset: 6px;
  border-radius: 50%;
  overflow: hidden;
  transition: box-shadow 0.25s ease;
}
.profile-avatar-mini {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}
.profile-avatar-empty-mini {
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--hover-bg);
  color: var(--text-dimmed);
  font-size: 0.85rem;
  font-weight: 700;
}
.level-badge-mini {
  position: absolute;
  bottom: -3px;
  left: 50%;
  transform: translateX(-50%);
  color: var(--text-color);
  font-size: 0.62rem;
  font-weight: 800;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  padding: 0 3.5px;
  border-radius: 4px;
  z-index: 2;
  white-space: nowrap;
  line-height: 1.15;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.08);
}
.pc-name-lock {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--text-muted);
  flex-shrink: 0;
  opacity: 0.85;
}
.pc-name-lock-svg {
  width: 12px;
  height: 12px;
}

/* ─── 信息区 ─── */
.pc-info {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  justify-content: center;
  text-align: left;
  gap: 4px;
  min-width: 0;
  flex: 1;
}
.pc-row {
  display: flex;
  align-items: center;
  width: 100%;
  min-height: 18px;
  line-height: 1.2;
}

/* 第一行：名字与微标 */
.pc-name-row {
  display: flex;
  align-items: center;
  gap: 5px;
  min-width: 0;
}
.name-text {
  font-size: 0.84rem;
  font-weight: 700;
  color: var(--text-color);
  max-width: 95px;
  min-width: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  cursor: pointer;
  transition: color 0.15s ease-in-out;
}
.name-text:hover {
  color: var(--primary-color);
}
.name-tags-group {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  min-width: 0;
  flex-wrap: nowrap;
}

/* 宿命徽标 */
.fate-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 0.58rem;
  font-weight: 800;
  width: 14px;
  height: 14px;
  border-radius: 3px;
  flex-shrink: 0;
  line-height: 1;
}
.fate-badge.ally {
  background: rgba(16, 185, 129, 0.15);
  color: #10b981;
  border: 1px solid rgba(16, 185, 129, 0.35);
}
.fate-badge.enemy {
  background: rgba(239, 68, 68, 0.15);
  color: #ef4444;
  border: 1px solid rgba(239, 68, 68, 0.35);
}

/* 标记玩家 */
.saved-badge {
  display: inline-flex;
  align-items: center;
  padding: 1px 5px;
  border-radius: 999px;
  font-size: 0.56rem;
  font-weight: 700;
  line-height: 1.3;
  background: var(--primary-color-alpha-15);
  color: var(--primary-color);
  border: 1px solid var(--primary-color-alpha-40);
  max-width: 55px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex-shrink: 0;
}
.saved-badge.met-only {
  background: var(--border-color);
  color: var(--text-muted);
  border: none;
}

/* 连胜 / 连败微徽标 */
.streak-badge {
  display: inline-flex;
  align-items: center;
  gap: 2.5px;
  padding: 1px 4px;
  border-radius: 3px;
  font-size: 0.58rem;
  font-weight: 800;
  line-height: 1;
  flex-shrink: 0;
}
.streak-badge.win {
  background: linear-gradient(135deg, rgba(249, 115, 22, 0.14), rgba(239, 68, 68, 0.14));
  color: #ea580c;
  border: 1px solid rgba(249, 115, 22, 0.35);
}
.streak-badge.loss {
  background: linear-gradient(135deg, rgba(56, 189, 248, 0.14), rgba(59, 130, 246, 0.14));
  color: #2563eb;
  border: 1px solid rgba(59, 130, 246, 0.35);
}
.streak-svg {
  width: 9.5px;
  height: 9.5px;
  flex-shrink: 0;
}
.streak-svg.flame {
  color: #ea580c;
}
.streak-svg.snow {
  color: #2563eb;
}
.streak-count {
  line-height: 1;
}

/* 招牌 / 练英雄微徽章 */
.mastery-role-badge {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  font-size: 0.54rem;
  font-weight: 800;
  line-height: 1;
  padding: 1px 4px;
  border-radius: 3px;
  flex-shrink: 0;
  white-space: nowrap;
}
.mastery-role-badge.top {
  background: linear-gradient(135deg, rgba(245, 158, 11, 0.16), rgba(217, 119, 6, 0.12));
  color: #d97706;
  border: 1px solid rgba(245, 158, 11, 0.45);
}
.mastery-role-badge.practice {
  background: rgba(100, 116, 139, 0.12);
  color: var(--text-muted);
  border: 1px solid rgba(148, 163, 184, 0.3);
}
.role-svg {
  width: 8.5px;
  height: 8.5px;
  flex-shrink: 0;
}
.role-svg.star {
  color: #d97706;
}

/* 第二行：排位段位与全赛季战绩 */
.pc-rank-row {
  display: flex;
  align-items: center;
  gap: 3px;
  font-size: 0.72rem;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
}
.tier-text {
  font-weight: 700;
}
.tier-text.IRON { color: #848c96; }
.tier-text.BRONZE { color: #b45309; }
.tier-text.SILVER { color: #8293a7; }
.tier-text.GOLD { color: #d97706; }
.tier-text.PLATINUM { color: #0d9488; }
.tier-text.EMERALD { color: #059669; }
.tier-text.DIAMOND { color: #3b82f6; }
.tier-text.MASTER { color: #8b5cf6; }
.tier-text.GRANDMASTER { color: #ef4444; }
.tier-text.CHALLENGER { color: #f59e0b; }

.tier-lp {
  font-weight: 600;
  color: var(--text-muted);
  font-size: 0.68rem;
}
.tier-unranked {
  color: var(--text-dimmed);
  font-size: 0.7rem;
  font-weight: 500;
}
.rank-winrate-stats {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  font-size: 0.66rem;
  color: var(--text-muted);
}
.stats-num {
  color: var(--text-dimmed);
  font-size: 0.64rem;
}
.stats-rate {
  font-weight: 700;
  font-size: 0.66rem;
}

/* 第三行：熟练度微标与近期表现 */
.pc-stats-row {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 0.7rem;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
}
.stats-divider {
  color: var(--text-dimmed);
  opacity: 0.5;
  font-weight: 700;
}

.pc-winrate-text {
  font-size: 0.68rem;
  font-weight: 700;
}
.pc-kda-text {
  font-size: 0.68rem;
  font-weight: 700;
}
.pc-kda-text.kda-orange {
  color: var(--accent-color);
}
.pc-kda-text.kda-blue {
  color: var(--tier-blue);
}
.pc-kda-text.kda-green {
  color: var(--win-color);
}
.pc-kda-text.kda-gray {
  color: var(--text-dimmed);
}

/* ─── 第二层：全宽底栏（横跨头像+右侧整张卡片） ─── */
.pc-bottom-bar {
  width: 100%;
  box-sizing: border-box;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
  padding: 3px 6px;
  background: rgba(0, 0, 0, 0.025);
  border-top: 1px dashed var(--border-color);
  border-radius: 4px;
}
.pc-bottom-left {
  display: flex;
  align-items: center;
  gap: 5px;
  min-width: 0;
  flex: 1;
}
.pc-bottom-right {
  display: flex;
  align-items: center;
  flex-shrink: 0;
}

/* 熟练度胶囊微标 */
.mastery-pill {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  font-size: 0.62rem;
  line-height: 1;
  padding: 1.5px 5px;
  border-radius: 3px;
  background: var(--hover-bg-strong);
  border: 1px solid var(--border-color);
  flex-shrink: 0;
}
.mastery-pill.is-top {
  background: rgba(245, 158, 11, 0.08);
  border-color: rgba(245, 158, 11, 0.3);
}
.mastery-lvl {
  font-weight: 800;
  color: var(--text-color);
}
.mastery-pill.is-top .mastery-lvl {
  color: #d97706;
}
.mastery-pts {
  color: var(--text-muted);
  font-weight: 600;
}

/* 招牌 / 练英雄微标 */
.mastery-role-badge {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  padding: 1.5px 5px;
  border-radius: 3px;
  font-size: 0.58rem;
  font-weight: 800;
  line-height: 1;
  flex-shrink: 0;
  letter-spacing: 0.2px;
}
.mastery-role-badge.top {
  background: linear-gradient(135deg, rgba(245, 158, 11, 0.16), rgba(217, 119, 6, 0.16));
  color: #b45309;
  border: 1px solid rgba(245, 158, 11, 0.45);
  box-shadow: 0 1px 3px rgba(245, 158, 11, 0.12);
}
.mastery-role-badge.practice {
  background: var(--hover-bg);
  color: var(--text-muted);
  border: 1px solid var(--border-color);
}
.role-svg.star {
  width: 8.5px;
  height: 8.5px;
  color: #f59e0b;
  flex-shrink: 0;
}

/* 连胜 / 连败微徽标 */
.streak-badge {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 1.5px 5px;
  border-radius: 3px;
  font-size: 0.6rem;
  font-weight: 800;
  line-height: 1;
  flex-shrink: 0;
}
.streak-badge.win {
  background: linear-gradient(135deg, rgba(249, 115, 22, 0.14), rgba(239, 68, 68, 0.14));
  color: #ea580c;
  border: 1px solid rgba(249, 115, 22, 0.35);
}
.streak-badge.loss {
  background: linear-gradient(135deg, rgba(59, 130, 246, 0.14), rgba(37, 99, 235, 0.14));
  color: #2563eb;
  border: 1px solid rgba(59, 130, 246, 0.35);
}
.streak-svg {
  width: 9.5px;
  height: 9.5px;
  flex-shrink: 0;
}
.streak-svg.flame {
  color: #ea580c;
}
.streak-svg.snow {
  color: #2563eb;
}
.streak-count {
  font-weight: 800;
  line-height: 1;
}

.wr-high {
  color: var(--win-color);
}
.wr-medium {
  color: var(--tier-blue);
}
.wr-low {
  color: var(--text-dimmed);
}
</style>
