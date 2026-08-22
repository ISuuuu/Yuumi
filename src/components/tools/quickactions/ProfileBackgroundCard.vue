<script setup lang="ts">
import { ref, watch, computed, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { lcuRequest } from "../../../api/lcu";
import { useToast } from "../../../composables/useToast";
import { useI18n } from "vue-i18n";
import { NButton, NCollapse, NCollapseItem } from "naive-ui";
import ChampionPicker from "../../ChampionPicker.vue";
import LcuImage from "../../LcuImage.vue";
import { useQuickActionsLoading } from "./shared";

const { showToast } = useToast();
const { t } = useI18n();
const loading = useQuickActionsLoading();

// 个人主页背景
const skinIdInput = ref<number | null>(null);
const bgChampion = ref<number[]>([]);

interface SkinInfo {
  id: number;
  name: string;
  splashPath: string;
  loadScreenPath: string;
}
const skinList = ref<SkinInfo[]>([]);
const selectedSkinId = ref<number | null>(null);
const skinLoading = ref(false);

// 皮肤弹窗状态
const showSkinModal = ref(false);
const activeSkinIndex = ref(0);

const currentSelectedSkin = computed(() => {
  return skinList.value.find((s) => s.id === selectedSkinId.value) || null;
});

// 监听背景英雄点选，自动加载该英雄的皮肤列表
watch(bgChampion, async (newVal: number[]) => {
  skinList.value = [];
  selectedSkinId.value = null;
  skinIdInput.value = null;
  activeSkinIndex.value = 0;
  if (!newVal || newVal.length === 0) return;

  skinLoading.value = true;
  try {
    const skins = await invoke<
      Array<{ id: number; name: string; load_screen_path: string }>
    >("get_champion_skins", {
      championId: newVal[0],
    });
    if (skins && skins.length > 0) {
      skinList.value = skins.map(
        (s: { id: number; name: string; load_screen_path: string }) => ({
          id: s.id,
          name: s.name,
          splashPath: s.load_screen_path,
          loadScreenPath: s.load_screen_path,
        }),
      );
      selectedSkinId.value = skinList.value[0].id;
      skinIdInput.value = skinList.value[0].id;
      activeSkinIndex.value = 0;
    } else {
      showToast(t("tools.background.noSkinData"), "error");
    }
  } catch (e) {
    console.error("加载皮肤列表失败:", e);
    showToast(t("tools.background.skinLoadFailed"), "error");
  } finally {
    skinLoading.value = false;
  }
});

// 键盘事件处理
function handleKeyDown(e: KeyboardEvent) {
  if (!showSkinModal.value) return;
  if (e.key === "ArrowLeft") {
    prevSkin();
  } else if (e.key === "ArrowRight") {
    nextSkin();
  } else if (e.key === "Enter") {
    confirmSkinSelection();
  } else if (e.key === "Escape") {
    showSkinModal.value = false;
  }
}

// 监听弹窗打开以注册/解绑键盘事件
watch(showSkinModal, (val) => {
  if (val) {
    window.addEventListener("keydown", handleKeyDown);
  } else {
    window.removeEventListener("keydown", handleKeyDown);
  }
});

onUnmounted(() => {
  window.removeEventListener("keydown", handleKeyDown);
});

function openSkinModal() {
  if (skinList.value.length === 0) {
    showToast(t("tools.background.pickHeroFirst"), "error");
    return;
  }
  const idx = skinList.value.findIndex((s) => s.id === selectedSkinId.value);
  if (idx !== -1) {
    activeSkinIndex.value = idx;
  } else {
    activeSkinIndex.value = 0;
  }
  showSkinModal.value = true;
}

function prevSkin() {
  if (skinList.value.length === 0) return;
  activeSkinIndex.value =
    (activeSkinIndex.value - 1 + skinList.value.length) % skinList.value.length;
}

function nextSkin() {
  if (skinList.value.length === 0) return;
  activeSkinIndex.value = (activeSkinIndex.value + 1) % skinList.value.length;
}

function selectSkin(index: number) {
  activeSkinIndex.value = index;
}

async function confirmSkinSelection() {
  const currentSkin = skinList.value[activeSkinIndex.value];
  if (currentSkin) {
    selectedSkinId.value = currentSkin.id;
    skinIdInput.value = currentSkin.id;
    showSkinModal.value = false;
    await handleApplyBackground();
  }
}

// 更换生涯背景
async function handleApplyBackground() {
  if (skinIdInput.value === null) return;
  loading.value = true;
  try {
    const resp = await lcuRequest<unknown>(
      "POST",
      "/lol-summoner/v1/current-summoner/summoner-profile",
      {
        key: "backgroundSkinId",
        value: skinIdInput.value,
      },
    );
    if (resp.success) {
      showToast(t("tools.background.success"));
    } else {
      showToast(t("tools.background.failed", { error: resp.error }), "error");
    }
  } catch (e: unknown) {
    showToast(t("tools.background.error", { error: String(e) }), "error");
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <!-- 个人主页背景 -->
  <n-collapse arrow-placement="right" class="collapse-card">
    <n-collapse-item name="profilebg">
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
                <rect
                  x="3"
                  y="3"
                  width="18"
                  height="18"
                  rx="2"
                  ry="2"
                ></rect>
                <circle cx="8.5" cy="8.5" r="1.5"></circle>
                <polyline points="21 15 16 10 5 21"></polyline>
              </svg>
            </div>
            <div class="title-container">
              <h3 class="card-title">{{ t("tools.background.title") }}</h3>
              <span class="card-desc">{{ t("tools.background.desc") }}</span>
            </div>
          </div>
          <div class="collapse-right-status">
            <span class="status-preview">{{
              t("tools.spectate.expand")
            }}</span>
          </div>
        </div>
      </template>
      <div class="setting-row no-border">
        <span class="setting-label">{{ t("tools.background.desc") }}</span>
      </div>
      <div class="setting-picker-row">
        <ChampionPicker v-model="bgChampion" :maxCount="1" />
      </div>

      <div v-if="skinLoading" class="skin-loading">
        <div class="loading-spinner"></div>
        <span>{{ t("tools.loading") }}</span>
      </div>

      <!-- 已选择皮肤的预览信息 -->
      <div v-else-if="skinList.length > 0" class="selected-skin-preview">
        <div class="preview-layout">
          <div class="preview-img-container">
            <LcuImage
              :src="currentSelectedSkin?.loadScreenPath"
              class="preview-img"
            />
          </div>
          <div class="preview-info-box">
            <span class="preview-title">{{
              t("tools.background.title")
            }}</span>
            <span class="preview-skin-name">{{
              currentSelectedSkin?.name
            }}</span>
            <n-button size="small" type="primary" @click="openSkinModal">{{
              t("tools.background.selectSkinBtn")
            }}</n-button>
          </div>
        </div>
      </div>
    </n-collapse-item>
  </n-collapse>

  <!-- 皮肤选择轮播图弹窗 -->
  <Transition name="fade">
    <div
      v-if="showSkinModal"
      class="skin-modal-overlay"
      @click.self="showSkinModal = false"
    >
      <div class="skin-modal-card">
        <!-- 弹窗头部 -->
        <div class="skin-modal-header">
          <h3>{{ t("tools.background.titleModal") }}</h3>
          <button class="modal-close-btn" @click="showSkinModal = false">
            ✕
          </button>
        </div>

        <!-- 轮播主图区 -->
        <div class="skin-carousel-container">
          <!-- 左箭头 -->
          <button
            class="carousel-nav-btn prev"
            @click="prevSkin"
            :title="t('titlebar.back')"
          >
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2.5"
            >
              <polyline points="15 18 9 12 15 6" />
            </svg>
          </button>

          <!-- 皮肤加载图展示 -->
          <div class="skin-carousel-slide">
            <div class="slide-img-wrapper">
              <LcuImage
                :src="skinList[activeSkinIndex]?.loadScreenPath"
                class="carousel-img"
              />
            </div>
            <div class="carousel-skin-name">
              {{ skinList[activeSkinIndex]?.name }}
            </div>
          </div>

          <!-- 右箭头 -->
          <button
            class="carousel-nav-btn next"
            @click="nextSkin"
            :title="t('titlebar.back')"
          >
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2.5"
            >
              <polyline points="9 18 15 12 9 6" />
            </svg>
          </button>
        </div>

        <!-- 底部小缩略图滑轨 -->
        <div class="thumbnail-slider-wrapper">
          <div class="thumbnail-slider">
            <div
              v-for="(skin, index) in skinList"
              :key="skin.id"
              :class="[
                'thumbnail-dot',
                { active: activeSkinIndex === index },
              ]"
              @click="selectSkin(index)"
              :title="skin.name"
            >
              <LcuImage :src="skin.loadScreenPath" class="thumbnail-img" />
            </div>
          </div>
        </div>

        <!-- 底部控制按钮 -->
        <div class="skin-modal-footer">
          <span class="carousel-counter"
            >{{ activeSkinIndex + 1 }} / {{ skinList.length }}</span
          >
          <div class="footer-actions">
            <button class="cancel-action-btn" @click="showSkinModal = false">
              {{ t("tools.cancel") }}
            </button>
            <button class="confirm-action-btn" @click="confirmSkinSelection">
              {{ t("tools.confirm") }}
            </button>
          </div>
        </div>
      </div>
    </div>
  </Transition>
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

.setting-row.no-border {
  border-bottom: none;
  padding-bottom: 6px;
}

.setting-label {
  font-size: 0.82rem;
  color: var(--text-muted);
}

.setting-picker-row {
  padding-top: 6px;
  padding-bottom: 10px;
  width: 100%;
}

/* Loading spinner */
.loading-spinner {
  width: 36px;
  height: 36px;
  border: 3px solid rgba(0, 0, 0, 0.05);
  border-top-color: var(--primary-color);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

/* Skin loading */
.skin-loading {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  color: var(--text-dimmed);
  font-size: 0.82rem;
  padding: 1.5rem 0;
}

/* Selected skin preview */
.selected-skin-preview {
  margin: 12px 0 16px;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: 12px;
  padding: 16px;
  box-shadow: var(--shadow-sm);
}

.preview-layout {
  display: flex;
  align-items: center;
  gap: 18px;
}

.preview-img-container {
  width: 130px;
  aspect-ratio: 16 / 9;
  border-radius: 8px;
  overflow: hidden;
  box-shadow: var(--shadow-sm);
  border: 1px solid var(--border-color);
  flex-shrink: 0;
}

.preview-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.preview-info-box {
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex: 1;
}

.preview-title {
  font-size: 0.72rem;
  color: var(--text-dimmed);
  font-weight: bold;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.preview-skin-name {
  font-size: 0.88rem;
  font-weight: 800;
  color: var(--text-color);
}

/* Skin modal overlay */
.skin-modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  background-color: rgba(0, 0, 0, 0.45);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
}

.skin-modal-card {
  width: 480px;
  background: var(--settings-card-bg);
  border: 1px solid var(--border-color);
  border-radius: 16px;
  box-shadow: var(--shadow-lg);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  animation: modalScaleIn 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
}

@keyframes modalScaleIn {
  from {
    opacity: 0;
    transform: scale(0.95) translateY(10px);
  }
  to {
    opacity: 1;
    transform: scale(1) translateY(0);
  }
}

.skin-modal-header {
  padding: 16px 20px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--border-color);
  background: rgba(0, 0, 0, 0.01);
}

.skin-modal-header h3 {
  font-size: 1rem;
  font-weight: 800;
  color: var(--text-color);
  margin: 0;
}

.modal-close-btn {
  background: none;
  border: none;
  font-size: 1.1rem;
  color: var(--text-muted);
  cursor: pointer;
  width: 28px;
  height: 28px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s;
}

.modal-close-btn:hover {
  background: rgba(0, 0, 0, 0.03);
  color: var(--text-color);
}

/* Carousel body */
.skin-carousel-container {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 24px 16px;
  position: relative;
  background: radial-gradient(
    circle at center,
    rgba(0, 0, 0, 0.01) 0%,
    rgba(0, 0, 0, 0.05) 100%
  );
}

.carousel-nav-btn {
  width: 40px;
  height: 40px;
  border-radius: 50%;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  color: var(--text-color);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: all 0.2s cubic-bezier(0.25, 0.8, 0.25, 1);
  box-shadow: var(--shadow-sm);
  z-index: 2;
}

.carousel-nav-btn svg {
  width: 20px;
  height: 20px;
}

.carousel-nav-btn:hover {
  background: var(--primary-color);
  border-color: var(--primary-color);
  color: white;
  transform: scale(1.1);
  box-shadow: 0 4px 12px var(--primary-color-alpha-30);
}

.skin-carousel-slide {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
  max-width: 320px;
}

.slide-img-wrapper {
  width: 200px;
  height: 330px;
  border-radius: 12px;
  overflow: hidden;
  box-shadow: var(--shadow-md);
  border: 2px solid var(--card-bg);
  transition: transform 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
}

.slide-img-wrapper:hover {
  transform: scale(1.02);
}

.carousel-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.carousel-skin-name {
  font-size: 0.88rem;
  font-weight: 800;
  color: var(--text-color);
  text-align: center;
  min-height: 24px;
}

/* Thumbnail slider */
.thumbnail-slider-wrapper {
  padding: 0 24px 16px;
  overflow-x: auto;
}

.thumbnail-slider-wrapper::-webkit-scrollbar {
  height: 4px;
}

.thumbnail-slider-wrapper::-webkit-scrollbar-thumb {
  background: var(--text-dimmed);
  border-radius: 2px;
  opacity: 0.3;
}

.thumbnail-slider {
  display: flex;
  gap: 6px;
  padding-bottom: 4px;
}

.thumbnail-dot {
  width: 44px;
  height: 26px;
  border-radius: 4px;
  overflow: hidden;
  cursor: pointer;
  opacity: 0.5;
  border: 1px solid var(--border-color);
  transition: all 0.2s ease;
  flex-shrink: 0;
}

.thumbnail-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.thumbnail-dot:hover {
  opacity: 0.85;
  transform: scale(1.05);
}

.thumbnail-dot.active {
  opacity: 1;
  border-color: var(--primary-color);
  box-shadow: 0 0 0 2px var(--primary-color-alpha-30);
  transform: scale(1.08);
}

/* Modal footer */
.skin-modal-footer {
  padding: 14px 20px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-top: 1px solid var(--border-color);
  background: rgba(0, 0, 0, 0.01);
}

.carousel-counter {
  font-size: 0.82rem;
  color: var(--text-dimmed);
  font-weight: bold;
}

.footer-actions {
  display: flex;
  gap: 8px;
}

.cancel-action-btn {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  color: var(--text-color);
  padding: 8px 16px;
  border-radius: 8px;
  font-size: 0.82rem;
  font-weight: bold;
  cursor: pointer;
  transition: all 0.2s;
}

.cancel-action-btn:hover {
  background: var(--card-bg);
}

.confirm-action-btn {
  background: var(--primary-color);
  color: white;
  border: none;
  padding: 8px 20px;
  border-radius: 8px;
  font-size: 0.82rem;
  font-weight: bold;
  cursor: pointer;
  transition: all 0.2s;
  box-shadow: 0 4px 12px var(--primary-color-alpha-30);
}

.confirm-action-btn:hover {
  background: var(--primary-color-hover);
  box-shadow: 0 6px 16px var(--primary-color-alpha-40);
}

/* Transitions */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.25s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
