<script setup lang="ts">
import { useI18n } from "vue-i18n";
import LcuImage from "../LcuImage.vue";
import type { MatchDisplay } from "../../api/lcu";
import { getQueueName } from "../../utils/queueName";

defineProps<{
  match: MatchDisplay;
  selected: boolean;
}>();

const emit = defineEmits<{
  (e: "select", gameId: number): void;
}>();

const { t, te } = useI18n();

function queueName(queueId: number, backendName: string): string {
  return getQueueName(queueId, backendName, { t, te });
}
</script>

<template>
  <div
    :class="['mini-match-card', match.win ? 'win' : 'lose', { selected }]"
    @click="emit('select', match.gameId)"
  >
    <div class="mini-avatar">
      <LcuImage :src="match.championIconUrl" alt="champ" />
    </div>
    <div class="mini-info">
      <span class="mini-mode">{{
        queueName(match.queueId, match.name)
      }}</span>
      <span class="mini-time-kda">
        {{ match.shortTime.split(" ")[0] }} &nbsp;&nbsp;
        {{ match.kills }}/<span class="death-red">{{ match.deaths }}</span
        >/{{ match.assists }}
      </span>
    </div>
  </div>
</template>

<style scoped>
.mini-match-card {
  display: flex;
  align-items: center;
  padding: 10px 12px;
  border-radius: 8px;
  border: 1px solid rgba(0, 0, 0, 0.04);
  cursor: pointer;
  transition: all 0.25s cubic-bezier(0.25, 0.8, 0.25, 1);
  background: var(--card-bg);
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.015);
}

.mini-match-card:hover {
  transform: translateY(-1.5px);
  box-shadow: 0 6px 16px rgba(31, 38, 135, 0.06);
}

.mini-match-card.win {
  background-color: var(--win-bg);
  border-color: var(--win-border);
}

.mini-match-card.win:hover {
  background-color: var(--win-bg);
  box-shadow: 0 6px 16px rgba(34, 197, 94, 0.12);
}

[data-theme="dark"] .mini-match-card.win:hover {
  background-color: rgba(34, 197, 94, 0.12);
}

.mini-match-card.lose {
  background-color: var(--loss-bg);
  border-color: var(--loss-border);
}

.mini-match-card.lose:hover {
  background-color: var(--loss-bg);
  box-shadow: 0 6px 16px rgba(239, 68, 68, 0.12);
}

[data-theme="dark"] .mini-match-card.lose:hover {
  background-color: rgba(239, 68, 68, 0.12);
}

.mini-match-card.selected.win {
  border: 2px solid var(--win-color);
  box-shadow: 0 0 12px rgba(34, 197, 94, 0.2);
}

.mini-match-card.selected.lose {
  border: 2px solid var(--loss-color);
  box-shadow: 0 0 12px rgba(239, 68, 68, 0.2);
}

.mini-avatar {
  width: 36px;
  height: 36px;
  border-radius: 50%;
  overflow: hidden;
  border: 1px solid rgba(0, 0, 0, 0.05);
  margin-right: 10px;
  flex-shrink: 0;
}

.mini-info {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
}

.mini-mode {
  font-size: 0.78rem;
  font-weight: 700;
  color: var(--text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.mini-time-kda {
  font-size: 0.72rem;
  color: var(--text-muted);
  margin-top: 2px;
}

.death-red {
  color: var(--death-color, var(--loss-color));
  font-weight: 600;
}
</style>
