<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { lcuRequest, fetchConfig } from "../api/lcu";
import LcuImage from "./LcuImage.vue";
import opggIcon from "../assets/opgg.svg";
import tierIcon1 from "../assets/tier/tier-1.svg";
import tierIcon2 from "../assets/tier/tier-2.svg";
import tierIcon3 from "../assets/tier/tier-3.svg";
import tierIcon4 from "../assets/tier/tier-4.svg";

// 筛选状态
const region = ref("kr");
const mode = ref("ranked");
const tier = ref("emerald_plus");
const position = ref("MID");
const view = ref<"tier" | "build">("tier");

// 自定义下拉状态
const showRegionDropdown = ref(false);
const showModeDropdown = ref(false);
const showTierDropdown = ref(false);
const showPositionDropdown = ref(false);
function closeAllDropdowns() {
  showRegionDropdown.value = false;
  showModeDropdown.value = false;
  showTierDropdown.value = false;
  showPositionDropdown.value = false;
}

const REGIONS = [
  { value: "kr", label: "韩服" },
  { value: "global", label: "全球" },
];

// 数据
const tierData = ref<any[]>([]);
const buildData = ref<any | null>(null);
const loading = ref(false);
const error = ref("");
const selectedChampId = ref<number | null>(null);
const selectedRuneIdx = ref(0);

// LCU 静态资源与符文配置数据
const gameDataAssets = ref<any>(null);
const perkStyles = ref<any[]>([]);
const perksMap = ref<Map<number, any>>(new Map());
const championsMap = ref<Map<number, string>>(new Map());
const opggVersion = ref<string>("");

// Toast 通知状态
const toast = ref<{ message: string; type: 'success' | 'error'; visible: boolean }>({
  message: '', type: 'success', visible: false
});
let toastTimer: ReturnType<typeof setTimeout> | null = null;
function showToast(message: string, type: 'success' | 'error' = 'success') {
  if (toastTimer) clearTimeout(toastTimer);
  toast.value = { message, type, visible: true };
  toastTimer = setTimeout(() => { toast.value.visible = false; }, 2500);
}

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

const MODES = [
  { value: "ranked", label: "召唤师峡谷 (排位)" },
  { value: "normal", label: "召唤师峡谷 (匹配)" },
  { value: "aram", label: "极地大乱斗" },
  { value: "arena", label: "斗魂竞技场" },
];

const TIERS = [
  { value: "all", label: "全部" },
  { value: "gold_plus", label: "黄金+" },
  { value: "platinum_plus", label: "铂金+" },
  { value: "emerald_plus", label: "翡翠+" },
  { value: "diamond_plus", label: "钻石+" },
  { value: "master", label: "大师" },
  { value: "master_plus", label: "大师+" },
  { value: "grandmaster", label: "宗师" },
  { value: "challenger", label: "王者" },
];

const POSITIONS = [
  { value: "TOP", label: "上单" },
  { value: "JUNGLE", label: "打野" },
  { value: "MID", label: "中单" },
  { value: "ADC", label: "ADC" },
  { value: "SUPPORT", label: "辅助" },
];

// 梯队徽章颜色（深色文字，用于 Tier 标签）
const TIER_COLORS: Record<string, string> = {
  "0": "#5B8DB8",
  "1": "#5B8DB8", // T1 蓝
  "2": "#5BA8A3", // T2 青
  "3": "#B89B52", // T3 金
  "4": "#7D8185", // T4 灰
  "5": "#7D8185",
  "":  "#7D8185",
};

// 梯队卡片行背景色
const TIER_CARD_BG: Record<string, string> = {
  "0": "#CDE5F8",
  "1": "#CDE5F8", // T1 蓝
  "2": "#CDECEA", // T2 青
  "3": "#F4EAD1", // T3 暖
  "4": "#E5E8EC", // T4 灰
  "5": "#E5E8EC",
  "":  "transparent",
};

// 出装标题栏复用同样配色
const TIER_BG_COLORS: Record<string, string> = {
  "0": "#CDE5F8",
  "1": "#CDE5F8",
  "2": "#CDECEA",
  "3": "#F4EAD1",
  "4": "#E5E8EC",
  "5": "#E5E8EC",
  "":  "#f8f9fa",
};

const TIER_BORDER_COLORS: Record<string, string> = {
  "0": "rgba(0, 0, 0, 0.095)",
  "1": "rgba(0, 0, 0, 0.095)",
  "2": "rgba(0, 0, 0, 0.095)",
  "3": "rgba(0, 0, 0, 0.095)",
  "4": "rgba(0, 0, 0, 0.095)",
  "5": "rgba(0, 0, 0, 0.095)",
  "":  "rgba(0, 0, 0, 0.095)",
};

const TIER_CARD_BORDER: Record<string, string> = {
  "0": "rgba(0, 0, 0, 0.095)",
  "1": "rgba(0, 0, 0, 0.095)",
  "2": "rgba(0, 0, 0, 0.095)",
  "3": "rgba(0, 0, 0, 0.095)",
  "4": "rgba(0, 0, 0, 0.095)",
  "5": "rgba(0, 0, 0, 0.095)",
  "":  "rgba(0, 0, 0, 0.095)",
};

// Tier 等级图标（盾牌 + 数字）
const TIER_ICONS: Record<string, string> = {
  "0": tierIcon1,
  "1": tierIcon1,
  "2": tierIcon2,
  "3": tierIcon3,
  "4": tierIcon4,
  "5": tierIcon4,
};

