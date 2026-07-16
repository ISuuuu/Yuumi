<script setup lang="ts">
import { ref, computed } from "vue";
import { useAutoSaveConfig } from "../../composables/useAutoSaveConfig";
import { useI18n } from "vue-i18n";
import { NSwitch, NInputNumber, NCollapse, NCollapseItem } from "naive-ui";
import ChampionPicker from "../ChampionPicker.vue";
import SpellPicker from "../SpellPicker.vue";

const { config, triggerAutoSave } = useAutoSaveConfig();
const { t } = useI18n();

// 分路选择状态
const hoverActiveLane = ref<"default" | "top" | "jug" | "mid" | "bot" | "sup">(
  "default",
);
const banActiveLane = ref<"default" | "top" | "jug" | "mid" | "bot" | "sup">(
  "default",
);
const spellActiveLane = ref<"default" | "top" | "jug" | "mid" | "bot" | "sup">(
  "default",
);

const LANE_OPTIONS = computed(
  () =>
    [
      { value: "default", label: t("tools.lane.default") },
      { value: "top", label: t("tools.lane.top") },
      { value: "jug", label: t("tools.lane.jug") },
      { value: "mid", label: t("tools.lane.mid") },
      { value: "bot", label: t("tools.lane.bot") },
      { value: "sup", label: t("tools.lane.sup") },
    ] as const,
);

function onPickerChange() {
  triggerAutoSave();
}
</script>

