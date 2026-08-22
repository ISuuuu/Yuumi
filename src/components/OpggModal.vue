<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from "vue";
import { useI18n } from "vue-i18n";
import { invoke } from "@tauri-apps/api/core";
import { lcuRequest, fetchConfig, cleanError } from "../api/lcu";
import type { GameDataAssets } from "../types/lcu";
import type {
  OpggTierPayload,
  TierListItem,
  OpggBuildPayload,
  OpggBuildData,
  OpggRunePreset,
  PerkStyle,
  Perk,
} from "../types/opgg";
import opggIcon from "../assets/opgg.svg";
import OpggFilterBar from "./opgg/OpggFilterBar.vue";
import TierListPanel from "./opgg/TierListPanel.vue";
import BuildDetailPanel from "./opgg/BuildDetailPanel.vue";
import { useToast } from "../composables/useToast";

const { locale } = useI18n();

// ─── LCU 英雄摘要数据结构 ───
interface ChampionSummary {
  id: number;
  name: string;
  alias?: string;
}

// 筛选状态
const region = ref("kr");
const mode = ref("ranked");
const tier = ref("emerald_plus");
const position = ref("MID");
const view = ref<"tier" | "build">("tier");

// 数据
const tierData = ref<TierListItem[]>([]);
const buildData = ref<OpggBuildData | null>(null);
const loading = ref(false);
const error = ref("");
const selectedChampId = ref<number | null>(null);

// LCU 静态资源与符文配置数据
const gameDataAssets = ref<GameDataAssets | null>(null);
const perkStyles = ref<PerkStyle[]>([]);
const perksMap = ref<Map<number, Perk>>(new Map());
const championsMap = ref<Map<number, string>>(new Map());
const opggVersion = ref<string>("");

const { showToast } = useToast();

// 主题监听响应
const dataTheme = ref("light");
let themeObserver: MutationObserver | null = null;

onMounted(async () => {
  // 初始化获取主题
  const savedTheme = localStorage.getItem("yuumi_theme");
  const root = document.documentElement;
  if (savedTheme === "Dark") {
    root.setAttribute("data-theme", "dark");
    dataTheme.value = "dark";
  } else if (savedTheme === "Light") {
    root.setAttribute("data-theme", "light");
    dataTheme.value = "light";
  } else {
    // 默认为 Auto 或无设置，跟随系统
    const isSystemDark = window.matchMedia(
      "(prefers-color-scheme: dark)",
    ).matches;
    root.setAttribute("data-theme", isSystemDark ? "dark" : "light");
    dataTheme.value = isSystemDark ? "dark" : "light";
  }

  // 监听 DOM 树的主题属性变化
  themeObserver = new MutationObserver((mutations) => {
    mutations.forEach((mutation) => {
      if (
        mutation.type === "attributes" &&
        mutation.attributeName === "data-theme"
      ) {
        const currentAttr = document.documentElement.getAttribute("data-theme");
        dataTheme.value = currentAttr === "dark" ? "dark" : "light";
      }
    });
  });
  themeObserver.observe(document.documentElement, { attributes: true });

  try {
    gameDataAssets.value = await invoke("get_game_data_assets");
  } catch (e) {
    console.error("加载 LCU 预加载静态资源映射失败:", e);
  }
  await loadPerkData();
  await loadChampionSummary();

  let targetChampionId: number | null = null;
  try {
    const config = await fetchConfig();
    const funcs = config.Functions;
    if (funcs && funcs.EnableAutoSelectChampion) {
      const list = [
        funcs.AutoSelectChampion,
        funcs.AutoSelectChampionMid,
        funcs.AutoSelectChampionTop,
        funcs.AutoSelectChampionSup,
        funcs.AutoSelectChampionJug,
        funcs.AutoSelectChampionBot,
      ];
      for (const arr of list) {
        if (arr && arr.length > 0 && arr[0] > 0) {
          targetChampionId = arr[0];
          break;
        }
      }
    }
  } catch (e) {
    console.error("获取应用配置失败:", e);
  }

  if (targetChampionId) {
    fetchBuild(targetChampionId);
  } else {
    fetchTierList();
  }
});

onUnmounted(() => {
  if (themeObserver) {
    themeObserver.disconnect();
  }
});

watch([region, mode, tier, position], () => {
  if (view.value === "tier") {
    fetchTierList();
  } else if (view.value === "build" && selectedChampId.value !== null) {
    fetchBuild(selectedChampId.value);
  }
});

async function loadPerkData() {
  try {
    const stylesResp = await lcuRequest<PerkStyle[] | { styles?: PerkStyle[] }>(
      "GET",
      "/lol-game-data/assets/v1/perkstyles.json",
    );
    if (stylesResp.success && stylesResp.data) {
      const raw = stylesResp.data;
      perkStyles.value = Array.isArray(raw) ? raw : raw.styles || [];
    }
  } catch (e) {
    console.error("获取 perkstyles.json 失败:", e);
  }

  try {
    const perksResp = await lcuRequest<Perk[]>(
      "GET",
      "/lol-game-data/assets/v1/perks.json",
    );
    if (perksResp.success && perksResp.data) {
      const arr = perksResp.data;
      const map = new Map<number, Perk>();
      for (const p of arr) {
        map.set(p.id, p);
      }
      perksMap.value = map;
    }
  } catch (e) {
    console.error("获取 perks.json 失败:", e);
  }
}

