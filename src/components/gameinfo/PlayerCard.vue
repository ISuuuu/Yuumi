<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { PlayerData } from "../../types/gameInfo";
import { PREMADE_COLORS } from "../../types/gameInfo";
import LcuImage from "../LcuImage.vue";

defineProps<{
  player: any;
  playerData?: PlayerData;
  premadeIdx: number;
  activeTab: "my" | "their";
  premadeCardStyle: Record<string, string>;
}>();

const { t } = useI18n();

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

function getChampionIcon(id: number): string {
  return id > 0 ? `/lol-game-data/assets/v1/champion-icons/${id}.png` : "";
}

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

function formatRank(q: any): string {
  if (!q || !q.tier || q.tier === "NONE") return "";
  const tier = TIER_MAP.value[q.tier] || q.tier;
  const div = q.rank && q.rank !== "NA" ? q.rank : "";
  const lp = q.leaguePoints !== undefined ? ` ${q.leaguePoints} LP` : "";
  return `${tier}${div}${lp}`;
}
</script>

<template>
  <div
    class="player-card"
    :class="{
      'premade-card': premadeIdx >= 0,
    }"
    :style="premadeCardStyle"
  >
    <div class="pc-avatar-area">
      <div class="profile-icon-wrapper-mini">
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
          <!-- 选人阶段：选了英雄或预选了英雄才显示英雄头像 -->
          <template v-if="player.championId || player.championPickIntent">
            <LcuImage
              :src="
                getChampionIcon(player.championId || player.championPickIntent)
              "
              class="profile-avatar-mini"
              alt="champ"
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

    <div class="pc-info centered">
      <div class="pc-row pc-name-row-centered">
        <span
          v-if="premadeIdx >= 0"
          class="premade-dot"
          :style="{
            background:
              PREMADE_COLORS[premadeIdx % PREMADE_COLORS.length].dot,
          }"
          :title="
            t('gameInfo.premadeIdx', {
              idx: premadeIdx + 1,
            })
          "
        ></span>
        <span class="name-group">
          <span class="name-text">{{
            playerData?.info?.gameName ||
            playerData?.info?.displayName ||
            player.displayName ||
            "未知"
          }}</span>
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
        </span>
      </div>

      <div
        class="pc-row pc-winrate-row"
        v-if="playerData?.winRate !== undefined"
      >
        <span
          :class="[
            'pc-winrate-badge-clean',
            getWinRateClass(playerData.winRate),
          ]"
        >
          {{ playerData.winRate }}{{ $t("gameInfo.winRateSuffix") }}
        </span>
      </div>

      <div
        class="pc-row pc-kda-row"
        v-if="playerData?.avgKda !== undefined"
      >
        <span
          :class="[
            'pc-kda-text',
            getKdaClass(playerData.avgKda),
          ]"
        >
          KDA: {{ playerData?.avgKda?.toFixed(2) ?? "0.00" }}
        </span>
      </div>

      <div class="pc-row pc-rank-row">
        <span
          class="rank-badge-clean"
          :title="
            formatRank(playerData?.ranked?.solo) ||
            $t('gameInfo.soloRankTitle')
          "
        >
          {{ $t("gameInfo.soloRank") }}:
          {{
            playerData?.ranked?.solo?.tier
              ? TIER_MAP[playerData.ranked.solo.tier]
              : $t("gameInfo.noRank")
          }}
        </span>
      </div>

      <div class="pc-row pc-rank-row">
        <span
          class="rank-badge-clean"
          :title="
            formatRank(playerData?.ranked?.flex) ||
            $t('gameInfo.flexRankTitle')
          "
        >
          {{ $t("gameInfo.flexRank") }}:
          {{
            playerData?.ranked?.flex?.tier
              ? TIER_MAP[playerData.ranked.flex.tier]
              : $t("gameInfo.noRank")
          }}
        </span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.player-card {
  flex: 1;
  min-height: 0;
  max-height: 142px;
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 12px 16px;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  transition: all 0.25s cubic-bezier(0.25, 0.8, 0.25, 1);
  box-sizing: border-box;
  box-shadow: var(--shadow-sm);
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
}
.player-card:hover {
  background: var(--card-bg-hover);
  transform: translateY(-2px) scale(1.01);
  border-color: var(--primary-color-alpha-40);
  box-shadow:
    var(--shadow-md),
    0 4px 16px var(--primary-color-alpha-15);
}

.profile-icon-wrapper-mini {
  position: relative;
  width: 58px;
  height: 58px;
  flex-shrink: 0;
  margin: 0 auto;
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
  stroke-width: 5;
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
  inset: 7px;
  border-radius: 50%;
  overflow: hidden;
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
  bottom: -2px;
  left: 50%;
  transform: translateX(-50%);
  color: var(--text-color);
  font-size: 0.65rem;
  font-weight: 800;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  padding: 0 4px;
  border-radius: 4px;
  z-index: 2;
  white-space: nowrap;
  line-height: 1.2;
}

.pc-info.centered {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  text-align: center;
  gap: 5px;
  width: 100%;
}
.pc-row {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  min-height: 20px;
}
.name-group {
  display: inline-flex;
  align-items: center;
  min-width: 0;
  max-width: 100%;
}
.name-text {
  font-size: 0.82rem;
  font-weight: 700;
  color: var(--text-color);
  max-width: 110px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.premade-dot {
  display: inline-block;
  width: 6px;
  height: 6px;
  border-radius: 50%;
  margin-right: 4px;
  vertical-align: middle;
  box-shadow: 0 0 4px currentColor;
}
.fate-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 0.6rem;
  font-weight: 800;
  width: 14px;
  height: 14px;
  border-radius: 3px;
  margin-left: 4px;
  vertical-align: middle;
  flex-shrink: 0;
  letter-spacing: 0;
}
.fate-badge.ally {
  background: rgba(5, 119, 72, 0.18);
  color: #10b981;
  border: 1px solid rgba(16, 185, 129, 0.4);
}
.fate-badge.enemy {
  background: rgba(191, 36, 42, 0.15);
  color: #f87171;
  border: 1px solid rgba(248, 113, 113, 0.4);
}
.pc-winrate-badge-clean {
  font-size: 0.72rem;
  font-weight: 600;
  color: var(--text-muted);
}
.pc-kda-text {
  font-size: 0.72rem;
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
.rank-badge-clean {
  font-size: 0.7rem;
  font-weight: 600;
  color: var(--text-dimmed);
}
</style>
