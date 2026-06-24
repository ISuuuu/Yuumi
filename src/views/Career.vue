<script setup lang="ts">
import { ref } from "vue";
import { useLcuStore } from "../store/lcuStore";
import { fetchCurrentSummoner, fetchMatchHistory } from "../api/lcu";
import type { SummonerDisplay, MatchDisplay } from "../api/lcu";
import LcuImage from "../components/LcuImage.vue";

const store = useLcuStore();
const summoner = ref<SummonerDisplay | null>(null);
const matches = ref<MatchDisplay[]>([]);
const loading = ref(false);
const error = ref("");

async function loadSummoner() {
  loading.value = true;
  error.value = "";
  try {
    summoner.value = await fetchCurrentSummoner();
  } catch (e: any) {
    error.value = e.toString();
  } finally {
    loading.value = false;
  }
}

async function loadMatches() {
  if (!summoner.value) {
    await loadSummoner();
  }
  if (!summoner.value) return;

  loading.value = true;
  error.value = "";
  try {
    matches.value = await fetchMatchHistory(summoner.value.puuid, 0, 20);
  } catch (e: any) {
    error.value = e.toString();
  } finally {
    loading.value = false;
  }
}

function getKdaClass(kda: string): string {
  const val = parseFloat(kda);
  if (isNaN(val)) return "kda-perfect";
  if (val >= 5) return "kda-great";
  if (val >= 3) return "kda-good";
  return "kda-normal";
}
</script>

<template>
  <div class="career">
    <h2>生涯战绩</h2>

    <div v-if="!store.isConnected" class="tip">
      请先启动英雄联盟客户端
    </div>

    <div v-else>
      <div class="actions">
        <button @click="loadSummoner" :disabled="loading">加载召唤师</button>
        <button @click="loadMatches" :disabled="loading">加载战绩</button>
      </div>

      <div v-if="error" class="error">{{ error }}</div>

      <!-- 召唤师信息 -->
      <div v-if="summoner" class="summoner-card">
        <LcuImage :src="summoner.profileIconUrl" class="icon" alt="icon" />
        <div class="info">
          <h3>{{ summoner.displayName }}</h3>
          <span class="tag">{{ summoner.gameName }}#{{ summoner.tagLine }}</span>
          <span class="level">Lv.{{ summoner.summonerLevel }}</span>
        </div>
      </div>

      <!-- 战绩列表 -->
      <div v-if="matches.length > 0" class="match-list">
        <div
          v-for="m in matches"
          :key="m.gameId"
          :class="['match-card', { win: m.win, lose: !m.win, remake: m.remake }]"
        >
          <div class="match-left">
            <span class="mode">{{ m.name }}</span>
            <span class="time">{{ m.shortTime }}</span>
            <span class="duration">{{ m.duration }}</span>
          </div>
          <div class="match-center">
            <div class="champ-col">
              <LcuImage :src="m.championIconUrl" class="champ-icon" alt="champ" />
              <LcuImage :src="m.runeIconUrl" class="rune-icon" alt="rune" />
            </div>
            <div class="spells">
              <LcuImage :src="m.spell1IconUrl" class="spell-icon" alt="s1" />
              <LcuImage :src="m.spell2IconUrl" class="spell-icon" alt="s2" />
            </div>
          </div>
          <div class="match-stats">
            <span class="kda" :class="getKdaClass(m.kda)">
              {{ m.kills }}/{{ m.deaths }}/{{ m.assists }}
            </span>
            <span class="kda-ratio">{{ m.kda }} KDA</span>
            <span class="cs">{{ m.cs }} CS</span>
            <span class="gold">{{ (m.gold / 1000).toFixed(1) }}k 金</span>
          </div>
          <div class="match-items">
            <LcuImage
              v-for="(icon, idx) in m.itemIconUrls"
              :key="idx"
              :src="icon"
              class="item-icon"
              alt="item"
            />
          </div>
          <div class="match-result">
            {{ m.remake ? "重开" : m.win ? "胜利" : "失败" }}
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.career { padding: 1rem; }
.tip { color: #888; margin-top: 2rem; text-align: center; }
.actions { display: flex; gap: 8px; margin-bottom: 1rem; }
.error { color: #e74c3c; margin-bottom: 1rem; }

.summoner-card {
  display: flex; align-items: center; gap: 12px;
  padding: 12px; border-radius: 8px; background: #f5f5f5; margin-bottom: 1rem;
}
.icon { width: 64px; height: 64px; border-radius: 50%; }
.info h3 { margin: 0; }
.tag { color: #888; font-size: 0.85rem; }
.level { margin-left: 8px; color: #666; font-size: 0.85rem; }

.match-list { display: flex; flex-direction: column; gap: 6px; }
.match-card {
  display: flex; align-items: center; gap: 12px;
  padding: 10px 12px; border-radius: 6px; border-left: 4px solid;
}
.match-card.win { background: #e8f5e9; border-color: #4caf50; }
.match-card.lose { background: #fbe9e7; border-color: #f44336; }
.match-card.remake { background: #f5f5f5; border-color: #9e9e9e; }

.match-left { display: flex; flex-direction: column; min-width: 80px; font-size: 0.8rem; color: #666; }
.match-center { display: flex; align-items: center; gap: 4px; }
.champ-col { display: flex; flex-direction: column; align-items: center; gap: 2px; }
.champ-icon { width: 40px; height: 40px; border-radius: 4px; }
.rune-icon { width: 20px; height: 20px; border-radius: 50%; }
.spells { display: flex; flex-direction: column; gap: 2px; }
.spell-icon { width: 16px; height: 16px; border-radius: 2px; }
.match-stats { display: flex; flex-direction: column; font-size: 0.85rem; }
.match-items { display: grid; grid-template-columns: repeat(4, 1fr); gap: 2px; max-width: 88px; }
.item-icon { width: 20px; height: 20px; border-radius: 2px; }
.kda-perfect { color: #ff9800; }
.kda-great { color: #e91e63; }
.kda-good { color: #2196f3; }
.kda-normal { color: #666; }
.kda-ratio { font-size: 0.75rem; color: #888; }
.cs, .gold { font-size: 0.75rem; color: #888; }
.match-result { margin-left: auto; font-weight: bold; font-size: 0.9rem; }
</style>
