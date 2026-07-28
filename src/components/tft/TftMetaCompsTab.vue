<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from "vue";
import { useI18n } from "vue-i18n";
import { NSpin, NEmpty, NButton, NTag, NModal } from "naive-ui";
import {
  useTftMetaDecks,
  getDeckDisplayName,
  getChampionDisplayName,
  getTraitDisplayName,
  getItemDisplayName,
  getItemIconUrl,
  type TftMetaDeck,
  type TftMetaUnit,
} from "../../composables/useTftMetaDecks";
import { useToast } from "../../composables/useToast";

const { t } = useI18n();
const { showToast } = useToast();
const { loading, error, decks, metadata, loadDecks, refresh } = useTftMetaDecks();
const selectedDeck = ref<TftMetaDeck | null>(null);
const showDetailModal = ref(false);

function calcScore(d: TftMetaDeck): number {
  const wr = d.stat?.win_rate ?? 0;
  const pr = d.stat?.pick_rate ?? 0;
  return wr * 100 + pr * 5;
}

const sortedDecks = computed(() => {
  const list = decks.value;
  if (!list.length) return [];
  const sorted = [...list].sort((a, b) => calcScore(b) - calcScore(a));
  const total = sorted.length;
  return sorted.map((deck, idx) => {
    const pct = idx / total;
    let tier = "D";
    if (pct < 0.2) tier = "S";
    else if (pct < 0.4) tier = "A";
    else if (pct < 0.6) tier = "B";
    else if (pct < 0.8) tier = "C";
    return { deck, tier };
  });
});

const tftVersion = computed(() => {
  let setStr = "";
  if (decks.value?.length) {
    for (const d of decks.value) {
      if (d.teamCode) {
        const match = d.teamCode.match(/TFTSet(\d+)/i);
        if (match) {
          setStr = `Set ${match[1]}`;
          break;
        }
      }
    }
  }
  let timeStr = "";
  if (metadata.value?.gameStatDateTime) {
    const d = new Date(metadata.value.gameStatDateTime);
    if (!isNaN(d.getTime())) {
      const month = String(d.getMonth() + 1).padStart(2, "0");
      const day = String(d.getDate()).padStart(2, "0");
      const hours = String(d.getHours()).padStart(2, "0");
      const mins = String(d.getMinutes()).padStart(2, "0");
      timeStr = `${month}-${day} ${hours}:${mins}`;
    }
  }
  if (setStr && timeStr) return `OP.GG ${setStr} (${timeStr} 更新)`;
  if (setStr) return `OP.GG ${setStr}`;
  if (timeStr) return `OP.GG (${timeStr} 更新)`;
  return "OP.GG 实时版本";
});

function formatPercent(v: number | undefined | null): string {
  if (v == null) return "--";
  return `${(v * 100).toFixed(1)}%`;
}

function formatPlacement(v: number | undefined | null): string {
  if (v == null) return "--";
  return `#${v.toFixed(2)}`;
}

function getDeckBadges(deck: TftMetaDeck): string[] {
  const badges: string[] = [];
  if (!deck.badge) return badges;
  for (const b of deck.badge) {
    if (b.key === "reroll" && typeof b.value === "number") {
      badges.push(`${b.value}级D牌`);
    } else if (b.key === "difficulty") {
      if (b.value === 1) badges.push("难度: 简单");
      else if (b.value === 2) badges.push("难度: 普通");
      else if (b.value === 3) badges.push("难度: 困难");
    } else if (b.key === "honey" && b.value === true) {
      badges.push("黑马上分");
    }
  }
  return badges;
}

async function copyTeamCode(code: string | undefined) {
  if (!code) return;
  try {
    await navigator.clipboard.writeText(code);
    showToast("阵容代码已复制到剪贴板，可进入游戏粘贴导入", "success");
  } catch (e) {
    console.error("复制代码失败:", e);
    showToast("复制阵容代码失败", "error");
  }
}

function getBoardUnit(units: TftMetaUnit[] | undefined, x: number, y: number): TftMetaUnit | undefined {
  if (!units) return undefined;
  return units.find((u) => u.cell && u.cell.x === x && u.cell.y === y);
}

