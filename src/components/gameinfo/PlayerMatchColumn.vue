<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { MatchDisplay, AppConfig } from "../../api/lcu";
import type { PlayerData, PremadePlayerLike } from "../../types/gameInfo";
import { PREMADE_COLORS } from "../../types/gameInfo";
import { usePlayerSearch } from "../../composables/usePlayerSearch";
import LcuImage from "../LcuImage.vue";

const props = defineProps<{
  player: PremadePlayerLike;
  playerData?: PlayerData;
  index: number;
  appConfig: AppConfig | null;
  compact?: boolean;
  side?: "ally" | "enemy";
  premadeIdx?: number;
}>();

const { t, te } = useI18n();
const { getPlayerSearchName, handleNameClick } = usePlayerSearch();

const premadeColor = computed(() => {
  if (props.premadeIdx === undefined || props.premadeIdx < 0) return null;
  return PREMADE_COLORS[props.premadeIdx % PREMADE_COLORS.length];
});

const colHeaderStyle = computed(() => {
  if (!premadeColor.value) return {};
  return {
    backgroundColor: premadeColor.value.bg,
    borderBottomColor: premadeColor.value.border,
  };
});

function getChampionIcon(id: number): string {
  return id > 0 ? `/lol-game-data/assets/v1/champion-icons/${id}.png` : "";
}

const currentChampId = computed(
  () => props.player?.championId || props.player?.championPickIntent || 0,
);

function getQueueName(queueId: number, backendName: string): string {
  const key = `gameModes.${queueId}`;
  if (te(key)) {
    const translation = t(key);
    if (
      (translation.includes("云顶") || translation.includes("TFT")) &&
      !backendName.includes("云顶") &&
      !backendName.includes("TFT")
    ) {
      return backendName;
    }
    return translation;
  }
  return backendName;
}

function formatKdaRatio(m: MatchDisplay): { text: string; clazz: string } {
  if (m.deaths === 0) {
    return { text: "Perf", clazz: "kda-gold" };
  }
  const ratio = (m.kills + m.assists) / m.deaths;
  const text = ratio >= 10 ? ratio.toFixed(0) : ratio.toFixed(1);
  if (ratio >= 5.0) return { text, clazz: "kda-gold" };
  if (ratio >= 3.0) return { text, clazz: "kda-blue" };
  if (ratio >= 2.0) return { text, clazz: "kda-green" };
  return { text, clazz: "kda-gray" };
}

function formatMatchDate(shortTime: string): string {
  if (!shortTime) return "";
  return shortTime.split(" ")[0] || "";
}

function getMatchCardStyle(m: MatchDisplay): Record<string, string> {
  if (!props.appConfig?.Personalization) return {};

  const colors = props.appConfig.Personalization;
  let color = "";

  if (m.remake) {
    color = colors.RemakeCardColor || "";
  } else if (m.win) {
    color = colors.WinCardColor || "";
  } else {
    color = colors.LoseCardColor || "";
  }

  if (color) {
    if (color.startsWith("#") && color.length === 9) {
      const alpha = parseInt(color.slice(1, 3), 16) / 255;
      const r = parseInt(color.slice(3, 5), 16);
      const g = parseInt(color.slice(5, 7), 16);
      const b = parseInt(color.slice(7, 9), 16);
      return { background: `rgba(${r}, ${g}, ${b}, ${alpha.toFixed(2)})` };
    } else if (color.startsWith("#") && color.length === 7) {
      return { background: `${color}1a` };
    }
    return { background: color };
  }
  return {};
}
</script>

