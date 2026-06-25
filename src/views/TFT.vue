<script setup lang="ts">
import { ref } from "vue";
import { useLcuStore } from "../store/lcuStore";

const store = useLcuStore();

// 占位数据 — 实际数据需接入 OP.GG / CDragon
const teamComps = ref([
  { name: "九五至尊", core: "瑟提 + 艾希 + 凯尔", traits: "8斗士 / 4射手", tier: "S" },
  { name: "重秘狐狸", core: "阿狸 + 莫甘娜 + 丽桑卓", traits: "4重装 / 3秘术", tier: "S" },
  { name: "决斗大师", core: "亚索 + 菲奥娜 + 永恩", traits: "6决斗 / 2浪人", tier: "A" },
  { name: "神盾战士", core: "贾克斯 + 盲僧 + 瑟提", traits: "6神盾 / 3铁甲", tier: "A" },
  { name: "魔导法师", core: "辛德拉 + 维克托 + 妮蔻", traits: "4魔导 / 2法师", tier: "B" },
]);

const augments = ref([
  { name: "组件百宝袋", desc: "获得 3 件随机基础装备", tier: "金" },
  { name: "升级咯！", desc: "立即获得 4 经验值", tier: "银" },
  { name: "利滚利", desc: "每回合多获得 1 金币利息", tier: "金" },
]);

const selectedComp = ref<number | null>(null);
</script>

<template>
  <div class="tft">
    <h2>🧩 云顶之弈助手</h2>

    <div v-if="!store.isConnected" class="tip">
      请先启动英雄联盟客户端
    </div>

    <div v-else>
      <!-- 阵容推荐 -->
      <section class="section">
        <h3>上分阵容推荐</h3>
        <p class="section-desc">当前赛季强势阵容（数据待接入 OP.GG）</p>

        <div class="comp-grid">
          <div
            v-for="(comp, idx) in teamComps"
            :key="idx"
            :class="['comp-card', { selected: selectedComp === idx }]"
            @click="selectedComp = idx"
          >
            <div class="comp-tier" :class="comp.tier.toLowerCase()">{{ comp.tier }}</div>
            <div class="comp-name">{{ comp.name }}</div>
            <div class="comp-core">核心: {{ comp.core }}</div>
            <div class="comp-traits">{{ comp.traits }}</div>
          </div>
        </div>

        <!-- 选中阵容详情 -->
        <div v-if="selectedComp !== null" class="comp-detail">
          <h4>{{ teamComps[selectedComp].name }} — 详细攻略</h4>
          <div class="detail-placeholder">
            <div class="detail-section">
              <span class="detail-label">前期过渡</span>
              <span class="detail-value">（英雄推荐待接入）</span>
            </div>
            <div class="detail-section">
              <span class="detail-label">核心装备</span>
              <span class="detail-value">（装备推荐待接入）</span>
            </div>
            <div class="detail-section">
              <span class="detail-label">站位图</span>
              <span class="detail-value">（站位推荐待接入）</span>
            </div>
          </div>
        </div>
      </section>

      <!-- 海克斯推荐 -->
      <section class="section">
        <h3>海克斯强化推荐</h3>
        <p class="section-desc">当前对局最佳海克斯选择</p>

        <div class="augment-list">
          <div v-for="(a, idx) in augments" :key="idx" class="augment-card">
            <span :class="['augment-tier', a.tier === '金' ? 'gold' : 'silver']">{{ a.tier }}</span>
            <div class="augment-info">
              <span class="augment-name">{{ a.name }}</span>
              <span class="augment-desc">{{ a.desc }}</span>
            </div>
          </div>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.tft {
  padding: 1.5rem 1.5rem 1.5rem 0.6rem;
  color: var(--text-color);
}
.tip { color: var(--text-dimmed); text-align: center; padding: 2rem; font-size: 0.95rem; }

.section { margin-bottom: 2rem; }
.section h3 { margin: 0 0 6px; font-size: 1.1rem; font-weight: 800; color: var(--text-color); }
.section-desc { color: var(--text-dimmed); font-size: 0.78rem; margin: 0 0 12px; }

.comp-grid {
  display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 10px;
}
.comp-card {
  padding: 12px; border-radius: var(--radius-md); background: var(--card-bg);
  cursor: pointer; transition: all 0.25s cubic-bezier(0.25, 0.8, 0.25, 1);
  border: 1px solid var(--border-color); backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
  box-shadow: var(--shadow-sm);
}
.comp-card:hover {
  background: var(--card-bg-hover);
  border-color: var(--border-color-hover);
  box-shadow: var(--shadow-md);
}
.comp-card.selected {
  border-color: var(--primary-color);
  background: var(--primary-color-alpha-15);
  box-shadow: 0 4px 12px var(--primary-color-alpha-15);
}
.comp-tier {
  display: inline-block; padding: 2px 8px; border-radius: 4px;
  font-weight: bold; font-size: 0.75rem; color: white; margin-bottom: 6px;
}
.comp-tier.s { background: var(--loss-color); box-shadow: 0 4px 10px var(--loss-glow); }
.comp-tier.a { background: #f59e0b; box-shadow: 0 4px 10px rgba(245, 158, 11, 0.2); }
.comp-tier.b { background: #3b82f6; box-shadow: 0 4px 10px rgba(59, 130, 246, 0.2); }
.comp-name { font-weight: bold; color: var(--text-color); margin-bottom: 4px; font-size: 0.85rem; }
.comp-core, .comp-traits { font-size: 0.78rem; color: var(--text-muted); }

.comp-detail {
  margin-top: 12px; padding: 16px; background: var(--card-bg);
  border-radius: var(--radius-md); border: 1px solid var(--border-color);
  backdrop-filter: var(--glass-filter); -webkit-backdrop-filter: var(--glass-filter);
  box-shadow: var(--shadow-sm);
}
.comp-detail h4 { margin: 0 0 12px; font-size: 0.9rem; font-weight: 700; color: var(--text-color); }
.detail-section { margin-bottom: 8px; font-size: 0.8rem; }
.detail-label { font-weight: 600; color: var(--text-color); margin-right: 8px; }
.detail-value { color: var(--text-muted); }

.augment-list { display: flex; flex-direction: column; gap: 8px; }
.augment-card {
  display: flex; align-items: center; gap: 12px;
  padding: 10px 14px; border-radius: var(--radius-md); background: var(--card-bg);
  border: 1px solid var(--border-color); backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
  box-shadow: var(--shadow-sm);
}
.augment-tier {
  padding: 3px 8px; border-radius: 4px;
  font-weight: bold; font-size: 0.72rem; color: white;
}
.augment-tier.gold { background: #fbbf24; color: #000; box-shadow: 0 4px 10px rgba(251, 191, 36, 0.2); }
.augment-tier.silver { background: #94a3b8; box-shadow: 0 4px 10px rgba(148, 163, 184, 0.2); }
.augment-name { font-weight: bold; color: var(--text-color); font-size: 0.85rem; }
.augment-desc { font-size: 0.78rem; color: var(--text-muted); display: block; margin-top: 2px; }
</style>
