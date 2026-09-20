<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue';
import { fetchChampionMasteryList, type ChampionMasteryInfo } from '../../api/lcu';
import { useLcuStore } from '../../store/lcuStore';
import { useToast } from '../../composables/useToast';
import LcuImage from '../LcuImage.vue';
import { NSelect, NInput, NSpin, NTag, NTooltip } from 'naive-ui';

const props = defineProps<{
  active?: boolean;
}>();

const store = useLcuStore();
const { showToast } = useToast();

const loading = ref(false);
const masteryList = ref<ChampionMasteryInfo[]>([]);
const hasLoaded = ref(false);

// 过滤与排序状态
const searchQuery = ref('');
const chestFilter = ref<'all' | 'eligible' | 'granted'>('all');
const levelFilter = ref<string>('all');
const sortBy = ref<'points' | 'level' | 'milestone' | 'lastPlay'>('points');
const sortOrder = ref<'desc' | 'asc'>('desc');

const chestFilterOptions = [
  { label: '全部宝箱状态', value: 'all' },
  { label: '可获取宝箱 📦', value: 'eligible' },
  { label: '已获取宝箱 🎁', value: 'granted' },
];

const levelFilterOptions = [
  { label: '全部等级', value: 'all' },
  { label: 'Lv. 10 及以上', value: '10+' },
  { label: 'Lv. 7 - 9', value: '7-9' },
  { label: 'Lv. 4 - 6', value: '4-6' },
  { label: 'Lv. 1 - 3', value: '1-3' },
];

const sortOptions = [
  { label: '熟练度分数', value: 'points' },
  { label: '熟练度等级', value: 'level' },
  { label: '赛段里程碑', value: 'milestone' },
  { label: '最近对局时间', value: 'lastPlay' },
];

// 加载数据
async function loadMasteryData(force = false) {
  if (!store.isConnected) return;
  if (loading.value) return;
  if (hasLoaded.value && !force) return;

  loading.value = true;
  try {
    const list = await fetchChampionMasteryList();
    masteryList.value = list;
    hasLoaded.value = true;
  } catch (err) {
    console.error('加载英雄熟练度失败:', err);
    showToast('获取英雄熟练度失败，请确认客户端已连接', 'error');
  } finally {
    loading.value = false;
  }
}

// 监听激活状态
watch(
  () => props.active,
  (active) => {
    if (active && !hasLoaded.value && store.isConnected) {
      loadMasteryData();
    }
  },
  { immediate: true },
);

// 监听连接状态
watch(
  () => store.isConnected,
  (connected) => {
    if (connected && props.active) {
      loadMasteryData(true);
    } else if (!connected) {
      masteryList.value = [];
      hasLoaded.value = false;
    }
  },
);

onMounted(() => {
  if (props.active && store.isConnected) {
    loadMasteryData();
  }
});

// 统计计算
const totalPoints = computed(() => {
  return masteryList.value.reduce((acc, cur) => acc + (cur.championPoints || 0), 0);
});

const eligibleChestCount = computed(() => {
  return masteryList.value.filter((m) => m.eligibleForChest && !m.chestGranted).length;
});

const grantedChestCount = computed(() => {
  return masteryList.value.filter((m) => m.chestGranted).length;
});

const maxLevelHero = computed(() => {
  if (masteryList.value.length === 0) return null;
  return [...masteryList.value].sort((a, b) => b.championLevel - a.championLevel)[0];
});

