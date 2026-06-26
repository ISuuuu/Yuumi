<script setup lang="ts">
import { ref, computed, onMounted, watch, nextTick } from "vue";
import { lcuRequest } from "../api/lcu";
import LcuImage from "./LcuImage.vue";

const props = defineProps<{
  modelValue: number[];
  maxCount?: number;
}>();

const emit = defineEmits<{
  (e: "update:modelValue", value: number[]): void;
}>();

interface ChampionEntry {
  id: number;
  name: string;
  iconPath: string;
}

const champions = ref<ChampionEntry[]>([]);
const loading = ref(true);
const searchQuery = ref("");
const showPicker = ref(false);
const searchInputRef = ref<HTMLInputElement | null>(null);
const tencentKeywords = ref<Record<number, string>>({});

const selected = computed(() => props.modelValue || []);

const filteredChampions = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  let list = champions.value;
  if (q) {
    list = list.filter((c) => {
      // 1. 本地名字或 ID 匹配
      if (c.name.toLowerCase().includes(q) || c.id.toString() === q) {
        return true;
      }
      // 2. 腾讯拼音/别名匹配
      const kw = tencentKeywords.value[c.id];
      if (kw && kw.toLowerCase().includes(q)) {
        return true;
      }
      return false;
    });
  }
  return list;
});

const selectedChampions = computed(() => {
  const map = new Map(champions.value.map((c) => [c.id, c]));
  return selected.value.map((id) => map.get(id)).filter(Boolean) as ChampionEntry[];
});

async function loadChampions() {
  if (champions.value.length > 0) return;
  loading.value = true;
  
  let success = false;
  let rawData: any = null;

  // 尝试 1: champion-summary.json (标准轻量汇总)
  try {
    const resp = await lcuRequest<any>("GET", "/lol-game-data/assets/v1/champion-summary.json");
    if (resp.success && resp.data) {
      rawData = resp.data;
      success = true;
      console.log("Yuumi - 成功通过 champion-summary.json 获取英雄列表");
    }
  } catch (e) {
    console.error("Yuumi - 通过 champion-summary.json 获取失败:", e);
  }

  // 尝试 2: champions.json (常规备选)
  if (!success) {
    try {
      const resp = await lcuRequest<any>("GET", "/lol-game-data/assets/v1/champions.json");
      if (resp.success && resp.data) {
        rawData = resp.data;
        success = true;
        console.log("Yuumi - 成功通过 champions.json 获取英雄列表");
      }
    } catch (e) {
      console.error("Yuumi - 通过 champions.json 获取失败:", e);
    }
  }

  // 解析并格式化数据，兼容数组和键值对字典结构
  if (success && rawData) {
    let list: any[] = [];
    if (Array.isArray(rawData)) {
      list = rawData;
    } else if (typeof rawData === "object" && rawData !== null) {
      list = Object.values(rawData);
    }

    if (list.length > 0) {
      champions.value = list
        .filter((c: any) => c && c.id > 0)
        .map((c: any) => ({
          id: c.id,
          name: c.name || c.alias || `#${c.id}`,
          iconPath: c.squarePortraitPath || `/lol-game-data/assets/v1/champion-icons/${c.id}.png`,
        }))
        .sort((a: ChampionEntry, b: ChampionEntry) => a.name.localeCompare(b.name, "zh"));
      console.log(`Yuumi - 成功渲染了 ${champions.value.length} 个英雄`);
    } else {
      console.error("Yuumi - 提取的英雄列表为空");
    }
  } else {
    console.error("Yuumi - 所有英雄端点请求均失败");
  }
  
  loading.value = false;
}

// 异步加载腾讯的英雄关键字（用于支持中文拼音和别称搜索）
async function loadTencentKeywords() {
  try {
    const res = await fetch("https://game.gtimg.cn/images/lol/act/img/js/heroList/hero_list.js");
    if (res.ok) {
      const data = await res.json();
      if (data && Array.isArray(data.hero)) {
        const map: Record<number, string> = {};
        for (const h of data.hero) {
          const id = Number(h.heroId);
          if (id > 0) {
            map[id] = (h.keywords || "") + "," + (h.title || "") + "," + (h.alias || "");
          }
        }
        // 补充 Python 项目里的额外自定义别名（Smolder, Naafiri, Milio, K'Sante 等较新英雄的戏称）
        if (map[901]) map[901] += ",小火龙";
        if (map[950]) map[950] += ",狗,那亚菲利";
        if (map[902]) map[902] += ",丁真,米利欧";
        if (map[897]) map[897] += ",黑龙,奎桑提";
        
        tencentKeywords.value = map;
        console.log("Yuumi - 成功加载腾讯英雄别称/拼音检索库");
      }
    }
  } catch (e) {
    console.warn("Yuumi - 载入腾讯别称接口失败 (将回退到默认本地搜索):", e);
  }
}

onMounted(() => {
  loadChampions();
  loadTencentKeywords();
});

// 监听弹窗打开状态，自动聚焦输入框并重置搜索词，且触发加载英雄列表（防初次未连接失败）
watch(showPicker, (newVal) => {
  if (newVal) {
    searchQuery.value = "";
    loadChampions();
    nextTick(() => {
      searchInputRef.value?.focus();
    });
  }
});

