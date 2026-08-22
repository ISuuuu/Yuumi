<script setup lang="ts">
import { ref, computed } from "vue";
import { lcuRequest } from "../../../api/lcu";
import { useToast } from "../../../composables/useToast";
import { useI18n } from "vue-i18n";
import { NSelect, NButton, NCollapse, NCollapseItem } from "naive-ui";
import { useQuickActionsLoading } from "./shared";

const { showToast } = useToast();
const { t } = useI18n();
const loading = useQuickActionsLoading();

// 段位展示状态
const spoofQueue = ref("RANKED_SOLO_5x5");
const spoofTier = ref("CHALLENGER");
const spoofDivision = ref("I");

const SPOOF_QUEUE_LABELS = computed(() => ({
  RANKED_TFT: t("tools.spoofQueue.RANKED_TFT"),
  RANKED_SOLO_5x5: t("tools.spoofQueue.RANKED_SOLO_5x5"),
  RANKED_FLEX_SR: t("tools.spoofQueue.RANKED_FLEX_SR"),
}));
const SPOOF_TIER_LABELS = computed<Record<string, string>>(() => ({
  UNRANKED: t("tools.spoofTier.UNRANKED"),
  CHALLENGER: t("tools.spoofTier.CHALLENGER"),
  GRANDMASTER: t("tools.spoofTier.GRANDMASTER"),
  MASTER: t("tools.spoofTier.MASTER"),
  DIAMOND: t("tools.spoofTier.DIAMOND"),
  EMERALD: t("tools.spoofTier.EMERALD"),
  PLATINUM: t("tools.spoofTier.PLATINUM"),
  GOLD: t("tools.spoofTier.GOLD"),
  SILVER: t("tools.spoofTier.SILVER"),
  BRONZE: t("tools.spoofTier.BRONZE"),
  IRON: t("tools.spoofTier.IRON"),
}));

// 伪装段位展示
async function handleApplyRankSpoof() {
  loading.value = true;
  try {
    const resp = await lcuRequest<unknown>("PUT", "/lol-chat/v1/me", {
      lol: {
        rankedLeagueQueue: spoofQueue.value,
        rankedLeagueTier: spoofTier.value,
        rankedLeagueDivision: spoofDivision.value,
      },
    });
    if (resp.success) {
      showToast(t("tools.rankSpoof.success"));
    } else {
      showToast(t("tools.rankSpoof.failed", { error: resp.error }), "error");
    }
  } catch (e: unknown) {
    showToast(t("tools.rankSpoof.error", { error: String(e) }), "error");
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <!-- 段位展示 -->
  <n-collapse arrow-placement="right" class="collapse-card">
    <n-collapse-item name="rankdisplay">
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
                <path d="M6 9H4.5a2.5 2.5 0 0 1 0-5H6"></path>
                <path d="M18 9h1.5a2.5 2.5 0 0 0 0-5H18"></path>
                <path d="M4 22h16"></path>
                <path
                  d="M10 14.66V17c0 .55-.45 1-1 1H4v2h16v-2h-5c-.55 0-1-.45-1-1v-2.34"
                ></path>
                <path d="M12 2a7 7 0 0 0-7 7h14a7 7 0 0 0-7-7z"></path>
              </svg>
            </div>
            <div class="title-container">
              <h3 class="card-title">{{ t("tools.rankSpoof.title") }}</h3>
              <span class="card-desc">{{ t("tools.rankSpoof.desc") }}</span>
            </div>
          </div>
          <div class="collapse-right-status">
            <span class="status-preview">{{
              t("tools.spectate.expand")
            }}</span>
          </div>
        </div>
      </template>
      <div class="setting-row">
        <span class="setting-label">{{
          t("tools.rankSpoof.queueLabel")
        }}</span>
        <n-select
          v-model:value="spoofQueue"
          :options="
            Object.entries(SPOOF_QUEUE_LABELS).map(([k, v]) => ({
              label: v,
              value: k,
            }))
          "
          style="width: 140px"
          size="small"
        />
      </div>
      <div class="setting-row">
        <span class="setting-label"
          >{{ t("tools.rankSpoof.tierLabel") }}
          {{ t("tools.rankSpoof.divisionLabel") }}</span
        >
        <div class="rank-select-group">
          <n-select
            v-model:value="spoofTier"
            :options="
              [
                'UNRANKED',
                'CHALLENGER',
                'GRANDMASTER',
                'MASTER',
                'DIAMOND',
                'EMERALD',
                'PLATINUM',
                'GOLD',
                'SILVER',
                'BRONZE',
                'IRON',
              ].map((t) => ({ label: SPOOF_TIER_LABELS[t], value: t }))
            "
            style="width: 130px"
            size="small"
          />
          <n-select
            v-model:value="spoofDivision"
            :options="
              ['I', 'II', 'III', 'IV'].map((d) => ({ label: d, value: d }))
            "
            :disabled="
              ['UNRANKED', 'MASTER', 'GRANDMASTER', 'CHALLENGER'].includes(
                spoofTier,
              )
            "
            style="width: 80px"
            size="small"
          />
          <n-button
            size="small"
            type="primary"
            @click="handleApplyRankSpoof"
            :disabled="loading"
            >{{ t("tools_extra.applySpoofBtn") }}</n-button
          >
        </div>
      </div>
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
  font-size: 0.88rem;
  font-weight: bold;
  color: var(--text-color);
  margin: 0;
}

.card-desc {
  font-size: 0.78rem;
  color: var(--text-muted);
  margin-top: 4px;
  line-height: 1.4;
}

.collapse-right-status {
  margin-left: auto;
  display: flex;
  align-items: center;
}

.status-preview {
  font-size: 0.78rem;
  color: var(--text-dimmed);
  margin-right: 10px;
}

.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 0;
  border-bottom: 1px dashed var(--border-color);
}

.setting-row:last-child {
  border-bottom: none;
}

.setting-label {
  font-size: 0.82rem;
  color: var(--text-muted);
}

.rank-select-group {
  display: flex;
  gap: 8px;
  align-items: center;
}
</style>
