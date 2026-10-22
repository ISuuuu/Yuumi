<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, computed } from "vue";
import { useI18n } from "vue-i18n";
import { useLcuStore } from "../store/lcuStore";
import {
  fetchCurrentSummoner,
  queryAllSavedPlayers,
  queryEncounteredGames,
  saveSavedPlayer,
  deleteSavedPlayer,
  backfillSavedPlayerIdentity,
  exportTaggedPlayersToJsonFile,
  importTaggedPlayersFromJsonFile,
} from "../api/lcu";
import type {
  SavedPlayer,
  EncounteredGame,
  PageResult,
} from "../api/lcu";
import { useToast } from "../composables/useToast";
import LcuImage from "../components/LcuImage.vue";

const { t: $t } = useI18n();
const { showToast } = useToast();
const store = useLcuStore();

const loading = ref(false);
const loadingMore = ref(false);
const loadMoreError = ref(false);
const players = ref<SavedPlayer[]>([]);
const total = ref(0);
const page = ref(1);
const pageSize = 50;
const filter = ref<"tagged" | "multiple" | "all">("all");
const searchQuery = ref("");
let searchTimer: ReturnType<typeof setTimeout> | null = null;
const selfPuuid = ref("");
const expandedPuuid = ref<string | null>(null);
const encounteredGames = ref<EncounteredGame[]>([]);
const encounteredTotal = ref(0);
const encounteredPage = ref(1);
const encounteredPageSize = 20;
const gamesLoading = ref(false);
const editingPuuid = ref<string | null>(null);
const editingTag = ref("");

const scrollContainerRef = ref<HTMLElement | null>(null);
const sentinelRef = ref<HTMLElement | null>(null);
let observer: IntersectionObserver | null = null;

const isConnected = computed(() => store.isConnected);
const hasMore = computed(() => players.value.length < total.value);

function profileIconUrl(id: number): string {
  return id > 0
    ? `/lol-game-data/assets/v1/profile-icons/${id}.jpg`
    : "/images/default-avatar.svg";
}

function championIconUrl(id: number): string {
  return id > 0 ? `/lol-game-data/assets/v1/champion-icons/${id}.png` : "";
}

