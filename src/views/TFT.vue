<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { NTabs, NTabPane, NSpin, NEmpty, NButton } from "naive-ui";
import { useLcuStore } from "../store/lcuStore";
import { useTftData, type TftMatchDisplay } from "../composables/useTftData";

import { lcuRequest } from "../api/lcu";

import TftRankHeader from "../components/tft/TftRankHeader.vue";
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
  () => store.isConnected,
  (connected) => {
    if (connected) {
      loadTftData();
      fetchClientVersion();
    }
  },
  { immediate: true }
);
</script>

<template>
  <div class="tft-view">
    <div class="tft-content">
      <div class="header-title-bar">
        <div class="title-wrap">
          <h2>{{ t("tftPage.title") }}</h2>
          <span v-if="clientVersion" class="client-ver-badge">v{{ clientVersion }}</span>
        </div>
        <n-button
          v-if="store.isConnected"
          size="small"
          quaternary
          :loading="loading"
          @click="refresh"
        >
          刷新
        </n-button>
      </div>

      <!-- 顶部 TFT 段位与数据统计汇总 Header（有连接时显示） -->
      <TftRankHeader v-if="store.isConnected" :rankedStats="rankedStats" :summary="summary" />

      <!-- 主视图 Tabs -->
      <n-tabs v-model:value="activeTab" type="line" class="tft-tabs">
        <!-- Tab 1: 云顶战绩 -->
        <n-tab-pane name="career" :tab="t('tftPage.tabs.career')">
          <div v-if="!store.isConnected" class="tip-container">
            <div class="offline-logo">🎮</div>
            <p class="tip">{{ $t("gameInfo.launchLolPrompt") }}</p>
          </div>

          <div v-else-if="loading" class="loading-box">
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

.title-wrap {
  display: flex;
  align-items: center;
  gap: 12px;
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
