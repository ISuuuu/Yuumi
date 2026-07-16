<script setup lang="ts">
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useI18n } from "vue-i18n";
import { useLcuStore } from "../../store/lcuStore";

defineProps<{
  pageHistory: string[];
  gamePhase: string;
  mapSideLabel: string;
}>();

const emit = defineEmits<{
  goBack: [];
  close: [];
}>();

const { t: $t } = useI18n();
const store = useLcuStore();
</script>

<template>
  <div class="titlebar" data-tauri-drag-region>
    <div class="titlebar-left">
      <div
        v-if="pageHistory.length > 0"
        class="titlebar-btn"
        @click="emit('goBack')"
        :title="$t('titlebar.back')"
      >
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <polyline points="15 18 9 12 15 6" />
        </svg>
      </div>
      <img src="/logo.png" class="titlebar-logo" alt="logo" />
      <span class="titlebar-title">
        Yummi
        <span
          v-if="store.isConnected && gamePhase !== 'None'"
          class="titlebar-phase"
        >
          · {{ $t("phase." + gamePhase)
          }}<span v-if="mapSideLabel"> - {{ mapSideLabel }}</span>
        </span>
      </span>
    </div>
    <div class="titlebar-controls">
      <div
        class="titlebar-btn"
        @click="getCurrentWindow().minimize()"
        :title="$t('titlebar.minimize')"
      >
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
        >
          <line x1="5" y1="12" x2="19" y2="12" />
        </svg>
      </div>
      <div
        class="titlebar-btn"
        @click="getCurrentWindow().toggleMaximize()"
        :title="$t('titlebar.maximize')"
      >
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
        >
          <rect x="5" y="5" width="14" height="14" rx="2" />
        </svg>
      </div>
      <div
        class="titlebar-btn close-btn"
        @click="emit('close')"
        :title="$t('titlebar.close')"
      >
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
        >
          <line x1="18" y1="6" x2="6" y2="18" />
          <line x1="6" y1="6" x2="18" y2="18" />
        </svg>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 自定义标题栏 */
.titlebar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 38px;
  background: var(--titlebar-bg);
  border-bottom: 1px solid var(--border-color);
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
  flex-shrink: 0;
  user-select: none;
  padding: 0 8px;
  z-index: 1000;
}

.titlebar-left {
  display: flex;
  align-items: center;
  gap: 4px;
}

.titlebar-logo {
  width: 20px;
  height: 20px;
  object-fit: contain;
  margin-left: 4px;
}

.titlebar-title {
  font-size: 0.8rem;
  font-weight: 700;
  color: var(--text-color);
  letter-spacing: 0.5px;
  margin-left: 2px;
  text-transform: none;
}

.titlebar-phase {
  font-weight: 600;
  color: var(--primary-color);
  font-size: 0.78rem;
  background: var(--primary-color-alpha-15);
  padding: 1px 6px;
  border-radius: 4px;
  margin-left: 6px;
}

.titlebar-controls {
  display: flex;
  align-items: center;
  gap: 2px;
}

.titlebar-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 26px;
  border-radius: 4px;
  cursor: pointer;
  color: var(--text-muted);
  transition: all 0.2s ease;
}

.titlebar-btn:hover {
  background-color: var(--hover-bg);
  color: var(--text-color);
}

.titlebar-btn svg {
  width: 14px;
  height: 14px;
}

.close-btn:hover {
  background-color: var(--loss-color) !important;
  color: white !important;
}
</style>
