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
.tft { padding: 1rem; }
.tip { color: #888; text-align: center; padding: 2rem; }

.section { margin-bottom: 2rem; }
.section h3 { margin: 0 0 4px; }
.section-desc { color: #888; font-size: 0.85rem; margin: 0 0 12px; }

.comp-grid {
  display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 10px;
}
.comp-card {
  padding: 12px; border-radius: 8px; background: #f5f5f5;
  cursor: pointer; transition: all 0.2s; border: 2px solid transparent;
}
.comp-card:hover { background: #eeeeee; }
.comp-card.selected { border-color: #1976d2; background: #e3f2fd; }
.comp-tier {
  display: inline-block; padding: 2px 8px; border-radius: 4px;
  font-weight: bold; font-size: 0.8rem; color: white; margin-bottom: 6px;
}
.comp-tier.s { background: #d32f2f; }
.comp-tier.a { background: #f57c00; }
.comp-tier.b { background: #1976d2; }
.comp-name { font-weight: 600; margin-bottom: 4px; }
.comp-core, .comp-traits { font-size: 0.8rem; color: #666; }

.comp-detail {
  margin-top: 12px; padding: 16px; background: #fafafa;
  border-radius: 8px; border: 1px solid #e0e0e0;
}
.comp-detail h4 { margin: 0 0 12px; }
.detail-section { margin-bottom: 8px; }
.detail-label { font-weight: 600; margin-right: 8px; }
.detail-value { color: #aaa; font-size: 0.85rem; }

.augment-list { display: flex; flex-direction: column; gap: 8px; }
.augment-card {
  display: flex; align-items: center; gap: 12px;
  padding: 10px 14px; border-radius: 8px; background: #f5f5f5;
}
.augment-tier {
  padding: 4px 10px; border-radius: 4px;
  font-weight: bold; font-size: 0.8rem; color: white;
}
.augment-tier.gold { background: #f9a825; }
.augment-tier.silver { background: #9e9e9e; }
.augment-name { font-weight: 600; }
.augment-desc { font-size: 0.8rem; color: #888; display: block; }
</style>