<template>
  <div
    class="player-column"
    :class="{
      compact,
      'side-ally': side === 'ally',
      'side-enemy': side === 'enemy',
    }"
  >
    <!-- 列头部：所选英雄头像 + 玩家名（展示组队背景色） -->
    <div
      class="col-header"
      :class="{
        'premade-header': premadeIdx !== undefined && premadeIdx >= 0,
      }"
      :style="colHeaderStyle"
      :title="
        premadeIdx !== undefined && premadeIdx >= 0
          ? t('gameInfo.premadeIdx', { idx: premadeIdx + 1 })
          : undefined
      "
    >
      <div class="col-header-top">
        <!-- 选人阶段英雄头像 -->
        <div class="col-champ-wrapper">
          <LcuImage
            v-if="currentChampId > 0"
            :src="getChampionIcon(currentChampId)"
            class="col-champ-avatar"
            alt="champ"
          />
          <div v-else class="col-champ-avatar col-champ-avatar-empty">?</div>
        </div>

        <div class="col-header-info">
          <div class="name-row">
            <span
              class="col-name"
              :title="getPlayerSearchName(player, playerData) ? `${$t('nav.search')} ${getPlayerSearchName(player, playerData)}` : undefined"
              @click="(e) => handleNameClick(e, player, playerData)"
            >
              {{
                playerData?.info?.gameName ||
                playerData?.info?.displayName ||
                player.displayName ||
                `玩家${index + 1}`
              }}
            </span>
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
            >
              {{
                playerData.fateFlag === "ally"
                  ? $t("gameInfo.fateAllyText")
                  : $t("gameInfo.fateEnemyText")
              }}
            </span>
          </div>

          <!-- 几胜几负统计（不显示胜率） -->
          <div
            v-if="playerData?.winCount !== undefined"
            class="col-summary"
          >
            <span class="summary-counts">
              <span class="summary-wins">{{ playerData.winCount }}{{ $t("career.win") }}</span>
              <span class="summary-losses">{{ playerData.lossesCount }}{{ $t("career.lose") }}</span>
            </span>
          </div>
        </div>
      </div>
    </div>

    <!-- 战绩加载态 -->
    <template v-if="playerData?.loading">
      <div class="col-loading">
        <div class="mini-spinner"></div>
      </div>
    </template>

    <!-- 战绩列表 -->
    <template v-else-if="playerData?.matches?.length">
      <div class="col-matches-list">
        <div
          v-for="m in playerData.matches"
          :key="m.gameId"
          :class="[
            'col-match',
            m.remake ? 'remake' : m.win ? 'win' : 'lose',
          ]"
          :style="getMatchCardStyle(m)"
        >
          <!-- 卡片顶部：游戏类型 + 对局日期（独立一行） -->
          <div class="cm-top-row">
            <span class="cm-mode" :title="getQueueName(m.queueId, m.name)">
              {{ getQueueName(m.queueId, m.name) }}
            </span>
            <span class="cm-date">{{ formatMatchDate(m.shortTime) }}</span>
          </div>

          <!-- 卡片主体：大尺寸英雄头像 + 核心战绩数据 -->
          <div class="cm-main-row">
            <div class="cm-champ-box">
              <LcuImage
                :src="m.championIconUrl"
                class="cm-champ-img"
                alt="champ"
              />
              <span class="cm-level">{{ m.champLevel }}</span>
            </div>

            <div class="cm-stats-box">
              <div class="cm-kda-line">
                <span class="k">{{ m.kills }}</span>
                <span class="sep">/</span>
                <span class="d">{{ m.deaths }}</span>
                <span class="sep">/</span>
                <span class="a">{{ m.assists }}</span>
              </div>
              <!-- 按队伍(5列)正常展示 KDA 倍率，全部战绩(10列)不展示 -->
              <div v-if="!compact" class="cm-sub-line">
                <span class="cm-kda-ratio" :class="formatKdaRatio(m).clazz">
                  {{ formatKdaRatio(m).text }} KDA
                </span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.player-column {
  border-right: 1px solid var(--border-color);
  border-top: 3px solid var(--border-color);
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
  transition: all 0.2s ease-in-out;
}
.player-column:last-child {
  border-right: none;
}

/* ─── 阵营顶部与高光 ─── */
.player-column.side-ally {
  border-top-color: #3b82f6;
}
.player-column.side-enemy {
  border-top-color: #f43f5e;
}

/* ─── 列头部 ─── */
.col-header {
  height: 48px;
  padding: 5px 8px;
  border-bottom: 1px solid var(--border-color);
  background: rgba(0, 0, 0, 0.02);
  flex-shrink: 0;
  display: flex;
  align-items: center;
  box-sizing: border-box;
  transition: all 0.2s ease-in-out;
}
.col-header.premade-header:hover {
  filter: brightness(1.08);
}
.col-header-top {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  min-width: 0;
}
.col-champ-wrapper {
  position: relative;
  width: 28px;
  height: 28px;
  flex-shrink: 0;
}
.col-champ-avatar {
  width: 28px;
  height: 28px;
  border-radius: 6px;
  object-fit: cover;
  border: 1.5px solid rgba(255, 255, 255, 0.4);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.15);
  display: block;
}
.col-champ-avatar-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: 6px;
  background: var(--hover-bg);
  color: var(--text-dimmed);
  font-size: 0.75rem;
  font-weight: 700;
  border: 1.5px solid var(--border-color);
  box-sizing: border-box;
}