function toggleChampion(id: number) {
  const list = [...selected.value];
  const idx = list.indexOf(id);
  if (idx >= 0) {
    list.splice(idx, 1);
  } else {
    if (props.maxCount && props.maxCount === 1) {
      // 限制为 1 位英雄时，点选新英雄直接覆盖替换
      list.splice(0, list.length, id);
    } else if (props.maxCount && list.length >= props.maxCount) {
      return;
    } else {
      list.push(id);
    }
  }
  emit("update:modelValue", list);
}

function removeChampion(id: number) {
  emit("update:modelValue", selected.value.filter((x) => x !== id));
}

function clearAll() {
  emit("update:modelValue", []);
}
</script>

<template>
  <div class="champion-picker">
    <!-- 已选 + 选择按钮 -->
    <div class="picker-trigger">
      <div class="selected-chips">
        <div
          v-for="champ in selectedChampions"
          :key="champ.id"
          class="chip"
          :title="champ.name"
        >
          <LcuImage :src="champ.iconPath" class="chip-icon" alt="champ" />
          <span class="chip-name">{{ champ.name }}</span>
          <span class="chip-remove" @click.stop="removeChampion(champ.id)">✕</span>
        </div>
        <span v-if="selectedChampions.length === 0" class="chip-empty">未选择英雄</span>
      </div>
      <div class="trigger-actions">
        <button v-if="selected.length > 0" class="clear-btn" @click="clearAll">清空</button>
        <button class="select-btn" @click="showPicker = true">选择</button>
      </div>
    </div>

    <!-- 弹出选择框 (Teleport) -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showPicker" class="modal-overlay" @click="showPicker = false">
          <div class="modal-content" @click.stop>
            <div class="modal-header">
              <h3 class="modal-title">选择英雄</h3>
              <button class="close-btn" @click="showPicker = false">✕</button>
            </div>
            
            <div class="search-box">
              <svg class="search-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <circle cx="11" cy="11" r="8"/>
                <line x1="21" y1="21" x2="16.65" y2="16.65"/>
              </svg>
              <input
                ref="searchInputRef"
                v-model="searchQuery"
                placeholder="输入名称、拼音缩写或别称 (例如: 瞎子, ys, hazn)..."
                class="search-input"
                type="text"
              />
            </div>

            <div v-if="loading" class="picker-loading">
              <div class="loading-spinner"></div>
              <span>加载中...</span>
            </div>
            <div v-else class="champion-grid">
              <div
                v-for="champ in filteredChampions"
                :key="champ.id"
                :class="['champ-cell', { selected: selected.includes(champ.id) }]"
                :title="champ.name"
                @click="toggleChampion(champ.id)"
              >
                <LcuImage :src="champ.iconPath" class="champ-icon" alt="champ" />
                <span class="champ-name">{{ champ.name }}</span>
                <div v-if="selected.includes(champ.id)" class="selected-badge">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
                    <polyline points="20 6 9 17 4 12"/>
                  </svg>
                </div>
              </div>
            </div>

            <div class="modal-footer">
              <span class="selected-count">已选择 {{ selected.length }} 个英雄</span>
              <button class="confirm-btn" @click="showPicker = false">确定</button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<style scoped>
.champion-picker {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

/* 触发区域：已选英雄 + 选择按钮 */
.picker-trigger {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.selected-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  flex: 1;
  min-height: 32px;
  align-items: center;
}

.chip {
  display: flex;
  align-items: center;
  gap: 4px;
  background: var(--card-bg);
  border: 1px solid var(--settings-card-border);
  border-radius: 6px;
  padding: 3px 8px 3px 3px;
  font-size: 0.78rem;
  color: var(--text-color);
}

.chip-icon {
  width: 22px;
  height: 22px;
  border-radius: 4px;
  overflow: hidden;
}

.chip-name {
  max-width: 60px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.chip-remove {
  cursor: pointer;
  color: var(--text-dimmed);
  font-size: 0.7rem;
  margin-left: 2px;
  padding: 0 2px;
}
.chip-remove:hover { color: var(--loss-color); }

.chip-empty {
  font-size: 0.82rem;
  color: var(--text-dimmed);
}

.trigger-actions {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}

.select-btn {
  padding: 6px 16px;
  border: 1px solid var(--settings-card-border);
  border-radius: 6px;
  background: var(--settings-card-bg);
  color: var(--text-muted);
  font-size: 0.82rem;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}
.select-btn:hover {
  background: var(--primary-color);
  color: white;
  border-color: var(--primary-color);
}

.clear-btn {
  padding: 6px 12px;
  border: 1px solid var(--settings-card-border);
  border-radius: 6px;
  background: var(--settings-card-bg);
  color: var(--text-dimmed);
  font-size: 0.78rem;
  cursor: pointer;
}
.clear-btn:hover {
  color: var(--loss-color);
  border-color: var(--loss-color);
}

/* 模态遮罩 */
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  background: rgba(0, 0, 0, 0.45);
  backdrop-filter: blur(5px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
}

/* 模态框主体 */
.modal-content {
  background: var(--card-bg);
  border-radius: 16px;
  width: 90%;
  max-width: 560px;
  max-height: 80vh;
  display: flex;
  flex-direction: column;
  box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.15), 0 10px 10px -5px rgba(0, 0, 0, 0.05);
  border: 1px solid var(--border-color);
  overflow: hidden;
  position: relative;
}

/* 头部 */
.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 24px;
  border-bottom: 1px solid var(--settings-separator);
}

