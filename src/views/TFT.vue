<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { NTabs, NTabPane, NSpin, NEmpty, NButton } from "naive-ui";
import { useLcuStore } from "../store/lcuStore";
import { useTftData, type TftMatchDisplay } from "../composables/useTftData";

import TftRankHeader from "../components/tft/TftRankHeader.vue";
import TftMatchCard from "../components/tft/TftMatchCard.vue";
import TftMatchDetailModal from "../components/tft/TftMatchDetailModal.vue";
import TftMetaCompsTab from "../components/tft/TftMetaCompsTab.vue";
import TftAugmentsTab from "../components/tft/TftAugmentsTab.vue";

const store = useLcuStore();
const { t } = useI18n();

const activeTab = ref("career");

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

onMounted(() => {
  if (store.isConnected) {
    loadTftData();
  }
});
</script>

<template>
  <div class="tft-view">
    <!-- 离线提示 -->
    <div v-if="!store.isConnected" class="tip-container">
      <div class="offline-logo">🎮</div>
      <p class="tip">{{ $t("gameInfo.launchLolPrompt") }}</p>
    </div>

    <div v-else class="tft-content">
      <div class="header-title-bar">
        <h2>{{ t("tftPage.title") }}</h2>
        <n-button
          size="small"
          quaternary
          :loading="loading"
          @click="refresh"
        >
          刷新
        </n-button>
      </div>

      <!-- 顶部 TFT 段位与数据统计汇总 Header -->
      <TftRankHeader :rankedStats="rankedStats" :summary="summary" />

      <!-- 主视图 Tabs -->
      <n-tabs v-model:value="activeTab" type="line" animated class="tft-tabs">
        <!-- Tab 1: 云顶战绩 -->
        <n-tab-pane name="career" :tab="t('tftPage.tabs.career')">
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
        <n-tab-pane name="recommend" :tab="t('tftPage.tabs.recommend')">
          <TftMetaCompsTab />
        </n-tab-pane>

        <!-- Tab 3: 海克斯强化 -->
        <n-tab-pane name="augments" :tab="t('tftPage.tabs.augments')">
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
  overflow-y: auto;
}

.header-title-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}

.header-title-bar h2 {
  margin: 0;
  font-size: 1.3rem;
  font-weight: 800;
  color: var(--text-color);
}

.tip-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 6rem 2rem;
  color: var(--text-muted);
  flex: 1;
}

.offline-logo {
  font-size: 3rem;
  margin-bottom: 1rem;
}

.tip {
  font-size: 0.95rem;
  color: var(--text-dimmed);
  margin: 0;
}

.tft-content {
  max-width: 1200px;
  width: 100%;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.tft-tabs {
  margin-top: 4px;
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