<template>
  <div v-if="config">
  <!-- 自动亮起/选用英雄 -->
  <n-collapse arrow-placement="right" class="collapse-card">
    <n-collapse-item name="autohover">
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
                <polyline points="9 11 12 14 22 4"></polyline>
                <path
                  d="M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11"
                ></path>
              </svg>
            </div>
            <div class="title-container">
              <h3 class="card-title">{{ t("tools.autoHover.title") }}</h3>
              <span class="card-desc">{{ t("tools.autoHover.desc") }}</span>
            </div>
          </div>
          <div class="collapse-right-status">
            <span class="status-preview">{{
              config.Functions.EnableAutoHoverChampion
                ? t("tools.autoHover.statusEnabled")
                : t("tools.autoHover.statusDisabled")
            }}</span>
          </div>
        </div>
      </template>
      <div class="setting-row">
        <span class="setting-label">{{ t("tools.autoHover.enableHover") }}</span>
        <n-switch
          :value="config.Functions.EnableAutoHoverChampion"
          @update:value="
            (v: boolean) => {
              if (config) config.Functions.EnableAutoHoverChampion = v;
              triggerAutoSave();
            }
          "
        />
      </div>
      <div class="setting-row">
        <span class="setting-label">{{ t("tools.autoHover.confirmTimeout") }}</span>
        <n-switch
          :value="config.Functions.AutoSelectConfirmOnTimeout"
          @update:value="
            (v: boolean) => {
              if (config) config.Functions.AutoSelectConfirmOnTimeout = v;
              triggerAutoSave();
            }
          "
        />
      </div>

      <!-- 分路选择选项卡 -->
      <div class="lane-tab-group">
        <button
          v-for="lane in LANE_OPTIONS"
          :key="lane.value"
          :class="['lane-tab-btn', { active: hoverActiveLane === lane.value }]"
          @click="hoverActiveLane = lane.value"
        >
          {{ lane.label }}
        </button>
      </div>

      <div class="setting-picker-row">
        <ChampionPicker
          v-if="hoverActiveLane === 'default'"
          v-model="config.Functions.AutoSelectChampion"
          :maxCount="1"
          @update:modelValue="onPickerChange"
        />
        <ChampionPicker
          v-else-if="hoverActiveLane === 'top'"
          v-model="config.Functions.AutoSelectChampionTop"
          :maxCount="1"
          @update:modelValue="onPickerChange"
        />
        <ChampionPicker
          v-else-if="hoverActiveLane === 'jug'"
          v-model="config.Functions.AutoSelectChampionJug"
          :maxCount="1"
          @update:modelValue="onPickerChange"
        />
        <ChampionPicker
          v-else-if="hoverActiveLane === 'mid'"
          v-model="config.Functions.AutoSelectChampionMid"
          :maxCount="1"
          @update:modelValue="onPickerChange"
        />
        <ChampionPicker
          v-else-if="hoverActiveLane === 'bot'"
          v-model="config.Functions.AutoSelectChampionBot"
          :maxCount="1"
          @update:modelValue="onPickerChange"
        />
        <ChampionPicker
          v-else-if="hoverActiveLane === 'sup'"
          v-model="config.Functions.AutoSelectChampionSup"
          :maxCount="1"
          @update:modelValue="onPickerChange"
        />
      </div>
    </n-collapse-item>
  </n-collapse>

  <!-- 自动禁用英雄 -->
  <n-collapse arrow-placement="right" class="collapse-card">
    <n-collapse-item name="autoban">
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
                <line x1="9" y1="9" x2="15" y2="15"></line>
                <line x1="15" y1="9" x2="9" y2="15"></line>
              </svg>
            </div>
            <div class="title-container">
              <h3 class="card-title">{{ t("tools.autoBan.title") }}</h3>
              <span class="card-desc">{{ t("tools.autoBan.desc") }}</span>
            </div>
          </div>
          <div class="collapse-right-status">
            <span class="status-preview">{{
              config.Functions.EnableAutoBanChampion
                ? t("tools.autoBan.statusEnabled")
                : t("tools.autoBan.statusDisabled")
            }}</span>
          </div>
        </div>
      </template>
      <div class="setting-row">
        <span class="setting-label">{{ t("tools.autoBan.enableBan") }}</span>
        <n-switch
          :value="config.Functions.EnableAutoBanChampion"
          @update:value="
            (v: boolean) => {
              if (config) config.Functions.EnableAutoBanChampion = v;
              triggerAutoSave();
            }
          "
        />
      </div>
      <div class="setting-row">
        <span class="setting-label">{{ t("tools.autoBan.pretendBan") }}</span>
        <n-switch
          :value="config.Functions.PretendBan"
          @update:value="
            (v: boolean) => {
              if (config) config.Functions.PretendBan = v;
              triggerAutoSave();
            }
          "
        />
      </div>
      <div class="setting-row">
        <span class="setting-label">{{ t("tools.autoBan.banDelay") }}</span>
        <n-input-number
          :value="config.Functions.AutoBanDelay"
          :min="0"
          :max="15"
          :step="0.5"
          @update:value="
            (v: number | null) => {
              if (config) config.Functions.AutoBanDelay = v ?? 0;
              triggerAutoSave();
            }
          "
          style="width: 120px"
          size="small"
        />
      </div>

      <!-- 分路选择选项卡 -->
      <div class="lane-tab-group">
        <button
          v-for="lane in LANE_OPTIONS"
          :key="lane.value"
          :class="['lane-tab-btn', { active: banActiveLane === lane.value }]"
          @click="banActiveLane = lane.value"
        >
          {{ lane.label }}
        </button>
      </div>

      <div class="setting-picker-row">
        <ChampionPicker
          v-if="banActiveLane === 'default'"
          v-model="config.Functions.AutoBanChampion"
          :maxCount="1"
          @update:modelValue="onPickerChange"
        />
        <ChampionPicker
          v-else-if="banActiveLane === 'top'"
          v-model="config.Functions.AutoBanChampionTop"
          :maxCount="1"
          @update:modelValue="onPickerChange"
        />
        <ChampionPicker
          v-else-if="banActiveLane === 'jug'"
          v-model="config.Functions.AutoBanChampionJug"
          :maxCount="1"
          @update:modelValue="onPickerChange"
        />
        <ChampionPicker
          v-else-if="banActiveLane === 'mid'"
          v-model="config.Functions.AutoBanChampionMid"
          :maxCount="1"
          @update:modelValue="onPickerChange"
        />
        <ChampionPicker
          v-else-if="banActiveLane === 'bot'"
          v-model="config.Functions.AutoBanChampionBot"
          :maxCount="1"
          @update:modelValue="onPickerChange"
        />
        <ChampionPicker
          v-else-if="banActiveLane === 'sup'"
          v-model="config.Functions.AutoBanChampionSup"
          :maxCount="1"
          @update:modelValue="onPickerChange"
        />
      </div>
    </n-collapse-item>
  </n-collapse>

  <!-- 自动设置召唤师技能 -->
  <n-collapse arrow-placement="right" class="collapse-card">
    <n-collapse-item name="autospells">
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
                <polygon points="12 2 2 7 12 12 22 7 12 2"></polygon>
                <polyline points="2 17 12 22 22 17"></polyline>
                <polyline points="2 12 12 17 22 12"></polyline>
              </svg>
            </div>
            <div class="title-container">
              <h3 class="card-title">{{ t("tools.autoSpells.title") }}</h3>
              <span class="card-desc">{{ t("tools.autoSpells.desc") }}</span>
            </div>
          </div>
          <div class="collapse-right-status">
            <span class="status-preview">{{
              config.Functions.EnableAutoSetSpells
                ? t("tools.autoSpells.statusEnabled")
                : t("tools.autoSpells.statusDisabled")
            }}</span>
          </div>
        </div>
      </template>
      <div class="setting-row">
        <span class="setting-label">{{ t("tools.autoSpells.label") }}</span>
        <n-switch
          :value="config.Functions.EnableAutoSetSpells"
          @update:value="
            (v: boolean) => {
              if (config) config.Functions.EnableAutoSetSpells = v;
              triggerAutoSave();
            }
          "
        />
      </div>

      <!-- 分路选择选项卡 -->
      <div class="lane-tab-group">
        <button
          v-for="lane in LANE_OPTIONS"
          :key="lane.value"
          :class="['lane-tab-btn', { active: spellActiveLane === lane.value }]"
          @click="spellActiveLane = lane.value"
        >
          {{ lane.label }}
        </button>
      </div>

      <div class="setting-picker-row">
        <SpellPicker
          v-if="spellActiveLane === 'default'"
          v-model="config.Functions.AutoSetSummonerSpell"
          @update:modelValue="onPickerChange"
        />
        <SpellPicker
          v-else-if="spellActiveLane === 'top'"
          v-model="config.Functions.AutoSetSummonerSpellTop"
          @update:modelValue="onPickerChange"
        />
        <SpellPicker
          v-else-if="spellActiveLane === 'jug'"
          v-model="config.Functions.AutoSetSummonerSpellJug"
          @update:modelValue="onPickerChange"
        />
        <SpellPicker
          v-else-if="spellActiveLane === 'mid'"
          v-model="config.Functions.AutoSetSummonerSpellMid"
          @update:modelValue="onPickerChange"
        />
        <SpellPicker
          v-else-if="spellActiveLane === 'bot'"
          v-model="config.Functions.AutoSetSummonerSpellBot"
          @update:modelValue="onPickerChange"
        />
        <SpellPicker
          v-else-if="spellActiveLane === 'sup'"
          v-model="config.Functions.AutoSetSummonerSpellSup"
          @update:modelValue="onPickerChange"
        />
      </div>
    </n-collapse-item>
  </n-collapse>
  </div>
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

.setting-picker-row {
  padding-top: 6px;
  padding-bottom: 10px;
  width: 100%;
}

.lane-tab-group {
  display: flex;
  background: rgba(0, 0, 0, 0.03);
  padding: 4px;
  border-radius: 8px;
  border: 1px solid var(--border-color);
  margin: 14px 0 8px;
  gap: 4px;
  width: 100%;
}

.lane-tab-btn {
  flex: 1;
  background: transparent;
  border: none;
  color: var(--text-muted);
  padding: 6px 10px;
  border-radius: 6px;
  font-size: 0.78rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease;
  white-space: nowrap;
  text-align: center;
}

.lane-tab-btn:hover {
  color: var(--text-color);
  background: var(--card-bg);
}

.lane-tab-btn.active {
  color: var(--primary-color);
  background: var(--card-bg-hover);
  box-shadow: var(--shadow-sm);
}
</style>
