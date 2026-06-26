<script setup lang="ts">
import { ref, computed, onMounted, watch } from "vue";
import { lcuRequest } from "../api/lcu";
import LcuImage from "./LcuImage.vue";

const props = defineProps<{
  modelValue: number[];
}>();

const emit = defineEmits<{
  (e: "update:modelValue", value: number[]): void;
}>();

interface SpellEntry {
  id: number;
  name: string;
  iconPath: string;
}

const spells = ref<SpellEntry[]>([]);
const loading = ref(true);
const showPicker = ref(false);
const activeSlot = ref<'D' | 'F' | null>(null);

const selectedIds = computed(() => props.modelValue || []);

// 获取 D 键选中的技能
const dSpell = computed(() => {
  const id = selectedIds.value[0];
  return spells.value.find(s => s.id === id);
});

// 获取 F 键选中的技能
const fSpell = computed(() => {
  const id = selectedIds.value[1];
  return spells.value.find(s => s.id === id);
});

async function loadSpells() {
  if (spells.value.length > 0) return;
  loading.value = true;
  try {
    const resp = await lcuRequest<any[]>("GET", "/lol-game-data/assets/v1/summoner-spells.json");
    if (resp.success && resp.data) {
      const seenNames = new Set<string>();
      const unique: SpellEntry[] = [];
      // 按照 ID 升序排列，这样名字重复时会优先保留 ID 较小的常规技能（比如普通闪现 ID 为 4）
      const sortedData = [...resp.data].sort((a, b) => (a.id || 0) - (b.id || 0));
      for (const s of sortedData) {
        const name = (s.name || "").trim();
        if (s.id > 0 && name && !name.toLowerCase().includes("placeholder") && !name.includes("占位") && !seenNames.has(name)) {
          seenNames.add(name);
          unique.push({
            id: s.id,
            name: name,
            iconPath: s.iconPath || "",
          });
        }
      }
      spells.value = unique.sort((a, b) => a.name.localeCompare(b.name, "zh"));
    }
  } catch (e) {
    console.error("加载召唤师技能列表失败:", e);
  } finally {
    loading.value = false;
  }
}

onMounted(() => {
  loadSpells();
});

watch(showPicker, (newVal) => {
  if (newVal) {
    loadSpells();
  }
});

function openPicker(slot: 'D' | 'F') {
  activeSlot.value = slot;
  showPicker.value = true;
}

function selectSpell(id: number) {
  const list = [...selectedIds.value];
  
  // 确保数组包含两个位置
  while (list.length < 2) {
    list.push(0);
  }

  if (activeSlot.value === 'D') {
    list[0] = id;
    // 如果 F 键也是这个技能，清空 F 键以防重复
    if (list[1] === id) {
      list[1] = 0;
    }
  } else if (activeSlot.value === 'F') {
    list[1] = id;
    // 如果 D 键也是这个技能，清空 D 键以防重复
    if (list[0] === id) {
      list[0] = 0;
    }
  }

  // 过滤掉未设置 (0) 的技能，传回后端
  emit("update:modelValue", list.filter(x => x > 0));
  showPicker.value = false;
  activeSlot.value = null;
}

function removeSpell(slot: 'D' | 'F') {
  const list = [...selectedIds.value];
  while (list.length < 2) {
    list.push(0);
  }
  if (slot === 'D') {
    list[0] = 0;
  } else {
    list[1] = 0;
  }
  emit("update:modelValue", list.filter(x => x > 0));
}
</script>