onMounted(async () => {
  document.addEventListener("click", closeAllDropdowns);
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
        funcs.AutoSelectChampionBot
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
  document.removeEventListener("click", closeAllDropdowns);
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
    const stylesResp = await lcuRequest<any>("GET", "/lol-game-data/assets/v1/perkstyles.json");
    if (stylesResp.success && stylesResp.data) {
      const raw = stylesResp.data;
      perkStyles.value = Array.isArray(raw) ? raw : (raw.styles || []);
    }
  } catch (e) {
    console.error("获取 perkstyles.json 失败:", e);
  }

  try {
    const perksResp = await lcuRequest<any[]>("GET", "/lol-game-data/assets/v1/perks.json");
    if (perksResp.success && perksResp.data) {
      const arr = perksResp.data;
      const map = new Map<number, any>();
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
    const resp = await lcuRequest<any[]>("GET", "/lol-game-data/assets/v1/champion-summary.json");
    if (resp.success && resp.data) {
      const map = new Map<number, string>();
      for (const c of resp.data) {
        map.set(c.id, c.name);
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
    const data = await invoke<any>("fetch_opgg_data", {
      region: region.value,
      mode: mode.value,
      tier: tier.value,
    });
    if (data?.data) {
      opggVersion.value = data?.meta?.version || "";
      if (mode.value === "ranked") {
        const pos = position.value;
        tierData.value = (data.data as any[])
          .filter((c: any) => c.positions?.some((p: any) => p.name === pos))
          .map((c: any) => {
            const p = c.positions.find((p: any) => p.name === pos);
            return {
              id: c.id, name: c.name, ...p?.stats,
              tier: p?.stats?.tier_data?.tier,
              rank: p?.stats?.tier_data?.rank,
              position: pos,
              counters: (p?.counters || []).map((ct: any) => ct.champion_id),
            };
          })
          .sort((a: any, b: any) => (a.rank || 999) - (b.rank || 999));
      } else {
        tierData.value = (data.data as any[])
          .filter((c: any) => c.average_stats?.rank != null)
          .map((c: any) => ({
            id: c.id, name: c.name,
            ...c.average_stats,
            tier: c.average_stats?.tier,
            rank: c.average_stats?.rank,
          }))
          .sort((a: any, b: any) => (a.rank || 999) - (b.rank || 999));
      }
    }
  } catch (e: any) {
    error.value = e.toString();
  } finally {
    loading.value = false;
  }
}

async function fetchBuild(championId: number) {
  loading.value = true;
  error.value = "";
  selectedChampId.value = championId;
  selectedRuneIdx.value = 0; // 重置选择的符文预设索引
  view.value = "build";
  try {
    const data = await invoke<any>("fetch_opgg_data", {
      region: region.value,
      mode: mode.value,
      tier: tier.value,
      championId,
      position: mode.value === "ranked" ? position.value : undefined,
    });
    buildData.value = data?.data || null;
    opggVersion.value = data?.meta?.version || "";
  } catch (e: any) {
    error.value = e.toString();
  } finally {
    loading.value = false;
  }
}

function pct(v: any): string {
  return v != null ? (v * 100).toFixed(1) + "%" : "-";
}
function fmtKda(k: any): string {
  return k != null ? Number(k).toFixed(2) : "-";
}

function getChampIcon(id: number): string {
  return `/lol-game-data/assets/v1/champion-icons/${id}.png`;
}
function getItemIcon(id: number): string {
  const preloadedPath = gameDataAssets.value?.items?.[id];
  if (preloadedPath) {
    return preloadedPath;
  }
  return `/lol-game-data/assets/v1/items/${id}.png`;
}
function getSpellIcon(id: number): string {
  const preloadedPath = gameDataAssets.value?.spells?.[id];
  if (preloadedPath) {
    return preloadedPath;
  }
  return `/lol-game-data/assets/v1/summoner-spells/${id}.png`;
}
function getRuneIcon(id: number): string {
  const preloadedPath = gameDataAssets.value?.runes?.[id];
  if (preloadedPath) {
    return preloadedPath;
  }
  const style = perkStyles.value?.find((s: any) => s.id === id);
  if (style?.iconPath) {
    return style.iconPath;
  }
  return `/lol-game-data/assets/v1/perks/${id}.png`;
}

// 出装页计算属性
const selectedStats = computed(() => {
  if (!buildData.value?.summary) return null;
  const s = buildData.value.summary;
  if (position.value && s.positions) {
    const p = s.positions.find((p: any) => p.name === position.value);
    return p?.stats || s.average_stats || null;
  }
  return s.average_stats || null;
});
const selectedTier = computed(() => selectedStats.value?.tier_data?.tier || selectedStats.value?.tier);
const selectedPosition = computed(() => mode.value === 'ranked' ? position.value : null);

const strongCounters = computed(() => {
  if (!buildData.value?.counters) return [];
  return buildData.value.counters
    .map((c: any) => ({ ...c, rate: c.win / c.play }))
    .filter((c: any) => c.rate >= 0.5)
    .sort((a: any, b: any) => b.rate - a.rate)
    .slice(0, 5);
});
const weakCounters = computed(() => {
  if (!buildData.value?.counters) return [];
  return buildData.value.counters
    .map((c: any) => ({ ...c, rate: c.win / c.play }))
    .filter((c: any) => c.rate < 0.5)
    .sort((a: any, b: any) => a.rate - b.rate)
    .slice(0, 5);
});

// 符文计算属性
const activeRune = computed(() => {
  if (!buildData.value?.runes?.length) return null;
  return buildData.value.runes[selectedRuneIdx.value] || null;
});

const primaryStyle = computed(() => {
  if (!activeRune.value || !perkStyles.value) return null;
  return perkStyles.value.find((s: any) => s.id === activeRune.value.primary_page_id) || null;
});

const secondaryStyle = computed(() => {
  if (!activeRune.value || !perkStyles.value) return null;
  return perkStyles.value.find((s: any) => s.id === activeRune.value.secondary_page_id) || null;
});

// 一键设置符文页
async function setRunePage() {
  if (!activeRune.value) return;
  try {
    const rune = activeRune.value;
    const perkIds = [
      ...(rune.primary_rune_ids || []),
      ...(rune.secondary_rune_ids || []),
      ...(rune.stat_mod_ids || [])
    ];
    const name = `Yuumi: ${buildData.value?.summary?.name || 'Rune'}`;
    await invoke("apply_rune_page", {
      params: {
        name,
        primary_style_id: rune.primary_page_id,
        sub_style_id: rune.secondary_page_id,
        selected_perk_ids: perkIds
      }
    });
    showToast("符文页应用成功：" + name, "success");
  } catch (e: any) {
    showToast("应用符文页失败: " + e.toString(), "error");
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
        <button :class="['tab-btn', { active: view === 'tier' }]" @click="view = 'tier'; fetchTierList()">梯队</button>
        <button :class="['tab-btn', { active: view === 'build' }]" :disabled="!selectedChampId" @click="view = 'build'; selectedChampId && fetchBuild(selectedChampId)">出装</button>
      </div>
      <div class="opgg-header-right">
        <span v-if="opggVersion" class="opgg-version">游戏版本: {{ opggVersion }}</span>
      </div>
    </div>

    <!-- 筛选栏 -->
    <div class="opgg-filters">
      <div class="filter-trigger" @click.stop="showRegionDropdown = !showRegionDropdown">
        <span>{{ REGIONS.find(r => r.value === region)?.label || region }}</span>
        <svg :class="['filter-arrow', { expanded: showRegionDropdown }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
        <div v-if="showRegionDropdown" class="filter-menu" @click.stop>
          <div v-for="r in REGIONS" :key="r.value" :class="['filter-item', { active: region === r.value }]" @click="region = r.value; showRegionDropdown = false">{{ r.label }}</div>
        </div>
      </div>
      <div class="filter-trigger" @click.stop="showModeDropdown = !showModeDropdown">
        <span>{{ MODES.find(m => m.value === mode)?.label || mode }}</span>
        <svg :class="['filter-arrow', { expanded: showModeDropdown }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
        <div v-if="showModeDropdown" class="filter-menu" @click.stop>
          <div v-for="m in MODES" :key="m.value" :class="['filter-item', { active: mode === m.value }]" @click="mode = m.value; showModeDropdown = false">{{ m.label }}</div>
        </div>
      </div>
      <div class="filter-trigger" @click.stop="showTierDropdown = !showTierDropdown">
        <span>{{ TIERS.find(t => t.value === tier)?.label || tier }}</span>
        <svg :class="['filter-arrow', { expanded: showTierDropdown }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
        <div v-if="showTierDropdown" class="filter-menu" @click.stop>
          <div v-for="t in TIERS" :key="t.value" :class="['filter-item', { active: tier === t.value }]" @click="tier = t.value; showTierDropdown = false">{{ t.label }}</div>
        </div>
      </div>
      <div v-if="mode === 'ranked'" class="filter-trigger" @click.stop="showPositionDropdown = !showPositionDropdown">
        <span>{{ POSITIONS.find(p => p.value === position)?.label || position }}</span>
        <svg :class="['filter-arrow', { expanded: showPositionDropdown }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"/></svg>
        <div v-if="showPositionDropdown" class="filter-menu" @click.stop>
          <div v-for="p in POSITIONS" :key="p.value" :class="['filter-item', { active: position === p.value }]" @click="position = p.value; showPositionDropdown = false">{{ p.label }}</div>
        </div>
      </div>
    </div>

    <!-- 加载 / 错误 -->
    <div v-if="loading" class="opgg-center"><div class="spinner"></div></div>
    <div v-else-if="error" class="opgg-center error-text">{{ error }}</div>

    <!-- 梯队列表 -->
    <div v-else-if="view === 'tier'" class="opgg-body tier-list-body">
      <!-- 表头 -->
      <div class="tier-header">
        <span class="tier-h-rank">#</span>
        <span class="tier-h-champ">英雄</span>
        <span class="tier-h-spacer"></span>
        <span class="tier-h-tier">Tier</span>
        <span class="tier-h-stat">胜率</span>
        <span class="tier-h-stat">登场率</span>
        <span class="tier-h-stat">禁用率</span>
        <span class="tier-h-stat">KDA</span>
        <span class="tier-h-counters">劣势对抗</span>
      </div>
      <!-- 卡片列表 -->
      <div class="tier-cards">
        <div v-for="(c, i) in tierData" :key="c.id"
             class="tier-card"
             :style="{
               background: TIER_CARD_BG[String(c.tier)] || 'transparent',
               borderColor: TIER_CARD_BORDER[String(c.tier)] || 'rgba(0, 0, 0, 0.095)'
             }"
             @click="fetchBuild(c.id)">
          <span class="tier-c-rank">{{ i + 1 }}</span>
          <LcuImage :src="getChampIcon(c.id)" class="tier-c-icon" />
          <span class="tier-c-name">{{ championsMap.get(c.id) || c.name }}</span>
          <span class="tier-c-spacer"></span>
          <img v-if="TIER_ICONS[String(c.tier)]" :src="TIER_ICONS[String(c.tier)]" class="tier-c-icon-svg" alt="" />
          <span v-else class="tier-c-tier-badge">{{ c.tier || '-' }}</span>
          <span class="tier-c-stat">{{ pct(c.win_rate) }}</span>
          <span class="tier-c-stat">{{ pct(c.pick_rate) }}</span>
          <span class="tier-c-stat">{{ pct(c.ban_rate) }}</span>
          <span class="tier-c-stat">{{ fmtKda(c.kda) }}</span>
          <div class="tier-c-counters">
            <LcuImage v-for="cid in (c.counters || []).slice(0, 3)" :key="cid"
                      :src="getChampIcon(cid)" class="tier-counter-icon" />
          </div>
        </div>
      </div>
    </div>

    <!-- 出装详情 -->
    <div v-else-if="view === 'build' && buildData" class="opgg-body build-view-bg">
      <!-- 英雄标题栏：图标 + 名称 + 胜率/登场率/禁用率 + Tier -->
      <div class="build-title-bar" v-if="buildData.summary" :style="{
        background: TIER_BG_COLORS[String(selectedTier)] || '#f8f9fa',
        borderColor: TIER_BORDER_COLORS[String(selectedTier)] || 'rgba(0, 0, 0, 0.04)'
      }">
        <LcuImage :src="getChampIcon(buildData.summary.id || selectedChampId!)" class="build-champ-icon" />
        <div class="build-title-info">
          <div class="build-title-name">{{ championsMap.get(buildData.summary.id || selectedChampId!) || buildData.summary.name || '英雄' }}</div>
          <div class="build-title-pos" v-if="selectedPosition">{{ selectedPosition }}</div>
        </div>
        <div class="build-title-stats">
          <div class="stat-col">
            <span class="stat-label">胜率</span>
            <span class="stat-value">{{ pct(selectedStats?.win_rate || selectedStats?.winRate) }}</span>
          </div>
          <div class="stat-divider"></div>
          <div class="stat-col">
            <span class="stat-label">登场率</span>
            <span class="stat-value">{{ pct(selectedStats?.pick_rate || selectedStats?.pickRate) }}</span>
          </div>
          <div class="stat-divider"></div>
          <div class="stat-col">
            <span class="stat-label">禁用率</span>
            <span class="stat-value">{{ pct(selectedStats?.ban_rate || selectedStats?.banRate) }}</span>
          </div>
        </div>
        <div class="build-title-tier" v-if="selectedTier">
          <span class="tier-badge-lg" :style="{ background: TIER_COLORS[String(selectedTier)] || '#adb5bd' }">
            {{ selectedTier }}
          </span>
        </div>
      </div>

      <!-- 召唤师技能：左右并排显示两组推荐 -->
      <div v-if="buildData.summoner_spells?.length" class="build-card spell-card">
        <div class="spell-presets-container">
          <div class="spell-pair" v-for="(s, i) in buildData.summoner_spells.slice(0, 2)" :key="i">
            <div class="spell-icons">
              <LcuImage v-for="id in s.ids?.slice(0, 2)" :key="id" :src="getSpellIcon(id)" class="spell-icon" />
            </div>
            <div class="spell-stats">
              <span class="spell-wr">{{ pct(s.win / s.play) }}</span>
              <span class="spell-games">{{ s.play?.toLocaleString() }} 场</span>
            </div>
            <span class="spell-pick">{{ pct(s.pick_rate) }}</span>
          </div>
        </div>
      </div>

      <!-- 符文推荐：OP.GG 风格双符文树与碎片 -->
      <div v-if="buildData.runes?.length && activeRune && primaryStyle && secondaryStyle" class="build-card rune-tree-card">
        <div class="rune-tree-container">
          <!-- 左侧：主系、副系、属性碎片三列 -->
          <div class="rune-tree-left">
            <!-- 主系符文列 -->
            <div class="rune-tree-column primary-column">
              <div class="tree-header">
                <LcuImage :src="primaryStyle.iconPath" class="tree-style-icon" :alt="primaryStyle.name" />
              </div>
              <div class="tree-slots">
                <div v-for="(slot, sIdx) in primaryStyle.slots.slice(0, 4)" :key="sIdx" :class="['tree-slot-row', `slot-${sIdx}`]">
                  <div v-for="perkId in slot.perks" :key="perkId" 
                       :class="['rune-item', { active: activeRune.primary_rune_ids?.includes(perkId), keystone: sIdx === 0 }]"
                       :title="perksMap.get(perkId) ? perksMap.get(perkId).name + '\n' + cleanDescription(perksMap.get(perkId).shortDesc) : ''">
                    <LcuImage :src="getRuneIcon(perkId)" class="rune-icon" />
                  </div>
                </div>
              </div>
            </div>

            <!-- 副系符文列 -->
            <div class="rune-tree-column secondary-column">
              <div class="tree-header">
                <LcuImage :src="secondaryStyle.iconPath" class="tree-style-icon" :alt="secondaryStyle.name" />
              </div>
              <div class="tree-slots">
                <div v-for="(slot, sIdx) in secondaryStyle.slots.slice(1, 4)" :key="sIdx" :class="['tree-slot-row', `slot-${sIdx + 1}`]">
                  <div v-for="perkId in slot.perks" :key="perkId" 
                       :class="['rune-item', { active: activeRune.secondary_rune_ids?.includes(perkId) }]"
                       :title="perksMap.get(perkId) ? perksMap.get(perkId).name + '\n' + cleanDescription(perksMap.get(perkId).shortDesc) : ''">
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
                <div v-for="(slot, sIdx) in primaryStyle.slots.slice(4, 7)" :key="sIdx" :class="['tree-slot-row', `slot-${sIdx + 4}`]">
                  <div v-for="perkId in slot.perks" :key="perkId" 
                       :class="['shard-item', { active: activeRune.stat_mod_ids?.includes(perkId) }]"
                       :title="perksMap.get(perkId) ? perksMap.get(perkId).name : ''">
                    <LcuImage :src="getRuneIcon(perkId)" class="shard-icon" />
                  </div>
                </div>
              </div>
            </div>
          </div>

          <!-- 右侧：预设切换列表与一键应用 -->
          <div class="rune-tree-right">
            <div class="rune-presets-box">
              <div v-for="(r, i) in buildData.runes.slice(0, 3)" :key="i"
                   :class="['rune-preset-row', { active: i === selectedRuneIdx }]"
                   @click="selectedRuneIdx = i">
                <LcuImage :src="getRuneIcon(r.primary_page_id)" class="preset-page-icon" />
                <div class="preset-info">
                  <div class="preset-wr">{{ pct(r.win / r.play) }}</div>
                  <div class="preset-games">{{ r.play?.toLocaleString() }} 场</div>
                </div>
                <span class="preset-pick">{{ pct(r.pick_rate) }}</span>
              </div>
            </div>
            <button class="apply-rune-btn" @click="setRunePage">
              设为当前符文页
            </button>
          </div>
        </div>
      </div>

      <!-- 技能加点：主技能图标 + 升级顺序 + 胜率 -->
      <div v-if="buildData.skills?.length || buildData.skill_masteries?.length" class="build-card skill-card">
        <div class="skill-section">
          <div class="skill-main">
            <template v-for="(sid, j) in (buildData.skill_masteries?.[0]?.ids || [])" :key="j">
              <span :class="['skill-icon-box', `key-${sid.toLowerCase()}`]">{{ sid }}</span>
              <span v-if="j < (buildData.skill_masteries?.[0]?.ids?.length || 0) - 1" class="skill-arrow">›</span>
            </template>
          </div>
          <div class="skill-order" v-if="buildData.skills?.[0]?.order">
            <span v-for="(s, j) in buildData.skills[0].order" :key="j" :class="['skill-order-box', `key-${s.toLowerCase()}`]">{{ s }}</span>
          </div>
          <div class="skill-stats">
            <span class="skill-wr">{{ pct(buildData.skills?.[0]?.win / buildData.skills?.[0]?.play) }}</span>
            <span class="skill-games">{{ buildData.skills?.[0]?.play?.toLocaleString() }} 场</span>
          </div>
          <span class="skill-pick">{{ pct(buildData.skills?.[0]?.pick_rate) }}</span>
        </div>
      </div>

      <!-- 出装：初始装备 + 鞋子 | 核心装备 → | 可选装备 -->
      <div v-if="buildData.boots?.length || buildData.core_items?.length || buildData.starter_items?.length || buildData.last_items?.length" class="build-card item-card">
        <!-- 初始装备 + 鞋子（并排） -->
        <div class="item-row-flex" v-if="buildData.starter_items?.length || buildData.boots?.length">
          <div class="item-group">
            <div v-for="(item, i) in (buildData.starter_items || []).slice(0, 3)" :key="'s'+i" class="item-entry">
              <LcuImage v-for="id in item.ids" :key="id" :src="getItemIcon(id)" class="item-icon" />
              <span class="item-entry-wr">{{ pct(item.win / item.play) }}</span>
              <span class="item-entry-games">{{ item.play?.toLocaleString() }}场</span>
            </div>
          </div>
          <div class="item-v-divider"></div>
          <div class="item-group">
            <div v-for="(item, i) in (buildData.boots || []).slice(0, 3)" :key="'b'+i" class="item-entry">
              <LcuImage v-for="id in item.ids" :key="id" :src="getItemIcon(id)" class="item-icon" />
              <span class="item-entry-wr">{{ pct(item.win / item.play) }}</span>
              <span class="item-entry-games">{{ item.play?.toLocaleString() }}场</span>
            </div>
          </div>
        </div>

        <div class="item-h-divider" v-if="buildData.core_items?.length"></div>

        <!-- 核心装备（图标链 + 箭头 + 胜率） -->
        <div v-for="(item, i) in (buildData.core_items || []).slice(0, 5)" :key="'c'+i" class="item-entry-row">
          <div class="item-icons-chain">
            <template v-for="(id, j) in item.ids" :key="j">
              <LcuImage :src="getItemIcon(id)" class="item-icon" />
              <span v-if="j < item.ids.length - 1" class="item-arrow">›</span>
            </template>
          </div>
          <div class="item-entry-stats">
            <span class="item-entry-wr">{{ pct(item.win / item.play) }}</span>
            <span class="item-entry-games">{{ item.play?.toLocaleString() }}场</span>
          </div>
          <span class="item-entry-pick">{{ pct(item.pick_rate) }}</span>
        </div>

        <div class="item-h-divider" v-if="buildData.last_items?.length"></div>

        <!-- 可选装备（一排图标） -->
        <div v-if="buildData.last_items?.length" class="last-items-row">
          <LcuImage v-for="(item, i) in buildData.last_items.slice(0, 10)" :key="'l'+i"
            :src="getItemIcon(item.ids?.[0])" class="item-icon" />
        </div>
      </div>

      <!-- 克制关系：克制 / 被克 双列 -->
      <div v-if="buildData.counters?.length" class="build-card counter-card">
        <div class="counter-columns">
          <!-- 克制（胜率 > 50%） -->
          <div class="counter-col">
            <div class="counter-col-title">克制</div>
            <div v-for="ct in strongCounters" :key="ct.champion_id" class="counter-row">
              <div class="counter-champ">
                <LcuImage :src="getChampIcon(ct.champion_id)" class="counter-icon" />
                <span class="counter-name">{{ championsMap.get(ct.champion_id) || ct.name || ct.champion_id }}</span>
              </div>
              <span class="counter-games">{{ ct.play?.toLocaleString() }}场</span>
              <span :class="['counter-wr-val', ct.rate >= 0.5 ? 'wr-good' : 'wr-bad']">
                {{ pct(ct.rate) }}
              </span>
            </div>
          </div>
          <div class="counter-v-divider"></div>
          <!-- 被克（胜率 < 50%） -->
          <div class="counter-col">
            <div class="counter-col-title">被克制</div>
            <div v-for="ct in weakCounters" :key="ct.champion_id" class="counter-row">
              <div class="counter-champ">
                <LcuImage :src="getChampIcon(ct.champion_id)" class="counter-icon" />
                <span class="counter-name">{{ championsMap.get(ct.champion_id) || ct.name || ct.champion_id }}</span>
              </div>
              <span class="counter-games">{{ ct.play?.toLocaleString() }}场</span>
              <span :class="['counter-wr-val', ct.rate >= 0.5 ? 'wr-good' : 'wr-bad']">
                {{ pct(ct.rate) }}
              </span>
            </div>
          </div>
        </div>
      </div>
    </div>
    <!-- Toast 通知 -->
    <Transition name="toast">
      <div v-if="toast.visible" :class="['toast-message', toast.type]">
        {{ toast.message }}
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.opgg-panel {
  width: 100%;
  height: 100vh;
  background: var(--bg-color);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

@keyframes slideIn {
  from { transform: translateX(24px); opacity: 0; }
  to { transform: translateX(0); opacity: 1; }
}

.opgg-header {
  display: flex; align-items: center; justify-content: space-between;
  padding: 10px 16px; border-bottom: 1px solid var(--border-color);
  flex-shrink: 0;
}

.opgg-header-left { display: flex; align-items: center; gap: 8px; }

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
  width: 22px; height: 22px; border-radius: 4px;
}

.opgg-logo {
  font-weight: 900; font-size: 0.95rem; color: #636FF9;
  letter-spacing: -0.3px;
}

.tab-btn {
  border: 1px solid var(--border-color); background: var(--card-bg); color: var(--text-muted);
  padding: 3px 12px; border-radius: 6px; font-size: 0.78rem;
  font-weight: 600; cursor: pointer; transition: all 0.15s;
}
.tab-btn:hover:not(:disabled) { background: var(--card-bg-hover); color: var(--text-color); }
.tab-btn.active { background: var(--primary-color); color: white; border-color: var(--primary-color); }
.tab-btn:disabled { opacity: 0.4; cursor: not-allowed; }

.opgg-filters {
  display: flex; gap: 6px; padding: 8px 16px;
  border-bottom: 1px solid var(--border-color);
  flex-shrink: 0;
  position: relative; z-index: 2;
  background: var(--bg-color);
}

.filter-trigger {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 4px 8px;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  font-size: 0.78rem;
  background: var(--card-bg);
  color: var(--text-color);
  cursor: pointer;
  position: relative;
  transition: all 0.2s;
  flex-shrink: 0;
  min-width: 0;
}
.filter-trigger:hover {
  background: var(--card-bg);
  border-color: var(--primary-color);
}
.filter-arrow {
  width: 11px; height: 11px;
  transition: transform 0.2s;
  flex-shrink: 0;
}
.filter-arrow.expanded {
  transform: rotate(180deg);
}
.filter-menu {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  box-shadow: var(--shadow-lg);
  z-index: 100;
  min-width: max-content;
  padding: 4px 0;
}
.filter-item {
  padding: 6px 14px;
  font-size: 0.78rem;
  color: var(--text-muted);
  cursor: pointer;
  transition: all 0.2s;
  white-space: nowrap;
}
.filter-item:hover {
  background: rgba(0, 0, 0, 0.02);
  color: var(--text-color);
}
.filter-item.active {
  color: var(--primary-color);
  font-weight: 600;
  background: var(--primary-color-alpha-15);
}

.opgg-body {
  flex: 1; overflow-y: auto; padding: 0;
}

.opgg-body::-webkit-scrollbar { width: 5px; }
.opgg-body::-webkit-scrollbar-thumb { background: rgba(0, 0, 0, 0.1); border-radius: 3px; }

.opgg-center {
  flex: 1; display: flex; align-items: center; justify-content: center;
  color: var(--text-dimmed); font-size: 0.85rem;
}
.error-text { color: var(--loss-color); }

.spinner {
  width: 28px; height: 28px; border: 3px solid rgba(0, 0, 0, 0.05);
  border-top-color: var(--primary-color); border-radius: 50%;
  animation: spin 0.7s linear infinite;
}
@keyframes spin { to { transform: rotate(360deg); } }

/* 梯队卡片列表（Python Seraphine 风格） */
.tier-list-body { padding: 8px 10px; }

.tier-header {
  display: flex; align-items: center; gap: 6px;
  padding: 6px 10px; margin-bottom: 3px;
  font-size: 0.72rem; font-weight: 600; color: var(--text-muted);
  text-transform: uppercase;
  border-bottom: 1px solid var(--border-color);
}
.tier-h-rank { width: 30px; text-align: center; flex-shrink: 0; }
.tier-h-champ { flex: 1; }
.tier-h-spacer { flex: 1; }
.tier-h-tier { width: 50px; text-align: center; flex-shrink: 0; }
.tier-h-stat { width: 65px; text-align: center; flex-shrink: 0; }

.tier-cards {
  display: flex; flex-direction: column; gap: 3px;
}

.tier-card {
  display: flex; align-items: center; gap: 6px;
  padding: 12px 14px;
  border: 1px solid rgba(0, 0, 0, 0.095);
  border-radius: 6px;
  cursor: pointer;
  transition: filter 0.15s;
  font-size: 0.78rem; color: var(--text-muted);
}
.tier-card:hover { filter: brightness(0.93); }

.tier-c-rank { width: 30px; text-align: center; flex-shrink: 0; color: var(--text-dimmed); }
.tier-c-icon { width: 34px; height: 34px; border-radius: 50%; border: 2px solid var(--border-color); flex-shrink: 0; }
.tier-c-name { flex-shrink: 0; font-weight: 500; color: var(--text-color); }
.tier-c-spacer { flex: 1; }
.tier-c-tier-badge {
  width: 50px; text-align: center; flex-shrink: 0;
  font-weight: 700; font-size: 0.7rem;
}
.tier-c-icon-svg { width: 24px; height: 24px; flex-shrink: 0; }
.tier-c-stat { width: 65px; text-align: center; flex-shrink: 0; }
.tier-h-counters { width: 90px; text-align: center; flex-shrink: 0; }
.tier-c-counters { display: flex; align-items: center; gap: 3px; flex-shrink: 0; width: 90px; justify-content: center; }
.tier-counter-icon { width: 22px; height: 22px; border-radius: 50%; border: 1px solid rgba(0, 0, 0, 0.095); }

/* ── 出装详情页 ── */

/* 通用卡片美化 */
.build-view-bg {
  background: #f1f3f7; /* 软灰背景，让白色卡片更具层次感 */
}
.build-card {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  margin: 10px 14px;
  padding: 12px 16px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.02);
}

/* 英雄标题栏 */
.build-title-bar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 14px 18px;
  margin: 10px 14px;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.02);
}
.build-champ-icon { width: 46px; height: 46px; border-radius: 50%; border: 2px solid var(--border-color); }
.build-title-info { display: flex; flex-direction: column; gap: 2px; min-width: 60px; }
.build-title-name { font-size: 0.95rem; font-weight: 700; color: var(--text-color); }
.build-title-pos { font-size: 0.7rem; color: var(--text-dimmed); }
.build-title-stats { display: flex; align-items: center; gap: 8px; margin-left: auto; }
.stat-col { display: flex; flex-direction: column; align-items: center; gap: 1px; }
.stat-label { font-size: 0.65rem; color: var(--text-dimmed); text-transform: uppercase; }
.stat-value { font-size: 0.82rem; font-weight: 700; color: var(--text-color); }
.stat-divider { width: 1px; height: 24px; background: var(--border-color); }
.build-title-tier { margin-left: 8px; }
.tier-badge-lg {
  display: inline-block; padding: 2px 10px; border-radius: 4px;
  color: white; font-weight: 800; font-size: 0.82rem; text-align: center;
}

