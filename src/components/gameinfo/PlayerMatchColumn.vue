<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { MatchDisplay, AppConfig } from "../../api/lcu";
import type { PlayerData } from "../../types/gameInfo";
import { usePlayerSearch } from "../../composables/usePlayerSearch";
import LcuImage from "../LcuImage.vue";

const props = defineProps<{
  player: any;
  playerData?: PlayerData;
  index: number;
  appConfig: AppConfig | null;
}>();

const { t, te } = useI18n();
const { getPlayerSearchName, handleNameClick } = usePlayerSearch();

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
  <div class="player-column">
    <div class="col-header">
      <div class="col-header-top">
        <div class="col-header-info">
          <span
            class="col-name"
            :title="getPlayerSearchName(player, playerData) ? `${$t('nav.search')} ${getPlayerSearchName(player, playerData)}` : undefined"
            @click="(e) => handleNameClick(e, player, playerData)"
          >{{
            playerData?.info?.gameName ||
            player.displayName ||
            `玩家${index + 1}`
          }}</span>
          <span
            v-if="playerData?.winRate !== undefined"
            class="col-summary"
          >
            <span class="summary-wins"
              >{{ playerData.winCount }}胜</span
            >
            <span class="summary-losses"
              >{{ playerData.lossesCount }}负</span
            >
          </span>
        </div>
      </div>
    </div>

    <template v-if="playerData?.loading">
      <div class="col-loading">
        <div class="mini-spinner"></div>
      </div>
    </template>
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
          <div class="cm-champ">
            <LcuImage
              :src="m.championIconUrl"
              class="cm-champ-img"
              alt="champ"
            />
            <span class="cm-level">{{ m.champLevel }}</span>
          </div>
          <div class="cm-detail">
            <div class="cm-top-row">
              <span class="cm-mode">{{ getQueueName(m.queueId, m.name) }}</span>
            </div>
            <div class="cm-bottom">
              <span class="cm-kda">
                <span class="k">{{ m.kills }}</span
                >/ <span class="d">{{ m.deaths }}</span
                >/
                <span class="a">{{ m.assists }}</span>
              </span>
              <span class="cm-date">{{
                m.shortTime.split(" ")[0]
              }}</span>
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
}
.player-column:last-child {
  border-right: none;
}

.col-header {
  height: 58px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--border-color);
  background: rgba(0, 0, 0, 0.02);
  flex-shrink: 0;
  display: flex;
  align-items: center;
  box-sizing: border-box;
}
.col-header-top {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
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
  cursor: pointer;
  transition: color 0.15s ease-in-out;
}
.col-name:hover {
  color: var(--primary-color);
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
@keyframes spin {
  to {
    transform: rotate(360deg);
  }
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
  color: var(--death-color, #ef4444);
}
.cm-date {
  font-size: 0.65rem;
  color: var(--text-dimmed);
  white-space: nowrap;
}
</style>
