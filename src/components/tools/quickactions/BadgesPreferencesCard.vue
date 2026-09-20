<script setup lang="ts">
import { ref, computed } from 'vue';
import {
  fetchPlayerChallenges,
  setPlayerPreferences,
  lcuRequest,
  type PlayerChallengeItem,
  type PlayerTitleItem,
} from '../../../api/lcu';
import { useToast } from '../../../composables/useToast';
import { useQuickActionsLoading } from './shared';
import {
  NButton,
  NModal,
  NSpin,
  NInput,
  NSelect,
} from 'naive-ui';

const { showToast } = useToast();
const globalLoading = useQuickActionsLoading();

const showModal = ref(false);
const modalLoading = ref(false);
const saving = ref(false);

// 全部挑战与称号
const challenges = ref<PlayerChallengeItem[]>([]);
const titles = ref<PlayerTitleItem[]>([]);

// 当前选中的称号 ID 与挑战 ID (最多 3 个)
const selectedTitleId = ref<string>('');
const selectedChallengeIds = ref<string[]>([]);

// 搜索与过滤
const titleSearch = ref('');
const badgeSearch = ref('');
const badgeTierFilter = ref<string>('all');

const tierFilterOptions = [
  { label: '全部段位', value: 'all' },
  { label: '王者 / 宗师', value: 'apex' },
  { label: '大师 / 钻石', value: 'high' },
  { label: '铂金 / 黄金', value: 'mid' },
  { label: '白银 / 黄铜 / 黑铁', value: 'low' },
];

// 打开装配弹窗
async function handleOpenModal() {
  showModal.value = true;
  modalLoading.value = true;
  try {
    // 1. 获取本地挑战数据
    const rawChallenges = await fetchPlayerChallenges();
    const challengeList: PlayerChallengeItem[] = Array.isArray(rawChallenges)
      ? rawChallenges
      : Object.values(rawChallenges);
    challenges.value = challengeList;

    // 2. 获取汇总信息（称号列表）
    const summaryResp = await lcuRequest<{
      title?: { itemId?: number | string; id?: number | string };
      titles?: PlayerTitleItem[];
    }>('GET', '/lol-challenges/v1/summary-player-data/local-player');

    if (summaryResp.success && summaryResp.data) {
      if (Array.isArray(summaryResp.data.titles)) {
        titles.value = summaryResp.data.titles;
      }
      if (summaryResp.data.title) {
        const cur = summaryResp.data.title.itemId ?? summaryResp.data.title.id;
        if (cur !== undefined && cur !== null) {
          selectedTitleId.value = String(cur);
        }
      }
    }

    // 3. 获取当前佩戴配置
    const prefResp = await lcuRequest<{
      title?: string;
      challengeIds?: (string | number)[];
    }>('GET', '/lol-challenges/v1/preferences/local-player');

    if (prefResp.success && prefResp.data) {
      if (prefResp.data.title !== undefined) {
        selectedTitleId.value = String(prefResp.data.title);
      }
      if (Array.isArray(prefResp.data.challengeIds)) {
        selectedChallengeIds.value = prefResp.data.challengeIds
          .map((id) => String(id))
          .filter((id) => id && id !== '0' && id !== '-1');
      }
    }
  } catch (err) {
    console.error('获取挑战与称号数据失败:', err);
    showToast('加载称号与勋章数据失败', 'error');
  } finally {
    modalLoading.value = false;
  }
}

// 称号过滤列表
const filteredTitles = computed(() => {
  let list = titles.value;
  if (titleSearch.value.trim()) {
    const q = titleSearch.value.trim().toLowerCase();
    list = list.filter((t) => t.name && t.name.toLowerCase().includes(q));
  }
  return list;
});

// 已获得的有效勋章列表（排除 NONE 等级）
const availableBadges = computed(() => {
  return challenges.value.filter((item) => {
    const level = (item.currentLevel || item.level || '').toUpperCase();
    return (
      item.id > 0 &&
      item.name &&
      level !== 'NONE' &&
      level !== ''
    );
  });
});

