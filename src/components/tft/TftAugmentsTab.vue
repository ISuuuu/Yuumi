<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from "vue";
import { NInput, NRadioGroup, NRadioButton, NSpin, NEmpty, NButton } from "naive-ui";
import { useTftAugments } from "../../composables/useTftData";
import LcuImage from "../LcuImage.vue";

const { loading, error, augments, loadAugments } = useTftAugments();

const searchQuery = ref("");
const selectedTier = ref<number | "all">("all");
const displayLimit = ref(40);

onMounted(() => {
  if (augments.value.length === 0) {
    loadAugments();
  }
  const viewEl = document.querySelector(".tft-view");
  if (viewEl) {
    viewEl.addEventListener("scroll", handleScroll);
  }
});

onUnmounted(() => {
  const viewEl = document.querySelector(".tft-view");
  if (viewEl) {
    viewEl.removeEventListener("scroll", handleScroll);
  }
});

function handleScroll(e: Event) {
  const target = e.target as HTMLElement;
  if (target && hasMore.value) {
    if (target.scrollTop + target.clientHeight >= target.scrollHeight - 300) {
      displayLimit.value += 40;
    }
  }
}

watch([searchQuery, selectedTier], () => {
  displayLimit.value = 40;
});

const tierCounts = computed(() => {
  const counts = { all: augments.value.length, 1: 0, 2: 0, 3: 0 };
  for (const a of augments.value) {
    if (a.tier === 1) counts[1]++;
    else if (a.tier === 2) counts[2]++;
    else if (a.tier === 3) counts[3]++;
  }
  return counts;
});

const filteredAugments = computed(() => {
  return augments.value.filter((item) => {
    if (selectedTier.value !== "all" && item.tier !== selectedTier.value) {
      return false;
    }
    if (searchQuery.value.trim()) {
      const q = searchQuery.value.trim().toLowerCase();
      const matchName = item.name.toLowerCase().includes(q);
      const matchDesc = item.desc.toLowerCase().includes(q);
      const matchApi = item.apiName.toLowerCase().includes(q);
      return matchName || matchDesc || matchApi;
    }
    return true;
  });
});

const displayedAugments = computed(() => {
  return filteredAugments.value.slice(0, displayLimit.value);
});

const hasMore = computed(() => {
  return displayLimit.value < filteredAugments.value.length;
});

function loadMore() {
  displayLimit.value += 40;
}

function getTierLabel(tier: number) {
  if (tier === 1) return "银色";
  if (tier === 2) return "金色";
  if (tier === 3) return "棱彩";
  return "未知";
}

function getTierClass(tier: number) {
  if (tier === 1) return "silver";
  if (tier === 2) return "gold";
  if (tier === 3) return "prismatic";
  return "";
}
</script>

<template>
  <div class="augments-tab">
    <!-- 顶部过滤与搜索栏 (Sticky 吸顶) -->
    <div class="filter-header">
      <div class="search-box">
        <n-input
          v-model:value="searchQuery"
          placeholder="搜索海克斯名称或效果描述..."
          clearable
          size="small"
        >
          <template #prefix>
            <svg class="search-svg" viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
              <path d="M15.5 14h-.79l-.28-.27C15.41 12.59 16 11.11 16 9.5 16 5.91 13.09 3 9.5 3S3 5.91 3 9.5 5.91 16 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z" />
            </svg>
          </template>
        </n-input>
      </div>

      <div class="tier-filter">
        <n-radio-group v-model:value="selectedTier" size="small">
          <n-radio-button value="all">
            全部 ({{ tierCounts.all }})
          </n-radio-button>
          <n-radio-button :value="1">
            <span class="tier-dot silver-dot"></span> 银色 ({{ tierCounts[1] }})
          </n-radio-button>
          <n-radio-button :value="2">
            <span class="tier-dot gold-dot"></span> 金色 ({{ tierCounts[2] }})
          </n-radio-button>
          <n-radio-button :value="3">
            <span class="tier-dot prismatic-dot"></span> 棱彩 ({{ tierCounts[3] }})
          </n-radio-button>
        </n-radio-group>

        <n-button
          size="small"
          circle
          secondary
          :loading="loading"
          title="刷新数据"
          @click="loadAugments(true)"
        >
          <template #icon>
            <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
              <path d="M17.65 6.35C16.2 4.9 14.21 4 12 4c-4.42 0-7.99 3.58-7.99 8s3.57 8 7.99 8c3.73 0 6.84-2.55 7.73-6h-2.08c-.82 2.33-3.04 4-5.65 4-3.31 0-6-2.69-6-6s2.69-6 6-6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35z" />
            </svg>
          </template>
        </n-button>
      </div>
    </div>

    <!-- 数据呈现状态 -->
    <div v-if="loading && augments.length === 0" class="loading-container">
      <n-spin size="medium">
        <template #description>正在加载海克斯强化百科...</template>
      </n-spin>
    </div>

    <div v-else-if="error && augments.length === 0" class="error-container">
      <n-empty :description="error">
        <template #extra>
          <n-button size="small" type="primary" @click="loadAugments(true)">
            重试加载
          </n-button>
        </template>
      </n-empty>
    </div>

    <div v-else-if="filteredAugments.length === 0" class="empty-container">
      <n-empty description="没有找到匹配的海克斯强化" />
    </div>

    <!-- 海克斯强化网格/列表 (按需增量加载) -->
    <div v-else class="augment-container">
      <div class="augment-grid">
        <div
          v-for="a in displayedAugments"
          :key="a.apiName"
          :class="['augment-card', getTierClass(a.tier)]"
        >
          <div class="card-left">
            <div class="icon-wrapper">
              <LcuImage
                v-if="a.iconPath"
                :src="a.iconPath"
                class="augment-icon"
                alt="augment"
              />
              <div v-else class="fallback-icon">
                <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor">
                  <path d="M7 2v11h3v9l7-12h-4l4-8z" />
                </svg>
              </div>
            </div>
          </div>

          <div class="card-body">
            <div class="title-row">
              <span class="augment-name">{{ a.name }}</span>
              <span :class="['tier-badge', getTierClass(a.tier)]">
                {{ getTierLabel(a.tier) }}
              </span>
            </div>
            <p class="augment-desc">{{ a.desc || "暂无效果描述" }}</p>
          </div>
        </div>
      </div>

      <!-- 加载更多提示 -->
      <div v-if="hasMore" class="load-more-box">
        <n-button size="small" secondary @click="loadMore">
          已展示 {{ displayedAugments.length }} / {{ filteredAugments.length }} 项 (向下滚动自动加载更多)
        </n-button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.augments-tab {
  display: flex;
  flex-direction: column;
  gap: 16px;
  height: 100%;
}

