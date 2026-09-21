<script setup lang="ts">
import { ref, computed } from "vue";
import { fetchLcuFriends, batchDeleteFriends, lcuRequest, type LcuFriend } from "../../api/lcu";
import { useToast } from "../../composables/useToast";
import { useI18n } from "vue-i18n";
import LcuImage from "../LcuImage.vue";
import {
  NButton,
  NInput,
  NSelect,
  NCheckbox,
  NModal,
  NTag,
  NCollapse,
  NCollapseItem,
} from "naive-ui";

const { showToast } = useToast();
const { t } = useI18n();

const loading = ref(false);
const deleting = ref(false);
const hasScanned = ref(false);
const friends = ref<LcuFriend[]>([]);
const selectedIds = ref<Set<string>>(new Set());
const searchQuery = ref("");
// 默认筛选为 180 天
const filterType = ref<"days180" | "days360" | "days90" | "never" | "hidden" | "all">("days180");
const showConfirmModal = ref(false);

interface FriendMatchStatus {
  lastGameTime: number | null;
  status: "ok" | "online" | "mastery" | "hidden" | "none";
}

// 好友最近对局战绩时间缓存：puuid -> FriendMatchStatus
const matchHistoryMap = ref<Map<string, FriendMatchStatus>>(new Map());

function getFriendName(f: LcuFriend): string {
  if (f.gameName) {
    return f.gameTag ? `${f.gameName}#${f.gameTag}` : f.gameName;
  }
  return f.name || "未知召唤师";
}

// 查询单个好友最后一场对局时间（结合在线状态、战绩与熟练度穿透检测）
async function checkFriendLastMatch(friend: LcuFriend) {
  const puuid = friend.puuid;
  if (!puuid) return;

  // 1. 在线保护：如果当前好友处于在线、游戏中或离开，直接认定活跃
  if (friend.availability !== "offline") {
    matchHistoryMap.value.set(puuid, { lastGameTime: Date.now(), status: "online" });
    return;
  }

  // 2. 查询官方比赛战绩
  try {
    const res = await lcuRequest<{
      games?: { games?: Array<{ gameCreation?: number; gameCreationDate?: string }> };
      httpStatus?: number;
      errorCode?: string;
      message?: string;
    }>(
      "GET",
      `/lol-match-history/v1/products/lol/${puuid}/matches?begIndex=0&endIndex=1`,
    );

    // 成功查到对局
    if (res.success && res.data?.games?.games && res.data.games.games.length > 0) {
      const g = res.data.games.games[0];
      const ts = g.gameCreation || (g.gameCreationDate ? new Date(g.gameCreationDate).getTime() : 0);
      matchHistoryMap.value.set(puuid, { lastGameTime: ts, status: "ok" });
      return;
    }

    // 3. 战绩受限/隐藏/为空时，尝试通过英雄熟练度 lastPlayTime 穿透获取真实最后游玩时间
    const masteryRes = await lcuRequest<Array<{ lastPlayTime?: number }>>(
      "GET",
      `/lol-champion-mastery/v1/${puuid}/champion-mastery`,
    );
    if (masteryRes.success && Array.isArray(masteryRes.data) && masteryRes.data.length > 0) {
      let maxPlayTime = 0;
      for (const m of masteryRes.data) {
        if (m.lastPlayTime && m.lastPlayTime > maxPlayTime) {
          maxPlayTime = m.lastPlayTime;
        }
      }
      if (maxPlayTime > 0) {
        matchHistoryMap.value.set(puuid, { lastGameTime: maxPlayTime, status: "mastery" });
        return;
      }
    }

    // 4. 判断是否被隐私保护（隐藏战绩）拦截
    const errText = (res.error || res.data?.message || "").toUpperCase();
    const isPrivate =
      !res.success &&
      (errText.includes("403") ||
        errText.includes("PRIVACY") ||
        errText.includes("PRIVATE") ||
        errText.includes("FORBIDDEN"));

    if (isPrivate) {
      matchHistoryMap.value.set(puuid, { lastGameTime: null, status: "hidden" });
    } else {
      matchHistoryMap.value.set(puuid, { lastGameTime: null, status: "none" });
    }
  } catch {
    matchHistoryMap.value.set(puuid, { lastGameTime: null, status: "hidden" });
  }
}