function isFrontlineUnit(unit: TftMetaUnit): boolean {
  const key = (unit.characterId ?? unit.key ?? "").toLowerCase();

  // 1. 明确的坦克/前排/近战英雄关键字
  const tankKeywords = [
    "blitzcrank", "pantheon", "shen", "tahmkench", "ornn", "riven", "leona",
    "mordekaiser", "aatrox", "ivernminion", "briar", "rhaast", "jax", "talon",
    "nunu", "garen", "poppy", "vi", "sion", "malphite", "galio", "taric", "braum",
    "alistar", "chogath", "darius", "drmundo", "hecarim", "illaoi", "jarvaniv",
    "ksante", "kled", "leesin", "nasus", "nautilus", "olaf", "rammus", "renekton",
    "sejuani", "sett", "shyvana", "singed", "skarner", "swain", "trundle", "udyr",
    "volibear", "warwick", "wukong", "xinzhao", "yorick", "zac"
  ];
  if (tankKeywords.some((k) => key.includes(k))) {
    return true;
  }

  // 2. 明确的后排/法师/射手英雄关键字
  const backlineKeywords = [
    "nami", "jinx", "missfortune", "xayah", "zoe", "viktor", "twistedfate",
    "aurelionsol", "vex", "jhin", "lissandra", "gwen", "ahri", "anivia", "annie",
    "ashe", "azir", "brand", "caitlyn", "cassiopeia", "corki", "draven", "ezreal",
    "heimerdinger", "hwei", "jayce", "kaisa", "karthus", "katarina", "kayle",
    "kogmaw", "leblanc", "lucian", "lux", "malzahar", "morgana", "neeko", "orianna",
    "ryze", "samira", "senna", "seraphine", "smolder", "sona", "soraka", "syndra",
    "taliyah", "teemo", "tristana", "twitch", "varus", "veigar", "velkoz", "xerath",
    "zeri", "ziggs", "zilean"
  ];
  if (backlineKeywords.some((k) => key.includes(k))) {
    return false;
  }

  // 3. 装备防御词条判断
  const items = (unit.items ?? []).filter(Boolean);
  const hasTankItem = items.some((it) => {
    const s = String(it).toLowerCase();
    return (
      s.includes("gargoyle") ||
      s.includes("warmog") ||
      s.includes("bramble") ||
      s.includes("dragon") ||
      s.includes("sunfire") ||
      s.includes("redemption") ||
      s.includes("steadfast") ||
      s.includes("protector") ||
      s.includes("ionic") ||
      s.includes("crownguard")
    );
  });
  if (hasTankItem) return true;

  return unit.isCore || unit.priority === 1;
}

const isFallbackBoard = ref(false);

const displayUnits = computed(() => {
  const units = selectedDeck.value?.units;
  if (!units?.length) {
    isFallbackBoard.value = false;
    return [];
  }

  const hasValidCells = units.some(
    (u) => u.cell && typeof u.cell.x === "number" && typeof u.cell.y === "number"
  );

  if (hasValidCells) {
    isFallbackBoard.value = false;
    return units;
  }

  // 兜底：当 OP.GG 接口对个别阵容返回 cell: null 时，智能精准区分前排与后排站位
  isFallbackBoard.value = true;
  const result: TftMetaUnit[] = [];
  const occupied = new Set<string>();

  const findFreeCell = (preferredY: number[]): { x: number; y: number } => {
    for (const y of preferredY) {
      for (const x of [4, 3, 5, 2, 6, 1, 7]) {
        const key = `${x},${y}`;
        if (!occupied.has(key)) {
          occupied.add(key);
          return { x, y };
        }
      }
    }
    for (const y of [4, 3, 2, 1]) {
      for (const x of [1, 2, 3, 4, 5, 6, 7]) {
        const key = `${x},${y}`;
        if (!occupied.has(key)) {
          occupied.add(key);
          return { x, y };
        }
      }
    }
    return { x: 1, y: 1 };
  };

  const frontlineUnits: TftMetaUnit[] = [];
  const backlineUnits: TftMetaUnit[] = [];

  units.forEach((u) => {
    if (isFrontlineUnit(u)) {
      frontlineUnits.push(u);
    } else {
      backlineUnits.push(u);
    }
  });

  // 前排放置在 y=4, y=3
  frontlineUnits.forEach((u) => {
    const copy = { ...u };
    copy.cell = findFreeCell([4, 3, 2]);
    result.push(copy);
  });

  // 后排放置在 y=1, y=2
  backlineUnits.forEach((u) => {
    const copy = { ...u };
    copy.cell = findFreeCell([1, 2, 3]);
    result.push(copy);
  });

  return result;
});