// 过滤与排序结果
const filteredList = computed(() => {
  let list = [...masteryList.value];

  // 1. 关键词搜索
  if (searchQuery.value.trim()) {
    const query = searchQuery.value.trim().toLowerCase();
    list = list.filter((m) => m.championName.toLowerCase().includes(query));
  }

  // 2. 宝箱过滤
  if (chestFilter.value === 'eligible') {
    list = list.filter((m) => m.eligibleForChest && !m.chestGranted);
  } else if (chestFilter.value === 'granted') {
    list = list.filter((m) => m.chestGranted);
  }

  // 3. 等级区间过滤
  if (levelFilter.value === '10+') {
    list = list.filter((m) => m.championLevel >= 10);
  } else if (levelFilter.value === '7-9') {
    list = list.filter((m) => m.championLevel >= 7 && m.championLevel <= 9);
  } else if (levelFilter.value === '4-6') {
    list = list.filter((m) => m.championLevel >= 4 && m.championLevel <= 6);
  } else if (levelFilter.value === '1-3') {
    list = list.filter((m) => m.championLevel >= 1 && m.championLevel <= 3);
  }

  // 4. 排序
  list.sort((a, b) => {
    let diff = 0;
    if (sortBy.value === 'points') {
      diff = a.championPoints - b.championPoints;
    } else if (sortBy.value === 'level') {
      diff = a.championLevel - b.championLevel || a.championPoints - b.championPoints;
    } else if (sortBy.value === 'milestone') {
      diff =
        a.championSeasonMilestone - b.championSeasonMilestone ||
        a.championPoints - b.championPoints;
    } else if (sortBy.value === 'lastPlay') {
      diff = a.lastPlayTime - b.lastPlayTime;
    }
    return sortOrder.value === 'desc' ? -diff : diff;
  });

  return list;
});

function toggleSortOrder() {
  sortOrder.value = sortOrder.value === 'desc' ? 'asc' : 'desc';
}

function formatPoints(points: number): string {
  if (!points) return '0';
  return points.toLocaleString();
}

function calcProgressPercentage(item: ChampionMasteryInfo): number {
  if (item.championPointsUntilNextLevel <= 0) return 100;
  const totalInLevel = item.championPointsSinceLastLevel + item.championPointsUntilNextLevel;
  if (totalInLevel <= 0) return 0;
  const pct = Math.round((item.championPointsSinceLastLevel / totalInLevel) * 100);
  return Math.min(Math.max(pct, 0), 100);
}
</script>

