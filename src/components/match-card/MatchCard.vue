<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { MatchDisplay } from "../../api/lcu";
import LcuImage from "../LcuImage.vue";
import PositionIcon from "../PositionIcon.vue";
import ReplayButton from "../ReplayButton.vue";
import { getPositionName } from "../../utils/matchAnalysis";
import { formatLocalDateTime } from "../../utils/matchTime";

/**
 * 生涯页单张对局卡片。
 *
 * 从 MatchHistoryTab 拆出，便于后续独立做折叠/展开与虚拟化（列表里绝大多数卡片
 * 只需要 116px 的摘要视图）。本组件只负责「一局」的展示，取数、筛选与分页仍留在父级。
 *
 * 样式为 scoped，随模板一并迁入；其中 .death-red / .win-text / .lose-text 在父组件
 * 仍有使用，父级各保留了一份同名定义（scoped 样式不跨组件生效）。
 */
const props = defineProps<{
  match: MatchDisplay;
}>();

const emit = defineEmits<{
  (e: "select", gameId: number): void;
}>();

const { t, te } = useI18n();

/** 模板内仍以 m 指代当前对局，保持与拆分前一致 */
const m = computed(() => props.match);

// ── 展示层辅助：纯函数，仅本卡片使用 ──

function getSpellIcon(row: MatchDisplay, slot: 1 | 2): string {
  return slot === 1 ? row.spell1IconUrl : row.spell2IconUrl;
}

function translateMapName(name: string): string {
  if (!name) return "";
  if (name.includes("峡谷") || name.includes("Rift")) return t("maps.11");
  if (name.includes("深渊") || name.includes("Abyss")) return t("maps.12");
  if (name.includes("闪击") || name.includes("Blitz")) return t("maps.21");
  if (name.includes("大厅") || name.includes("Lobby")) return t("maps.22");
  return name;
}

function getQueueName(row: MatchDisplay): string {
  const key = `gameModes.${row.queueId}`;
  if (te(key)) {
    const translation = t(key);
    if (
      (translation.includes("云顶") || translation.includes("TFT")) &&
      !row.name.includes("云顶") &&
      !row.name.includes("TFT")
    ) {
      return row.name;
    }
    return translation;
  }
  return row.name;
}

const TFT_QUEUES = [1090, 1100, 1130, 1160];

function getResultText(row: MatchDisplay): string {
  if (row.placement && TFT_QUEUES.includes(row.queueId)) {
    return `第 ${row.placement} 名`;
  }
  return row.win ? t("career.victory") : t("career.defeat");
}

function getResultClass(row: MatchDisplay): string {
  if (row.placement && TFT_QUEUES.includes(row.queueId)) {
    if (row.placement === 1) return "win-text gold-text";
    if (row.placement <= 4) return "win-text";
    return "lose-text";
  }
  return row.win ? "win-text" : "lose-text";
}

function getKdaClass(kda: string): string {
  const val = parseFloat(kda);
  if (isNaN(val)) return "kda-perfect";
  if (val >= 5) return "kda-great";
  if (val >= 3) return "kda-good";
  return "kda-normal";
}
</script>