function formatTime(ts: number | null): string {
  if (!ts) return $t("savedPlayersPage.noLastMet");
  const d = new Date(ts);
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(
    d.getHours(),
  )}:${pad(d.getMinutes())}`;
}

function getQueueName(queueType: string | undefined | null): string {
  if (!queueType) return "";
  const key = `gameModes.${queueType}`;
  const translated = $t(key);
  if (translated && !translated.startsWith("gameModes.")) {
    return translated;
  }

  const fallbackMap: Record<string, string> = {
    "0": "自定义模式",
    "400": "征召模式",
    "420": "单双排位",
    "430": "匹配模式",
    "440": "灵活排位",
    "450": "极地大乱斗",
    "480": "快速模式",
    "490": "快速模式",
    "900": "无限火力",
    "1010": "随机无限火力",
    "1020": "克隆模式",
    "1300": "极限闪击",
    "1700": "斗魂竞技场",
    "1710": "斗魂竞技场",
    "2400": "海克斯大乱斗",
    "2450": "经典海斗",
  };
  return fallbackMap[queueType] || queueType;
}

/// 列表请求代际：快速改搜索词/切筛选时，只让最后一次请求的结果落地
let listReqSeq = 0;

async function loadPlayers() {
  const reqId = ++listReqSeq;
  page.value = 1;
  loading.value = true;
  loadMoreError.value = false;
  try {
    const trimmed = searchQuery.value.trim();
    const res: PageResult<SavedPlayer> = await queryAllSavedPlayers(
      selfPuuid.value,
      1,
      pageSize,
      filter.value === "all" ? undefined : filter.value,
      trimmed || undefined,
    );
    if (reqId !== listReqSeq) return;
    players.value = res.data;
    total.value = res.count;
    if (scrollContainerRef.value) {
      scrollContainerRef.value.scrollTop = 0;
    }
  } catch (e) {
    if (reqId !== listReqSeq) return;
    console.error("[SavedPlayers] 加载失败:", e);
    showToast($t("savedPlayersPage.loadFailed"), "error");
  } finally {
    if (reqId === listReqSeq) loading.value = false;
  }
}

async function loadMorePlayers() {
  if (loading.value || loadingMore.value || !hasMore.value) return;
  const reqId = listReqSeq;
  loadingMore.value = true;
  loadMoreError.value = false;
  const nextPage = page.value + 1;
  try {
    const trimmed = searchQuery.value.trim();
    const res: PageResult<SavedPlayer> = await queryAllSavedPlayers(
      selfPuuid.value,
      nextPage,
      pageSize,
      filter.value === "all" ? undefined : filter.value,
      trimmed || undefined,
    );
    // 期间搜索条件已变，丢弃这一页的追加结果
    if (reqId !== listReqSeq) return;
    page.value = nextPage;
    const existingKeys = new Set(
      players.value.map((p) => `${p.puuid}_${p.selfPuuid}`),
    );
    const newItems = res.data.filter(
      (p) => !existingKeys.has(`${p.puuid}_${p.selfPuuid}`),
    );
    players.value = [...players.value, ...newItems];
    total.value = res.count;
  } catch (e) {
    console.error("[SavedPlayers] 加载更多失败:", e);
    loadMoreError.value = true;
  } finally {
    loadingMore.value = false;
  }
}

function onScroll(e: Event) {
  if (loading.value || loadingMore.value || !hasMore.value) return;
  const target = e.target as HTMLElement;
  if (!target) return;
  if (target.scrollHeight - target.scrollTop - target.clientHeight <= 300) {
    void loadMorePlayers();
  }
}

function onSearchInput() {
  if (searchTimer) clearTimeout(searchTimer);
  searchTimer = setTimeout(() => {
    loadPlayers();
  }, 300);
}

function triggerSearch() {
  if (searchTimer) clearTimeout(searchTimer);
  loadPlayers();
}

function clearSearch() {
  if (searchTimer) clearTimeout(searchTimer);
  searchQuery.value = "";
  loadPlayers();
}

function switchFilter(next: "tagged" | "multiple" | "all") {
  filter.value = next;
  loadPlayers();
}

/// 相遇记录请求代际：快速展开不同玩家时，只让最后一次请求的结果落地
let gamesReqSeq = 0;

async function loadGames(puuid: string, playerSelfPuuid?: string) {
  if (expandedPuuid.value === puuid) {
    expandedPuuid.value = null;
    encounteredGames.value = [];
    return;
  }
  const reqId = ++gamesReqSeq;
  expandedPuuid.value = puuid;
  gamesLoading.value = true;
  try {
    const targetSelfPuuid = playerSelfPuuid || selfPuuid.value || "";
    const res: PageResult<EncounteredGame> = await queryEncounteredGames(
      targetSelfPuuid,
      puuid,
      undefined,
      1,
      encounteredPageSize,
    );
    if (reqId !== gamesReqSeq) return;
    encounteredGames.value = res.data;
    encounteredTotal.value = res.count;
    encounteredPage.value = 1;
  } catch (e) {
    if (reqId !== gamesReqSeq) return;
    console.error("[SavedPlayers] 相遇记录加载失败:", e);
    encounteredGames.value = [];
    encounteredTotal.value = 0;
  } finally {
    if (reqId === gamesReqSeq) gamesLoading.value = false;
  }
}

async function loadMoreGames() {
  const puuid = expandedPuuid.value;
  if (!puuid || gamesLoading.value) return;
  const targetPlayer = players.value.find((p) => p.puuid === puuid);
  gamesLoading.value = true;
  try {
    const targetSelfPuuid = targetPlayer?.selfPuuid || selfPuuid.value || "";
    const res: PageResult<EncounteredGame> = await queryEncounteredGames(
      targetSelfPuuid,
      puuid,
      undefined,
      encounteredPage.value + 1,
      encounteredPageSize,
    );
    // 翻页期间可能已切换到其他玩家，丢弃过期响应
    if (expandedPuuid.value !== puuid) return;
    encounteredGames.value = [...encounteredGames.value, ...res.data];
    encounteredTotal.value = res.count;
    encounteredPage.value += 1;
  } catch (e) {
    console.error("[SavedPlayers] 相遇记录加载更多失败:", e);
  } finally {
    gamesLoading.value = false;
  }
}

function startEdit(player: SavedPlayer) {
  editingPuuid.value = player.puuid;
  editingTag.value = player.tag ?? "";
}

async function saveTag(player: SavedPlayer) {
  try {
    await saveSavedPlayer({
      puuid: player.puuid,
      selfPuuid: player.selfPuuid,
      region: player.region,
      rsoPlatformId: player.rsoPlatformId,
      tag: editingTag.value.trim() || null,
      summonerName: player.summonerName,
      profileIconId: player.profileIconId,
    });
    player.tag = editingTag.value.trim() || null;
    editingPuuid.value = null;
  } catch (e) {
    console.error("[SavedPlayers] 保存标记失败:", e);
    showToast($t("savedPlayersPage.loadFailed"), "error");
  }
}

function cancelEdit() {
  editingPuuid.value = null;
}

async function removePlayer(player: SavedPlayer) {
  if (!window.confirm($t("savedPlayersPage.deleteConfirm"))) return;
  try {
    await deleteSavedPlayer(player.puuid, player.selfPuuid);
    if (expandedPuuid.value === player.puuid) {
      expandedPuuid.value = null;
      encounteredGames.value = [];
    }
    await loadPlayers();
  } catch (e) {
    console.error("[SavedPlayers] 删除失败:", e);
    showToast($t("savedPlayersPage.loadFailed"), "error");
  }
}

async function handleExport() {
  try {
    const path = await exportTaggedPlayersToJsonFile();
    if (path) {
      showToast($t("savedPlayersPage.exportSuccess", { path }));
    }
  } catch (e) {
    console.error("[SavedPlayers] 导出失败:", e);
    showToast($t("savedPlayersPage.loadFailed"), "error");
  }
}

async function handleImport() {
  try {
    const count = await importTaggedPlayersFromJsonFile();
    if (count > 0) {
      showToast($t("savedPlayersPage.importSuccess", { count }));
      await loadPlayers();
    }
  } catch (e) {
    console.error("[SavedPlayers] 导入失败:", e);
    showToast($t("savedPlayersPage.loadFailed"), "error");
  }
}

let refreshing = false;

async function refreshRoster() {
  if (refreshing) return;
  refreshing = true;
  try {
    if (isConnected.value) {
      try {
        if (!selfPuuid.value) {
          const me = await fetchCurrentSummoner(5);
          selfPuuid.value = me.puuid;
        }
        await backfillSavedPlayerIdentity();
      } catch (e) {
        console.warn("[SavedPlayers] 回填召唤师 ID 或获取当前召唤师失败:", e);
      }
    }
    await loadPlayers();
  } catch (e) {
    console.warn("[SavedPlayers] 刷新路人集失败:", e);
  } finally {
    refreshing = false;
  }
}

function initObserver() {
  if (typeof IntersectionObserver === "undefined") return;
  cleanupObserver();
  observer = new IntersectionObserver(
    (entries) => {
      const entry = entries[0];
      if (entry?.isIntersecting) {
        void loadMorePlayers();
      }
    },
    {
      root: scrollContainerRef.value || null,
      rootMargin: "0px 0px 300px 0px",
    },
  );
  if (sentinelRef.value) {
    observer.observe(sentinelRef.value);
  }
}

function cleanupObserver() {
  if (observer) {
    observer.disconnect();
    observer = null;
  }
}

watch(sentinelRef, (el) => {
  if (!observer) return;
  observer.disconnect();
  if (el) {
    observer.observe(el);
  }
});

onMounted(() => {
  void refreshRoster();
  initObserver();
});

onUnmounted(() => {
  cleanupObserver();
  if (searchTimer) {
    clearTimeout(searchTimer);
    searchTimer = null;
  }
});

let connectedBefore = isConnected.value;
watch(isConnected, (connected) => {
  if (connected && !connectedBefore) {
    void refreshRoster();
  }
  connectedBefore = connected;
});
</script>

<template>
  <div class="saved-players-view">
    <div
      ref="scrollContainerRef"
      class="saved-players-content"
      @scroll.passive="onScroll"
    >
      <div class="page-header">
        <div class="header-left">
          <h1 class="page-title">{{ $t("savedPlayersPage.title") }}</h1>
          <span v-if="total > 0" class="player-count-badge">{{ total }}</span>
          <span class="desc-dot">·</span>
          <span class="page-desc" :title="$t('savedPlayersPage.desc')">
            {{ $t("savedPlayersPage.desc") }}
          </span>
        </div>
        <div class="page-actions">
          <button class="action-btn" :title="$t('savedPlayersPage.import')" @click="handleImport">
            <svg class="action-btn-icon" viewBox="0 0 24 24" width="13" height="13">
              <path fill="currentColor" d="M19 9h-4V3H9v6H5l7 7 7-7zM5 18v2h14v-2H5z"/>
            </svg>
            <span>{{ $t("savedPlayersPage.import") }}</span>
          </button>
          <button class="action-btn" :title="$t('savedPlayersPage.export')" @click="handleExport">
            <svg class="action-btn-icon" viewBox="0 0 24 24" width="13" height="13">
              <path fill="currentColor" d="M5 15v4h14v-4h2v4c0 1.1-.9 2-2 2H5c-1.1 0-2-.9-2-2v-4h2zm7-13l7 7h-4v6H9V9H5l7-7z"/>
            </svg>
            <span>{{ $t("savedPlayersPage.export") }}</span>
          </button>
        </div>
      </div>

      <div class="toolbar-row">
        <div class="search-box">
          <svg class="search-icon" viewBox="0 0 24 24" width="14" height="14">
            <path
              fill="currentColor"
              d="M15.5 14h-.79l-.28-.27A6.471 6.471 0 0 0 16 9.5 6.5 6.5 0 1 0 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z"
            />
          </svg>
          <input
            v-model="searchQuery"
            class="search-input"
            type="text"
            :placeholder="$t('savedPlayersPage.searchPlaceholder')"
            @input="onSearchInput"
            @keyup.enter="triggerSearch"
          />
          <button
            v-if="searchQuery"
            class="search-clear-btn"
            title="清空"
            @click="clearSearch"
          >
            ✕
          </button>
        </div>

        <div class="filter-group">
          <button
            class="filter-btn"
            :class="{ active: filter === 'all' }"
            @click="switchFilter('all')"
          >
            {{ $t("savedPlayersPage.filterAll") }}
          </button>
          <button
            class="filter-btn"
            :class="{ active: filter === 'tagged' }"
            @click="switchFilter('tagged')"
          >
            {{ $t("savedPlayersPage.filterTagged") }}
          </button>
          <button
            class="filter-btn"
            :class="{ active: filter === 'multiple' }"
            @click="switchFilter('multiple')"
          >
            {{ $t("savedPlayersPage.filterMultiple") }}
          </button>
        </div>
      </div>

      <div v-if="loading" class="tip-container">
        <div class="loading-spinner"></div>
        <p class="tip">{{ $t("settings.loadingData") }}</p>
      </div>

      <div v-else-if="players.length === 0" class="empty-container">
        <template v-if="searchQuery.trim()">
          <p class="empty-tip">
            {{ $t("savedPlayersPage.searchEmpty", { keyword: searchQuery.trim() }) }}
          </p>
          <button class="mini-btn clear-search-btn" @click="clearSearch">
            {{ $t("savedPlayersPage.clearSearch") }}
          </button>
        </template>
        <template v-else>
          <p class="empty-tip">{{ $t("savedPlayersPage.empty") }}</p>
          <p class="empty-hint">{{ $t("savedPlayersPage.emptyHint") }}</p>
        </template>
      </div>

    <div v-else class="player-list">
      <div
        v-for="player in players"
        :key="player.puuid + '_' + player.selfPuuid"
        class="player-card"
      >
        <div class="player-main" @click="loadGames(player.puuid, player.selfPuuid)">
          <div class="player-avatar">
            <LcuImage
              :src="profileIconUrl(player.profileIconId)"
              fallback-src="/images/default-avatar.svg"
              alt="avatar"
            />
          </div>
          <div v-if="player.championId > 0" class="player-champ">
            <LcuImage
              :src="championIconUrl(player.championId)"
              fallback-src="/images/default-avatar.svg"
              alt="champ"
            />
          </div>
          <div class="player-info">
            <span class="player-name-line">
              <span class="player-name">{{ player.summonerName }}</span>
              <span class="player-tagline">
                {{ player.tagLine ? `#${player.tagLine}` : "#—" }}
              </span>
            </span>
            <div class="player-badges">
              <span v-if="player.tag" class="tag-badge">{{ player.tag }}</span>
              <span v-if="player.lastQueueType" class="queue-badge">
                {{ getQueueName(player.lastQueueType) }}
              </span>
              <span v-if="player.encounterCount >= 2" class="encounter-badge">
                {{ $t("savedPlayersPage.encounterCount", { count: player.encounterCount }) }}
              </span>
            </div>
          </div>
          <span class="player-last-met">{{ formatTime(player.lastMetAt) }}</span>
          <span class="expand-arrow" :class="{ open: expandedPuuid === player.puuid }">
            ▸
          </span>
        </div>

        <div class="player-edit-row">
          <div v-if="editingPuuid === player.puuid" class="edit-actions">
            <input
              v-model="editingTag"
              class="tag-input"
              :placeholder="$t('savedPlayersPage.tagPlaceholder')"
              @keyup.enter="saveTag(player)"
            />
            <button class="mini-btn" @click="saveTag(player)">
              {{ $t("savedPlayersPage.tagSave") }}
            </button>
            <button class="mini-btn" @click="cancelEdit">
              {{ $t("savedPlayersPage.tagCancel") }}
            </button>
          </div>
          <div v-else class="edit-actions">
            <button class="mini-btn" @click="startEdit(player)">
              {{ $t("savedPlayersPage.tagEdit") }}
            </button>
            <button class="mini-btn danger" @click="removePlayer(player)">
              {{ $t("savedPlayersPage.delete") }}
            </button>
          </div>
        </div>

        <div
          v-if="expandedPuuid === player.puuid"
          class="encountered-section"
        >
          <div v-if="gamesLoading" class="tip-container">
            <div class="loading-spinner"></div>
          </div>
          <div v-else-if="encounteredGames.length === 0" class="encountered-empty">
            {{ $t("savedPlayersPage.noEncounteredGames") }}
          </div>
          <div v-else class="encountered-list">
            <div v-for="game in encounteredGames" :key="game.id" class="encountered-item">
              <span class="encountered-queue">{{ getQueueName(game.queueType) }}</span>
              <span class="encountered-time">{{ formatTime(game.updateAt) }}</span>
            </div>
            <div v-if="encounteredTotal > encounteredGames.length" class="pagination-row">
              <button
                class="mini-btn"
                @click="loadMoreGames"
                :disabled="gamesLoading"
              >
                {{ $t("savedPlayersPage.loadMore") }}
              </button>
            </div>
          </div>
        </div>
      </div>

        <!-- 底部哨兵与自动加载更多状态 -->
        <div ref="sentinelRef" class="scroll-sentinel">
          <div v-if="loadingMore" class="scroll-status loading">
            <div class="mini-spinner"></div>
            <span>{{ $t("savedPlayersPage.loadingMore") }}</span>
          </div>
          <button
            v-else-if="loadMoreError"
            class="mini-btn retry-btn"
            @click="loadMorePlayers"
          >
            {{ $t("savedPlayersPage.loadMoreRetry") }}
          </button>
          <div v-else-if="!hasMore && players.length > 0" class="scroll-status ended">
            <span>{{ $t("savedPlayersPage.allLoaded") }}</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.saved-players-view {
  padding: 1rem 1.5rem 1rem 0.6rem;
  background-color: transparent;
  flex: 1;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.saved-players-content {
  max-width: 1000px;
  width: 100%;
  margin: 0 auto;
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow-y: auto;
}

.page-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
  gap: 12px;
  min-height: 32px;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  flex: 1;
}