<template>
  <div class="mastery-tab-wrapper">
    <!-- 顶部数据概览卡片 -->
    <div class="mastery-stats-grid">
      <div class="stat-card">
        <div class="stat-label">熟练英雄总数</div>
        <div class="stat-value">{{ masteryList.length }} <span class="stat-unit">位</span></div>
        <div class="stat-sub">累计拥有熟练度记录</div>
      </div>

      <div class="stat-card">
        <div class="stat-label">熟练度总分</div>
        <div class="stat-value highlight">{{ formatPoints(totalPoints) }}</div>
        <div class="stat-sub">全英雄点数总和</div>
      </div>

      <div class="stat-card">
        <div class="stat-label">最高等级英雄</div>
        <div class="stat-value text-accent">
          {{ maxLevelHero ? maxLevelHero.championName : '--' }}
          <span v-if="maxLevelHero" class="stat-level-tag">Lv. {{ maxLevelHero.championLevel }}</span>
        </div>
        <div class="stat-sub">{{ maxLevelHero ? formatPoints(maxLevelHero.championPoints) + ' 分' : '--' }}</div>
      </div>

      <div class="stat-card">
        <div class="stat-label">里程碑宝箱</div>
        <div class="stat-value chest-value">
          <span class="granted-count" title="已解锁宝箱">{{ grantedChestCount }}</span>
          <span class="divider">/</span>
          <span class="eligible-count" title="可获取宝箱">+{{ eligibleChestCount }} 可得</span>
        </div>
        <div class="stat-sub">本赛段里程碑宝箱追踪</div>
      </div>
    </div>

    <!-- 过滤与搜索工具栏 -->
    <div class="mastery-filter-toolbar">
      <div class="filter-left">
        <n-input
          v-model:value="searchQuery"
          placeholder="搜索英雄名称..."
          clearable
          class="search-input"
          size="small"
        >
          <template #prefix>
            <svg class="search-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="11" cy="11" r="8" />
              <line x1="21" y1="21" x2="16.65" y2="16.65" />
            </svg>
          </template>
        </n-input>

        <n-select
          v-model:value="chestFilter"
          :options="chestFilterOptions"
          size="small"
          class="filter-select"
        />

        <n-select
          v-model:value="levelFilter"
          :options="levelFilterOptions"
          size="small"
          class="filter-select"
        />
      </div>

      <div class="filter-right">
        <span class="sort-label">排序:</span>
        <n-select
          v-model:value="sortBy"
          :options="sortOptions"
          size="small"
          class="sort-select"
        />

        <button class="icon-action-btn" @click="toggleSortOrder" :title="sortOrder === 'desc' ? '当前降序 (点击升序)' : '当前升序 (点击降序)'">
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            class="order-icon"
            :class="{ asc: sortOrder === 'asc' }"
          >
            <path d="M3 4h13M3 8h9M3 12h5m8 0l4 4m0 0l4-4m-4 4V4" />
          </svg>
        </button>

        <button class="refresh-btn" @click="loadMasteryData(true)" :disabled="loading" title="刷新数据">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="refresh-icon" :class="{ spinning: loading }">
            <path d="M23 4v6h-6M1 20v-6h6" />
            <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15" />
          </svg>
        </button>
      </div>
    </div>

    <!-- 英雄卡片列表区域 -->
    <div class="mastery-content-area">
      <div v-if="loading && masteryList.length === 0" class="loading-state">
        <n-spin size="large" />
        <span class="loading-text">正在同步英雄熟练度与里程碑数据...</span>
      </div>

      <div v-else-if="filteredList.length === 0" class="empty-state">
        <div class="empty-icon">🛡️</div>
        <div class="empty-title">未找到匹配的英雄</div>
        <div class="empty-sub">尝试清除搜索条件或刷新客户端数据</div>
      </div>

      <div v-else class="mastery-grid">
        <div
          v-for="item in filteredList"
          :key="item.championId"
          class="mastery-card"
          :class="{ 'card-eligible': item.eligibleForChest && !item.chestGranted }"
        >
          <!-- 头部：头像与基本信息 -->
          <div class="card-header">
            <div class="avatar-box">
              <LcuImage
                :src="item.championIconUrl"
                class="champ-icon"
                alt="champion"
              />
              <div class="level-badge" :class="item.championLevel >= 10 ? 'level-high' : ''">
                {{ item.championLevel }}
              </div>
            </div>

            <div class="header-info">
              <div class="champ-name-row">
                <span class="champ-name" :title="item.championName">{{ item.championName }}</span>
              </div>
              <div class="points-text">
                <span class="points-num">{{ formatPoints(item.championPoints) }}</span>
                <span class="points-unit">分</span>
              </div>
            </div>
          </div>

          <!-- 升级进度条 -->
          <div class="progress-section">
            <div class="progress-bar-track">
              <div
                class="progress-bar-fill"
                :style="{ width: `${calcProgressPercentage(item)}%` }"
              ></div>
            </div>
            <div class="progress-label-row">
              <span class="progress-percent">
                {{ calcProgressPercentage(item) }}%
              </span>
              <span class="progress-sub">
                <template v-if="item.championPointsUntilNextLevel > 0">
                  距下一级还需 {{ formatPoints(item.championPointsUntilNextLevel) }}
                </template>
                <template v-else-if="item.markRequiredForNextLevel > 0">
                  需等级标记: {{ item.tokensEarned }}/{{ item.markRequiredForNextLevel }}
                </template>
                <template v-else>
                  已满级
                </template>
              </span>
            </div>
          </div>

          <!-- 赛段里程碑与宝箱状态 -->
          <div class="milestone-footer">
            <div class="milestone-stage">
              <span class="stage-tag">里程碑 {{ item.championSeasonMilestone }}</span>
              <!-- 评级徽章 -->
              <div v-if="item.milestoneGrades && item.milestoneGrades.length > 0" class="grades-wrapper">
                <n-tooltip trigger="hover">
                  <template #trigger>
                    <div class="grades-tags">
                      <span
                        v-for="(grade, idx) in item.milestoneGrades.slice(0, 3)"
                        :key="idx"
                        class="grade-pill"
                      >
                        {{ grade }}
                      </span>
                      <span v-if="item.milestoneGrades.length > 3" class="grade-pill more">
                        +{{ item.milestoneGrades.length - 3 }}
                      </span>
                    </div>
                  </template>
                  当前赛段已获得评级: {{ item.milestoneGrades.join(', ') }}
                </n-tooltip>
              </div>
            </div>

            <!-- 宝箱提示 -->
            <div class="chest-badge-container">
              <n-tag
                v-if="item.chestGranted"
                size="tiny"
                round
                type="success"
                class="chest-tag granted"
              >
                已获宝箱 🎁
              </n-tag>
              <n-tag
                v-else-if="item.eligibleForChest"
                size="tiny"
                round
                type="info"
                class="chest-tag eligible"
              >
                可获宝箱 📦
              </n-tag>
              <n-tag
                v-else
                size="tiny"
                round
                class="chest-tag none"
              >
                未解锁
              </n-tag>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mastery-tab-wrapper {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  gap: 12px;
  overflow: hidden;
}