<template>
  <div
    :class="['match-card', m.win ? 'win' : 'lose']"
    @click="emit('select', m.gameId)"
    style="cursor: pointer"
  >
    <!-- 1. 英雄头像、等级、技能、符文 -->
    <div class="champ-panel">
      <div class="champ-avatar-box">
        <LcuImage :src="m.championIconUrl" class="champ-avatar" alt="champ" />
        <div class="level-overlay">{{ m.champLevel }}</div>
      </div>
      <div class="spells-runes">
        <div class="spells-col">
          <div class="spell-slot">
            <LcuImage :src="getSpellIcon(m, 1)" class="mini-icon" alt="s1" />
          </div>
          <div class="spell-slot">
            <LcuImage :src="getSpellIcon(m, 2)" class="mini-icon" alt="s2" />
          </div>
        </div>
        <div
          v-if="m.queueId !== 2400 && m.queueId !== 2450"
          class="rune-slot"
        >
          <LcuImage
            :src="m.runeIconUrl"
            class="mini-icon circular"
            alt="rune"
          />
        </div>
      </div>
    </div>

    <!-- 2. 胜负状态与游戏模式/时长 -->
    <div class="result-panel">
      <div class="result-head-row">
        <span :class="['result-text', getResultClass(m)]">
          {{ getResultText(m) }}
        </span>
        <span
          v-if="m.position"
          class="result-pos-badge"
          :title="`位置: ${getPositionName(m.position)}`"
        >
          <PositionIcon
            :position="m.position"
            :use-position-color="true"
            :size="13"
          />
        </span>
      </div>
      <div class="result-sub-info">
        <span class="queue-mode">{{ getQueueName(m) }}</span>
        <span class="game-duration">{{ m.duration }}</span>
      </div>
    </div>

    <!-- 3. KDA 数字与文字 -->
    <div class="kda-panel">
      <div class="kda-numbers">
        <span class="bold">{{ m.kills }}</span> /
        <span class="bold death-red">{{ m.deaths }}</span> /
        <span class="bold">{{ m.assists }}</span>
      </div>
      <div class="kda-desc">
        <span class="kda-ratio" :class="getKdaClass(m.kda)">{{ m.kda }} KDA</span>
      </div>
    </div>

    <!-- 4. 补刀数 (靠左) -->
    <div class="cs-panel" :title="'补刀: ' + m.cs">
      <svg class="cs-icon" viewBox="0 0 24 24" fill="currentColor">
        <path d="M12 2a7 7 0 0 0-7 7v4.5c0 1.2.6 2.3 1.6 3l1.9 1.3v2.2a1 1 0 0 0 1.5.8l2-1.3 2 1.3a1 1 0 0 0 1.5-.8v-2.2l1.9-1.3c1-.7 1.6-1.8 1.6-3V9a7 7 0 0 0-7-7zm-3.5 8a1.5 1.5 0 1 1 0-3 1.5 1.5 0 0 1 0 3zm7 0a1.5 1.5 0 1 1 0-3 1.5 1.5 0 0 1 0 3z" />
      </svg>
      <span class="cs-count">{{ m.cs }}</span>
    </div>

    <!-- 5. 装备栏 (海克斯强化 + 前 6 件常规装备 + 第 7 件饰品) -->
    <div class="items-panel">
      <!-- 海克斯强化（仅海克斯大乱斗） -->
      <div
        v-if="(m.queueId === 2400 || m.queueId === 2450) && Boolean(m.augmentIconUrls?.length)"
        class="augment-grid"
      >
        <n-tooltip
          v-for="(url, idx) in m.augmentIconUrls"
          :key="'aug-' + idx"
          trigger="hover"
          placement="top"
        >
          <template #trigger>
            <div class="augment-slot">
              <LcuImage :src="url" class="augment-img" alt="augment" />
            </div>
          </template>
          <div class="career-aug-tooltip">
            <div class="career-aug-tooltip-name">
              {{ m.augmentNames?.[idx] || "海克斯强化" }}
            </div>
          </div>
        </n-tooltip>
      </div>
      <div class="items-grid">
        <div v-for="idx in 6" :key="idx" class="item-slot">
          <LcuImage
            v-if="m.itemIconUrls[idx - 1]"
            :src="m.itemIconUrls[idx - 1]"
            class="item-img"
            alt="item"
          />
        </div>
      </div>
      <!-- 饰品独立显示 -->
      <div class="ward-slot">
        <LcuImage
          v-if="m.itemIconUrls[6]"
          :src="m.itemIconUrls[6]"
          class="item-img"
          alt="ward"
        />
      </div>
    </div>

    <!-- 6. 还原经济面板 (位于装备栏右侧) -->
    <div class="gold-panel" :title="'经济: ' + m.gold.toLocaleString() + ' 金币'">
      <span class="gold-count">{{ m.gold.toLocaleString() }}</span>
      <svg class="gold-icon" viewBox="0 0 24 24" fill="currentColor">
        <circle
          cx="12"
          cy="12"
          r="9"
          stroke="currentColor"
          stroke-width="1.8"
          fill="none"
        />
        <path
          d="M12 6.5v11M14.5 9.5H11.5a1.5 1.5 0 0 0 0 3h1a1.5 1.5 0 0 1 0 3H9.5"
          stroke="currentColor"
          stroke-width="1.8"
          stroke-linecap="round"
          stroke-linejoin="round"
          fill="none"
        />
      </svg>
    </div>

    <!-- 7. 地图模式与日期 -->
    <div class="time-panel">
      <span class="game-map">{{ translateMapName(m.map) }}</span>
      <span class="match-time">{{ formatLocalDateTime(m.timeStamp) }}</span>
    </div>

    <!-- 8. 回放按钮 -->
    <div class="replay-action-panel" @click.stop>
      <ReplayButton :game-id="m.gameId" />
    </div>
  </div>
