<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { useLoot } from '../../composables/useLoot'
import { useLcuStore } from '../../store/lcuStore'
import LcuImage from '../LcuImage.vue'
import { NSelect, NInputGroup, NInputNumber, NProgress } from 'naive-ui'

const props = defineProps<{
  refreshSummoner?: () => void
}>()

const store = useLcuStore()
const loot = useLoot()

// Alias for template: original Career.vue used "loadOpenableLoots"
const loadOpenableLoots = loot.loadLootData

const {
  openableLoots,
  sortedOpenableLoots,
  lootLoading,
  lootError,
  lootFetched,
  selectedLoot,
  openQuantity,
  isOpening,
  openProgress,
  openTotal,
  openResults,
  showOpenPanel,
  maxOpenQuantity,
  keyDisplayName,
  currentKeyCount,
  progressPanelTitle,
  openPercentage,
  isInventoryLoading,
  inventoryError,
  selectedLootIds,
  filterType,
  filterOwned,
  filterValueType,
  filterOperator,
  filterMaxValue,
  filteredInventory,
  canUpgrade,
  upgradeBtnText,
  canReroll,
  gainBlueEssence,
  gainOrangeEssence,
  blueEssenceCount,
  orangeEssenceCount,
  showConfirmModal,
  confirmModalConfig,
  executeConfirmedAction,
  loadLootInventory,
  openLootModal,
  closeLootModal,
  closeOpenPanel,
  handleSelectAllFiltered,
  handleClearSelection,
  toggleSelectItem,
  handleBatchDisenchant,
  handleBatchUpgrade,
  handleBatchReroll,
  getLootDisplayName,
  isKeyFragmentLoot,
  totalKeyCount,
} = loot

function handleSmartOpenAll() {
  loot.handleSmartOpenAll(() => props.refreshSummoner?.())
}

function handleBatchOpenWithRefresh() {
  loot.handleBatchOpen(() => props.refreshSummoner?.())
}

onMounted(() => {
  loot.setRefreshCallback(() => props.refreshSummoner?.())
  if (store.isConnected) {
    loot.loadAllData()
  }
})

onUnmounted(() => {
  loot.cleanup()
})
</script>