/* 顶部统计卡片 */
.mastery-stats-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 12px;
  flex-shrink: 0;
}

.stat-card {
  background: rgba(255, 255, 255, 0.05);
  backdrop-filter: blur(10px);
  border: 1px solid var(--border-color);
  border-radius: 10px;
  padding: 10px 14px;
  display: flex;
  flex-direction: column;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.04);
}

.stat-label {
  font-size: 0.76rem;
  font-weight: 600;
  color: var(--text-muted);
  margin-bottom: 2px;
}

.stat-value {
  font-size: 1.25rem;
  font-weight: 800;
  color: var(--text-color);
  display: flex;
  align-items: baseline;
  gap: 4px;
}

.stat-unit {
  font-size: 0.8rem;
  font-weight: 500;
  color: var(--text-muted);
}

.stat-value.highlight {
  color: #38ef7d;
  background: linear-gradient(135deg, #11998e, #38ef7d);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
}

.stat-value.text-accent {
  color: #00f2fe;
  background: linear-gradient(135deg, #4facfe, #00f2fe);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
}

.stat-level-tag {
  font-size: 0.72rem;
  background: rgba(0, 242, 254, 0.15);
  color: #00f2fe;
  border-radius: 4px;
  padding: 1px 6px;
  -webkit-text-fill-color: initial;
  font-weight: 700;
}

.chest-value {
  display: flex;
  align-items: baseline;
  gap: 6px;
}

.granted-count {
  color: #38ef7d;
}

.divider {
  color: var(--text-muted);
  font-size: 0.9rem;
}

.eligible-count {
  color: #00c6ff;
  font-size: 0.85rem;
  font-weight: 700;
}

.stat-sub {
  font-size: 0.7rem;
  color: var(--text-muted);
  margin-top: 2px;
}

/* 过滤工具栏 */
.mastery-filter-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  background: rgba(255, 255, 255, 0.04);
  backdrop-filter: blur(8px);
  border: 1px solid var(--border-color);
  border-radius: 10px;
  padding: 8px 12px;
  flex-shrink: 0;
}

.filter-left {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1;
}

.search-input {
  width: 180px;
}

.search-icon {
  width: 14px;
  height: 14px;
  color: var(--text-muted);
}

.filter-select {
  width: 130px;
}

.filter-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.sort-label {
  font-size: 0.76rem;
  color: var(--text-muted);
  font-weight: 600;
}

.sort-select {
  width: 130px;
}

.icon-action-btn,
.refresh-btn {
  background: rgba(255, 255, 255, 0.08);
  border: 1px solid var(--border-color);
  border-radius: 6px;
  padding: 5px 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  color: var(--text-color);
  transition: all 0.2s;
}

.icon-action-btn:hover,
.refresh-btn:hover {
  background: rgba(255, 255, 255, 0.15);
  border-color: rgba(255, 255, 255, 0.3);
}

.order-icon {
  width: 16px;
  height: 16px;
  transition: transform 0.2s ease;
}

.order-icon.asc {
  transform: scaleY(-1);
}

.refresh-icon {
  width: 16px;
  height: 16px;
}

.refresh-icon.spinning {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

/* 内容区域 */
.mastery-content-area {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding-right: 4px;
}

.loading-state,
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 260px;
  gap: 12px;
  color: var(--text-muted);
}

.loading-text {
  font-size: 0.85rem;
}

.empty-icon {
  font-size: 2.2rem;
}

.empty-title {
  font-size: 1rem;
  font-weight: 700;
  color: var(--text-color);
}

.empty-sub {
  font-size: 0.8rem;
}

/* 熟练度卡片网格 */
.mastery-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 12px;
  padding-bottom: 12px;
}