// 过滤后的勋章列表
const filteredBadges = computed(() => {
  let list = availableBadges.value;

  // 搜索
  if (badgeSearch.value.trim()) {
    const q = badgeSearch.value.trim().toLowerCase();
    list = list.filter(
      (b) =>
        (b.name && b.name.toLowerCase().includes(q)) ||
        (b.description && b.description.toLowerCase().includes(q)),
    );
  }

  // 段位过滤
  if (badgeTierFilter.value === 'apex') {
    list = list.filter((b) => {
      const lvl = (b.currentLevel || b.level || '').toUpperCase();
      return lvl === 'CHALLENGER' || lvl === 'GRANDMASTER';
    });
  } else if (badgeTierFilter.value === 'high') {
    list = list.filter((b) => {
      const lvl = (b.currentLevel || b.level || '').toUpperCase();
      return lvl === 'MASTER' || lvl === 'DIAMOND';
    });
  } else if (badgeTierFilter.value === 'mid') {
    list = list.filter((b) => {
      const lvl = (b.currentLevel || b.level || '').toUpperCase();
      return lvl === 'PLATINUM' || lvl === 'EMERALD' || lvl === 'GOLD';
    });
  } else if (badgeTierFilter.value === 'low') {
    list = list.filter((b) => {
      const lvl = (b.currentLevel || b.level || '').toUpperCase();
      return lvl === 'SILVER' || lvl === 'BRONZE' || lvl === 'IRON';
    });
  }

  return list;
});

// 切换勋章选择
function toggleBadge(badgeId: number | string) {
  const idStr = String(badgeId);
  const idx = selectedChallengeIds.value.indexOf(idStr);
  if (idx >= 0) {
    selectedChallengeIds.value.splice(idx, 1);
  } else {
    if (selectedChallengeIds.value.length >= 3) {
      showToast('最多只能佩戴 3 枚挑战勋章', 'warning');
      return;
    }
    selectedChallengeIds.value.push(idStr);
  }
}

// 移除单个已选勋章
function removeSelectedBadge(index: number) {
  selectedChallengeIds.value.splice(index, 1);
}

// 获取勋章详情
function getBadgeDetail(badgeIdStr: string): PlayerChallengeItem | undefined {
  const id = Number(badgeIdStr);
  return challenges.value.find((c) => c.id === id);
}

// 等级中文映射
function formatLevel(lvl?: string): string {
  if (!lvl) return '未定级';
  const map: Record<string, string> = {
    CHALLENGER: '王者',
    GRANDMASTER: '宗师',
    MASTER: '大师',
    DIAMOND: '钻石',
    EMERALD: '翡翠',
    PLATINUM: '铂金',
    GOLD: '黄金',
    SILVER: '白银',
    BRONZE: '黄铜',
    IRON: '黑铁',
  };
  return map[lvl.toUpperCase()] || lvl;
}

function getLevelClass(lvl?: string): string {
  const l = (lvl || '').toUpperCase();
  if (l === 'CHALLENGER') return 'level-challenger';
  if (l === 'GRANDMASTER') return 'level-grandmaster';
  if (l === 'MASTER') return 'level-master';
  if (l === 'DIAMOND') return 'level-diamond';
  if (l === 'EMERALD') return 'level-emerald';
  if (l === 'PLATINUM') return 'level-platinum';
  if (l === 'GOLD') return 'level-gold';
  return 'level-normal';
}

// 清空所有装配
function handleClearAll() {
  selectedTitleId.value = '';
  selectedChallengeIds.value = [];
}

// 保存装配
async function handleSavePreferences() {
  saving.value = true;
  globalLoading.value = true;
  try {
    await setPlayerPreferences(
      selectedTitleId.value,
      selectedChallengeIds.value,
    );
    showToast('称号与挑战勋章装配成功！', 'success');
    showModal.value = false;
  } catch (err) {
    console.error('保存装配失败:', err);
    showToast(`保存失败: ${String(err)}`, 'error');
  } finally {
    saving.value = false;
    globalLoading.value = false;
  }
}
</script>