.modal-title {
  margin: 0;
  font-size: 1.15rem;
  font-weight: 700;
  color: var(--text-color);
}

.close-btn {
  background: transparent;
  border: none;
  font-size: 1.2rem;
  color: var(--text-dimmed);
  cursor: pointer;
  padding: 4px;
  line-height: 1;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  width: 28px;
  height: 28px;
}
.close-btn:hover {
  background: var(--settings-card-bg-hover);
  color: var(--loss-color);
}

/* 搜索框 */
.search-box {
  padding: 14px 24px 6px;
  position: relative;
  display: flex;
  align-items: center;
}

.search-icon {
  position: absolute;
  left: 36px;
  width: 16px;
  height: 16px;
  color: var(--text-dimmed);
  pointer-events: none;
}

.search-input {
  width: 100%;
  padding: 9px 12px 9px 36px;
  border: 1px solid var(--settings-card-border);
  border-radius: 8px;
  font-size: 0.88rem;
  color: var(--text-color);
  outline: none;
  transition: all 0.25s;
  background: var(--card-bg);
}
.search-input:focus {
  border-color: var(--primary-color);
  background: var(--card-bg);
  box-shadow: 0 0 0 3px var(--primary-color-alpha-15);
}

/* 加载中 */
.picker-loading {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 3rem;
  color: var(--text-dimmed);
  font-size: 0.9rem;
}

.loading-spinner {
  width: 28px;
  height: 28px;
  border: 3px solid var(--settings-card-border);
  border-top-color: var(--primary-color);
  border-radius: 50%;
  animation: spin 1s infinite linear;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

/* 英雄网格 */
.champion-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(68px, 1fr));
  gap: 8px;
  max-height: 420px;
  overflow-y: auto;
  padding: 12px 24px;
}

/* 自定义滚动条 */
.champion-grid::-webkit-scrollbar {
  width: 6px;
}
.champion-grid::-webkit-scrollbar-track {
  background: transparent;
}
.champion-grid::-webkit-scrollbar-thumb {
  background: var(--settings-card-border);
  border-radius: 3px;
}
.champion-grid::-webkit-scrollbar-thumb:hover {
  background: var(--text-dimmed);
}

.champ-cell {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  padding: 6px;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
  border: 2px solid transparent;
  user-select: none;
}
.champ-cell:hover {
  background: var(--settings-card-bg-hover);
  transform: translateY(-2px);
}
.champ-cell.selected {
  border-color: var(--primary-color);
  background: var(--primary-color-alpha-15);
}

.champ-icon {
  width: 46px;
  height: 46px;
  border-radius: 8px;
  overflow: hidden;
  transition: transform 0.2s;
}
.champ-cell:hover .champ-icon {
  transform: scale(1.04);
}

.champ-name {
  font-size: 0.72rem;
  color: var(--text-color);
  text-align: center;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 58px;
  font-weight: 500;
}
.champ-cell.selected .champ-name {
  color: var(--primary-color);
  font-weight: 600;
}

/* 选中角标 */
.selected-badge {
  position: absolute;
  top: 2px;
  right: 2px;
  background: var(--primary-color);
  color: white;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 1px 3px rgba(0,0,0,0.15);
  border: 1px solid white;
}
.selected-badge svg {
  width: 9px;
  height: 9px;
}

/* 底部 */
.modal-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px 24px;
  border-top: 1px solid var(--settings-separator);
  background: var(--settings-card-bg);
}

.selected-count {
  font-size: 0.82rem;
  color: var(--text-dimmed);
}

.confirm-btn {
  padding: 8px 24px;
  background: var(--primary-color);
  color: white;
  border: none;
  border-radius: 8px;
  font-size: 0.88rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
  box-shadow: 0 2px 6px var(--primary-color-alpha-30);
}
.confirm-btn:hover {
  background: var(--primary-color-hover);
  box-shadow: 0 4px 12px var(--primary-color-alpha-40);
}

/* 渐变过渡动画 */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.25s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
.fade-enter-active .modal-content {
  animation: modal-zoom-in 0.25s cubic-bezier(0.34, 1.56, 0.64, 1);
}
.fade-leave-active .modal-content {
  animation: modal-zoom-out 0.2s ease-in;
}

@keyframes modal-zoom-in {
  from {
    transform: scale(0.9) translateY(15px);
    opacity: 0;
  }
  to {
    transform: scale(1) translateY(0);
    opacity: 1;
  }
}
@keyframes modal-zoom-out {
  from {
    transform: scale(1) translateY(0);
    opacity: 1;
  }
  to {
    transform: scale(0.95) translateY(10px);
    opacity: 0;
  }
}
</style>
