<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { SummonerDisplay, AppConfig } from "../../api/lcu";
import LcuImage from "../LcuImage.vue";
import opggIcon from "../../assets/opgg.svg";

defineProps<{
  currentPage: string;
  isSidebarExpanded: boolean;
  appConfig: AppConfig | null;
  summoner: SummonerDisplay | null;
  regionName: string;
}>();

const emit = defineEmits<{
  navigate: [page: string];
  toggleSidebar: [];
  openOpgg: [];
  reconnect: [];
}>();

const { t: $t } = useI18n();
</script>

<template>
  <aside :class="['sidebar', isSidebarExpanded ? 'expanded' : 'collapsed']">
    <!-- 顶部折叠按钮 -->
    <div class="sidebar-header">
      <div class="hamburger-btn" @click="emit('toggleSidebar')">
        <svg
          class="hamburger-icon"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
        >
          <path
            d="M3 12h18M3 6h18M3 18h18"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
      </div>
    </div>

    <!-- 中间功能导航 -->
    <nav class="sidebar-nav">
      <div
        :class="['nav-item', { active: currentPage === 'home' }]"
        @click="emit('navigate', 'home')"
        :title="$t('nav.home')"
      >
        <span class="nav-icon">
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <path
              d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"
            />
            <polyline points="9 22 9 12 15 12 15 22" />
          </svg>
        </span>
        <span class="nav-label">{{ $t("nav.home") }}</span>
      </div>

      <div
        :class="['nav-item', { active: currentPage === 'career' }]"
        @click="emit('navigate', 'career')"
        :title="$t('nav.career')"
      >
        <span class="nav-icon">
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
            <circle cx="12" cy="7" r="4" />
          </svg>
        </span>
        <span class="nav-label">{{ $t("nav.career") }}</span>
      </div>

      <div
        :class="['nav-item', { active: currentPage === 'search' }]"
        @click="emit('navigate', 'search')"
        :title="$t('nav.search')"
      >
        <span class="nav-icon">
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <circle cx="11" cy="11" r="8" />
            <line x1="21" y1="21" x2="16.65" y2="16.65" />
          </svg>
        </span>
        <span class="nav-label">{{ $t("nav.search") }}</span>
      </div>

      <div
        :class="['nav-item', { active: currentPage === 'gameinfo' }]"
        @click="emit('navigate', 'gameinfo')"
        :title="$t('nav.gameinfo')"
      >
        <span class="nav-icon">
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <rect x="2" y="6" width="20" height="12" rx="3" />
            <path d="M6 12h4M8 10v4M15 11h.01M18 13h.01" />
          </svg>
        </span>
        <span class="nav-label">{{ $t("nav.gameinfo") }}</span>
      </div>

      <div
        v-if="!appConfig?.Functions?.HideTft"
        :class="['nav-item', { active: currentPage === 'tft' }]"
        @click="emit('navigate', 'tft')"
        :title="$t('nav.tft')"
      >
        <span class="nav-icon">
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <polygon points="12 2 2 7 12 12 22 7 12 2" />
            <polyline points="2 17 12 22 22 17" />
            <polyline points="2 12 12 17 22 12" />
          </svg>
        </span>
        <span class="nav-label">{{ $t("nav.tft") }}</span>
      </div>

      <div
        :class="['nav-item', { active: currentPage === 'tools' }]"
        @click="emit('navigate', 'tools')"
        :title="$t('nav.tools')"
      >
        <span class="nav-icon">
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <path
              d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"
            />
          </svg>
        </span>
        <span class="nav-label">{{ $t("nav.tools") }}</span>
      </div>
    </nav>

    <!-- 底部附加操作 -->
    <div class="sidebar-bottom">
      <div
        class="nav-item"
        @click="emit('openOpgg')"
        :title="$t('nav.opgg')"
      >
        <span class="nav-icon"
          ><img
            :src="opggIcon"
            style="width: 18px; height: 18px; border-radius: 3px"
        /></span>
        <span class="nav-label">{{ $t("nav.opgg") }}</span>
      </div>

      <div
        class="nav-item"
        @click="emit('reconnect')"
        :title="$t('nav.reconnect')"
      >
        <span class="nav-icon">
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <path
              d="M23 4v6h-6M1 20v-6h6M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"
            />
          </svg>
        </span>
        <span class="nav-label">{{ $t("nav.reconnect") }}</span>
      </div>

      <div
        :class="['nav-item', { active: currentPage === 'notice' }]"
        @click="emit('navigate', 'notice')"
        :title="$t('nav.notice')"
      >
        <span class="nav-icon">
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <path
              d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9M13.73 21a2 2 0 0 1-3.46 0"
            />
          </svg>
        </span>
        <span class="nav-label">{{ $t("nav.notice") }}</span>
      </div>

      <!-- 召唤师简短信息 -->
      <div
        v-if="summoner"
        class="user-card"
        @click="emit('navigate', 'career')"
        :title="`${summoner.displayName} (${regionName})`"
      >
        <div class="user-avatar">
          <LcuImage :src="summoner.profileIconUrl" alt="avatar" />
        </div>
        <div class="user-info">
          <span class="user-name">{{ summoner.displayName }}</span>
          <span class="user-region">{{ regionName }}</span>
        </div>
      </div>

      <div
        :class="['nav-item', { active: currentPage === 'settings' }]"
        @click="emit('navigate', 'settings')"
        :title="$t('nav.settings')"
      >
        <span class="nav-icon">
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <circle cx="12" cy="12" r="3" />
            <path
              d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"
            />
          </svg>
        </span>
        <span class="nav-label">{{ $t("nav.settings") }}</span>
      </div>
    </div>
  </aside>