</template>

<style scoped>
.match-card {
  display: flex;
  align-items: center;
  padding: 14px 20px;
  border-radius: 10px;
  border: 1px solid var(--border-color);
  transition: all 0.25s cubic-bezier(0.25, 0.8, 0.25, 1);
  cursor: pointer;
}

.match-card:hover {
  box-shadow: var(--shadow-md);
}

.match-card.win {
  background-color: var(--win-bg);
  border-color: var(--win-border);
}

.match-card.win:hover {
  background-color: rgba(16, 185, 129, 0.12);
}

.match-card.lose {
  background-color: var(--loss-bg);
  border-color: var(--loss-border);
}

.match-card.lose:hover {
  background-color: rgba(239, 68, 68, 0.11);
}

/* 1. 英雄面板 */
.champ-panel {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 100px;
}

.champ-avatar-box {
  position: relative;
  width: 52px;
  height: 52px;
}

.champ-avatar {
  width: 52px;
  height: 52px;
  border-radius: 50%;
  overflow: hidden;
  border: 1.5px solid var(--border-color);
}

.level-overlay {
  position: absolute;
  bottom: -2px;
  right: -2px;
  width: 18px;
  height: 18px;
  line-height: 16px;
  background-color: var(--card-bg);
  color: var(--text-color);
  border-radius: 50%;
  font-size: 0.7rem;
  font-weight: 700;
  text-align: center;
  border: 1px solid var(--border-color);
}

[data-theme="dark"] .level-overlay {
  background-color: var(--card-bg);
  color: var(--text-color);
  border-color: rgba(255, 255, 255, 0.15);
}

.spells-runes {
  display: flex;
  gap: 3px;
  align-items: center;
}

