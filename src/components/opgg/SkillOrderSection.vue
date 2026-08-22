<script setup lang="ts">
import type { OpggSkillOrder, OpggSkillMastery } from "../../types/opgg";
import { pct } from "./shared";

defineProps<{
  skills?: OpggSkillOrder[];
  masteries?: OpggSkillMastery[];
}>();
</script>

<template>
  <!-- 技能加点：主技能图标 + 升级顺序 + 胜率 -->
  <div v-if="skills?.length || masteries?.length" class="build-card skill-card">
    <div class="skill-section">
      <div class="skill-main">
        <template
          v-for="(sid, j) in masteries?.[0]?.ids || []"
          :key="j"
        >
          <span :class="['skill-icon-box', `key-${sid.toLowerCase()}`]">{{
            sid
          }}</span>
          <span
            v-if="j < (masteries?.[0]?.ids?.length || 0) - 1"
            class="skill-arrow"
            >›</span
          >
        </template>
      </div>
      <div class="skill-order" v-if="skills?.[0]?.order">
        <span
          v-for="(s, j) in skills?.[0]?.order || []"
          :key="j"
          :class="['skill-order-box', `key-${s.toLowerCase()}`]"
          >{{ s }}</span
        >
      </div>
      <div class="skill-stats">
        <span class="skill-wr">{{
          pct((skills?.[0]?.win ?? 0) / (skills?.[0]?.play ?? 0))
        }}</span>
        <span class="skill-games">{{
          $t("career.gamesCount", { count: skills?.[0]?.play })
        }}</span>
      </div>
      <span class="skill-pick">{{
        pct(skills?.[0]?.pick_rate)
      }}</span>
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

/* 技能加点：彩色按钮美化 */
.skill-section {
  display: flex;
  align-items: center;
  gap: 10px;
}
.skill-main {
  display: flex;
  align-items: center;
  gap: 4px;
}
.skill-icon-box {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border-radius: 4px;
  font-size: 0.75rem;
  font-weight: 700;
}
.skill-arrow {
  color: var(--text-dimmed);
  font-size: 0.82rem;
  margin: 0 4px;
  font-weight: bold;
  line-height: 1;
}
.skill-order {
  display: flex;
  gap: 2px;
}
.skill-order-box {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  border-radius: 3px;
  font-size: 0.62rem;
}
.skill-stats {
  display: flex;
  flex-direction: column;
  align-items: center;
  margin-left: auto;
}
.skill-wr {
  font-size: 0.78rem;
  font-weight: 600;
  color: var(--text-color);
}
.skill-games {
  font-size: 0.65rem;
  color: var(--text-dimmed);
}
.skill-pick {
  font-size: 0.75rem;
  font-weight: 700;
  color: var(--text-muted);
  min-width: 40px;
  text-align: right;
}

/* 技能独立按键上色 */
.skill-icon-box.key-q,
.skill-order-box.key-q {
  background: rgba(51, 154, 240, 0.08);
  color: #1c7ed6;
  border: 1px solid rgba(51, 154, 240, 0.15);
}
.skill-icon-box.key-w,
.skill-order-box.key-w {
  background: rgba(55, 178, 77, 0.08);
  color: #2b8a3e;
  border: 1px solid rgba(55, 178, 77, 0.15);
}
.skill-icon-box.key-e,
.skill-order-box.key-e {
  background: rgba(247, 103, 7, 0.08);
  color: #d9480f;
  border: 1px solid rgba(247, 103, 7, 0.15);
}
.skill-icon-box.key-r,
.skill-order-box.key-r {
  background: rgba(240, 62, 62, 0.08);
  color: #c92a2a;
  border: 1px solid rgba(240, 62, 62, 0.15);
  font-weight: 800;
}
</style>