.page-title {
  font-size: 1.35rem;
  font-weight: 800;
  color: var(--text-color);
  margin: 0;
  line-height: 1;
  white-space: nowrap;
}

.player-count-badge {
  font-size: 11px;
  font-weight: 600;
  padding: 1px 7px;
  border-radius: 999px;
  background: var(--primary-color-alpha-15);
  color: var(--primary-color);
  border: 1px solid var(--primary-color-alpha-30);
  line-height: 1.4;
  white-space: nowrap;
}

.desc-dot {
  color: var(--text-dimmed);
  opacity: 0.5;
  font-weight: 700;
  font-size: 12px;
  user-select: none;
}

.page-desc {
  font-size: 12px;
  color: var(--text-dimmed);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  min-width: 0;
}

.page-actions {
  display: flex;
  gap: 8px;
  flex-shrink: 0;
}

.action-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 5px 12px;
  border-radius: var(--radius-md);
  border: 1px solid var(--border-color);
  background: var(--card-bg);
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
  color: var(--text-color);
  cursor: pointer;
  font-size: 12px;
  font-weight: 500;
  transition: all 0.2s;
  white-space: nowrap;
}

.action-btn:hover:not(:disabled) {
  border-color: var(--primary-color);
  color: var(--primary-color);
  background: var(--primary-color-alpha-10, rgba(0, 159, 170, 0.1));
}