async function loadChampionSummary() {
  try {
    const resp = await lcuRequest<ChampionSummary[]>(
      "GET",
      "/lol-game-data/assets/v1/champion-summary.json",
    );
    if (resp.success && resp.data) {
      const map = new Map<number, string>();
      const isEnglish = locale.value === "en_US";
      for (const c of resp.data) {
        map.set(c.id, isEnglish && c.alias ? c.alias : c.name);
      }
      championsMap.value = map;
    }
  } catch (e) {
    console.error("加载 LCU 英雄摘要映射失败:", e);
  }
}

async function fetchTierList() {
  loading.value = true;
  error.value = "";
  try {
    const data = await invoke<OpggTierPayload>("fetch_opgg_data", {
      region: region.value,
      mode: mode.value,
      tier: tier.value,
    });
    if (data?.data) {
      opggVersion.value = data?.meta?.version || "";
      const list = data.data;
      if (mode.value === "ranked") {
        const pos = position.value;
        tierData.value = list
          .filter((c) => c.positions?.some((p) => p.name === pos))
          .map((c): TierListItem => {
            const p = c.positions?.find((p) => p.name === pos);
            return {
              id: c.id,
              name: c.name,
              ...p?.stats,
              tier: p?.stats?.tier_data?.tier,
              rank: p?.stats?.tier_data?.rank,
              position: pos,
              counters: (p?.counters || []).map((ct) => ct.champion_id),
            };
          })
          .sort((a, b) => (a.rank || 999) - (b.rank || 999));
      } else {
        tierData.value = list
          .filter((c) => c.average_stats?.rank != null)
          .map((c): TierListItem => ({
            id: c.id,
            name: c.name,
            ...c.average_stats,
            tier: c.average_stats?.tier,
            rank: c.average_stats?.rank,
          }))
          .sort((a, b) => (a.rank || 999) - (b.rank || 999));
      }
    }
  } catch (e: unknown) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

async function fetchBuild(championId: number) {
  loading.value = true;
  error.value = "";
  selectedChampId.value = championId;
  view.value = "build";
  try {
    const data = await invoke<OpggBuildPayload>("fetch_opgg_data", {
      region: region.value,
      mode: mode.value,
      tier: tier.value,
      championId,
      position: mode.value === "ranked" ? position.value : undefined,
    });
    buildData.value = data?.data || null;
    opggVersion.value = data?.meta?.version || "";
  } catch (e: unknown) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

// 出装页计算属性
const selectedPosition = computed(() =>
  mode.value === "ranked" ? position.value : null,
);

// 一键设置符文页
async function setRunePage(rune: OpggRunePreset) {
  if (!rune) return;
  try {
    const perkIds = [
      ...(rune.primary_rune_ids || []),
      ...(rune.secondary_rune_ids || []),
      ...(rune.stat_mod_ids || []),
    ];
    const name = `Yuumi: ${buildData.value?.summary?.name || "Rune"}`;
    await invoke("apply_rune_page", {
      params: {
        name,
        primary_style_id: rune.primary_page_id,
        sub_style_id: rune.secondary_page_id,
        selected_perk_ids: perkIds,
      },
    });
    showToast("符文页应用成功：" + name, "success");
  } catch (e: unknown) {
    showToast("应用符文页失败: " + cleanError(e), "error");
  }
}
</script>

<template>
  <div class="opgg-panel">
    <!-- 头部 -->
    <div class="opgg-header">
      <div class="opgg-header-left">
        <img :src="opggIcon" class="opgg-icon" />
        <span class="opgg-logo">OP.GG</span>
        <button
          :class="['tab-btn', { active: view === 'tier' }]"
          @click="
            view = 'tier';
            fetchTierList();
          "
        >
          {{ $t("opgg.tabs.tier") }}
        </button>
        <button
          :class="['tab-btn', { active: view === 'build' }]"
          :disabled="!selectedChampId"
          @click="
            view = 'build';
            selectedChampId && fetchBuild(selectedChampId);
          "
        >
          {{ $t("opgg.tabs.build") }}
        </button>
      </div>
      <div class="opgg-header-right">
        <span v-if="opggVersion" class="opgg-version"
          >{{ $t("opgg.gameVersion") }}{{ opggVersion }}</span
        >
      </div>
    </div>

    <!-- 筛选栏 -->
    <OpggFilterBar
      v-model:region="region"
      v-model:mode="mode"
      v-model:tier="tier"
      v-model:position="position"
    />

    <!-- 加载 / 错误 -->
    <div v-if="loading" class="opgg-center"><div class="spinner"></div></div>
    <div v-else-if="error" class="opgg-center error-text">{{ error }}</div>

    <!-- 梯队列表 -->
    <div v-else-if="view === 'tier'" class="opgg-body tier-list-body">
      <TierListPanel
        :items="tierData"
        :champions-map="championsMap"
        :data-theme="dataTheme"
        @select="fetchBuild"
      />
    </div>

    <!-- 出装详情 -->
    <div
      v-else-if="view === 'build' && buildData"
      class="opgg-body build-view-bg"
    >
      <BuildDetailPanel
        :build="buildData"
        :champion-id="selectedChampId"
        :position="selectedPosition"
        :champions-map="championsMap"
        :game-data-assets="gameDataAssets"
        :perk-styles="perkStyles"
        :perks-map="perksMap"
        :data-theme="dataTheme"
        @apply-rune="setRunePage"
      />
    </div>
  </div>
</template>

<style scoped>
.opgg-panel {
  width: 100%;
  height: 100vh;
  background: var(--bg-color);
  color: var(--text-color);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

@keyframes slideIn {
  from {
    transform: translateX(24px);
    opacity: 0;
  }
  to {
    transform: translateX(0);
    opacity: 1;
  }
}

.opgg-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 16px;
  border-bottom: 1px solid var(--border-color);
  flex-shrink: 0;
}

.opgg-header-left {
  display: flex;
  align-items: center;
  gap: 8px;
}

.opgg-header-right {
  display: flex;
  align-items: center;
  gap: 12px;
}

.opgg-version {
  font-size: 0.72rem;
  color: var(--text-dimmed);
}

.opgg-icon {
  width: 22px;
  height: 22px;
  border-radius: 4px;
}

.opgg-logo {
  font-weight: 900;
  font-size: 0.95rem;
  color: #636ff9;
  letter-spacing: -0.3px;
}

.tab-btn {
  border: 1px solid var(--border-color);
  background: var(--card-bg);
  color: var(--text-muted);
  padding: 3px 12px;
  border-radius: 6px;
  font-size: 0.78rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.15s;
}
.tab-btn:hover:not(:disabled) {
  background: var(--card-bg-hover);
  color: var(--text-color);
}
.tab-btn.active {
  background: var(--primary-color);
  color: white;
  border-color: var(--primary-color);
}
.tab-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.opgg-body {
  flex: 1;
  overflow-y: auto;
  padding: 0;
}

.opgg-body::-webkit-scrollbar {
  width: 5px;
}
.opgg-body::-webkit-scrollbar-thumb {
  background: rgba(0, 0, 0, 0.1);
  border-radius: 3px;
}

.opgg-center {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-dimmed);
  font-size: 0.85rem;
}
.error-text {
  color: var(--loss-color);
}

.spinner {
  width: 28px;
  height: 28px;
  border: 3px solid rgba(0, 0, 0, 0.05);
  border-top-color: var(--primary-color);
  border-radius: 50%;
  animation: spin 0.7s linear infinite;
}
@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

/* 梯队卡片列表 */
.tier-list-body {
  padding: 8px 10px;
}

/* ── 出装详情页 ── */

/* 通用卡片美化 */
.build-view-bg {
  background: #f1f3f7; /* 软灰背景，让白色卡片更具层次感 */
  transition: background 0.3s;
}
[data-theme="dark"] .build-view-bg {
  background: #0b0f19; /* 暗色模式深空黑背景 */
}
</style>

<!-- 非 scoped：CSS 变量需作用于 :root / html 元素，scoped 会加哈希导致选择器失效 -->
<style>
:root {
  --primary-color: #00d2c4;
  --primary-color-hover: #00b3a7;
  --primary-color-alpha-15: rgba(0, 210, 196, 0.15);
  --primary-color-alpha-30: rgba(0, 210, 196, 0.3);
  --primary-color-alpha-40: rgba(0, 210, 196, 0.4);

  --bg-color: #f8fafc;
  --card-bg: rgba(255, 255, 255, 0.7);
  --card-bg-hover: rgba(255, 255, 255, 0.9);
  --border-color: rgba(0, 0, 0, 0.05);
  --hover-bg: rgba(0, 0, 0, 0.03);
  --hover-bg-strong: rgba(0, 0, 0, 0.06);

  --text-color: #0f172a;
  --text-muted: #475569;
  --text-dimmed: #64748b;

  --win-color: #10b981;
  --loss-color: #f43f5e;
  --accent-color: #f59e0b;
}

[data-theme="dark"] {
  --bg-color: #0b0f19;
  --card-bg: rgba(30, 41, 59, 0.55);
  --card-bg-hover: rgba(30, 41, 59, 0.75);
  --border-color: rgba(255, 255, 255, 0.06);
  --hover-bg: rgba(255, 255, 255, 0.04);
  --hover-bg-strong: rgba(255, 255, 255, 0.08);

  --text-color: #f8fafc;
  --text-muted: #94a3b8;
  --text-dimmed: #64748b;

  --win-color: #34d399;
  --loss-color: #fb7185;
}
</style>