function selectDeck(item: { deck: TftMetaDeck }) {
  selectedDeck.value = item.deck;
  showDetailModal.value = true;
}

const selectedDeckIndex = computed(() => {
  if (!selectedDeck.value || !sortedDecks.value.length) return -1;
  return sortedDecks.value.findIndex((item) => item.deck.id === selectedDeck.value?.id);
});

const hasPrevDeck = computed(() => selectedDeckIndex.value > 0);
const hasNextDeck = computed(
  () => selectedDeckIndex.value >= 0 && selectedDeckIndex.value < sortedDecks.value.length - 1
);

function prevDeck() {
  if (hasPrevDeck.value) {
    selectedDeck.value = sortedDecks.value[selectedDeckIndex.value - 1].deck;
  }
}

function nextDeck() {
  if (hasNextDeck.value) {
    selectedDeck.value = sortedDecks.value[selectedDeckIndex.value + 1].deck;
  }
}

function handleKeydown(e: KeyboardEvent) {
  if (!showDetailModal.value) return;
  if (e.key === "ArrowLeft") {
    prevDeck();
  } else if (e.key === "ArrowRight") {
    nextDeck();
  }
}

onMounted(() => {
  window.addEventListener("keydown", handleKeydown);
  loadDecks();
});

onUnmounted(() => {
  window.removeEventListener("keydown", handleKeydown);
});
</script>