/* 召唤师技能：横向并列展示 */
.spell-presets-container {
  display: flex;
  gap: 12px;
  width: 100%;
}
.spell-pair {
  flex: 1;
  display: flex;
  align-items: center;
  background: rgba(0, 0, 0, 0.015);
  border: 1px solid var(--border-color);
  border-radius: 6px;
  padding: 6px 12px;
  gap: 12px;
}
.spell-icons { display: flex; gap: 4px; }
.spell-icon { width: 28px; height: 28px; border-radius: 5px; border: 1px solid var(--border-color); }
.spell-stats { display: flex; flex-direction: column; align-items: flex-start; gap: 1px; flex: 1; }
.spell-wr { font-size: 0.78rem; font-weight: 700; color: var(--text-color); }
.spell-games { font-size: 0.62rem; color: var(--text-dimmed); }
.spell-pick { font-size: 0.75rem; font-weight: 700; color: var(--text-muted); text-align: right; }

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

/* 技能加点：彩色按钮美化 */
.skill-section { display: flex; align-items: center; gap: 10px; }
.skill-main { display: flex; align-items: center; gap: 4px; }
.skill-icon-box {
  display: inline-flex; align-items: center; justify-content: center;
  width: 26px; height: 26px; border-radius: 4px;
  font-size: 0.75rem; font-weight: 700;
}
.skill-arrow { color: var(--text-dimmed); font-size: 0.82rem; margin: 0 4px; font-weight: bold; line-height: 1; }
.skill-order { display: flex; gap: 2px; }
.skill-order-box {
  display: inline-flex; align-items: center; justify-content: center;
  width: 18px; height: 18px; border-radius: 3px;
  font-size: 0.62rem;
}
.skill-stats { display: flex; flex-direction: column; align-items: center; margin-left: auto; }
.skill-wr { font-size: 0.78rem; font-weight: 600; color: var(--text-color); }
.skill-games { font-size: 0.65rem; color: var(--text-dimmed); }
.skill-pick { font-size: 0.75rem; font-weight: 700; color: var(--text-muted); min-width: 40px; text-align: right; }