<template>
  <div class="spell-picker">
    <!-- D/F 两个技能槽 -->
    <div class="slots-container">
      <!-- D 键技能 -->
      <div class="spell-slot-card" @click="openPicker('D')">
        <span class="slot-badge">D 键</span>
        <div v-if="dSpell" class="slot-content">
          <LcuImage :src="dSpell.iconPath" class="slot-icon" alt="spell" />
          <span class="slot-name">{{ dSpell.name }}</span>
          <span class="slot-remove" @click.stop="removeSpell('D')">✕</span>
        </div>
        <div v-else class="slot-placeholder">
          <span class="plus-icon">+</span>
          <span>选择技能</span>
        </div>
      </div>

      <!-- F 键技能 -->
      <div class="spell-slot-card" @click="openPicker('F')">
        <span class="slot-badge">F 键</span>
        <div v-if="fSpell" class="slot-content">
          <LcuImage :src="fSpell.iconPath" class="slot-icon" alt="spell" />
          <span class="slot-name">{{ fSpell.name }}</span>
          <span class="slot-remove" @click.stop="removeSpell('F')">✕</span>
        </div>
        <div v-else class="slot-placeholder">
          <span class="plus-icon">+</span>
          <span>选择技能</span>
        </div>
      </div>
    </div>

    <!-- 技能选择面板 -->
    <div v-show="showPicker" class="picker-panel">
      <div class="panel-header">
        <span class="panel-title">选择绑定到 {{ activeSlot }} 键的技能</span>
        <button class="panel-close" @click="showPicker = false; activeSlot = null">✕</button>
      </div>
      <div v-if="loading" class="picker-loading">加载中...</div>
      <div v-else class="spell-grid">
        <div
          v-for="spell in spells"
          :key="spell.id"
          :class="['spell-cell', { 
            selected: (activeSlot === 'D' && dSpell?.id === spell.id) || (activeSlot === 'F' && fSpell?.id === spell.id),
            disabled: (activeSlot === 'D' && fSpell?.id === spell.id) || (activeSlot === 'F' && dSpell?.id === spell.id)
          }]"
          :title="spell.name"
          @click="selectSpell(spell.id)"
        >
          <LcuImage :src="spell.iconPath" class="spell-icon" alt="spell" />
          <span class="spell-name">{{ spell.name }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.spell-picker {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.slots-container {
  display: flex;
  gap: 16px;
  margin-bottom: 8px;
}

.spell-slot-card {
  position: relative;
  flex: 1;
  max-width: 140px;
  height: 56px;
  border: 1.5px dashed var(--settings-card-border);
  border-radius: 10px;
  cursor: pointer;
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--card-bg);
  padding: 8px 12px;
  user-select: none;
}
.spell-slot-card:hover {
  border-color: var(--primary-color);
  background: var(--settings-card-bg-hover);
}

.slot-badge {
  position: absolute;
  top: -8px;
  left: 12px;
  background: var(--primary-color);
  color: white;
  font-size: 0.65rem;
  font-weight: 700;
  padding: 1px 6px;
  border-radius: 4px;
  box-shadow: 0 2px 4px var(--primary-color-alpha-30);
}

.slot-content {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
}

.slot-icon {
  width: 36px;
  height: 36px;
  border-radius: 6px;
  overflow: hidden;
  box-shadow: 0 2px 4px rgba(0,0,0,0.1);
  flex-shrink: 0;
}

.slot-name {
  font-size: 0.82rem;
  font-weight: 600;
  color: var(--text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex: 1;
}

.slot-remove {
  cursor: pointer;
  color: #c0c4cc;
  font-size: 0.75rem;
  padding: 4px;
  border-radius: 50%;
  transition: all 0.2s;
}
.slot-remove:hover {
  background: var(--loss-bg);
  color: var(--loss-color);
}

.slot-placeholder {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--text-dimmed);
  font-size: 0.8rem;
  font-weight: 500;
}

.plus-icon {
  font-size: 1.1rem;
  line-height: 1;
}

/* 技能网格选择面板 */
.picker-panel {
  background: var(--settings-card-bg);
  border: 1px solid var(--settings-card-border);
  border-radius: 12px;
  padding: 14px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  animation: slide-down 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

@keyframes slide-down {
  from { opacity: 0; transform: translateY(-5px); }
  to { opacity: 1; transform: translateY(0); }
}

.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-bottom: 1px solid var(--settings-separator);
  padding-bottom: 6px;
}

.panel-title {
  font-size: 0.82rem;
  color: var(--text-muted);
  font-weight: 600;
}

.panel-close {
  background: transparent;
  border: none;
  font-size: 0.85rem;
  color: var(--text-dimmed);
  cursor: pointer;
}
.panel-close:hover { color: var(--loss-color); }

.picker-loading {
  text-align: center;
  padding: 1rem;
  color: var(--text-dimmed);
  font-size: 0.85rem;
}

.spell-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(76px, 1fr));
  gap: 8px;
  max-height: 200px;
  overflow-y: auto;
  padding: 2px;
}

.spell-cell {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  padding: 6px 4px;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.15s;
  border: 2px solid transparent;
}
.spell-cell:hover { background: var(--settings-card-bg-hover); }
.spell-cell.selected {
  border-color: var(--primary-color);
  background: var(--primary-color-alpha-15);
}
.spell-cell.disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.spell-icon {
  width: 32px;
  height: 32px;
  border-radius: 6px;
  overflow: hidden;
}

.spell-name {
  font-size: 0.68rem;
  color: var(--text-muted);
  text-align: center;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 68px;
}
</style>