<template>
  <div class="loot-tab-root">
  <!-- 战利品面板：整页上下平铺 -->
  <div class="loot-tab-container">

    <!-- 区块一：可开启战利品（箱子/法球） -->
    <div class="loot-section-card">
      <!-- 头部操作栏 -->
      <div class="loot-section-header">
        <div class="header-left">
          <span class="section-title">&#x1F4E6; {{ $t("tools.lootManager.chestOpen") }}</span>
          <button
            class="action-btn"
            :disabled="!store.isConnected || lootLoading"
            @click="loadOpenableLoots"
          >
            {{ lootLoading ? '正在刷新...' : $t("tools.lootOpener.refreshBtn") }}
          </button>
          <span v-if="openableLoots.length > 0" class="loot-key-summary">
            &#x1F511; x{{ totalKeyCount }}
          </span>
        </div>
        <button
          v-if="openableLoots.length > 0"
          class="action-btn"
          :disabled="!store.isConnected || isOpening || totalKeyCount <= 0"
          @click="handleSmartOpenAll"
        >
          {{ $t("tools.lootOpener.smartOpenAll") }}
        </button>
      </div>

      <!-- 内容主体 -->
      <div class="loot-section-content">
        <div v-if="lootLoading" class="loot-loading-inline">
          <div class="loading-spinner"></div>
          <span>{{ $t("tools.lootOpener.loading") }}</span>
        </div>
        <div v-else-if="lootError" class="loot-error-inline">{{ lootError }}</div>
        <div v-else-if="sortedOpenableLoots.length > 0" class="loot-grid">
          <div
            v-for="lootLoot in sortedOpenableLoots"
            :key="lootLoot.lootId"
            class="loot-card-item"
            @click="openLootModal(lootLoot)"
          >
            <div class="loot-card-icon-container">
              <LcuImage :src="lootLoot.tilePath ?? undefined" class="loot-card-icon" />
            </div>
            <div class="loot-card-info">
              <div class="loot-card-header">
                <span class="loot-card-name" :title="getLootDisplayName(lootLoot)">{{ getLootDisplayName(lootLoot) }}</span>
                <span class="loot-card-count">x{{ lootLoot.count }}</span>
              </div>
              <div class="loot-card-footer">
                <span v-if="isKeyFragmentLoot(lootLoot)" class="loot-no-key-badge" style="background: rgba(16, 185, 129, 0.15); color: #10b981;">
                  {{ $t("tools.lootOpener.forge3in1") }}
                </span>
                <span v-else-if="lootLoot.needKey" class="loot-key-badge">{{ $t("tools.lootOpener.needKey") }}</span>
                <span v-else class="loot-no-key-badge">{{ $t("tools.lootOpener.noKeyNeeded") }}</span>
                <span class="loot-open-btn">
                  {{ isKeyFragmentLoot(lootLoot) ? $t("tools.lootOpener.forgeBtn") : $t("tools.lootOpener.openBtn") }}
                </span>
              </div>
            </div>
          </div>
        </div>
        <div v-else class="loot-empty-inline">
          {{ lootFetched ? $t("tools.lootOpener.noLootFound") : $t("tools.lootOpener.clickRefresh") }}
        </div>
      </div>
    </div>

    <!-- 区块二：碎片库存管理 -->
    <div class="loot-section-card">
      <!-- 头部操作栏 -->
      <div class="loot-section-header">
        <div class="header-left">
          <span class="section-title">&#x1F48E; {{ $t("tools.lootManager.title") }}</span>
          <button
            class="action-btn"
            :disabled="!store.isConnected || isInventoryLoading"
            @click="loadLootInventory"
          >
            {{ isInventoryLoading ? '正在刷新...' : $t("tools.lootManager.refreshBtn") }}
          </button>
        </div>
        <div class="header-right essence-header-balance">
          <span class="blue-essence-text">&#x1F535; {{ blueEssenceCount }}</span>
          <span class="orange-essence-text">&#x1F536; {{ orangeEssenceCount }}</span>
        </div>
      </div>

      <!-- 紧凑精美的水平筛选栏 -->
      <div class="loot-filter-bar-horizontal">
        <!-- 碎片类型 -->
        <div class="horizontal-filter-item">
          <span class="filter-label-inline">{{ $t("tools.lootManager.filterType") }}</span>
          <n-select
            v-model:value="filterType"
            :options="[
              { label: $t('tools.lootManager.filterAll'), value: 'ALL' },
              { label: '英雄', value: 'CHAMPION' },
              { label: '皮肤', value: 'SKIN' },
              { label: '表情', value: 'EMOTE' },
              { label: '守卫', value: 'WARDSKIN' },
              { label: '图标', value: 'SUMMONERICON' },
              { label: '永恒星碑', value: 'ETERNAL' },
              { label: '材料/宝箱', value: 'MATERIAL' },
            ]"
            size="small"
            style="width: 120px"
          />
        </div>

        <!-- 拥有状态 -->
        <div class="horizontal-filter-item">
          <span class="filter-label-inline">{{ $t("tools.lootManager.filterOwned") }}</span>
          <n-select
            v-model:value="filterOwned"
            :options="[
              { label: '全部', value: 'ALL' },
              { label: '已拥有', value: 'OWNED' },
              { label: '未拥有', value: 'NOT_OWNED' },
            ]"
            size="small"
            style="width: 110px"
          />
        </div>

        <!-- 价值基准 -->
        <div class="horizontal-filter-item">
          <span class="filter-label-inline">{{ $t("tools.lootManager.filterValueType") }}</span>
          <n-select
            v-model:value="filterValueType"
            :options="[
              { label: $t('tools.lootManager.filterValueTypeDisenchant'), value: 'disenchantValue' },
              { label: $t('tools.lootManager.filterValueTypeStore'), value: 'value' },
            ]"
            size="small"
            style="width: 120px"
          />
        </div>

        <!-- 价值范围过滤 -->
        <div class="horizontal-filter-item">
          <span class="filter-label-inline">价值</span>
          <n-input-group>
            <n-select
              v-model:value="filterOperator"
              :options="[
                { label: '小于等于 (<=)', value: '<=' },
                { label: '大于等于 (>=)', value: '>=' },
                { label: '等于 (=)', value: '=' }
              ]"
              size="small"
              style="width: 115px"
            />
            <n-input-number
              v-model:value="filterMaxValue"
              :min="0"
              placeholder="不限"
              size="small"
              clearable
              style="width: 125px"
            />
          </n-input-group>
        </div>
      </div>

      <!-- 内容主体 -->
      <div class="loot-section-content">
        <!-- 加载态 -->
        <div v-if="isInventoryLoading" class="loot-loading-inline">
          <div class="loading-spinner"></div>
          <span>{{ $t("tools.lootManager.loading") }}</span>
        </div>

        <!-- 错误态 -->
        <div v-else-if="inventoryError" class="loot-error-inline">{{ inventoryError }}</div>

        <!-- 空态 -->
        <div v-else-if="filteredInventory.length === 0" class="loot-empty-inline">
          {{ $t("tools.lootManager.empty") }}
        </div>

        <!-- 碎片卡片网格 -->
        <div v-else class="loot-grid loot-inventory-grid">
          <div
            v-for="item in filteredInventory"
            :key="item.lootId"
            :class="['loot-card-item', { selected: selectedLootIds.includes(item.lootId) }]"
            @click="toggleSelectItem(item.lootId)"
            style="position: relative;"
          >
            <!-- 选中状态角标 -->
            <div v-if="selectedLootIds.includes(item.lootId)" class="selected-checkmark-badge">
              &#x2713;
            </div>
            <div class="loot-card-icon-container">
              <LcuImage :src="item.tilePath ?? undefined" class="loot-card-icon" />
            </div>
            <div class="loot-card-info">
              <div class="loot-card-header">
                <span class="loot-card-name" :title="item.itemDesc">{{ item.itemDesc }}</span>
                <span class="loot-card-count">x{{ item.count }}</span>
              </div>
              <div class="loot-card-footer loot-manager-card-footer">
                <span :class="item.itemStatus === 'OWNED' ? 'loot-badge-owned' : 'loot-badge-not-owned'">
                  {{ item.itemStatus === 'OWNED' ? $t("tools.lootManager.ownedBadge") : $t("tools.lootManager.notOwnedBadge") }}
                </span>
                <span v-if="item.displayCategories.toUpperCase() === 'SKIN' && item.itemStatus !== 'OWNED' && item.parentItemStatus !== 'OWNED'" class="loot-badge-no-parent">
                  {{ $t("tools.lootManager.noChampionBadge") }}
                </span>
                <span class="essence-badge" :class="item.displayCategories === 'CHAMPION' ? 'blue-essence-text' : 'orange-essence-text'">
                  {{ item.displayCategories === 'CHAMPION' ? '&#x1F535;' : '&#x1F536;' }} {{ item.disenchantValue }}
                </span>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- 底部浮动控制栏 -->
      <div v-if="filteredInventory.length > 0" class="loot-batch-toolbar">
        <div class="toolbar-left">
          <button class="action-btn" @click="handleSelectAllFiltered">
            {{ $t("tools.lootManager.selectAll") }}
          </button>
          <button class="action-btn" @click="handleClearSelection">
            {{ $t("tools.lootManager.clearAll") }}
          </button>
          <span class="selected-count">
            {{ $t("tools.lootManager.selectedInfo", { count: selectedLootIds.length }) }}
          </span>
        </div>
        <div class="toolbar-right">
          <span v-if="selectedLootIds.length > 0" class="essence-preview">
            {{ $t("tools.lootManager.estimateEssence") }}
            <span class="blue-essence-text">&#x1F535; {{ gainBlueEssence }}</span>
            <span class="orange-essence-text">&#x1F536; {{ gainOrangeEssence }}</span>
          </span>
          <button
            class="action-btn"
            :disabled="selectedLootIds.length === 0 || isOpening || !store.isConnected"
            @click="handleBatchDisenchant"
          >
            {{ $t("tools.lootManager.disenchantBtn") }}
          </button>
          <button
            class="action-btn"
            :disabled="!canReroll || isOpening || !store.isConnected"
            @click="handleBatchReroll"
          >
            {{ $t("tools.lootManager.rerollBtn") }}
          </button>
          <button
            class="action-btn"
            :disabled="!canUpgrade || isOpening || !store.isConnected"
            @click="handleBatchUpgrade"
          >
            {{ upgradeBtnText }}
          </button>
        </div>
      </div>
    </div>
  </div>

  <!-- 自定义确认操作弹窗 -->
  <Transition name="fade">
    <div
      v-if="showConfirmModal"
      class="loot-modal-overlay"
      @click.self="showConfirmModal = false"
    >
      <div class="loot-modal-card confirm-modal-card">
        <div class="loot-modal-header confirm-modal-header" :class="confirmModalConfig.type">
          <h3>&#x26A0;&#xFE0F; {{ confirmModalConfig.title }}</h3>
          <button class="modal-close-btn" @click="showConfirmModal = false">&#x2715;</button>
        </div>
        <div class="loot-modal-body confirm-modal-body">
          <p class="confirm-message">{{ confirmModalConfig.message }}</p>

          <!-- 额外详情信息 -->
          <div v-if="confirmModalConfig.details" class="confirm-details-box">
            <div
              v-for="(detail, index) in confirmModalConfig.details"
              :key="index"
              class="confirm-detail-row"
            >
              <span class="detail-label">{{ detail.label }}</span>
              <span class="detail-value" :class="detail.class">{{ detail.value }}</span>
            </div>
          </div>

          <div class="loot-modal-actions confirm-modal-actions">
            <button class="action-btn" @click="showConfirmModal = false">
              {{ confirmModalConfig.cancelText }}
            </button>
            <button
              class="action-btn"
              @click="executeConfirmedAction"
            >
              {{ confirmModalConfig.confirmText }}
            </button>
          </div>
        </div>
      </div>
    </div>
  </Transition>

  <!-- 战利品开启数量选择弹窗 -->
  <Transition name="fade">
    <div
      v-if="selectedLoot"
      class="loot-modal-overlay"
      @click.self="closeLootModal"
    >
      <div class="loot-modal-card">
        <div class="loot-modal-header">
          <h3>
            {{ isKeyFragmentLoot(selectedLoot) ? $t("tools.lootOpener.batchForge") : $t("tools.lootOpener.batchOpen") }} - {{ getLootDisplayName(selectedLoot) }}
          </h3>
          <button class="modal-close-btn" @click="closeLootModal">&#x2715;</button>
        </div>
        <div class="loot-modal-body">
          <div class="loot-modal-preview">
            <div class="loot-modal-icon-container">
              <LcuImage :src="selectedLoot.tilePath ?? undefined" class="loot-modal-icon" />
            </div>
            <div class="loot-modal-details">
              <span class="loot-modal-name">{{ getLootDisplayName(selectedLoot) }}</span>
              <span class="loot-modal-owned">{{ $t("tools.lootOpener.owned") }}: x{{ selectedLoot.count }}</span>
            </div>
          </div>
          <div v-if="selectedLoot.needKey && selectedLoot.keyLootId" class="loot-info-row">
            <span class="loot-info-label">{{ $t("tools.lootOpener.keyRequired") }}</span>
            <span class="loot-info-value loot-key-count">
              {{ keyDisplayName }} x{{ openQuantity }} ({{ $t("tools.lootOpener.owned") }}: {{ currentKeyCount }})
            </span>
          </div>
          <div class="loot-quantity-row">
            <span class="loot-info-label">
              {{ isKeyFragmentLoot(selectedLoot) ? '合成次数' : $t("tools.lootOpener.quantity") }}
            </span>
            <n-input-number
              v-model:value="openQuantity"
              :min="1"
              :max="maxOpenQuantity"
              style="width: 120px"
              size="small"
            />
          </div>
          <div v-if="maxOpenQuantity <= 0" class="loot-insufficient">
            {{ isKeyFragmentLoot(selectedLoot) ? $t("tools.lootOpener.insufficientFragments") : (selectedLoot.needKey ? $t("tools.lootOpener.insufficientKeys") : '') }}
          </div>
              <div class="loot-modal-actions">
                <button class="action-btn" @click="closeLootModal">
                  {{ $t("tools.cancel") }}
                </button>
                <button
                  class="action-btn"
                  :disabled="maxOpenQuantity <= 0"
                  @click="handleBatchOpenWithRefresh"
                >
                  {{ isKeyFragmentLoot(selectedLoot) ? $t("tools.lootOpener.startForge", { count: openQuantity }) : $t("tools.lootOpener.startOpen", { count: openQuantity }) }}
                </button>
              </div>
        </div>
      </div>
    </div>
  </Transition>

  <!-- 批量开启进度面板 -->
  <Transition name="slide-up">
    <div v-if="showOpenPanel" class="loot-progress-overlay">
      <div class="loot-progress-card">
        <div class="loot-progress-header">
          <h3>{{ progressPanelTitle }}</h3>
          <button
            class="modal-close-btn"
            :disabled="isOpening"
            @click="closeOpenPanel"
          >&#x2715;</button>
        </div>
        <div class="loot-progress-body">
          <n-progress
            type="line"
            :percentage="openPercentage"
            :indicator-placement="'inside'"
            :border-radius="4"
            :height="20"
            status="success"
          />
          <div class="loot-progress-info">
            {{ openProgress }} / {{ openTotal }}
          </div>
          <div class="loot-results-list">
            <div
              v-for="(result, idx) in openResults"
              :key="idx"
              :class="['loot-result-item', result.success ? 'success' : 'error']"
            >
              <span class="loot-result-icon">{{ result.success ? '&#x1F389;' : '&#x274C;' }}</span>
              <span class="loot-result-text">
                [{{ result.current }}/{{ result.total }}]
                {{ result.success ? result.rewardName : result.errorMsg }}
              </span>
            </div>
          </div>
          <div v-if="!isOpening" class="loot-progress-actions" style="margin-top: 14px;">
            <n-button type="primary" size="medium" block @click="closeOpenPanel">
              确定
            </n-button>
          </div>
        </div>
      </div>
    </div>
  </Transition>
  </div><!-- /loot-tab-root -->