// 计算距最后一场对局的天数 (null 表示从未对局或战绩已隐藏)
function getInactiveDays(f: LcuFriend): number | null {
  const info = matchHistoryMap.value.get(f.puuid);
  if (!info || info.status === "hidden" || info.lastGameTime === null || info.lastGameTime <= 0) {
    return null;
  }
  const diffMs = Date.now() - info.lastGameTime;
  return Math.max(0, Math.floor(diffMs / (1000 * 60 * 60 * 24)));
}

function formatLastMatchTime(f: LcuFriend): string {
  const info = matchHistoryMap.value.get(f.puuid);
  if (info?.status === "online") return "当前在线/游戏中";
  if (info?.status === "hidden") return "🔒 战绩已隐藏";
  if (info?.status === "none") return "从未对局 (0场战绩)";

  const days = getInactiveDays(f);
  if (days === null) {
    return "无对局记录";
  }
  if (days === 0) return "今天曾对局";
  if (days < 30) return `${days} 天前对局`;
  if (days < 365) return `${Math.floor(days / 30)} 个月前 (${days}天)`;
  return `${(days / 365).toFixed(1)} 年前 (${days}天)`;
}

function getTagType(f: LcuFriend): "success" | "error" | "warning" | "default" {
  const info = matchHistoryMap.value.get(f.puuid);
  if (info?.status === "online") return "success";
  if (info?.status === "hidden") return "warning";
  if (info?.status === "none") return "error";

  const days = getInactiveDays(f);
  if (days === null) return "default";
  if (days >= 180) return "error";
  if (days >= 90) return "warning";
  if (days < 30) return "success";
  return "default";
}

// 动态下拉统计选项
const filterOptions = computed(() => {
  const countAll = friends.value.length;
  const count360 = friends.value.filter((f) => {
    const d = getInactiveDays(f);
    return d !== null && d >= 360;
  }).length;
  const count180 = friends.value.filter((f) => {
    const d = getInactiveDays(f);
    return d !== null && d >= 180;
  }).length;
  const count90 = friends.value.filter((f) => {
    const d = getInactiveDays(f);
    return d !== null && d >= 90;
  }).length;
  const countNever = friends.value.filter((f) => {
    const info = matchHistoryMap.value.get(f.puuid);
    return info?.status === "none";
  }).length;
  const countHidden = friends.value.filter((f) => {
    const info = matchHistoryMap.value.get(f.puuid);
    return info?.status === "hidden";
  }).length;

  return [
    { label: `超过 180 天未打游戏 (${count180})`, value: "days180" },
    { label: `超过 360 天未打游戏 (${count360})`, value: "days360" },
    { label: `超过 90 天未打游戏 (${count90})`, value: "days90" },
    { label: `从未打过游戏 (${countNever})`, value: "never" },
    { label: `🔒 战绩已隐藏 (${countHidden})`, value: "hidden" },
    { label: `全部好友 (${countAll})`, value: "all" },
  ];
});

// 筛选后的好友列表
const filteredFriends = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  return friends.value.filter((f) => {
    if (q) {
      const name = getFriendName(f).toLowerCase();
      const note = (f.note || "").toLowerCase();
      if (!name.includes(q) && !note.includes(q)) return false;
    }

    const info = matchHistoryMap.value.get(f.puuid);
    if (filterType.value === "all") return true;
    if (filterType.value === "hidden") return info?.status === "hidden";
    if (filterType.value === "never") return info?.status === "none";

    // 对于 90/180/360 天筛选，绝对排除隐藏战绩的号，防止误删！
    if (info?.status === "hidden") return false;

    const days = getInactiveDays(f);
    if (filterType.value === "days90") return days !== null && days >= 90;
    if (filterType.value === "days180") return days !== null && days >= 180;
    if (filterType.value === "days360") return days !== null && days >= 360;
    return true;
  });
});

// 扫描好友并自动检测每位好友的最后对局时间
async function handleScanFriends() {
  loading.value = true;
  try {
    const resp = await fetchLcuFriends();
    if (resp.success && Array.isArray(resp.data)) {
      friends.value = resp.data;
      hasScanned.value = true;
      selectedIds.value.clear();

      // 并发分批检测每位好友的最后对局与活跃状态
      const validFriends = resp.data.filter((f) => !!f.puuid);
      const chunkSize = 5;
      for (let i = 0; i < validFriends.length; i += chunkSize) {
        const chunk = validFriends.slice(i, i + chunkSize);
        await Promise.all(chunk.map((f) => checkFriendLastMatch(f)));
      }

      showToast(`扫描完成，共获取 ${resp.data.length} 位好友并完成战绩分析`, "success");
    } else {
      showToast("获取好友列表失败，请确认客户端已登录", "error");
    }
  } catch (err) {
    showToast("扫描好友失败: " + String(err), "error");
  } finally {
    loading.value = false;
  }
}