.col-header-info {
  display: flex;
  flex-direction: column;
  justify-content: center;
  min-width: 0;
  flex: 1;
  gap: 1px;
}
.name-row {
  display: flex;
  align-items: center;
  gap: 4px;
  min-width: 0;
  width: 100%;
}
.fate-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 0.54rem;
  font-weight: 800;
  width: 13px;
  height: 13px;
  border-radius: 3px;
  flex-shrink: 0;
  line-height: 1;
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

.col-name {
  font-size: 0.78rem;
  font-weight: 700;
  color: var(--text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  cursor: pointer;
  transition: color 0.15s ease-in-out;
}
.col-name:hover {
  color: var(--primary-color);
}

.col-summary {
  display: flex;
  align-items: center;
  font-size: 0.62rem;
  color: var(--text-muted);
  gap: 4px;
  line-height: 1;
}
.summary-counts {
  display: flex;
  align-items: center;
  gap: 4px;
}
.summary-wins {
  color: var(--win-color);
  font-weight: 700;
}
.summary-losses {
  color: var(--loss-color);
  font-weight: 700;
}

/* ─── 加载中状态 ─── */
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
@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

/* ─── 战绩列表 ─── */
.col-matches-list {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  gap: 6px;
  padding: 6px 6px;
  box-sizing: border-box;
  overflow-y: auto;
  scrollbar-width: thin;
}

/* ─── 基础战绩卡片样式 ─── */
.col-match {
  flex: 0 0 calc((100% - 54px) / 10);
  height: calc((100% - 54px) / 10);
  min-height: 44px;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  padding: 4px 7px;
  margin: 0;
  border-radius: 8px;
  border: 1px solid transparent;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.04);
  transition: all 0.18s cubic-bezier(0.4, 0, 0.2, 1);
  box-sizing: border-box;
  position: relative;
  overflow: hidden;
  backdrop-filter: blur(6px);
  -webkit-backdrop-filter: blur(6px);
}

/* 胜利卡片 */
.col-match.win {
  background: linear-gradient(
    135deg,
    rgba(16, 185, 129, 0.14) 0%,
    rgba(6, 95, 70, 0.04) 100%
  );
  border-color: rgba(16, 185, 129, 0.25);
}
.col-match.win:hover {
  background: linear-gradient(
    135deg,
    rgba(16, 185, 129, 0.22) 0%,
    rgba(6, 95, 70, 0.08) 100%
  );
  border-color: rgba(16, 185, 129, 0.45);
  box-shadow: 0 3px 12px rgba(16, 185, 129, 0.18);
  transform: translateY(-1px);
}

/* 失败卡片 */
.col-match.lose {
  background: linear-gradient(
    135deg,
    rgba(239, 68, 68, 0.13) 0%,
    rgba(159, 18, 57, 0.04) 100%
  );
  border-color: rgba(239, 68, 68, 0.25);
}
.col-match.lose:hover {
  background: linear-gradient(
    135deg,
    rgba(239, 68, 68, 0.2) 0%,
    rgba(159, 18, 57, 0.08) 100%
  );
  border-color: rgba(239, 68, 68, 0.45);
  box-shadow: 0 3px 12px rgba(239, 68, 68, 0.18);
  transform: translateY(-1px);
}

/* 重开卡片 */
.col-match.remake {
  background: linear-gradient(
    135deg,
    rgba(148, 163, 184, 0.12) 0%,
    rgba(100, 116, 139, 0.04) 100%
  );
  border-color: rgba(148, 163, 184, 0.25);
}
.col-match.remake:hover {
  border-color: rgba(148, 163, 184, 0.45);
  box-shadow: 0 2px 8px rgba(148, 163, 184, 0.15);
  transform: translateY(-1px);
}