.action-btn-icon {
  flex-shrink: 0;
  opacity: 0.8;
}

.action-btn:hover:not(:disabled) .action-btn-icon {
  opacity: 1;
}

.toolbar-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 16px;
  flex-wrap: wrap;
}

.search-box {
  position: relative;
  display: flex;
  align-items: center;
  flex: 1;
  min-width: 240px;
  max-width: 360px;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  padding: 0 8px 0 10px;
  height: 32px;
  box-sizing: border-box;
  transition: all 0.2s ease;
}

.search-box:focus-within {
  border-color: var(--primary-color);
  box-shadow: 0 0 0 2px var(--primary-color-alpha-15, rgba(99, 102, 241, 0.15));
}

.search-icon {
  color: var(--text-dimmed);
  flex-shrink: 0;
  margin-right: 6px;
}

.search-input {
  flex: 1;
  min-width: 0;
  border: none;
  outline: none;
  background: transparent;
  color: var(--text-color);
  font-size: 13px;
}

.search-input::placeholder {
  color: var(--text-dimmed);
}

.search-clear-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  border: none;
  background: var(--border-color);
  color: var(--text-muted);
  border-radius: 50%;
  cursor: pointer;
  font-size: 10px;
  padding: 0;
  line-height: 1;
  transition: all 0.15s;
  flex-shrink: 0;
  margin-left: 4px;
}

