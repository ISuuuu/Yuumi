<script setup lang="ts">
import MatchListItem from "./MatchListItem.vue";
import type { MatchDisplay } from "../../api/lcu";

defineProps<{
  matches: MatchDisplay[];
  selectedGameId: number | null;
  currentPageNum: number;
  hasMore: boolean;
}>();

const emit = defineEmits<{
  (e: "select", gameId: number): void;
  (e: "prev"): void;
  (e: "next"): void;
}>();
</script>

<template>
  <div class="mini-match-list">
    <MatchListItem
      v-for="m in matches"
      :key="m.gameId"
      :match="m"
      :selected="selectedGameId === m.gameId"
      @select="emit('select', $event)"
    />
  </div>

  <!-- 翻页控制 -->
  <div class="pagination">
    <button
      class="page-btn"
      :disabled="currentPageNum <= 1"
      @click="emit('prev')"
    >
      <svg
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
      >
        <polyline points="15 18 9 12 15 6" />
      </svg>
    </button>
    <span class="page-num">{{ currentPageNum }}</span>
    <button
      class="page-btn"
      :disabled="!hasMore"
      @click="emit('next')"
    >
      <svg
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
      >
        <polyline points="9 18 15 12 9 6" />
      </svg>
    </button>
  </div>
</template>

<style scoped>
.mini-match-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex: 1;
  overflow-y: auto;
  padding-right: 2px;
}

/* 分页 */
.pagination {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 16px;
  margin-top: 8px;
  background: var(--card-bg);
  border: 1px solid rgba(0, 0, 0, 0.05);
  padding: 6px;
  border-radius: 6px;
  box-shadow: var(--shadow-sm);
}

.page-btn {
  background: transparent;
  border: none;
  border-radius: 4px;
  width: 28px;
  height: 28px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  color: var(--text-color);
  transition: all 0.2s;
}

.page-btn:hover:not(:disabled) {
  background: rgba(0, 0, 0, 0.03);
  color: var(--primary-color);
}

.page-btn:disabled {
  opacity: 0.3;
  cursor: not-allowed;
}

.page-btn svg {
  width: 14px;
  height: 14px;
}

.page-num {
  font-size: 0.82rem;
  font-weight: 700;
  color: var(--text-color);
}
</style>
