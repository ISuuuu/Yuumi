<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { NTabs, NTabPane, NSpin, NEmpty, NButton } from "naive-ui";
import { useLcuStore } from "../store/lcuStore";
import { useTftData, type TftMatchDisplay } from "../composables/useTftData";

import { lcuRequest } from "../api/lcu";

import TftRankHeader from "../components/tft/TftRankHeader.vue";
import LcuOfflineState from "../components/LcuOfflineState.vue";
import TftMatchCard from "../components/tft/TftMatchCard.vue";
import TftMatchDetailModal from "../components/tft/TftMatchDetailModal.vue";
import TftMetaCompsTab from "../components/tft/TftMetaCompsTab.vue";
import TftAugmentsTab from "../components/tft/TftAugmentsTab.vue";

const store = useLcuStore();
const { t } = useI18n();

const activeTab = ref("career");
const clientVersion = ref("");

const selectedMatch = ref<TftMatchDisplay | null>(null);
const showDetailModal = ref(false);

function handleSelectMatch(m: TftMatchDisplay) {
  selectedMatch.value = m;
  showDetailModal.value = true;
}

const {
  loading,
  error,
  rankedStats,
  summary,
  matches,
  loadTftData,
  refresh,
} = useTftData();

async function fetchClientVersion() {
  try {
    let res = await lcuRequest<string>("GET", "/lol-patch/v1/game-version");
    if (!res.success || !res.data) {
      res = await lcuRequest<string>("GET", "/lol-game-data/assets/v1/game-version.json");
    }
    if (res.success && res.data) {
      const raw = typeof res.data === "string" ? res.data : String(res.data);
      const parts = raw.replace(/^v/i, "").split(".");
      if (parts.length >= 2) {
        clientVersion.value = `${parts[0]}.${parts[1]}`;
      } else {
        clientVersion.value = raw;
      }
    }
  } catch (e) {
    console.warn("读取客户端版本失败:", e);
  }
}

watch(
  () => [store.isConnected, store.currentPage],
  ([connected, page]) => {
    if (connected && page === "tft") {
      loadTftData();
      fetchClientVersion();
    }
  },
  { immediate: true }
);
</script>

<template>
  <div class="tft-view">
    <!-- 未连接 LCU：整页启动提示，高度与其他页面保持一致 -->
    <LcuOfflineState v-if="!store.isConnected" />

    <div v-else class="tft-content">
      <!-- 主视图 Tabs（版本号与页签同一行，靠右） -->
      <n-tabs v-model:value="activeTab" type="line" class="tft-tabs">
        <template #suffix>
          <span v-if="clientVersion" class="client-ver-badge">v{{ clientVersion }}</span>
        </template>
        <!-- Tab 1: 云顶战绩 -->
        <n-tab-pane name="career" :tab="t('tftPage.tabs.career')">
          <!-- 段位与数据统计汇总（单排/狂暴/双人/胜利） -->
          <TftRankHeader :rankedStats="rankedStats" :summary="summary" />

          <div v-if="loading" class="loading-box">
            <n-spin size="medium" />
          </div>

          <div v-else-if="error" class="error-box">
            <p>{{ error }}</p>
            <n-button size="small" type="primary" @click="refresh">重试</n-button>
          </div>

          <div v-else-if="matches.length === 0" class="empty-box">
            <n-empty :description="t('tftPage.match.noMatches')" />
          </div>

          <div v-else class="matches-list">
            <TftMatchCard
              v-for="m in matches"
              :key="m.gameId"
              :match="m"
              @click="handleSelectMatch(m)"
            />
          </div>
        </n-tab-pane>

        <!-- Tab 2: 阵容推荐 -->
        <n-tab-pane name="recommend" :tab="t('tftPage.tabs.recommend')" lazy>
          <TftMetaCompsTab />
        </n-tab-pane>

        <!-- Tab 3: 海克斯强化 -->
        <n-tab-pane name="augments" :tab="t('tftPage.tabs.augments')" lazy>
          <TftAugmentsTab />
        </n-tab-pane>
      </n-tabs>
    </div>

    <!-- 对局详情 8人弹窗 -->
    <TftMatchDetailModal
      v-model:show="showDetailModal"
      :match="selectedMatch"
    />
  </div>
</template>

<style scoped>
.tft-view {
  padding: 1rem 1.5rem 1rem 0.6rem;
  background-color: transparent;
  flex: 1;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.client-ver-badge {
  font-size: 0.72rem;
  font-weight: 700;
  color: var(--primary-color);
  background: var(--primary-color-alpha-15);
  border: 1px solid var(--primary-color-alpha-20);
  padding: 2px 8px;
  border-radius: 12px;
}

.tft-content {
  max-width: 1200px;
  width: 100%;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 16px;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.tft-tabs {
  margin-top: 4px;
}

/* 毛玻璃容器式页签栏 */
.tft-tabs :deep(.n-tabs-nav) {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: 14px;
  padding: 6px;
  box-shadow: var(--shadow-sm);
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
}

.tft-tabs :deep(.n-tabs-nav-scroll-wrapper) {
  border-bottom: none;
  padding-bottom: 0;
}

.tft-tabs :deep(.n-tabs-bar) {
  display: none;
}

.tft-tabs :deep(.n-tabs-tab) {
  font-size: 0.88rem;
  font-weight: 700;
  color: var(--text-muted);
  padding: 8px 18px;
  margin: 0 4px;
  border-radius: 9px;
  background: transparent;
  transition: all 0.25s ease;
}

.tft-tabs :deep(.n-tabs-tab:hover) {
  color: var(--primary-color);
  background: var(--card-bg-hover);
}

.tft-tabs :deep(.n-tabs-tab--active) {
  color: var(--primary-color);
  background: var(--primary-color-alpha-10);
  box-shadow: inset 0 0 0 1px var(--primary-color-alpha-20);
}

.tft-tabs :deep(.n-tabs-pane-wrapper),
.tft-tabs :deep(.n-tab-pane) {
  overflow: visible !important;
}

.loading-box,
.error-box,
.empty-box {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 40px 0;
  gap: 12px;
}

.matches-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
</style>