/* 技能独立按键上色 */
.skill-icon-box.key-q, .skill-order-box.key-q {
  background: rgba(51, 154, 240, 0.08);
  color: #1c7ed6;
  border: 1px solid rgba(51, 154, 240, 0.15);
}
.skill-icon-box.key-w, .skill-order-box.key-w {
  background: rgba(55, 178, 77, 0.08);
  color: #2b8a3e;
  border: 1px solid rgba(55, 178, 77, 0.15);
}
.skill-icon-box.key-e, .skill-order-box.key-e {
  background: rgba(247, 103, 7, 0.08);
  color: #d9480f;
  border: 1px solid rgba(247, 103, 7, 0.15);
}
.skill-icon-box.key-r, .skill-order-box.key-r {
  background: rgba(240, 62, 62, 0.08);
  color: #c92a2a;
  border: 1px solid rgba(240, 62, 62, 0.15);
  font-weight: 800;
}

/* 出装 */
.item-row-flex { display: flex; gap: 0; }
.item-group { flex: 1; display: flex; flex-direction: column; gap: 4px; }
.item-v-divider { width: 1px; background: var(--border-color); margin: 0 8px; }
.item-h-divider { height: 1px; background: var(--border-color); margin: 6px 0; }
.item-entry { display: flex; align-items: center; gap: 4px; }
.item-icon { width: 26px; height: 26px; border-radius: 4px; border: 1px solid var(--border-color); }
.item-entry-wr { font-size: 0.72rem; font-weight: 600; color: var(--text-color); margin-left: auto; }
.item-entry-games { font-size: 0.62rem; color: var(--text-dimmed); min-width: 50px; }
.item-entry-row { display: flex; align-items: center; gap: 8px; padding: 3px 0; }
.item-icons-chain { display: flex; align-items: center; gap: 3px; }
.item-arrow { color: var(--text-dimmed); font-size: 0.7rem; margin: 0 2px; }
.item-entry-stats { display: flex; flex-direction: column; align-items: center; margin-left: auto; }
.item-entry-pick { font-size: 0.72rem; font-weight: 700; color: var(--text-muted); min-width: 40px; text-align: right; }
.last-items-row { display: flex; gap: 4px; flex-wrap: wrap; }