</template>

<style scoped>
/* 侧边栏样式 */
.sidebar {
  display: flex;
  flex-direction: column;
  height: 100%;
  background-color: var(--sidebar-bg);
  border-right: 1px solid var(--border-color);
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
  transition: width 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
  overflow: hidden;
  flex-shrink: 0;
  user-select: none;
}

.sidebar.collapsed {
  width: 64px;
}

.sidebar.expanded {
  width: 210px;
}

/* 顶部折叠图标 */
.sidebar-header {
  display: flex;
  align-items: center;
  height: 52px;
  padding: 0 10px;
  gap: 4px;
}

.hamburger-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border-radius: 8px;
  cursor: pointer;
  color: var(--text-muted);
  transition: all 0.2s cubic-bezier(0.25, 0.8, 0.25, 1);
  flex-shrink: 0;
}

.hamburger-btn:hover {
  background-color: var(--hover-bg);
  color: var(--text-color);
}

.hamburger-icon {
  width: 20px;
  height: 20px;
  flex-shrink: 0;
}

/* 导航链接 */
.sidebar-nav {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 8px;
  flex: 1;
}

.nav-item {
  display: flex;
  align-items: center;
  height: 42px;
  padding: 0 14px;
  border-radius: var(--radius-md);
  cursor: pointer;
  color: var(--text-muted);
  position: relative;
  border: 1px solid transparent;
  transition: all 0.25s cubic-bezier(0.25, 0.8, 0.25, 1);
}

.sidebar.collapsed .nav-item {
  justify-content: center;
  padding: 0;
}

.nav-item:hover {
  background-color: var(--hover-bg);
  color: var(--text-color);
  border-color: var(--border-color);
  transform: translateY(-1px);
}

.sidebar.collapsed .nav-item:hover {
  transform: none;
}

.nav-item.active {
  background-color: var(--primary-color-alpha-15);
  color: var(--text-color);
  font-weight: 600;
  border-color: var(--primary-color-alpha-30);
  box-shadow: 0 4px 12px var(--primary-color-alpha-15);
}

/* 左侧指示发光条 */
.nav-item.active::before {
  content: "";
  position: absolute;
  left: 0;
  top: 10px;
  bottom: 10px;
  width: 4px;
  background-color: var(--primary-color);
  border-radius: 0 4px 4px 0;
  box-shadow: 0 0 10px var(--primary-color);
}

.sidebar.collapsed .nav-item.active::before {
  left: 3px;
}

.nav-icon {
  width: 18px;
  height: 18px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  margin-right: 12px;
  color: inherit;
  transition: transform 0.25s ease;
}

.nav-item:hover .nav-icon {
  transform: scale(1.1) rotate(2deg);
}

.nav-icon svg {
  width: 18px;
  height: 18px;
}

.sidebar.collapsed .nav-icon {
  margin-right: 0;
}

.nav-label {
  font-size: 0.88rem;
  white-space: nowrap;
  opacity: 1;
  transition: opacity 0.2s;
}

.sidebar.collapsed .nav-label {
  opacity: 0;
  width: 0;
  overflow: hidden;
  pointer-events: none;
  display: none;
}

/* 底部功能栏 */
.sidebar-bottom {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 8px;
  border-top: 1px solid var(--border-color);
}

/* 召唤师信息卡片 */
.user-card {
  display: flex;
  align-items: center;
  height: 52px;
  padding: 0 10px;
  margin: 8px 0;
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: all 0.25s ease;
  overflow: hidden;
  border: 1px solid transparent;
  background: rgba(255, 255, 255, 0.02);
}

.sidebar.collapsed .user-card {
  justify-content: center;
  padding: 0;
}

.user-card:hover {
  background-color: var(--hover-bg-strong);
  border-color: var(--border-color-hover);
  box-shadow: var(--shadow-sm);
}

.user-avatar {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  overflow: hidden;
  flex-shrink: 0;
  border: 1.5px solid var(--border-color);
  transition: all 0.25s ease;
}

.user-card:hover .user-avatar {
  border-color: var(--primary-color);
  box-shadow: 0 0 8px var(--primary-color-alpha-40);
  transform: scale(1.05);
}

.user-info {
  display: flex;
  flex-direction: column;
  margin-left: 10px;
  min-width: 0;
}

.sidebar.collapsed .user-info {
  display: none;
}

.user-name {
  font-size: 0.82rem;
  font-weight: 700;
  color: var(--text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.user-region {
  font-size: 0.68rem;
  color: var(--text-muted);
}
</style>