</template>

<style scoped>
.loot-tab-root {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
}

.loot-tab-container {
  display: flex;
  flex-direction: column;
  gap: 16px;
  flex: 1;
  overflow-y: auto;
  padding-right: 4px;
  min-height: 0;
}

.loot-action-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: 12px;
  box-shadow: var(--shadow-sm);
  backdrop-filter: blur(8px);
}

.action-bar-left {
  display: flex;
  align-items: center;
  gap: 12px;
}

.loot-loading,
.loot-error,
.loot-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 3rem;
  color: var(--text-dimmed);
  font-size: 0.88rem;
  gap: 12px;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: 16px;
}

.loot-error {
  color: var(--loss-color);
}

.loot-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 16px;
  padding-bottom: 24px;
}

.loot-card-item {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: 16px;
  padding: 12px 14px;
  cursor: pointer;
  transition: all 0.25s cubic-bezier(0.25, 0.8, 0.25, 1);
  display: flex;
  align-items: center;
  gap: 14px;
  box-shadow: var(--shadow-sm);
}

.loot-card-item:hover {
  border-color: var(--primary-color);
  box-shadow: var(--shadow-md), 0 0 0 1px var(--primary-color-alpha-30);
  transform: translateY(-2px);
  background: var(--card-bg-hover);
}