.search-clear-btn:hover {
  background: var(--primary-color);
  color: #fff;
}

.clear-search-btn {
  margin-top: 6px;
}


.filter-group {
  display: flex;
  gap: 4px;
  padding: 3px;
  border-radius: var(--radius-lg);
  background: var(--bg-subtle);
  border: 1px solid var(--border-color);
  align-self: center;
}

.filter-btn {
  padding: 4px 12px;
  border-radius: var(--radius-md);
  border: none;
  background: transparent;
  color: var(--text-dimmed);
  cursor: pointer;
  font-size: 12px;
  transition: all 0.2s;
  white-space: nowrap;
}

.filter-btn:hover:not(:disabled) {
  color: var(--text-color);
}

.filter-btn.active {
  background: var(--primary-color);
  color: #fff;
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.15);
}

.filter-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.action-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.tip-container,
.empty-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 6rem 2rem;
  gap: 10px;
}

.tip-container {
  flex: 1;
}

.loading-spinner {
  width: 32px;
  height: 32px;
  border: 3px solid var(--border-color);
  border-top-color: var(--primary-color);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.tip,
.empty-tip {
  color: var(--text-muted);
  font-size: 0.95rem;
  margin: 0;
}

.empty-hint {
  color: var(--text-dimmed);
  font-size: 13px;
  margin: 0;
}

.player-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.player-card {
  background: var(--card-bg);
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-sm);
  padding: 12px 16px;
  transition: border-color 0.2s;
}