<template>
  <div class="card-item border-bottom">
    <div class="card-left">
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
          <path d="M6 9H4.5a2.5 2.5 0 0 1 0-5H6" />
          <path d="M18 9h1.5a2.5 2.5 0 0 0 0-5H18" />
          <path d="M4 22h16" />
          <path d="M10 14.66V17c0 .55-.47.98-.97 1.21C7.85 18.75 7 20.24 7 22" />
          <path d="M14 14.66V17c0 .55.47.98.97 1.21C16.15 18.75 17 20.24 17 22" />
          <path d="M18 2H6v7a6 6 0 0 0 12 0V2Z" />
        </svg>
      </div>
      <div class="title-container">
        <h3 class="card-title">装配称号与挑战勋章</h3>
        <span class="card-desc">自定义佩戴个人名片的称号与 3 枚挑战勋章</span>
      </div>
    </div>
    <div class="card-right">
      <n-button
        class="action-btn text-primary"
        @click="handleOpenModal"
        :loading="globalLoading"
      >
        自定义装配
      </n-button>
    </div>

    <!-- 装配配置弹窗 -->
    <n-modal
      v-model:show="showModal"
      preset="card"
      title="装配称号与挑战勋章"
      class="preferences-modal"
      :style="{ maxWidth: '720px', width: '92%' }"
    >
      <div v-if="modalLoading" class="modal-loading-box">
        <n-spin size="large" />
        <span>正在读取您的挑战与称号数据...</span>
      </div>

      <div v-else class="modal-body-content">
        <!-- 1. 称号单选区 -->
        <div class="config-section">
          <div class="section-title-row">
            <span class="section-title">佩戴称号</span>
            <span class="section-hint">单选一个已解锁的称号</span>
          </div>

          <div class="title-filter-row">
            <n-input
              v-model:value="titleSearch"
              placeholder="搜索拥有的称号..."
              size="small"
              clearable
              class="search-bar"
            />
          </div>

          <div class="titles-scroll-container">
            <!-- 不佩戴称号项 -->
            <div
              class="title-chip"
              :class="{ active: selectedTitleId === '' || selectedTitleId === '0' || selectedTitleId === '-1' }"
              @click="selectedTitleId = ''"
            >
              <span class="chip-name">（无称号）</span>
            </div>

            <div
              v-for="item in filteredTitles"
              :key="item.itemId || item.id || item.name"
              class="title-chip"
              :class="{ active: String(item.itemId ?? item.id) === selectedTitleId }"
              @click="selectedTitleId = String(item.itemId ?? item.id)"
            >
              <span class="chip-name">{{ item.name }}</span>
            </div>
          </div>
        </div>

        <!-- 2. 已佩戴的勋章槽位展示 -->
        <div class="config-section">
          <div class="section-title-row">
            <span class="section-title">已选挑战勋章 ({{ selectedChallengeIds.length }}/3)</span>
            <span class="section-hint">点击槽位中的「×」可卸下该勋章</span>
          </div>

          <div class="slots-row">
            <div
              v-for="slotIdx in 3"
              :key="slotIdx"
              class="badge-slot"
              :class="{ filled: selectedChallengeIds[slotIdx - 1] }"
            >
              <template v-if="selectedChallengeIds[slotIdx - 1]">
                <div class="slot-badge-info">
                  <div class="slot-name">
                    {{ getBadgeDetail(selectedChallengeIds[slotIdx - 1])?.name || `勋章 #${selectedChallengeIds[slotIdx - 1]}` }}
                  </div>
                  <div class="slot-tier" :class="getLevelClass(getBadgeDetail(selectedChallengeIds[slotIdx - 1])?.currentLevel)">
                    {{ formatLevel(getBadgeDetail(selectedChallengeIds[slotIdx - 1])?.currentLevel) }}
                  </div>
                </div>
                <button
                  class="slot-remove-btn"
                  @click="removeSelectedBadge(slotIdx - 1)"
                  title="卸下此勋章"
                >
                  ✕
                </button>
              </template>
              <template v-else>
                <span class="slot-empty-text">槽位 {{ slotIdx }} (空)</span>
              </template>
            </div>
          </div>
        </div>

        <!-- 3. 可选挑战勋章候选列表 -->
        <div class="config-section">
          <div class="section-title-row">
            <span class="section-title">候选勋章库 (已获 {{ availableBadges.length }} 枚)</span>
            <span class="section-hint">点击添加或取消</span>
          </div>

          <div class="badge-filter-row">
            <n-input
              v-model:value="badgeSearch"
              placeholder="搜索挑战勋章名称或说明..."
              size="small"
              clearable
              class="search-bar"
            />
            <n-select
              v-model:value="badgeTierFilter"
              :options="tierFilterOptions"
              size="small"
              class="tier-select"
            />
          </div>

          <div class="badges-scroll-container">
            <div v-if="filteredBadges.length === 0" class="badges-empty">
              未找到符合条件的勋章
            </div>
            <div
              v-for="badge in filteredBadges"
              :key="badge.id"
              class="badge-item-card"
              :class="{
                selected: selectedChallengeIds.includes(String(badge.id)),
              }"
              @click="toggleBadge(badge.id)"
            >
              <div class="badge-card-header">
                <span class="badge-card-name">{{ badge.name }}</span>
                <span
                  class="badge-card-tier"
                  :class="getLevelClass(badge.currentLevel || badge.level)"
                >
                  {{ formatLevel(badge.currentLevel || badge.level) }}
                </span>
              </div>
              <div class="badge-card-desc" :title="badge.description">
                {{ badge.description || '暂无描述' }}
              </div>
              <div v-if="selectedChallengeIds.includes(String(badge.id))" class="badge-selected-tag">
                ✓ 已选
              </div>
            </div>
          </div>
        </div>
      </div>

      <template #footer>
        <div class="modal-footer-actions">
          <n-button @click="handleClearAll" size="small" tertiary>
            清空已选
          </n-button>
          <div class="footer-right-buttons">
            <n-button @click="showModal = false" size="small">
              取消
            </n-button>
            <n-button
              type="primary"
              size="small"
              :loading="saving"
              @click="handleSavePreferences"
            >
              保存并佩戴
            </n-button>
          </div>
        </div>
      </template>
    </n-modal>
  </div>