.loot-card-icon-container {
  width: 52px;
  height: 52px;
  border-radius: 10px;
  overflow: hidden;
  background: rgba(0, 0, 0, 0.05);
  border: 1px solid var(--border-color);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.loot-card-icon {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.loot-card-info {
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex: 1;
  min-width: 0;
}

.loot-card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.loot-card-name {
  font-size: 0.85rem;
  font-weight: 800;
  color: var(--text-color);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
}

.loot-card-count {
  font-size: 0.78rem;
  font-weight: 800;
  color: var(--primary-color);
  background: var(--primary-color-alpha-15);
  padding: 2px 8px;
  border-radius: 6px;
  margin-left: 8px;
  white-space: nowrap;
}

.loot-card-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.loot-manager-card-footer {
  justify-content: flex-start;
  gap: 4px;
  flex-wrap: nowrap;
  min-width: 0;
}

.loot-key-badge {
  font-size: 0.7rem;
  color: var(--warning-color, #e6a23c);
  background: var(--warning-color-alpha-10, rgba(230, 162, 60, 0.1));
  border: 1px solid var(--warning-color-alpha-20, rgba(230, 162, 60, 0.2));
  padding: 2px 8px;
  border-radius: 4px;
  font-weight: 600;
}

.loot-no-key-badge {
  font-size: 0.7rem;
  color: var(--text-dimmed);
  font-weight: 600;
  padding: 2px 8px;
  border-radius: 4px;
}

.loot-open-btn {
  font-size: 0.82rem;
  font-weight: 600;
  padding: 6px 16px;
  border-radius: 6px;
  border: 1px solid var(--border-color);
  background: var(--card-bg);
  color: var(--text-color);
  cursor: pointer;
  transition: all 0.2s;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  box-sizing: border-box;
}

.loot-open-btn:hover {
  background: var(--card-bg-hover);
  color: var(--text-color);
  border-color: var(--primary-color);
}

.loot-key-summary {
  font-size: 0.78rem;
  font-weight: 700;
  color: var(--warning-color, #e6a23c);
  background: var(--warning-color-alpha-10, rgba(230, 162, 60, 0.1));
  padding: 4px 12px;
  border-radius: 8px;
  border: 1px solid var(--warning-color-alpha-20, rgba(230, 162, 60, 0.2));
}

/* 数量选择弹窗 */
.loot-modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  background-color: rgba(15, 23, 42, 0.4);
  backdrop-filter: blur(16px);
  -webkit-backdrop-filter: blur(16px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
}

.loot-modal-card {
  width: 380px;
  background: var(--settings-card-bg, rgba(255, 255, 255, 0.95));
  border: 1px solid var(--border-color);
  border-radius: 16px;
  box-shadow: var(--shadow-lg);
  overflow: hidden;
  animation: modalScaleIn 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
}

.loot-modal-header {
  padding: 16px 20px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--border-color);
  background: var(--hover-bg);
}

.loot-modal-header h3 {
  font-size: 0.95rem;
  font-weight: 800;
  color: var(--text-color);
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
}

.loot-modal-body {
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.loot-modal-preview {
  display: flex;
  align-items: center;
  gap: 16px;
  background: var(--hover-bg);
  border: 1px solid var(--border-color);
  padding: 12px;
  border-radius: 10px;
}

.loot-modal-icon-container {
  width: 56px;
  height: 56px;
  border-radius: 8px;
  overflow: hidden;
  border: 1px solid var(--border-color);
  background: rgba(0, 0, 0, 0.05);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.loot-modal-icon {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.loot-modal-details {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.loot-modal-name {
  font-size: 0.88rem;
  font-weight: 800;
  color: var(--text-color);
}

.loot-modal-owned {
  font-size: 0.78rem;
  color: var(--text-muted);
}

.loot-info-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.loot-info-label {
  font-size: 0.82rem;
  color: var(--text-muted);
}

.loot-info-value {
  font-size: 0.85rem;
  font-weight: 700;
  color: var(--text-color);
}

.loot-key-count {
  color: var(--warning-color, #e6a23c);
}

.loot-quantity-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 0;
  border-top: 1px dashed var(--border-color);
  border-bottom: 1px dashed var(--border-color);
}

.loot-insufficient {
  font-size: 0.78rem;
  color: var(--loss-color);
  font-weight: 600;
  text-align: center;
}

.loot-modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  padding-top: 6px;
}

/* 批量开启进度面板 */
.loot-progress-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  background-color: rgba(15, 23, 42, 0.4);
  backdrop-filter: blur(16px);
  -webkit-backdrop-filter: blur(16px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
}

.loot-progress-card {
  width: 420px;
  max-height: 80vh;
  background: var(--settings-card-bg, rgba(255, 255, 255, 0.95));
  border: 1px solid var(--border-color);
  border-radius: 16px;
  box-shadow: var(--shadow-lg);
  overflow: hidden;
  display: flex;
  flex-direction: column;
  animation: modalScaleIn 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
}

.loot-progress-header {
  padding: 16px 20px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--border-color);
  background: var(--hover-bg);
}

.loot-progress-header h3 {
  font-size: 0.95rem;
  font-weight: 800;
  color: var(--text-color);
  margin: 0;
}

.loot-progress-body {
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  overflow-y: auto;
}

.loot-progress-info {
  font-size: 0.78rem;
  color: var(--text-dimmed);
  font-weight: 600;
  text-align: center;
}

.loot-results-list {
  max-height: 300px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.loot-result-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-radius: 6px;
  font-size: 0.78rem;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  animation: lootResultIn 0.3s ease-out;
}

.loot-result-item.success {
  border-color: var(--win-border);
  background: var(--win-bg);
}

.loot-result-item.error {
  border-color: var(--loss-border);
  background: var(--loss-bg);
}

.loot-result-icon {
  font-size: 0.9rem;
  flex-shrink: 0;
}

.loot-result-text {
  color: var(--text-color);
  word-break: break-all;
}

@keyframes lootResultIn {
  from {
    opacity: 0;
    transform: translateX(-8px);
  }
  to {
    opacity: 1;
    transform: translateX(0);
  }
}

/* ═════════ 战利品区块卡片 & 碎片库存管理 ═════════ */

.loot-section-card {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: 16px;
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 16px;
  box-shadow: var(--shadow-sm);
  transition: all 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
  backdrop-filter: blur(20px);
  -webkit-backdrop-filter: blur(20px);
}

.loot-section-card:hover {
  border-color: var(--primary-color-alpha-30);
  box-shadow: var(--shadow-md), 0 4px 20px rgba(0, 0, 0, 0.02);
}

.loot-section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-bottom: 1px solid var(--border-color);
  padding-bottom: 14px;
}

.essence-header-balance {
  display: flex;
  align-items: center;
  gap: 14px;
  font-size: 0.85rem;
  font-weight: 700;
  background: var(--hover-bg);
  padding: 4px 12px;
  border-radius: 20px;
  border: 1px solid var(--border-color);
}

.loot-section-header .header-left {
  display: flex;
  align-items: center;
  gap: 12px;
}

.section-title {
  font-size: 1rem;
  font-weight: 800;
  color: var(--text-color);
}

.loot-section-content {
  min-height: 80px;
  display: flex;
  flex-direction: column;
}

/* 水平筛选栏 */
.loot-filter-bar-horizontal {
  display: flex;
  flex-wrap: wrap;
  gap: 20px;
  align-items: center;
  background: rgba(0, 0, 0, 0.02);
  border: 1px solid var(--border-color);
  border-radius: 10px;
  padding: 10px 16px;
}

.horizontal-filter-item {
  display: flex;
  align-items: center;
  gap: 8px;
}

.filter-label-inline {
  font-size: 0.78rem;
  font-weight: 700;
  color: var(--text-muted);
  white-space: nowrap;
}

/* 碎片卡片选择态 */
.loot-card-item.selected {
  border-color: var(--primary-color) !important;
  box-shadow: 0 8px 24px var(--primary-color-alpha-30), 0 0 0 2px var(--primary-color) !important;
  background: var(--primary-color-alpha-15) !important;
  transform: translateY(-4px) scale(1.02) !important;
}

/* 选中标记角标 */
.selected-checkmark-badge {
  position: absolute;
  top: -6px;
  right: -6px;
  width: 18px;
  height: 18px;
  background: var(--primary-color);
  color: white;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.75rem;
  font-weight: 900;
  box-shadow: 0 2px 8px var(--primary-color-alpha-40);
  border: 1.5px solid #ffffff;
  z-index: 10;
  animation: popIn 0.25s cubic-bezier(0.175, 0.885, 0.32, 1.275);
}

@keyframes popIn {
  from {
    transform: scale(0);
  }
  to {
    transform: scale(1);
  }
}

/* 精品率/拥有状态标签 */
.loot-badge-owned {
  background: rgba(16, 185, 129, 0.12);
  color: #10b981;
  border: 1px solid rgba(16, 185, 129, 0.25);
  font-size: 0.65rem;
  padding: 1px 6px;
  border-radius: 4px;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  white-space: nowrap;
  flex-shrink: 0;
}

.loot-badge-not-owned {
  background: rgba(107, 114, 128, 0.1);
  color: var(--text-muted);
  border: 1px solid rgba(107, 114, 128, 0.2);
  font-size: 0.65rem;
  padding: 1px 6px;
  border-radius: 4px;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  white-space: nowrap;
  flex-shrink: 0;
}

.loot-badge-no-parent {
  background: rgba(239, 68, 68, 0.15);
  color: #ef4444;
  border-radius: 4px;
  padding: 1px 5px;
  font-size: 0.65rem;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  white-space: nowrap;
  flex-shrink: 0;
}

/* 精粹数值 */
.essence-badge {
  font-size: 0.72rem;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  gap: 2px;
  white-space: nowrap;
  flex-shrink: 0;
}
.blue-essence-text { color: #2563eb; }
.orange-essence-text { color: #d97706; }

/* 内置加载/错误/空态 */
.loot-loading-inline,
.loot-error-inline,
.loot-empty-inline {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 40px;
  color: var(--text-dimmed);
  font-size: 0.88rem;
  gap: 12px;
  width: 100%;
  box-sizing: border-box;
}

.loot-error-inline {
  color: var(--loss-color);
}

/* 底部浮动控制栏 */
.loot-batch-toolbar {
  position: sticky;
  bottom: 0;
  background: var(--settings-card-bg);
  border: 1px solid var(--border-color);
  border-radius: 12px;
  padding: 12px 18px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  box-shadow: var(--shadow-lg);
  z-index: 10;
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  margin-top: 16px;
}

.loot-batch-toolbar .toolbar-left,
.loot-batch-toolbar .toolbar-right {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.selected-count {
  font-size: 0.82rem;
  font-weight: 700;
  color: var(--text-color);
  margin-left: 6px;
}

.essence-preview {
  font-size: 0.82rem;
  font-weight: 700;
  display: flex;
  gap: 8px;
  align-items: center;
  margin-right: 6px;
}

/* 碎片网格负边距修正 */
.loot-inventory-grid {
  padding-bottom: 4px;
}

/* 自定义确认弹窗特殊样式 */
.confirm-modal-card {
  width: 350px !important;
}

.confirm-message {
  font-size: 0.85rem;
  color: var(--text-color);
  line-height: 1.5;
  margin: 0 0 12px 0;
}

.confirm-details-box {
  background: var(--hover-bg);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 10px 14px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 8px;
}

.confirm-detail-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 0.78rem;
}

.confirm-detail-row .detail-label {
  color: var(--text-muted);
}

.confirm-detail-row .detail-value {
  font-weight: 700;
  color: var(--text-color);
}

.action-btn {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  color: var(--text-color);
  padding: 6px 16px;
  border-radius: 6px;
  font-size: 0.82rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  box-sizing: border-box;
  line-height: 1.4;
  font-family: inherit;
}

.action-btn:hover {
  background: var(--card-bg-hover);
  color: var(--text-color);
  border-color: var(--primary-color);
}

.action-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
  pointer-events: none;
}

.modal-close-btn {
  background: none;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  font-size: 1rem;
  padding: 4px 6px;
  border-radius: 6px;
  transition: all 0.2s;
  line-height: 1;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.modal-close-btn:hover {
  background: var(--card-bg-hover);
  color: var(--text-color);
}

.modal-close-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

@keyframes modalScaleIn {
  from {
    opacity: 0;
    transform: scale(0.95);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}
</style>