.player-card:hover {
  border-color: var(--primary-color-alpha-30);
}

.player-main {
  display: flex;
  align-items: center;
  gap: 12px;
  cursor: pointer;
}

.player-avatar {
  width: 44px;
  height: 44px;
  border-radius: 50%;
  overflow: hidden;
  border: 2px solid var(--border-color);
  flex-shrink: 0;
}

.player-champ {
  width: 34px;
  height: 34px;
  border-radius: 8px;
  overflow: hidden;
  border: 1px solid var(--border-color);
  flex-shrink: 0;
}

.player-info {
  display: flex;
  flex-direction: column;
  gap: 4px;
  flex: 1;
  min-width: 0;
}

.player-name-line {
  display: flex;
  align-items: baseline;
  gap: 6px;
  min-width: 0;
}

.player-name {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.player-tagline {
  font-size: 12px;
  color: var(--text-dimmed);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex-shrink: 1;
}

.player-badges {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
}

.tag-badge {
  display: inline-block;
  padding: 2px 8px;
  border-radius: 999px;
  background: var(--primary-color-alpha-15);
  color: var(--primary-color);
  font-size: 12px;
  max-width: 200px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  align-self: flex-start;
}

.queue-badge {
  display: inline-block;
  padding: 2px 8px;
  border-radius: var(--radius-sm);
  background: var(--border-color);
  color: var(--text-color);
  font-size: 11px;
  white-space: nowrap;
}

.encounter-badge {
  display: inline-block;
  padding: 2px 8px;
  border-radius: var(--radius-sm);
  background: var(--border-color);
  color: var(--text-muted);
  font-size: 11px;
  font-weight: 500;
  white-space: nowrap;
}

.player-last-met {
  font-size: 12px;
  color: var(--text-dimmed);
  flex-shrink: 0;
}

.expand-arrow {
  color: var(--text-muted);
  font-size: 14px;
  transition: transform 0.2s;
  flex-shrink: 0;
}

.expand-arrow.open {
  transform: rotate(90deg);
}

.player-edit-row {
  display: flex;
  justify-content: flex-end;
  margin-top: 8px;
}

.tag-input {
  width: 220px;
  padding: 6px 10px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border-color);
  background: transparent;
  color: var(--text-color);
  font-size: 13px;
  outline: none;
}