.spells-col {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.spell-slot,
.rune-slot {
  width: 22px;
  height: 22px;
  border-radius: 3px;
  overflow: hidden;
  border: 1px solid var(--border-color);
}

.mini-icon {
  width: 100%;
  height: 100%;
  display: block;
}

.mini-icon.circular {
  border-radius: 50%;
}

/* 2. 胜负面板 */
.result-panel {
  display: flex;
  flex-direction: column;
  justify-content: center;
  min-width: 90px;
}

.result-head-row {
  display: flex;
  align-items: center;
  gap: 6px;
}

.result-pos-badge {
  display: inline-flex;
  align-items: center;
  opacity: 0.9;
  transition: opacity 0.15s ease;
}
.result-pos-badge:hover {
  opacity: 1;
}

.result-text {
  font-size: 1rem;
  font-weight: 800;
}

.win-text {
  color: var(--win-color);
}

.win-text.gold-text {
  color: #f59e0b;
}

.lose-text {
  color: var(--loss-color);
}

.result-sub-info {
  display: flex;
  flex-direction: column;
  gap: 1px;
  margin-top: 2px;
}

.queue-mode {
  font-size: 0.8rem;
  color: var(--text-muted);
}

.game-duration {
  font-size: 0.76rem;
  color: var(--text-dimmed);
  font-weight: 600;
}

/* 3. KDA面板 */
.kda-panel {
  display: flex;
  flex-direction: column;
  justify-content: center;
  min-width: 105px;
}

.kda-numbers {
  font-size: 1rem;
  color: var(--text-muted);
}

.bold {
  font-weight: 700;
  color: var(--text-color);
}

.kda-desc {
  margin-top: 2px;
}

.kda-ratio {
  color: var(--text-muted);
  font-size: 0.82rem;
  font-weight: 700;
}

.kda-perfect {
  color: #d97706;
}
.kda-great {
  color: #db2777;
}
.kda-good {
  color: #2563eb;
}
.kda-normal {
  color: var(--text-dimmed);
}

/* 4. 补刀与经济统计列 */
/* 4. 补刀面板 (靠左) */
.cs-panel {
  display: flex;
  align-items: center;
  gap: 4px;
  min-width: 44px;
  margin-right: 6px;
}

.cs-count {
  font-size: 0.85rem;
  font-weight: 600;
  color: var(--text-muted);
}

.cs-icon {
  width: 14px;
  height: 14px;
  color: #94a3b8;
  flex-shrink: 0;
}

/* 6. 金币面板 (固定右侧，与最右侧时间面板保持固定 24px 间距) */
.gold-panel {
  display: flex;
  align-items: center;
  gap: 4px;
  min-width: 62px;
  margin-left: auto;
  margin-right: 24px;
}

.gold-count {
  font-size: 0.85rem;
  font-weight: 600;
  color: var(--text-muted);
}

.gold-icon {
  width: 14px;
  height: 14px;
  color: #d97706;
  flex-shrink: 0;
}

/* 5. 装备面板 */
.items-panel {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}

.items-grid {
  display: flex;
  gap: 2px;
}

.item-slot,
.ward-slot {
  width: 28px;
  height: 28px;
  background-color: var(--hover-bg);
  border-radius: 4px;
  overflow: hidden;
  border: 1px solid var(--border-color);
}

.item-img {
  width: 100%;
  height: 100%;
  display: block;
}

.ward-slot {
  border-color: rgba(245, 158, 11, 0.3);
  background-color: rgba(245, 158, 11, 0.05);
  margin-left: 2px;
}

/* 海克斯强化图标 */
.augment-grid {
  display: flex;
  gap: 2px;
  margin-right: 4px;
}
.augment-slot {
  width: 22px;
  height: 22px;
  border-radius: 4px;
  overflow: hidden;
  border: 1px solid rgba(168, 85, 247, 0.45);
  background-color: rgba(147, 51, 234, 0.12);
  transition: all 0.2s ease;
  cursor: pointer;
}
.augment-slot:hover {
  border-color: #c084fc;
  box-shadow: 0 0 6px rgba(192, 132, 252, 0.6);
  transform: translateY(-1px) scale(1.05);
}
.augment-img {
  width: 100%;
  height: 100%;
  display: block;
}

/* 7. 时间面板 */
.time-panel {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  min-width: 80px;
  font-size: 0.82rem;
  color: var(--text-dimmed);
  white-space: nowrap;
  flex-shrink: 0;
}

.game-map,
.match-time {
  white-space: nowrap;
}

.map-name {
  font-weight: 600;
  color: var(--text-muted);
}

.match-time {
  margin-top: 4px;
}


/*
 * .death-red 在父组件的汇总栏同样使用；scoped 样式不跨组件生效，此处保留一份定义。
 * .win-text / .lose-text 随卡片样式一起迁入（见下方 card_css_a），父组件另行保留副本。
 */
.death-red {
  color: var(--death-color, var(--loss-color));
}

.career-aug-tooltip {
  max-width: 280px;
  padding: 8px 10px;
  text-align: left;
  border-radius: var(--radius-md);
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  box-shadow: var(--shadow-md);
}

.career-aug-tooltip-name {
  font-weight: 600;
  font-size: 0.85rem;
  color: var(--text-color);
  letter-spacing: 0.2px;
}

@keyframes fadeIn {
  from {
    opacity: 0;
    transform: translateY(6px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.replay-action-panel {
  display: flex;
  align-items: center;
  margin-left: 14px;
  flex-shrink: 0;
  opacity: 0;
  pointer-events: none;
  transition: opacity 0.2s cubic-bezier(0.4, 0, 0.2, 1);
}

.match-card:hover .replay-action-panel,
.replay-action-panel:focus-within {
  opacity: 1;
  pointer-events: auto;
}
</style>