// 选择控制
function toggleSelect(id: string) {
  const next = new Set(selectedIds.value);
  if (next.has(id)) {
    next.delete(id);
  } else {
    next.add(id);
  }
  selectedIds.value = next;
}

function selectAllFiltered() {
  const next = new Set(selectedIds.value);
  filteredFriends.value.forEach((f) => next.add(f.id));
  selectedIds.value = next;
}

function clearSelection() {
  selectedIds.value.clear();
}

const selectedFriendsList = computed(() => {
  return friends.value.filter((f) => selectedIds.value.has(f.id));
});

function confirmBatchDelete() {
  if (selectedIds.value.size === 0) return;
  showConfirmModal.value = true;
}

// 执行批量删除
async function executeBatchDelete() {
  if (selectedIds.value.size === 0) return;
  deleting.value = true;
  const idsToDelete = Array.from(selectedIds.value);
  try {
    const deletedCount = await batchDeleteFriends(idsToDelete);
    showToast(`成功清理 ${deletedCount} 位好友`, "success");
    showConfirmModal.value = false;
    // 重新扫描刷新
    await handleScanFriends();
  } catch (err) {
    showToast("清理好友过程中出错: " + String(err), "error");
  } finally {
    deleting.value = false;
  }
}
</script>

<template>
  <n-collapse arrow-placement="right" class="collapse-card">
    <n-collapse-item name="friendcleaner">
      <template #header>
        <div class="collapse-header-wrapper">
          <div class="collapse-left">
            <div class="icon-container">
              <svg
                class="header-icon"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"></path>
                <circle cx="9" cy="7" r="4"></circle>
                <path d="M23 21v-2a4 4 0 0 0-3-3.87"></path>
                <path d="M16 3.13a4 4 0 0 1 0 7.75"></path>
              </svg>
            </div>
            <div class="title-container">
              <h3 class="card-title">{{ t("tools.friendCleaner.title") }}</h3>
              <span class="card-desc">{{ t("tools.friendCleaner.desc") }}</span>
            </div>
          </div>
          <div class="collapse-right-status">
            <span class="status-preview" v-if="hasScanned">
              共 {{ friends.length }} 人 / 筛选出 {{ filteredFriends.length }} 人
            </span>
          </div>
        </div>
      </template>

      <!-- 扫描与操作栏 -->
      <div class="cleaner-actions-wrapper">
        <div class="cleaner-row-filter">
          <n-button
            type="primary"
            size="small"
            :loading="loading"
            @click="handleScanFriends"
          >
            {{ hasScanned ? "重新扫描好友" : "开始扫描好友" }}
          </n-button>

          <template v-if="hasScanned">
            <n-select
              v-model:value="filterType"
              :options="filterOptions"
              size="small"
              style="width: 250px"
            />
            <n-input
              v-model:value="searchQuery"
              placeholder="搜索游戏名/备注"
              size="small"
              clearable
              style="width: 170px"
            />
          </template>
        </div>

        <div v-if="hasScanned" class="cleaner-row-actions">
          <n-button size="small" secondary @click="selectAllFiltered">
            勾选当前列表
          </n-button>
          <n-button size="small" secondary @click="clearSelection">
            清空勾选
          </n-button>
          <n-button
            type="error"
            size="small"
            :disabled="selectedIds.size === 0"
            @click="confirmBatchDelete"
          >
            批量删除选中的 {{ selectedIds.size }} 人
          </n-button>
        </div>
      </div>

      <!-- 好友网格列表 -->
      <div v-if="hasScanned" class="friends-list-container">
        <div v-if="filteredFriends.length === 0" class="empty-hint">
          无符合当前筛选条件的好友
        </div>
        <div v-else class="friends-grid">
          <div
            v-for="friend in filteredFriends"
            :key="friend.id"
            class="friend-item"
            :class="{ selected: selectedIds.has(friend.id) }"
            @click="toggleSelect(friend.id)"
          >
            <n-checkbox
              :checked="selectedIds.has(friend.id)"
              @click.stop="toggleSelect(friend.id)"
            />
            <LcuImage
              :src="`/lol-game-data/assets/v1/profile-icons/${friend.icon}.png`"
              class="friend-avatar"
            />
            <div class="friend-info">
              <div class="friend-name-row">
                <span class="friend-name" :title="getFriendName(friend)">
                  {{ getFriendName(friend) }}
                </span>
                <n-tag
                  size="tiny"
                  :bordered="false"
                  :type="getTagType(friend)"
                >
                  {{ formatLastMatchTime(friend) }}
                </n-tag>
              </div>
              <div class="friend-meta-row">
                <span v-if="friend.note" class="friend-note" :title="friend.note">
                  备注: {{ friend.note }}
                </span>
                <span v-else class="friend-group">
                  分组: {{ friend.groupName || "默认" }}
                </span>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- 二次确认弹窗 -->
      <n-modal
        v-model:show="showConfirmModal"
        preset="dialog"
        type="error"
        title="确认批量删除好友？"
        positive-text="确认删除"
        negative-text="取消"
        :loading="deleting"
        @positive-click="executeBatchDelete"
      >
        <div class="confirm-content">
          <p class="warning-text">
            您即将从游戏中解除并删除以下 <strong>{{ selectedIds.size }}</strong> 位好友关系。此操作<strong>无法撤销</strong>，请仔细核对！
          </p>
          <div class="delete-preview-list">
            <div
              v-for="f in selectedFriendsList"
              :key="f.id"
              class="preview-item"
            >
              <span>{{ getFriendName(f) }}</span>
              <span class="preview-days">{{ formatLastMatchTime(f) }}</span>
            </div>
          </div>
        </div>
      </n-modal>
    </n-collapse-item>
  </n-collapse>