.tag-input:focus {
  border-color: var(--primary-color);
}

.edit-actions {
  display: flex;
  gap: 8px;
}

.mini-btn {
  padding: 4px 12px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border-color);
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
  font-size: 12px;
  transition: all 0.2s;
}

.mini-btn:hover {
  color: var(--primary-color);
  border-color: var(--primary-color);
}

.mini-btn.danger:hover {
  color: var(--death-color);
  border-color: var(--death-color);
}

.encountered-section {
  margin-top: 10px;
  padding-top: 10px;
  border-top: 1px dashed var(--border-color);
}

.encountered-empty {
  color: var(--text-dimmed);
  font-size: 13px;
  padding: 8px 0;
}

.encountered-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.encountered-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 10px;
  border-radius: var(--radius-sm);
  background: rgba(0, 0, 0, 0.04);
}

.encountered-queue {
  font-size: 13px;
  color: var(--text-color);
}

.encountered-time {
  font-size: 12px;
  color: var(--text-dimmed);
}

.pagination-row {
  display: flex;
  justify-content: center;
  padding: 12px 0;
}

.scroll-sentinel {
  display: flex;
  justify-content: center;
  align-items: center;
  min-height: 48px;
  padding: 12px 0 20px 0;
}

.scroll-status {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  font-size: 12px;
  color: var(--text-dimmed);
}

.scroll-status.ended {
  opacity: 0.6;
}

.mini-spinner {
  width: 14px;
  height: 14px;
  border: 2px solid var(--border-color);
  border-top-color: var(--primary-color);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

.retry-btn {
  color: var(--primary-color);
  border-color: var(--primary-color-alpha-30);
}
</style>