<template>
  <div class="meta-comps-tab">
    <div class="header-row">
      <div class="header-left">
        <span class="header-desc">{{ t("tftPage.recommendDesc") }}</span>
        <span v-if="tftVersion" class="version-badge">🌐 数据版本: {{ tftVersion }}</span>
      </div>
      <n-button size="tiny" quaternary :loading="loading" @click="refresh">
        刷新
      </n-button>
    </div>

    <div v-if="loading && decks.length === 0" class="loading-box">
      <n-spin size="medium" />
    </div>

    <div v-else-if="error && decks.length === 0" class="error-box">
      <p>{{ error }}</p>
      <n-button size="small" type="primary" @click="refresh">重试</n-button>
    </div>

    <div v-else-if="decks.length === 0" class="empty-box">
      <n-empty :description="t('tftPage.noMetaDecks')" />
    </div>

    <template v-else>
      <div class="comp-grid">
        <div
          v-for="item in sortedDecks"
          :key="item.deck.id"
          :class="['comp-card', { selected: selectedDeck?.id === item.deck.id }]"
          @click="selectDeck(item)"
        >
          <div class="card-header">
            <div :class="['tier-badge', (item.tier || 'c').toLowerCase()]">
              {{ item.tier }}
            </div>
            <div class="deck-name">{{ getDeckDisplayName(item.deck) }}</div>
            <div v-if="item.deck.cost != null" class="cost-badge">
              {{ item.deck.cost }}费
            </div>
          </div>

          <div class="stats-row">
            <div class="stat" v-if="item.deck.stat?.win_rate != null">
              <span class="stat-val win">{{ formatPercent(item.deck.stat.win_rate) }}</span>
              <span class="stat-lbl">{{ t("tftPage.winRate") }}</span>
            </div>
            <div class="stat" v-if="item.deck.stat?.top4_rate != null">
              <span class="stat-val top4">{{ formatPercent(item.deck.stat.top4_rate) }}</span>
              <span class="stat-lbl">Top4</span>
            </div>
            <div class="stat" v-if="item.deck.stat?.avg_placement != null">
              <span class="stat-val place">{{ formatPlacement(item.deck.stat.avg_placement) }}</span>
              <span class="stat-lbl">{{ t("tftPage.avgPlace") }}</span>
            </div>
          </div>

          <div class="badges-row" v-if="getDeckBadges(item.deck).length">
            <span v-for="(badge, bIdx) in getDeckBadges(item.deck)" :key="bIdx" class="badge-tag">
              {{ badge }}
            </span>
          </div>

          <div class="traits-row" v-if="item.deck.traits?.length">
            <n-tag
              v-for="tr in item.deck.traits.slice(0, 5)"
              :key="tr.key"
              size="tiny"
              :bordered="false"
              class="trait-tag"
            >
              {{ getTraitDisplayName(tr.key) }}
            </n-tag>
            <span v-if="item.deck.traits.length > 5" class="more-tag"
              >+{{ item.deck.traits.length - 5 }}</span
            >
          </div>
        </div>
      </div>

    <!-- 阵容详情弹窗 -->
    <n-modal
      v-model:show="showDetailModal"
      preset="card"
      class="deck-detail-modal"
      :style="{ width: '1080px', maxWidth: '95vw' }"
      :auto-focus="false"
    >
      <template #header>
        <div class="modal-deck-header" v-if="selectedDeck">
          <span class="modal-deck-name">{{ getDeckDisplayName(selectedDeck) }}</span>
          <span v-if="selectedDeck.cost != null" class="cost-badge">
            {{ selectedDeck.cost }}费
          </span>
        </div>
      </template>

      <template #header-extra>
        <div class="modal-nav-extra">
          <div class="modal-nav-buttons" v-if="sortedDecks.length > 1">
            <n-button
              size="small"
              secondary
              :disabled="!hasPrevDeck"
              @click.stop="prevDeck"
              title="切换上一个阵容 (← 快捷键)"
            >
              ◀ 上一个
            </n-button>
            <span class="nav-counter">{{ selectedDeckIndex + 1 }} / {{ sortedDecks.length }}</span>
            <n-button
              size="small"
              secondary
              :disabled="!hasNextDeck"
              @click.stop="nextDeck"
              title="切换下一个阵容 (→ 快捷键)"
            >
              下一个 ▶
            </n-button>
          </div>

          <n-button
            v-if="selectedDeck?.teamCode"
            size="small"
            type="primary"
            class="copy-code-btn"
            @click.stop="copyTeamCode(selectedDeck.teamCode)"
          >
            📋 复制阵容代码
          </n-button>
        </div>
      </template>

      <div v-if="selectedDeck" class="detail-panel-body">
        <div class="board-traits-layout">
          <!-- 左侧：4x7 棋盘 -->
          <div class="tft-mini-board">
            <div v-for="y in [4, 3, 2, 1]" :key="'row-' + y" class="board-row">
              <div v-for="x in 7" :key="'cell-' + x + '-' + y" class="board-cell">
                <template v-if="getBoardUnit(displayUnits, x, y)">
                  <div class="board-unit-wrap">
                    <div
                      :class="['board-unit-box', { 'is-core': getBoardUnit(displayUnits, x, y)?.isCore }]"
                      :title="getChampionDisplayName(getBoardUnit(displayUnits, x, y)?.characterId)"
                    >
                      <img
                        v-if="getBoardUnit(displayUnits, x, y)?.iconUrl"
                        :src="getBoardUnit(displayUnits, x, y)?.iconUrl"
                        class="board-unit-img"
                        loading="lazy"
                      />
                      <span v-else class="board-unit-text">
                        {{ getChampionDisplayName(getBoardUnit(displayUnits, x, y)?.characterId).slice(0, 2) }}
                      </span>
                      <!-- 星级标记 -->
                      <span v-if="getBoardUnit(displayUnits, x, y)?.tier === 3" class="board-stars gold">★★★</span>
                      <span v-else-if="getBoardUnit(displayUnits, x, y)?.tier === 2" class="board-stars silver">★★</span>
                      <!-- 核心标记 -->
                      <span v-if="getBoardUnit(displayUnits, x, y)?.isCore" class="board-core-badge">C</span>
                      <!-- 优先级标记 -->
                      <span v-if="getBoardUnit(displayUnits, x, y)?.priority" class="board-priority-badge">P{{ getBoardUnit(displayUnits, x, y)?.priority }}</span>
                    </div>
                    <!-- 装备图标 -->
                    <div v-if="getBoardUnit(displayUnits, x, y)?.items?.filter(Boolean).length" class="board-unit-items">
                      <div
                        v-for="(item, iIdx) in getBoardUnit(displayUnits, x, y)!.items.filter(Boolean)"
                        :key="iIdx"
                        class="board-item-icon"
                        :title="getItemDisplayName(item)"
                      >
                        <img
                          v-if="getItemIconUrl(item)"
                          :src="getItemIconUrl(item)"
                          class="board-item-img"
                          loading="lazy"
                        />
                      </div>
                    </div>
                  </div>
                </template>
              </div>
            </div>
            <div class="board-legend-tip">
              <span v-if="isFallbackBoard">⚠️ 提示: OP.GG 未提供此阵容的具体坐标，已自动智能微调排版 | </span>
              💡 C: 阵容核心 | P1/P2/P3: 装备分配优先级 (P1优先度最高)
            </div>
          </div>

          <!-- 右侧：激活羁绊 + 前中期过渡 -->
          <div class="board-right-side">
            <!-- 激活羁绊 -->
            <div class="right-block" v-if="selectedDeck.traits?.length">
              <h4>{{ t("tftPage.traits") }}</h4>
              <div class="traits-side-list">
                <span
                  v-for="tr in selectedDeck.traits"
                  :key="tr.key"
                  class="trait-side-item"
                >
                  {{ getTraitDisplayName(tr.key) }}
                  <span class="trait-count">({{ tr.numUnits }})</span>
                </span>
              </div>
            </div>

            <!-- 前期过渡 -->
            <div class="right-block" v-if="selectedDeck.early">
              <h4>{{ t("tftPage.earlyGame") }} ({{ t("tftPage.earlyLevel") }}: {{ selectedDeck.early.level }})</h4>
              <div class="mini-units" v-if="selectedDeck.early.units?.length">
                <span
                  v-for="(u, idx) in selectedDeck.early.units"
                  :key="u.characterId ?? 'early-' + idx"
                  class="mini-unit"
                >
                  {{ getChampionDisplayName(u.characterId) }}
                </span>
              </div>
            </div>

            <!-- 中期过渡 -->
            <div class="right-block" v-if="selectedDeck.middle">
              <h4>{{ t("tftPage.midGame") }} ({{ t("tftPage.earlyLevel") }}: {{ selectedDeck.middle.level }})</h4>
              <div class="mini-units" v-if="selectedDeck.middle.units?.length">
                <span
                  v-for="(u, idx) in selectedDeck.middle.units"
                  :key="u.characterId ?? 'mid-' + idx"
                  class="mini-unit"
                >
                  {{ getChampionDisplayName(u.characterId) }}
                </span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </n-modal>
    </template>
  </div>
