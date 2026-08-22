<script setup lang="ts">
import { useAutoSaveConfig } from "../../composables/useAutoSaveConfig";
import { useI18n } from "vue-i18n";
import { provideQuickActionsLoading } from "./quickactions/shared";
import BenchOverlayCard from "./quickactions/BenchOverlayCard.vue";
import AutoReconnectCard from "./quickactions/AutoReconnectCard.vue";
import AutoCreateLobbyCard from "./quickactions/AutoCreateLobbyCard.vue";
import LockGameSettingsCard from "./quickactions/LockGameSettingsCard.vue";
import FixWindowCard from "./quickactions/FixWindowCard.vue";
import RestartClientCard from "./quickactions/RestartClientCard.vue";
import SignatureCard from "./quickactions/SignatureCard.vue";
import ProfileBackgroundCard from "./quickactions/ProfileBackgroundCard.vue";
import RankSpoofCard from "./quickactions/RankSpoofCard.vue";
import OnlineStatusCard from "./quickactions/OnlineStatusCard.vue";
import ClearBadgesCard from "./quickactions/ClearBadgesCard.vue";
import ClearBorderCard from "./quickactions/ClearBorderCard.vue";

const { config } = useAutoSaveConfig();
const { t } = useI18n();

// 各子块共享的操作进行中状态（与拆分前一致：任一操作执行期间全部按钮禁用）
provideQuickActionsLoading();
</script>

<template>
  <div v-if="config">
    <BenchOverlayCard />
    <AutoReconnectCard />

    <AutoCreateLobbyCard />

    <LockGameSettingsCard />

    <!-- 3. 客户端组 -->
    <div class="group-header">{{ t("tools_extra.clientGroupTitle") }}</div>

    <FixWindowCard />
    <RestartClientCard />

    <!-- 4. 个人主页组 -->
    <div class="group-header">{{ t("tools_extra.profileGroupTitle") }}</div>

    <SignatureCard />
    <ProfileBackgroundCard />
    <RankSpoofCard />
    <OnlineStatusCard />

    <ClearBadgesCard />
    <ClearBorderCard />
  </div>
</template>

<style scoped>
.group-header {
  font-size: 0.78rem;
  font-weight: 700;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.5px;
  margin: 1.2rem 0 0.6rem 0;
  padding-left: 4px;
}
</style>
