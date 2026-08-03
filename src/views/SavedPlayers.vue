<script setup lang="ts">
import { ref, onMounted, watch, computed } from "vue";
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
import LcuOfflineState from "../components/LcuOfflineState.vue";

const { t: $t } = useI18n();
const { showToast } = useToast();
const store = useLcuStore();

const loading = ref(false);
const players = ref<SavedPlayer[]>([]);
const total = ref(0);
const page = ref(1);
const pageSize = 50;
const filter = ref<"tagged" | "multiple" | "all">("all");
const selfPuuid = ref("");
const expandedPuuid = ref<string | null>(null);
const encounteredGames = ref<EncounteredGame[]>([]);
const encounteredTotal = ref(0);
const encounteredPage = ref(1);
const encounteredPageSize = 20;
const gamesLoading = ref(false);
const editingPuuid = ref<string | null>(null);
const editingTag = ref("");

const isConnected = computed(() => store.isConnected);

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
  };
  return fallbackMap[queueType] || queueType;
}

async function loadPlayers(append = false) {
  if (!selfPuuid.value) return;
  if (!append) {
    page.value = 1;
  }
  loading.value = true;
  try {
    const res: PageResult<SavedPlayer> = await queryAllSavedPlayers(
      selfPuuid.value,
      page.value,
      pageSize,
      filter.value === "all" ? undefined : filter.value,
    );
    if (append) {
      players.value = [...players.value, ...res.data];
    } else {
      players.value = res.data;
    }
    total.value = res.count;
  } catch (e) {
    console.error("[SavedPlayers] 加载失败:", e);
    showToast($t("savedPlayersPage.loadFailed"), "error");
  } finally {
    loading.value = false;
  }
}

function switchFilter(next: "tagged" | "multiple" | "all") {
  filter.value = next;
  loadPlayers();
}

async function loadGames(puuid: string) {
  if (expandedPuuid.value === puuid) {
    expandedPuuid.value = null;
    encounteredGames.value = [];
    return;
  }
  expandedPuuid.value = puuid;
  gamesLoading.value = true;
  try {
    const res: PageResult<EncounteredGame> = await queryEncounteredGames(
      selfPuuid.value,
      puuid,
      undefined,
      1,
      encounteredPageSize,
    );
    encounteredGames.value = res.data;
    encounteredTotal.value = res.count;
    encounteredPage.value = 1;
  } catch (e) {
    console.error("[SavedPlayers] 相遇记录加载失败:", e);
    encounteredGames.value = [];
    encounteredTotal.value = 0;
  } finally {
    gamesLoading.value = false;
  }
}

async function loadMoreGames() {
  const puuid = expandedPuuid.value;
  if (!puuid || gamesLoading.value) return;
  gamesLoading.value = true;
  try {
    const res: PageResult<EncounteredGame> = await queryEncounteredGames(
      selfPuuid.value,
      puuid,
      undefined,
      encounteredPage.value + 1,
      encounteredPageSize,
    );
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
    if (!selfPuuid.value) {
      const me = await fetchCurrentSummoner(5);
      selfPuuid.value = me.puuid;
    }
    if (isConnected.value) {
      try {
        await backfillSavedPlayerIdentity();
      } catch (e) {
        console.warn("[SavedPlayers] 回填召唤师 ID 失败:", e);
      }
    }
    await loadPlayers();
  } catch (e) {
    console.warn("[SavedPlayers] 未连接客户端:", e);
  } finally {
    refreshing = false;
  }
}

onMounted(() => {
  void refreshRoster();
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
    <LcuOfflineState v-if="!isConnected" />

    <div v-else class="saved-players-content">
      <div class="page-header">
        <div>
          <h1 class="page-title">{{ $t("savedPlayersPage.title") }}</h1>
          <span class="page-desc">{{ $t("savedPlayersPage.desc") }}</span>
        </div>
        <div class="page-actions">
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
          <button class="action-btn" @click="handleImport">
            {{ $t("savedPlayersPage.import") }}
          </button>
          <button class="action-btn" @click="handleExport">
            {{ $t("savedPlayersPage.export") }}
          </button>
        </div>
      </div>

      <div v-if="loading" class="tip-container">
        <div class="loading-spinner"></div>
        <p class="tip">{{ $t("settings.loadingData") }}</p>
      </div>

      <div v-else-if="players.length === 0" class="empty-container">
        <p class="empty-tip">{{ $t("savedPlayersPage.empty") }}</p>
        <p class="empty-hint">{{ $t("savedPlayersPage.emptyHint") }}</p>
      </div>

    <div v-else class="player-list">
      <div v-for="player in players" :key="player.puuid" class="player-card">
        <div class="player-main" @click="loadGames(player.puuid)">
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

      <div v-if="total > players.length" class="pagination-row">
        <button
          class="mini-btn"
          @click="page++; loadPlayers(true)"
          :disabled="loading"
        >
          {{ $t("savedPlayersPage.loadMore") }}
        </button>
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
  align-items: flex-start;
  justify-content: space-between;
  margin-bottom: 20px;
}

.page-title {
  font-size: 1.4rem;
  font-weight: 800;
  color: var(--text-color);
  margin: 0 0 0.4rem 0;
}

.page-desc {
  display: block;
  margin-top: 4px;
  font-size: 13px;
  color: var(--text-dimmed);
}

.page-actions {
  display: flex;
  gap: 8px;
}

.action-btn {
  padding: 6px 14px;
  border-radius: var(--radius-md);
  border: 1px solid var(--border-color);
  background: var(--card-bg);
  color: var(--text-color);
  cursor: pointer;
  font-size: 13px;
  transition: all 0.2s;
}

.action-btn:hover:not(:disabled) {
  border-color: var(--primary-color);
  color: var(--primary-color);
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
</style>