/* ─── 卡片内部：顶部行（游戏类型 + 日期） ─── */
.cm-top-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-width: 0;
  gap: 4px;
  line-height: 1.1;
}
.cm-mode {
  font-size: 0.62rem;
  font-weight: 700;
  color: var(--text-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  letter-spacing: 0.1px;
}
.cm-date {
  font-size: 0.52rem;
  color: var(--text-dimmed);
  opacity: 0.75;
  white-space: nowrap;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  flex-shrink: 0;
}

/* ─── 卡片内部：主体行（大头像 + 战绩数据） ─── */
.cm-main-row {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  flex: 1;
  margin-top: 1px;
}

/* 英雄头像与等级 */
.cm-champ-box {
  position: relative;
  width: 32px;
  height: 32px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}
.cm-champ-img {
  width: 32px;
  height: 32px;
  border-radius: 6px;
  overflow: hidden;
  border: 1.5px solid rgba(255, 255, 255, 0.4);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.15);
  display: block;
  object-fit: cover;
}
.cm-level {
  position: absolute;
  bottom: -2px;
  right: -2px;
  min-width: 11px;
  height: 11px;
  padding: 0 1.5px;
  line-height: 11px;
  background: rgba(15, 23, 42, 0.88);
  color: #fff;
  border-radius: 3px;
  font-size: 0.46rem;
  font-weight: 800;
  text-align: center;
  border: 1px solid rgba(255, 255, 255, 0.4);
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.35);
  font-variant-numeric: tabular-nums;
}

/* 战绩数据区域 */
.cm-stats-box {
  display: flex;
  flex-direction: column;
  justify-content: center;
  min-width: 0;
  flex: 1;
  gap: 1px;
}
.cm-kda-line {
  font-size: 0.86rem;
  font-weight: 800;
  color: var(--text-color);
  letter-spacing: 0.2px;
  white-space: nowrap;
  line-height: 1.15;
  font-variant-numeric: tabular-nums;
}
.cm-kda-line .k {
  color: var(--text-color);
}
.cm-kda-line .d {
  color: var(--death-color, #ef4444);
  font-weight: 800;
}
.cm-kda-line .a {
  color: var(--text-color);
}
.cm-kda-line .sep {
  color: var(--text-dimmed);
  opacity: 0.35;
  font-size: 0.68rem;
  margin: 0 1.5px;
  font-weight: 500;
}

.cm-sub-line {
  display: flex;
  align-items: center;
  min-width: 0;
  line-height: 1;
}
.cm-kda-ratio {
  font-size: 0.56rem;
  font-weight: 700;
  white-space: nowrap;
  padding: 1px 4px;
  border-radius: 4px;
  background: rgba(255, 255, 255, 0.05);
  display: inline-block;
  font-variant-numeric: tabular-nums;
}
.kda-gold {
  color: #f59e0b;
  background: rgba(245, 158, 11, 0.1);
}
.kda-blue {
  color: #3b82f6;
  background: rgba(59, 130, 246, 0.1);
}
.kda-green {
  color: #10b981;
  background: rgba(16, 185, 129, 0.1);
}
.kda-gray {
  color: var(--text-dimmed);
}

/* ─── compact 紧凑模式专属微调（10 列并排） ─── */
.compact {
  border-top-width: 3px;
}
.compact .col-header {
  height: 42px;
  padding: 4px 6px;
}
.compact .col-champ-wrapper {
  width: 24px;
  height: 24px;
}
.compact .col-champ-avatar {
  width: 24px;
  height: 24px;
  border-radius: 5px;
}
.compact .col-champ-avatar-empty {
  width: 24px;
  height: 24px;
  border-radius: 5px;
  font-size: 0.65rem;
}
.compact .col-name {
  font-size: 0.72rem;
}
.compact .col-summary {
  font-size: 0.54rem;
}
.compact .summary-counts {
  gap: 3px;
  white-space: nowrap;
}
.compact .summary-wins,
.compact .summary-losses {
  font-weight: 700;
  white-space: nowrap;
}
.compact .col-matches-list {
  gap: 4px;
  padding: 4px 2px;
}
.compact .col-match {
  flex: 0 0 calc((100% - 36px) / 10);
  height: calc((100% - 36px) / 10);
  min-height: 38px;
  padding: 3px 4px;
  border-radius: 6px;
}
.compact .cm-main-row {
  gap: 4px;
}
.compact .cm-champ-box {
  width: 29px;
  height: 29px;
}
.compact .cm-champ-img {
  width: 29px;
  height: 29px;
  border-radius: 6px;
}
.compact .cm-level {
  min-width: 10px;
  height: 10px;
  line-height: 10px;
  font-size: 0.42rem;
  bottom: -2px;
  right: -2px;
  padding: 0 1px;
}
.compact .cm-kda-line {
  font-size: 0.76rem;
  font-weight: 800;
  letter-spacing: -0.2px;
}
.compact .cm-kda-line .sep {
  margin: 0 0.5px;
  font-size: 0.62rem;
}
.compact .cm-mode {
  font-size: 0.54rem;
}
.compact .cm-date {
  font-size: 0.46rem;
}
</style>
