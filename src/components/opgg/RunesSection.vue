<script setup lang="ts">
import { ref, computed, watch } from "vue";
import LcuImage from "../LcuImage.vue";
import type { GameDataAssets } from "../../types/lcu";
import type { OpggRunePreset, PerkStyle, Perk } from "../../types/opgg";
import { pct, resolveRuneIcon } from "./shared";

const props = defineProps<{
  runes: OpggRunePreset[];
  perkStyles: PerkStyle[];
  perksMap: Map<number, Perk>;
  gameDataAssets: GameDataAssets | null;
}>();

const emit = defineEmits<{
  (e: "apply", rune: OpggRunePreset): void;
}>();

const selectedRuneIdx = ref(0);

// 数据刷新后重置选择的符文预设索引
watch(
  () => props.runes,
  () => {
    selectedRuneIdx.value = 0;
  },
);

// 过滤 HTML 标签的工具函数
function cleanDescription(text: string): string {
  if (!text) return "";
  return text
    .replace(/<[^>]*>/g, "") // remove html tags
    .replace(/&nbsp;/g, " ")
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .trim();
}

// 符文悬浮提示（模板中多次复用，统一收敛空值）
function perkTitle(perkId: number): string {
  const perk = props.perksMap.get(perkId);
  return perk
    ? perk.name + "\n" + cleanDescription(perk.shortDesc ?? "")
    : "";
}

// 符文计算属性
const activeRune = computed(() => {
  if (!props.runes?.length) return null;
  return props.runes[selectedRuneIdx.value] || null;
});

const primaryStyle = computed(() => {
  const rune = activeRune.value;
  if (!rune || !props.perkStyles) return null;
  return props.perkStyles.find((s) => s.id === rune.primary_page_id) || null;
});

const secondaryStyle = computed(() => {
  const rune = activeRune.value;
  if (!rune || !props.perkStyles) return null;
  return props.perkStyles.find((s) => s.id === rune.secondary_page_id) || null;
});

// 一键设置符文页
function applyActiveRune() {
  const rune = activeRune.value;
  if (!rune) return;
  emit("apply", rune);
}

function getRuneIcon(id: number): string {
  return resolveRuneIcon(props.gameDataAssets, props.perkStyles, id);
}
</script>

<template>
  <!-- 符文推荐：OP.GG 风格双符文树与碎片 -->
  <div
    v-if="runes?.length && activeRune && primaryStyle && secondaryStyle"
    class="build-card rune-tree-card"
  >
    <div class="rune-tree-container">
      <!-- 左侧：主系、副系、属性碎片三列 -->
      <div class="rune-tree-left">
        <!-- 主系符文列 -->
        <div class="rune-tree-column primary-column">
          <div class="tree-header">
            <LcuImage
              :src="primaryStyle.iconPath"
              class="tree-style-icon"
              :alt="primaryStyle.name"
            />
          </div>
          <div class="tree-slots">
            <div
              v-for="(slot, sIdx) in primaryStyle.slots.slice(0, 4)"
              :key="sIdx"
              :class="['tree-slot-row', `slot-${sIdx}`]"
            >
              <div
                v-for="perkId in slot.perks"
                :key="perkId"
                :class="[
                  'rune-item',
                  {
                    active: activeRune.primary_rune_ids?.includes(perkId),
                    keystone: sIdx === 0,
                  },
                ]"
                :title="perkTitle(perkId)"
              >
                <LcuImage :src="getRuneIcon(perkId)" class="rune-icon" />
              </div>
            </div>
          </div>
        </div>

        <!-- 副系符文列 -->
        <div class="rune-tree-column secondary-column">
          <div class="tree-header">
            <LcuImage
              :src="secondaryStyle.iconPath"
              class="tree-style-icon"
              :alt="secondaryStyle.name"
            />
          </div>
          <div class="tree-slots">
            <div
              v-for="(slot, sIdx) in secondaryStyle.slots.slice(1, 4)"
              :key="sIdx"
              :class="['tree-slot-row', `slot-${sIdx + 1}`]"
            >
              <div
                v-for="perkId in slot.perks"
                :key="perkId"
                :class="[
                  'rune-item',
                  {
                    active: activeRune.secondary_rune_ids?.includes(perkId),
                  },
                ]"
                :title="perkTitle(perkId)"
              >
                <LcuImage :src="getRuneIcon(perkId)" class="rune-icon" />
              </div>
            </div>
          </div>
        </div>

        <!-- 属性碎片列 -->
        <div class="rune-tree-column shards-column">
          <div class="tree-header">
            <!-- 占位保持对齐 -->
            <div class="shards-placeholder"></div>
          </div>
          <div class="tree-slots">
            <div
              v-for="(slot, sIdx) in primaryStyle.slots.slice(4, 7)"
              :key="sIdx"
              :class="['tree-slot-row', `slot-${sIdx + 4}`]"
            >
              <div
                v-for="perkId in slot.perks"
                :key="perkId"
                :class="[
                  'shard-item',
                  { active: activeRune.stat_mod_ids?.includes(perkId) },
                ]"
                :title="perksMap.get(perkId)?.name || ''"
              >
                <LcuImage :src="getRuneIcon(perkId)" class="shard-icon" />
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- 右侧：预设切换列表与一键应用 -->
      <div class="rune-tree-right">
        <div class="rune-presets-box">
          <div
            v-for="(r, i) in runes.slice(0, 3)"
            :key="i"
            :class="['rune-preset-row', { active: i === selectedRuneIdx }]"
            @click="selectedRuneIdx = i"
          >
            <LcuImage
              :src="getRuneIcon(r.primary_page_id)"
              class="preset-page-icon"
            />
            <div class="preset-info">
              <div class="preset-wr">{{ pct(r.win / r.play) }}</div>
              <div class="preset-games">
                {{ $t("career.gamesCount", { count: r.play }) }}
              </div>
            </div>
            <span class="preset-pick">{{ pct(r.pick_rate) }}</span>
          </div>
        </div>
        <button class="apply-rune-btn" @click="applyActiveRune">
          {{ $t("opgg.applyRunePage") }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 通用卡片美化 */
.build-card {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  margin: 10px 14px;
  padding: 12px 16px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.02);
}