/* 克制关系 */
.counter-columns { display: flex; gap: 0; }
.counter-col { flex: 1; display: flex; flex-direction: column; gap: 4px; }
.counter-v-divider { width: 1px; background: var(--border-color); margin: 0 8px; }
.counter-col-title { font-size: 0.7rem; font-weight: 700; color: var(--text-dimmed); text-transform: uppercase; margin-bottom: 4px; }
.counter-row { display: flex; align-items: center; gap: 6px; height: 28px; }
.counter-champ { display: flex; align-items: center; gap: 5px; min-width: 0; }
.counter-icon { width: 22px; height: 22px; border-radius: 50%; border: 1px solid var(--border-color); flex-shrink: 0; }
.counter-name { font-size: 0.72rem; color: var(--text-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.counter-games { font-size: 0.62rem; color: var(--text-dimmed); margin-left: auto; min-width: 45px; text-align: center; }
.counter-wr-val { font-size: 0.75rem; font-weight: 700; min-width: 40px; text-align: right; }

.wr-good { color: var(--win-color); }
.wr-bad { color: var(--loss-color); }

/* Toast 样式 */
.toast-message {
  position: fixed;
  bottom: 30px;
  left: 50%;
  transform: translateX(-50%);
  padding: 8px 18px;
  border-radius: 6px;
  color: white;
  font-size: 0.78rem;
  font-weight: 600;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  z-index: 9999;
}
.toast-message.success {
  background: var(--primary-color);
}
.toast-message.error {
  background: #f03e3e;
}

/* Toast 过渡效果 */
.toast-enter-active, .toast-leave-active {
  transition: all 0.25s ease;
}
.toast-enter-from {
  opacity: 0;
  transform: translate(-50%, 10px);
}
.toast-leave-to {
  opacity: 0;
  transform: translate(-50%, -10px);
}
</style>