</template>

<style scoped>
.card-item {
  background: var(--settings-card-bg);
  border: 1px solid var(--settings-card-border);
  border-radius: 12px;
  margin-bottom: 8px;
  box-shadow: var(--shadow-sm);
  transition:
    box-shadow 0.25s cubic-bezier(0.25, 0.8, 0.25, 1),
    border-color 0.25s,
    background-color 0.25s,
    transform 0.2s;
  position: relative;
  padding: 16px 24px;
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.card-item:hover {
  border-color: var(--settings-card-border-hover);
  background-color: var(--settings-card-bg-hover);
}

.card-left {
  display: flex;
  align-items: center;
  gap: 16px;
}

.icon-container {
  width: 36px;
  height: 36px;
  border-radius: 8px;
  background: var(--hover-bg);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-color);
  flex-shrink: 0;
}

.header-icon {
  width: 20px;
  height: 20px;
}

.title-container {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.card-title {
  margin: 0;
  font-size: 0.95rem;
  font-weight: 600;
  color: var(--text-color);
}

.card-desc {
  font-size: 0.8rem;
  color: var(--text-muted);
}

.card-right {
  display: flex;
  align-items: center;
  gap: 12px;
}

.action-btn {
  font-weight: 600;
}

/* 弹窗内容 */
.modal-loading-box {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 40px 0;
  gap: 12px;
  color: var(--text-muted);
}

.modal-body-content {
  display: flex;
  flex-direction: column;
  gap: 16px;
  max-height: 60vh;
  overflow-y: auto;
  padding-right: 4px;
}

.config-section {
  display: flex;
  flex-direction: column;
  gap: 8px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid var(--border-color);
  border-radius: 10px;
  padding: 12px 14px;
}

.section-title-row {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
}

.section-title {
  font-size: 0.88rem;
  font-weight: 700;
  color: var(--text-color);
}

.section-hint {
  font-size: 0.72rem;
  color: var(--text-muted);
}

.title-filter-row,
.badge-filter-row {
  display: flex;
  gap: 10px;
}

.search-bar {
  flex: 1;
}

.tier-select {
  width: 140px;
}

/* 称号列表 */
.titles-scroll-container {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  max-height: 100px;
  overflow-y: auto;
  padding: 4px 0;
}

.title-chip {
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid var(--border-color);
  border-radius: 6px;
  padding: 4px 10px;
  font-size: 0.78rem;
  cursor: pointer;
  color: var(--text-color);
  transition: all 0.2s;
}

.title-chip:hover {
  background: rgba(255, 255, 255, 0.12);
  border-color: rgba(255, 255, 255, 0.3);
}

.title-chip.active {
  background: rgba(0, 242, 254, 0.15);
  border-color: #00f2fe;
  color: #00f2fe;
  font-weight: 700;
}

/* 槽位展示 */
.slots-row {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 10px;
}

.badge-slot {
  background: rgba(255, 255, 255, 0.02);
  border: 1px dashed var(--border-color);
  border-radius: 8px;
  padding: 8px 12px;
  min-height: 50px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  transition: all 0.2s;
}

.badge-slot.filled {
  border-style: solid;
  border-color: rgba(0, 242, 254, 0.4);
  background: rgba(0, 242, 254, 0.04);
}

.slot-empty-text {
  font-size: 0.76rem;
  color: var(--text-muted);
  width: 100%;
  text-align: center;
}

.slot-badge-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.slot-name {
  font-size: 0.8rem;
  font-weight: 700;
  color: var(--text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.slot-tier {
  font-size: 0.68rem;
  font-weight: 600;
}

.slot-remove-btn {
  background: transparent;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 0.8rem;
  transition: all 0.2s;
}

.slot-remove-btn:hover {
  background: rgba(255, 80, 80, 0.2);
  color: #ff4d4f;
}

/* 勋章候选列表 */
.badges-scroll-container {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
  gap: 8px;
  max-height: 220px;
  overflow-y: auto;
  padding: 4px 2px;
}

.badges-empty {
  grid-column: 1 / -1;
  text-align: center;
  padding: 24px;
  font-size: 0.8rem;
  color: var(--text-muted);
}

.badge-item-card {
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 8px 10px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  cursor: pointer;
  position: relative;
  transition: all 0.2s;
}

.badge-item-card:hover {
  background: rgba(255, 255, 255, 0.08);
  border-color: rgba(255, 255, 255, 0.25);
  transform: translateY(-1px);
}

.badge-item-card.selected {
  border-color: #00f2fe;
  background: rgba(0, 242, 254, 0.08);
}

.badge-card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 4px;
}

.badge-card-name {
  font-size: 0.8rem;
  font-weight: 700;
  color: var(--text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.badge-card-tier {
  font-size: 0.65rem;
  font-weight: 700;
  padding: 1px 5px;
  border-radius: 4px;
  flex-shrink: 0;
}

.badge-card-desc {
  font-size: 0.7rem;
  color: var(--text-muted);
  line-height: 1.3;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.badge-selected-tag {
  font-size: 0.65rem;
  color: #00f2fe;
  font-weight: 700;
  margin-top: 2px;
}

/* 等级色彩体系 */
.level-challenger {
  color: #ffd700;
  background: rgba(255, 215, 0, 0.15);
}

.level-grandmaster {
  color: #ff4d4f;
  background: rgba(255, 77, 79, 0.15);
}

.level-master {
  color: #b37feb;
  background: rgba(179, 127, 235, 0.15);
}

.level-diamond {
  color: #40a9ff;
  background: rgba(64, 169, 255, 0.15);
}

.level-emerald {
  color: #52c41a;
  background: rgba(82, 196, 26, 0.15);
}

.level-platinum {
  color: #13c2c2;
  background: rgba(19, 194, 194, 0.15);
}

.level-gold {
  color: #faad14;
  background: rgba(250, 173, 20, 0.15);
}

.level-normal {
  color: var(--text-muted);
  background: rgba(255, 255, 255, 0.08);
}

/* 底部操作 */
.modal-footer-actions {
  display: flex;
  justify-content: space-between;
  align-items: center;
  width: 100%;
}

.footer-right-buttons {
  display: flex;
  gap: 10px;
}
</style>
