<script setup lang="ts">
import { ref, onMounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import LcuImage from "./LcuImage.vue";

const emit = defineEmits<{ close: [] }>();

// 筛选状态
const mode = ref("ranked");
const tier = ref("emerald_plus");
const position = ref("MID");
const view = ref<"tier" | "build">("tier");

// 数据
const tierData = ref<any[]>([]);
const buildData = ref<any | null>(null);
const loading = ref(false);
const error = ref("");
const selectedChampId = ref<number | null>(null);

const MODES = [
  { value: "ranked", label: "排位" },
  { value: "normal", label: "匹配" },
  { value: "aram", label: "大乱斗" },
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

const TIER_COLORS: Record<string, string> = {
  "1": "#ff6b6b", "2": "#ffa94d", "3": "#ffd43b",
  "4": "#51cf66", "5": "#339af0", "": "#adb5bd",
};

onMounted(() => { fetchTierList(); });

watch([mode, tier, position], () => {
  if (view.value === "tier") fetchTierList();
});

async function fetchTierList() {
  loading.value = true;
  error.value = "";
  try {
    const data = await invoke<any>("fetch_opgg_data", {
      region: "kr",
      mode: mode.value,
      tier: tier.value,
    });
    if (data?.data) {
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
  view.value = "build";
  try {
    const data = await invoke<any>("fetch_opgg_data", {
      region: "kr",
      mode: mode.value,
      tier: tier.value,
      championId,
      position: mode.value === "ranked" ? position.value : undefined,
    });
    buildData.value = data?.data || null;
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
</script>

<template>
  <div class="opgg-overlay" @click.self="emit('close')">
    <div class="opgg-modal">
      <!-- 头部 -->
      <div class="opgg-header">
        <div class="opgg-header-left">
          <span class="opgg-logo">OP.GG</span>
          <button :class="['tab-btn', { active: view === 'tier' }]" @click="view = 'tier'; fetchTierList()">梯队</button>
          <button :class="['tab-btn', { active: view === 'build' }]" :disabled="!selectedChampId" @click="view = 'build'; selectedChampId && fetchBuild(selectedChampId)">出装</button>
        </div>
        <button class="close-btn" @click="emit('close')">&times;</button>
      </div>

      <!-- 筛选栏 -->
      <div class="opgg-filters">
        <select v-model="mode" class="filter-select">
          <option v-for="m in MODES" :key="m.value" :value="m.value">{{ m.label }}</option>
        </select>
        <select v-model="tier" class="filter-select">
          <option v-for="t in TIERS" :key="t.value" :value="t.value">{{ t.label }}</option>
        </select>
        <select v-if="mode === 'ranked'" v-model="position" class="filter-select">
          <option v-for="p in POSITIONS" :key="p.value" :value="p.value">{{ p.label }}</option>
        </select>
      </div>

      <!-- 加载 / 错误 -->
      <div v-if="loading" class="opgg-center"><div class="spinner"></div></div>
      <div v-else-if="error" class="opgg-center error-text">{{ error }}</div>

      <!-- 梯队列表 -->
      <div v-else-if="view === 'tier'" class="opgg-body">
        <table class="tier-table">
          <thead>
            <tr>
              <th class="col-rank">#</th>
              <th class="col-champ">英雄</th>
              <th class="col-tier">Tier</th>
              <th>胜率</th>
              <th>登场率</th>
              <th>禁用率</th>
              <th>KDA</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(c, i) in tierData" :key="c.id" class="tier-row" @click="fetchBuild(c.id)">
              <td class="col-rank">{{ i + 1 }}</td>
              <td class="col-champ">
                <div class="champ-cell">
                  <LcuImage :src="getChampIcon(c.id)" class="champ-icon" />
                  <span>{{ c.name }}</span>
                </div>
              </td>
              <td class="col-tier">
                <span class="tier-badge" :style="{ background: TIER_COLORS[c.tier] || '#adb5bd' }">
                  {{ c.tier || '-' }}
                </span>
              </td>
              <td>{{ pct(c.win_rate) }}</td>
              <td>{{ pct(c.pick_rate) }}</td>
              <td>{{ pct(c.ban_rate) }}</td>
              <td>{{ fmtKda(c.kda) }}</td>
            </tr>
          </tbody>
        </table>
      </div>

      <!-- 出装详情 -->
      <div v-else-if="view === 'build' && buildData" class="opgg-body">
        <div class="build-header">
          <button class="back-btn" @click="view = 'tier'">&larr; 返回梯队</button>
        </div>

        <!-- 英雄信息 -->
        <div class="build-summary" v-if="buildData.summary">
          <LcuImage :src="getChampIcon(buildData.summary.id || selectedChampId!)" class="build-champ-icon" />
          <div class="build-info">
            <h3>{{ buildData.summary.name || '英雄' }}</h3>
            <div class="build-stats">
              <span>胜率 <b>{{ pct(buildData.summary.win_rate) }}</b></span>
              <span>登场率 <b>{{ pct(buildData.summary.pick_rate) }}</b></span>
              <span>禁用率 <b>{{ pct(buildData.summary.ban_rate) }}</b></span>
              <span>KDA <b>{{ fmtKda(buildData.summary.kda) }}</b></span>
            </div>
          </div>
        </div>

        <!-- 召唤师技能 -->
        <div v-if="buildData.summoner_spells?.length" class="build-section">
          <h4>召唤师技能</h4>
          <div class="spell-rows">
            <div v-for="(s, i) in buildData.summoner_spells.slice(0, 3)" :key="i" class="spell-row">
              <span class="spell-ids">{{ s.ids?.join(' + ') }}</span>
              <span class="spell-stat">胜率 {{ pct(s.win / s.play) }} · 场次 {{ s.play }}</span>
            </div>
          </div>
        </div>

        <!-- 技能加点 -->
        <div v-if="buildData.skills?.length" class="build-section">
          <h4>技能加点</h4>
          <div class="skill-order">
            <span v-for="(s, i) in buildData.skills.slice(0, 1)" :key="i">
              {{ s.order?.join(' > ') }}
              <small>胜率 {{ pct(s.win / s.play) }} · 场次 {{ s.play }}</small>
            </span>
          </div>
        </div>

        <!-- 出装 -->
        <div v-if="buildData.core_items?.length" class="build-section">
          <h4>核心装备</h4>
          <div class="item-rows">
            <div v-for="(item, i) in buildData.core_items.slice(0, 5)" :key="i" class="item-row">
              <span class="item-ids">物品 {{ item.ids?.join(' → ') }}</span>
              <span class="item-stat">胜率 {{ pct(item.win / item.play) }} · 场次 {{ item.play }}</span>
            </div>
          </div>
        </div>

        <!-- 符文 -->
        <div v-if="buildData.runes?.length" class="build-section">
          <h4>符文推荐</h4>
          <div class="rune-rows">
            <div v-for="(r, i) in buildData.runes.slice(0, 3)" :key="i" class="rune-row">
              <span>主系 {{ r.primary_page_id }} / 副系 {{ r.secondary_page_id }}</span>
              <span class="rune-stat">胜率 {{ pct(r.win / r.play) }} · 场次 {{ r.play }}</span>
            </div>
          </div>
        </div>

        <!-- 克制关系 -->
        <div v-if="buildData.counters?.length" class="build-section">
          <h4>克制关系</h4>
          <div class="counter-rows">
            <div v-for="(ct, i) in buildData.counters.slice(0, 6)" :key="i" class="counter-item">
              <LcuImage :src="getChampIcon(ct.champion_id)" class="counter-icon" />
              <span :class="['counter-wr', ct.win / ct.play >= 0.5 ? 'wr-good' : 'wr-bad']">
                {{ pct(ct.win / ct.play) }}
              </span>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.opgg-overlay {
  position: fixed; inset: 0; z-index: 9000;
  background: rgba(0, 0, 0, 0.45);
  display: flex; align-items: center; justify-content: center;
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
}

.opgg-modal {
  width: 680px; max-width: 95vw; height: 80vh;
  background: var(--bg-color); border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  display: flex; flex-direction: column;
  box-shadow: var(--shadow-lg);
  overflow: hidden;
}

.opgg-header {
  display: flex; align-items: center; justify-content: space-between;
  padding: 12px 20px; border-bottom: 1px solid var(--border-color);
  background: rgba(0, 0, 0, 0.01);
}

.opgg-header-left { display: flex; align-items: center; gap: 12px; }

.opgg-logo {
  font-weight: 900; font-size: 1.1rem; color: var(--primary-color);
  letter-spacing: -0.5px;
}

.tab-btn {
  border: 1px solid var(--border-color); background: rgba(255, 255, 255, 0.5); color: var(--text-muted);
  padding: 4px 14px; border-radius: 6px; font-size: 0.82rem;
  font-weight: 600; cursor: pointer; transition: all 0.15s;
}
.tab-btn:hover:not(:disabled) { background: rgba(255, 255, 255, 0.95); color: var(--text-color); }
.tab-btn.active { background: var(--primary-color); color: white; border-color: var(--primary-color); }
.tab-btn:disabled { opacity: 0.4; cursor: not-allowed; }

.close-btn {
  background: none; border: none; font-size: 1.5rem;
  color: var(--text-muted); cursor: pointer; line-height: 1;
}
.close-btn:hover { color: var(--text-color); }

.opgg-filters {
  display: flex; gap: 8px; padding: 10px 20px;
  border-bottom: 1px solid var(--border-color);
}

.filter-select {
  padding: 5px 10px; border: 1px solid var(--border-color);
  border-radius: 6px; font-size: 0.82rem; background: rgba(255, 255, 255, 0.6);
  color: var(--text-color); outline: none;
}
.filter-select:focus { border-color: var(--primary-color); }

.opgg-body {
  flex: 1; overflow-y: auto; padding: 0;
}

.opgg-body::-webkit-scrollbar { width: 5px; }
.opgg-body::-webkit-scrollbar-thumb { background: rgba(0, 0, 0, 0.1); border-radius: 3px; }

.opgg-center {
  flex: 1; display: flex; align-items: center; justify-content: center;
  color: var(--text-dimmed); font-size: 0.88rem;
}
.error-text { color: var(--loss-color); }

.spinner {
  width: 32px; height: 32px; border: 3px solid rgba(0, 0, 0, 0.05);
  border-top-color: var(--primary-color); border-radius: 50%;
  animation: spin 0.7s linear infinite;
}
@keyframes spin { to { transform: rotate(360deg); } }

/* 梯队表格 */
.tier-table {
  width: 100%; border-collapse: collapse; font-size: 0.82rem;
}
.tier-table th {
  position: sticky; top: 0; background: rgba(0, 0, 0, 0.01);
  padding: 10px 12px; text-align: left; font-weight: 600;
  color: var(--text-muted); border-bottom: 1px solid var(--border-color);
  font-size: 0.75rem; text-transform: uppercase;
}
.tier-table td { padding: 8px 12px; border-bottom: 1px solid var(--border-color); color: var(--text-muted); }
.tier-row { cursor: pointer; transition: background 0.1s; }
.tier-row:hover { background: rgba(0, 0, 0, 0.02); }

.col-rank { width: 40px; text-align: center; color: var(--text-dimmed); }
.col-champ { min-width: 120px; }
.col-tier { width: 50px; }

.champ-cell { display: flex; align-items: center; gap: 8px; }
.champ-icon { width: 28px; height: 28px; border-radius: 50%; border: 1.5px solid var(--border-color); }

.tier-badge {
  display: inline-block; padding: 1px 8px; border-radius: 4px;
  color: white; font-weight: 700; font-size: 0.75rem;
  text-align: center;
}

/* 出装详情 */
.build-header { padding: 12px 20px 0; }
.back-btn {
  background: none; border: none; color: var(--primary-color);
  font-size: 0.82rem; font-weight: 600; cursor: pointer;
}
.back-btn:hover { text-decoration: underline; }

.build-summary {
  display: flex; align-items: center; gap: 16px;
  padding: 16px 20px; border-bottom: 1px solid var(--border-color);
}
.build-champ-icon { width: 56px; height: 56px; border-radius: 50%; border: 2px solid var(--border-color); }
.build-info h3 { margin: 0 0 6px; font-size: 1.05rem; color: var(--text-color); }
.build-stats { display: flex; gap: 16px; font-size: 0.78rem; color: var(--text-muted); }
.build-stats b { color: var(--text-color); }

.build-section {
  padding: 14px 20px; border-bottom: 1px solid var(--border-color);
}
.build-section h4 {
  margin: 0 0 10px; font-size: 0.85rem; color: var(--text-color);
  font-weight: 700;
}

.spell-rows, .item-rows, .rune-rows { display: flex; flex-direction: column; gap: 6px; }
.spell-row, .item-row, .rune-row {
  display: flex; justify-content: space-between; align-items: center;
  font-size: 0.78rem; color: var(--text-muted); padding: 4px 0;
}
.spell-stat, .item-stat, .rune-stat { color: var(--text-dimmed); font-size: 0.75rem; }

.skill-order { font-size: 0.82rem; color: var(--text-color); }
.skill-order small { color: var(--text-dimmed); margin-left: 10px; font-size: 0.75rem; }

.counter-rows { display: flex; gap: 12px; flex-wrap: wrap; }
.counter-item { display: flex; flex-direction: column; align-items: center; gap: 4px; }
.counter-icon { width: 32px; height: 32px; border-radius: 50%; border: 1.5px solid var(--border-color); }
.counter-wr { font-size: 0.75rem; font-weight: 600; }
.wr-good { color: var(--win-color); }
.wr-bad { color: var(--loss-color); }
</style>