.filter-header {
  position: sticky;
  top: -1rem;
  z-index: 50;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
  padding: 10px 14px;
  margin-top: -6px;
  background: var(--card-bg);
  border: none;
  border-bottom: 1px solid var(--primary-color-alpha-15);
  border-radius: 0 0 var(--radius-md) var(--radius-md);
  backdrop-filter: var(--glass-filter, blur(20px));
  -webkit-backdrop-filter: var(--glass-filter, blur(20px));
  box-shadow: 0 6px 16px -4px var(--primary-color-alpha-15);
  transition: all 0.2s ease;
}

.search-box {
  flex: 1;
  min-width: 200px;
  max-width: 320px;
}

.search-svg {
  opacity: 0.6;
}

.tier-filter {
  display: flex;
  align-items: center;
  gap: 12px;
}

.tier-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  margin-right: 4px;
}

.silver-dot {
  background: #94a3b8;
}

.gold-dot {
  background: #fbbf24;
}

.prismatic-dot {
  background: linear-gradient(135deg, #a855f7, #ec4899, #3b82f6);
}

.loading-container,
.error-container,
.empty-container {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 240px;
  background: var(--card-bg);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-color);
}

.augment-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
  gap: 12px;
}

.augment-card {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 14px;
  border-radius: var(--radius-md);
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
  box-shadow: var(--shadow-sm);
  transition: all 0.2s ease;
  position: relative;
  overflow: hidden;
}

.augment-card::before {
  content: "";
  position: absolute;
  top: 0;
  left: 0;
  width: 4px;
  height: 100%;
}

.augment-card.silver::before {
  background: #94a3b8;
}

.augment-card.gold::before {
  background: #fbbf24;
}

.augment-card.prismatic::before {
  background: linear-gradient(180deg, #a855f7, #ec4899, #3b82f6);
}

.augment-card:hover {
  transform: translateY(-2px);
  box-shadow: var(--shadow-md);
  border-color: rgba(255, 255, 255, 0.2);
}

.icon-wrapper {
  width: 44px;
  height: 44px;
  border-radius: 8px;
  overflow: hidden;
  background: rgba(0, 0, 0, 0.2);
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--border-color);
  flex-shrink: 0;
}

.augment-icon {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.fallback-icon {
  font-size: 1.4rem;
  color: var(--text-muted);
}

.card-body {
  flex: 1;
  min-width: 0;
}

.title-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 6px;
}

.augment-name {
  font-weight: 700;
  font-size: 0.95rem;
  color: var(--text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.tier-badge {
  font-size: 0.7rem;
  padding: 2px 7px;
  border-radius: 4px;
  font-weight: bold;
  white-space: nowrap;
  flex-shrink: 0;
}

.tier-badge.silver {
  background: rgba(148, 163, 184, 0.15);
  color: #94a3b8;
  border: 1px solid rgba(148, 163, 184, 0.3);
}

.tier-badge.gold {
  background: rgba(251, 191, 36, 0.15);
  color: #fbbf24;
  border: 1px solid rgba(251, 191, 36, 0.3);
}

.tier-badge.prismatic {
  background: rgba(168, 85, 247, 0.15);
  color: #c084fc;
  border: 1px solid rgba(168, 85, 247, 0.3);
}

.augment-desc {
  font-size: 0.8rem;
  color: var(--text-muted);
  line-height: 1.45;
  white-space: pre-line;
  word-break: break-word;
  margin: 0;
}

.load-more-box {
  display: flex;
  justify-content: center;
  margin-top: 16px;
  padding: 8px 0;
}
</style>