</template>

<style scoped>
.header-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 14px;
}
.header-left {
  display: flex;
  align-items: center;
  gap: 10px;
}
.header-desc {
  font-size: 0.82rem;
  color: var(--text-muted);
}
.client-ver-badge {
  font-size: 0.72rem;
  font-weight: 700;
  color: #3b82f6;
  background: rgba(59, 130, 246, 0.15);
  border: 1px solid rgba(59, 130, 246, 0.3);
  padding: 2px 10px;
  border-radius: 14px;
  backdrop-filter: blur(4px);
}
.client-ver-badge.offline {
  color: #9ca3af;
  background: rgba(255, 255, 255, 0.05);
  border-color: rgba(255, 255, 255, 0.12);
}
.version-badge {
  font-size: 0.72rem;
  font-weight: 700;
  color: var(--primary-color);
  background: var(--primary-color-alpha-15);
  border: 1px solid var(--primary-color-alpha-25);
  padding: 2px 10px;
  border-radius: 14px;
  backdrop-filter: blur(4px);
  box-shadow: 0 2px 8px var(--primary-color-alpha-10);
}

.loading-box,
.error-box,
.empty-box {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 40px 12px;
  gap: 12px;
}

.comp-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 14px;
}

.comp-card {
  padding: 14px 16px;
  border-radius: var(--radius-md);
  background: rgba(255, 255, 255, 0.03);
  cursor: pointer;
  transition: all 0.28s cubic-bezier(0.25, 0.8, 0.25, 1);
  border: 1px solid rgba(255, 255, 255, 0.08);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  box-shadow: var(--shadow-sm);
  position: relative;
  overflow: hidden;
}
.comp-card:hover {
  background: rgba(255, 255, 255, 0.06);
  border-color: rgba(255, 255, 255, 0.2);
  transform: translateY(-2px);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25), 0 0 0 1px var(--primary-color-alpha-30);
}
.comp-card.selected {
  border-color: var(--primary-color);
  background: linear-gradient(135deg, var(--primary-color-alpha-15) 0%, rgba(139, 92, 246, 0.08) 100%);
  box-shadow: 0 0 24px var(--primary-color-alpha-25), 0 4px 16px rgba(0, 0, 0, 0.3);
}