</template>

<style scoped>
.collapse-header-wrapper {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
}

.collapse-left {
  display: flex;
  align-items: center;
  flex: 1;
  gap: 14px;
}

.icon-container {
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-muted);
  flex-shrink: 0;
}

.header-icon {
  width: 18px;
  height: 18px;
  stroke-width: 2px;
}

.title-container {
  display: flex;
  flex-direction: column;
}

.card-title {
  font-size: 0.95rem;
  font-weight: 600;
  color: var(--text-color);
  margin: 0;
}

.card-desc {
  font-size: 0.78rem;
  color: var(--text-muted);
  margin-top: 2px;
}

.collapse-right-status {
  display: flex;
  align-items: center;
  margin-right: 8px;
}

.status-preview {
  font-size: 0.8rem;
  color: var(--text-muted);
}

.cleaner-actions-wrapper {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 8px 4px 14px 4px;
  border-bottom: 1px dashed var(--border-color);
}

.cleaner-row-filter {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
}

.cleaner-row-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
}

.friends-list-container {
  margin-top: 14px;
}

.empty-hint {
  padding: 30px;
  text-align: center;
  font-size: 0.85rem;
  color: var(--text-muted);
}

.friends-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 10px;
  max-height: 420px;
  overflow-y: auto;
  padding-right: 4px;
}

.friend-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border-radius: 8px;
  background: var(--bg-hover);
  border: 1px solid transparent;
  cursor: pointer;
  transition: all 0.2s ease;
}

.friend-item:hover {
  background: rgba(0, 0, 0, 0.06);
}

.friend-item.selected {
  border-color: #f43f5e;
  background: rgba(244, 63, 94, 0.08);
}

.friend-avatar {
  width: 34px;
  height: 34px;
  border-radius: 50%;
  flex-shrink: 0;
}

.friend-info {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
}

.friend-name-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
}

.friend-name {
  font-size: 0.84rem;
  font-weight: 500;
  color: var(--text-color);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.friend-meta-row {
  display: flex;
  align-items: center;
  font-size: 0.72rem;
  color: var(--text-muted);
  margin-top: 2px;
}

.friend-note,
.friend-group {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.confirm-content {
  font-size: 0.88rem;
}

.warning-text {
  color: var(--text-color);
  line-height: 1.5;
  margin-bottom: 12px;
}

.delete-preview-list {
  max-height: 180px;
  overflow-y: auto;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  padding: 6px 10px;
  background: var(--bg-hover);
}

.preview-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 4px 0;
  font-size: 0.8rem;
  border-bottom: 1px solid rgba(0, 0, 0, 0.04);
}

.preview-days {
  color: var(--text-muted);
  font-size: 0.75rem;
}
</style>
