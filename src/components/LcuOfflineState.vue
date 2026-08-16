<script setup lang="ts">
import { useI18n } from "vue-i18n";

const { t: $t } = useI18n();

withDefaults(
  defineProps<{
    icon?: string;
    message?: string;
  }>(),
  {
    icon: "🎮",
    message: "gameInfo.launchLolPrompt",
  },
);
</script>

<template>
  <div class="offline-state">
    <div class="offline-state-icon">{{ icon }}</div>
    <p class="offline-state-tip">{{ message.startsWith("gameInfo.") ? $t(message) : message }}</p>
  </div>
</template>

<style scoped>
.offline-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 6rem 2rem;
  color: var(--text-muted);
  flex: 1;
}

.offline-state-icon {
  font-size: 3rem;
  line-height: 1;
  margin-bottom: 1rem;
}

.offline-state-tip {
  font-size: 0.95rem;
  line-height: 1.6;
  color: var(--text-dimmed);
  margin: 0;
}
</style>