.card-header {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 10px;
}
.cost-badge {
  margin-left: auto;
  font-size: 0.72rem;
  font-weight: 800;
  color: #f59e0b;
  background: rgba(245, 158, 11, 0.14);
  border: 1px solid rgba(245, 158, 11, 0.3);
  padding: 2px 8px;
  border-radius: 12px;
  white-space: nowrap;
  flex-shrink: 0;
  box-shadow: 0 2px 6px rgba(245, 158, 11, 0.15);
}

.tier-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: 7px;
  font-weight: 800;
  font-size: 0.85rem;
  color: #fff;
  flex-shrink: 0;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.4);
}
.tier-badge.s {
  background: linear-gradient(135deg, #ef4444 0%, #dc2626 100%);
  box-shadow: 0 3px 10px rgba(239, 68, 68, 0.45);
}
.tier-badge.a {
  background: linear-gradient(135deg, #f59e0b 0%, #d97706 100%);
  box-shadow: 0 3px 10px rgba(245, 158, 11, 0.35);
}
.tier-badge.b {
  background: linear-gradient(135deg, #3b82f6 0%, #2563eb 100%);
  box-shadow: 0 3px 10px rgba(59, 130, 246, 0.35);
}
.tier-badge.c {
  background: linear-gradient(135deg, #8b5cf6 0%, #7c3aed 100%);
  box-shadow: 0 3px 10px rgba(139, 92, 246, 0.35);
}
.tier-badge.d {
  background: linear-gradient(135deg, #6b7280 0%, #4b5563 100%);
}

.deck-name {
  font-weight: 700;
  color: var(--text-color);
  font-size: 0.95rem;
  line-height: 1.3;
}

.stats-row {
  display: flex;
  gap: 8px;
  margin-bottom: 10px;
}
.stat {
  display: flex;
  flex-direction: column;
  align-items: center;
  flex: 1;
  background: rgba(0, 0, 0, 0.2);
  padding: 4px 6px;
  border-radius: 6px;
  border: 1px solid rgba(255, 255, 255, 0.05);
}
.stat-val {
  font-weight: 800;
  font-size: 0.85rem;
  color: var(--text-color);
}
.stat-val.win { color: #10b981; }
.stat-val.top4 { color: #3b82f6; }
.stat-val.place { color: #f59e0b; }
.stat-lbl {
  font-size: 0.63rem;
  color: var(--text-muted);
  margin-top: 1px;
}

.badges-row {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-bottom: 8px;
}
.badge-tag {
  font-size: 0.68rem;
  font-weight: 700;
  padding: 2px 8px;
  border-radius: 12px;
  background: var(--primary-color-alpha-15);
  color: var(--primary-color);
  border: 1px solid var(--primary-color-alpha-25);
}

.traits-row {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.trait-tag {
  font-size: 0.7rem;
}
.more-tag {
  font-size: 0.7rem;
  color: var(--text-muted);
  padding: 1px 6px;
}

/* 棋盘 + 羁绊并排布局 */
.board-traits-layout {
  display: flex;
  gap: 18px;
  align-items: flex-start;
  flex-wrap: wrap;
}

.tft-mini-board {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 14px 16px;
  background: rgba(10, 14, 26, 0.6);
  border-radius: 12px;
  border: 1px solid rgba(255, 255, 255, 0.12);
  box-shadow: inset 0 2px 12px rgba(0, 0, 0, 0.5);
  flex-shrink: 0;
}
.board-legend-tip {
  font-size: 0.72rem;
  font-weight: 500;
  color: rgba(226, 232, 240, 0.65);
  text-align: center;
  margin-top: 10px;
  padding: 5px 10px;
  background: rgba(0, 0, 0, 0.25);
  border-radius: 6px;
  border: 1px solid rgba(255, 255, 255, 0.05);
  letter-spacing: 0.2px;
}
.board-row {
  display: flex;
  gap: 4px;
  justify-content: center;
}
.board-cell {
  width: 80px;
  min-height: 84px;
  background: rgba(255, 255, 255, 0.02);
  border: 1px dashed rgba(255, 255, 255, 0.08);
  border-radius: 8px;
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding-top: 6px;
}
.board-unit-wrap {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 3px;
}
.board-unit-box {
  width: 58px;
  height: 58px;
  border-radius: 8px;
  overflow: hidden;
  border: 2px solid var(--border-color);
  box-shadow: 0 3px 8px rgba(0, 0, 0, 0.35);
  background: var(--card-bg);
  position: relative;
  flex-shrink: 0;
  transition: all 0.25s cubic-bezier(0.34, 1.56, 0.64, 1);
}
.board-unit-box:hover {
  transform: scale(1.08);
  z-index: 2;
}
.board-unit-box.is-core {
  border-color: #f59e0b;
  box-shadow: 0 0 16px rgba(245, 158, 11, 0.7);
}
.board-unit-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.board-unit-text {
  font-size: 0.9rem;
  font-weight: 700;
  color: var(--text-color);
  text-align: center;
  line-height: 58px;
  display: block;
}

/* 棋盘内星级标记 */
.board-stars {
  position: absolute;
  bottom: 0px;
  left: 50%;
  transform: translateX(-50%);
  font-size: 0.68rem;
  line-height: 1;
  text-shadow: 0 1px 3px rgba(0, 0, 0, 0.9);
  white-space: nowrap;
}
.board-stars.gold { color: #f59e0b; }
.board-stars.silver { color: #9ca3af; }

/* 棋盘内核心标记 */
.board-core-badge {
  position: absolute;
  top: -1px;
  right: -1px;
  font-size: 0.6rem;
  font-weight: 800;
  background: #f59e0b;
  color: #000;
  width: 16px;
  height: 16px;
  line-height: 16px;
  text-align: center;
  border-radius: 3px;
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.4);
}

/* 棋盘内优先级标记 */
.board-priority-badge {
  position: absolute;
  top: -1px;
  left: -1px;
  font-size: 0.55rem;
  font-weight: 800;
  background: var(--primary-color);
  color: #fff;
  padding: 1px 3px;
  border-radius: 3px;
  line-height: 14px;
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.4);
}

/* 棋盘内装备图标 */
.board-unit-items {
  display: flex;
  gap: 3px;
  justify-content: center;
}
.board-item-icon {
  width: 22px;
  height: 22px;
  border-radius: 4px;
  overflow: hidden;
  border: 1px solid rgba(255, 255, 255, 0.2);
  background: rgba(0, 0, 0, 0.4);
  transition: all 0.2s ease;
}
.board-item-icon:hover {
  transform: scale(1.25);
  border-color: var(--primary-color);
  z-index: 3;
  box-shadow: 0 0 8px var(--primary-color);
}
.board-item-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

/* 右侧面板 (羁绊 + 前中期过渡) */
.board-right-side {
  display: flex;
  flex-direction: column;
  gap: 12px;
  flex: 1;
  min-width: 200px;
  max-width: 300px;
}
.right-block {
  background: rgba(255, 255, 255, 0.03);
  border-radius: var(--radius-sm);
  padding: 10px 12px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  backdrop-filter: blur(8px);
}
.right-block h4 {
  margin: 0 0 8px;
  font-size: 0.8rem;
  font-weight: 800;
  color: var(--primary-color);
  letter-spacing: 0.5px;
}
.traits-side-list {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
}
.trait-side-item {
  font-size: 0.72rem;
  background: rgba(255, 255, 255, 0.05);
  padding: 3px 8px;
  border-radius: 4px;
  color: var(--text-color);
  line-height: 1.4;
  border: 1px solid rgba(255, 255, 255, 0.1);
  font-weight: 600;
}
.trait-count {
  color: var(--text-muted);
  margin-left: 2px;
  font-size: 0.68rem;
}

.mini-units {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
}
.mini-unit {
  font-size: 0.72rem;
  background: var(--primary-color-alpha-15);
  color: var(--primary-color);
  padding: 3px 8px;
  border-radius: 4px;
  border: 1px solid var(--primary-color-alpha-30);
  font-weight: 600;
}

/* 阵容详情弹窗 header */
.modal-deck-header {
  display: flex;
  align-items: center;
  gap: 10px;
}
.modal-deck-name {
  font-size: 1.25rem;
  font-weight: 800;
  color: #ffffff;
  text-shadow: 0 1px 3px rgba(0, 0, 0, 0.6);
  letter-spacing: 0.3px;
}
.copy-code-btn {
  font-weight: 700 !important;
  box-shadow: 0 2px 10px var(--primary-color-alpha-30);
}

/* 弹窗顶部切换导航 */
.modal-nav-extra {
  display: flex;
  align-items: center;
  gap: 16px;
}
.modal-nav-buttons {
  display: flex;
  align-items: center;
  gap: 8px;
}
.nav-counter {
  font-size: 0.8rem;
  font-weight: 700;
  color: var(--text-muted);
  padding: 0 4px;
}

</style>