.mastery-card {
  background: rgba(255, 255, 255, 0.04);
  backdrop-filter: blur(8px);
  border: 1px solid var(--border-color);
  border-radius: 10px;
  padding: 10px 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  transition: transform 0.2s, border-color 0.2s, box-shadow 0.2s;
}

.mastery-card:hover {
  transform: translateY(-2px);
  border-color: rgba(255, 255, 255, 0.25);
  box-shadow: 0 6px 18px rgba(0, 0, 0, 0.08);
}

.mastery-card.card-eligible {
  border-color: rgba(0, 198, 255, 0.35);
  background: linear-gradient(180deg, rgba(0, 198, 255, 0.04) 0%, rgba(255, 255, 255, 0.03) 100%);
}

.card-header {
  display: flex;
  align-items: center;
  gap: 10px;
}

.avatar-box {
  position: relative;
  width: 44px;
  height: 44px;
  flex-shrink: 0;
}

.champ-icon {
  width: 100%;
  height: 100%;
  border-radius: 8px;
  object-fit: cover;
  border: 1px solid rgba(255, 255, 255, 0.2);
}

.level-badge {
  position: absolute;
  bottom: -4px;
  right: -4px;
  background: rgba(20, 20, 25, 0.88);
  border: 1px solid rgba(255, 255, 255, 0.3);
  color: #fff;
  font-size: 0.65rem;
  font-weight: 800;
  padding: 1px 5px;
  border-radius: 6px;
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.4);
}

.level-badge.level-high {
  background: linear-gradient(135deg, #f12711, #f5af19);
  border-color: #ffd200;
  color: #fff;
}

.header-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.champ-name-row {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.champ-name {
  font-size: 0.9rem;
  font-weight: 700;
  color: var(--text-color);
}

.points-text {
  font-size: 0.78rem;
  color: var(--text-muted);
  display: flex;
  align-items: baseline;
  gap: 2px;
  margin-top: 1px;
}

.points-num {
  font-weight: 700;
  color: var(--text-color);
}

.points-unit {
  font-size: 0.7rem;
}

/* 进度条 */
.progress-section {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.progress-bar-track {
  width: 100%;
  height: 5px;
  background: rgba(255, 255, 255, 0.08);
  border-radius: 3px;
  overflow: hidden;
}

.progress-bar-fill {
  height: 100%;
  background: linear-gradient(90deg, #4facfe 0%, #00f2fe 100%);
  border-radius: 3px;
  transition: width 0.3s ease;
}

.progress-label-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 0.68rem;
  color: var(--text-muted);
}

.progress-percent {
  font-weight: 700;
  color: #4facfe;
}

.progress-sub {
  font-size: 0.66rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 里程碑底部 */
.milestone-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-top: 1px solid rgba(255, 255, 255, 0.06);
  padding-top: 6px;
  font-size: 0.72rem;
}

.milestone-stage {
  display: flex;
  align-items: center;
  gap: 6px;
}

.stage-tag {
  color: var(--text-muted);
  font-size: 0.68rem;
}

.grades-wrapper {
  display: flex;
}

.grades-tags {
  display: flex;
  gap: 2px;
}

.grade-pill {
  background: rgba(255, 215, 0, 0.12);
  color: #ffd700;
  font-size: 0.62rem;
  font-weight: 700;
  padding: 0 4px;
  border-radius: 3px;
  border: 1px solid rgba(255, 215, 0, 0.25);
}

.grade-pill.more {
  background: rgba(255, 255, 255, 0.08);
  color: var(--text-muted);
  border-color: rgba(255, 255, 255, 0.15);
}

.chest-badge-container {
  display: flex;
  align-items: center;
}

.chest-tag.granted {
  font-weight: 600;
}

.chest-tag.eligible {
  font-weight: 700;
  animation: pulse-glow 2s infinite ease-in-out;
}

.chest-tag.none {
  opacity: 0.6;
}

@keyframes pulse-glow {
  0%, 100% {
    box-shadow: 0 0 0 0 rgba(0, 198, 255, 0.4);
  }
  50% {
    box-shadow: 0 0 6px 2px rgba(0, 198, 255, 0.6);
  }
}
</style>