/* 符文推荐树状图样式 */
.rune-tree-container {
  display: flex;
  gap: 16px;
  background: var(--hover-bg);
  border: 1px solid var(--border-color);
  border-radius: 6px;
  padding: 16px;
  align-items: stretch;
  width: 100%;
}

/* 符文推荐树状图样式 */
.rune-tree-card {
  padding: 12px 16px;
}
.rune-tree-container {
  display: flex;
  gap: 16px;
  background: rgba(0, 0, 0, 0.01);
  border: 1px solid var(--border-color);
  border-radius: 6px;
  padding: 16px;
  align-items: stretch;
  width: 100%;
}
.rune-tree-left {
  display: flex;
  gap: 20px;
  flex: 1;
  justify-content: space-around;
  align-items: flex-start;
}
.rune-tree-column {
  display: flex;
  flex-direction: column;
  align-items: center;
}
.tree-header {
  height: 36px;
  display: flex;
  align-items: center;
  justify-content: center;
  margin-bottom: 12px;
}
.tree-style-icon {
  width: 24px;
  height: 24px;
}
.shards-placeholder {
  height: 36px;
}
.tree-slots {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.tree-slot-row {
  display: flex;
  justify-content: center;
  gap: 8px;
  align-items: center;
}
.rune-item {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.05);
  border: 1px solid transparent;
  transition: all 0.2s;
  opacity: 0.35;
  filter: grayscale(1);
}
.rune-item.keystone {
  width: 38px;
  height: 38px;
}
.rune-item.active {
  opacity: 1;
  filter: none;
  border-color: #e5a93b;
  box-shadow: 0 0 8px rgba(229, 169, 59, 0.3);
  background: rgba(229, 169, 59, 0.1);
  transform: scale(1.08);
}
.rune-icon {
  width: 80%;
  height: 80%;
}
.shard-item {
  width: 18px;
  height: 18px;
  border-radius: 50%;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.08);
  border: 1px solid transparent;
  transition: all 0.2s;
  opacity: 0.3;
  filter: grayscale(1);
}
.shard-item.active {
  opacity: 1;
  filter: none;
  border-color: #51cf66;
  box-shadow: 0 0 6px rgba(81, 207, 102, 0.4);
  background: rgba(81, 207, 102, 0.15);
  transform: scale(1.1);
}
.shard-icon {
  width: 80%;
  height: 80%;
}
.rune-tree-right {
  width: 170px;
  border-left: 1px solid var(--border-color);
  padding-left: 16px;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  gap: 12px;
}
.rune-presets-box {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.rune-preset-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border-radius: 6px;
  border: 1px solid var(--border-color);
  cursor: pointer;
  background: var(--bg-color);
  transition: all 0.2s;
}
.rune-preset-row:hover {
  border-color: var(--primary-color);
}
.rune-preset-row.active {
  border-color: var(--primary-color);
  background: var(--primary-color-alpha-15);
}
.preset-page-icon {
  width: 20px;
  height: 20px;
  border-radius: 50%;
}
.preset-info {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 1px;
}
.preset-wr {
  font-size: 0.72rem;
  font-weight: 700;
  color: var(--text-color);
}
.preset-games {
  font-size: 0.6rem;
  color: var(--text-dimmed);
}
.preset-pick {
  font-size: 0.72rem;
  font-weight: 700;
  color: var(--text-muted);
}
.apply-rune-btn {
  width: 100%;
  padding: 8px;
  background: #2fbc5d;
  color: white;
  border: none;
  border-radius: 6px;
  font-size: 0.75rem;
  font-weight: 700;
  cursor: pointer;
  text-align: center;
  transition: background 0.15s;
}
.apply-rune-btn:hover {
  background: #25a44e;
}
</style>
