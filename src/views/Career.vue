<script setup lang="ts">
import { ref, watch, computed, inject, type Ref } from "vue";
import { useLcuStore } from "../store/lcuStore";
import { fetchCurrentSummoner, fetchMatchHistory, lcuRequest } from "../api/lcu";
import type { SummonerDisplay, MatchDisplay } from "../api/lcu";
import LcuImage from "../components/LcuImage.vue";

const store = useLcuStore();
const summoner = ref<SummonerDisplay | null>(null);
const matches = ref<MatchDisplay[]>([]);
const rankedQueues = ref<any[]>([]);
const loading = ref(false);
const error = ref("");
const copied = ref(false);

// 从 App.vue 注入 Career → Search 跳转状态
const navigateSearchPayload = inject<Ref<{ name: string; gameId: number | null } | null>>("navigateSearchPayload")!;

const TIER_MAP: Record<string, string> = {
  NONE: "无段位",
  IRON: "坚韧黑铁",
  BRONZE: "英勇黄铜",
  SILVER: "不屈白银",
  GOLD: "荣耀黄金",
  PLATINUM: "华贵铂金",
  EMERALD: "流光翡翠",
  DIAMOND: "璀璨钻石",
  MASTER: "超凡大师",
  GRANDMASTER: "傲世宗师",
  CHALLENGER: "最强王者",
};

async function loadSummoner() {
  loading.value = true;
  error.value = "";
  try {
    summoner.value = await fetchCurrentSummoner();
    if (summoner.value?.puuid) {
      await Promise.all([
        loadRankedStats(summoner.value.puuid),
        loadMatches(summoner.value.puuid)
      ]);
    }
  } catch (e: any) {
    error.value = e.toString();
  } finally {
    loading.value = false;
  }
}

async function loadRankedStats(puuid: string) {
  try {
    const resp = await lcuRequest<any>("GET", `/lol-ranked/v1/ranked-stats/${puuid}`);
    if (resp.success && resp.data && resp.data.queues) {
      rankedQueues.value = resp.data.queues;
    }
  } catch (e) {
    console.error("获取排位段位数据失败:", e);
  }
}

async function loadMatches(puuid: string) {
  try {
    matches.value = await fetchMatchHistory(puuid, 0, 20);
  } catch (e) {
    console.error("获取战绩历史失败:", e);
  }
}

function getKdaClass(kda: string): string {
  const val = parseFloat(kda);
  if (isNaN(val)) return "kda-perfect";
  if (val >= 5) return "kda-great";
  if (val >= 3) return "kda-good";
  return "kda-normal";
}

function formatRank(queue: any) {
  if (!queue || !queue.tier || queue.tier === "NONE") return "--";
  const tierCn = TIER_MAP[queue.tier] || queue.tier;
  const division = queue.rank === "NA" ? "" : " " + queue.rank;
  return `${tierCn}${division}`;
}

function formatHighestRank(queue: any) {
  if (!queue || !queue.highestTier || queue.highestTier === "NONE") return "--";
  return TIER_MAP[queue.highestTier] || queue.highestTier;
}

function formatPrevSeasonRank(queue: any) {
  if (!queue || !queue.previousSeasonEndTier || queue.previousSeasonEndTier === "NONE") return "--";
  return TIER_MAP[queue.previousSeasonEndTier] || queue.previousSeasonEndTier;
}

// 复制完整 Riot ID
async function copyRiotId() {
  if (!summoner.value) return;
  const fullId = `${summoner.value.gameName || summoner.value.displayName}#${summoner.value.tagLine}`;
  try {
    await navigator.clipboard.writeText(fullId);
    copied.value = true;
    setTimeout(() => { copied.value = false; }, 1500);
  } catch {
    // fallback
    const ta = document.createElement("textarea");
    ta.value = fullId;
    document.body.appendChild(ta);
    ta.select();
    document.execCommand("copy");
    document.body.removeChild(ta);
    copied.value = true;
    setTimeout(() => { copied.value = false; }, 1500);
  }
}

// 点击对局卡片 → 跳转到战绩查询页面并自动搜索
function goToMatchDetail(gameId: number) {
  if (!summoner.value) return;
  const name = summoner.value.gameName || summoner.value.displayName;
  const fullName = summoner.value.tagLine
    ? `${name}#${summoner.value.tagLine}`
    : name;
  navigateSearchPayload.value = { name: fullName, gameId };
}

// 点击历史战绩 → 跳转到战绩查询页面并搜索当前召唤师
function goToHistory() {
  if (!summoner.value) return;
  const name = summoner.value.gameName || summoner.value.displayName;
  const fullName = summoner.value.tagLine
    ? `${name}#${summoner.value.tagLine}`
    : name;
  // gameId 设为 -1 表示只搜索不指定对局
  navigateSearchPayload.value = { name: fullName, gameId: -1 };
}

// 召唤师技能图标：直接使用后端解析的 URL（后端已含完整 LCU 路径）
function getSpellIcon(m: MatchDisplay, slot: 1 | 2): string {
  return slot === 1 ? m.spell1IconUrl : m.spell2IconUrl;
}

// 自动加载逻辑
watch(() => store.isConnected, (connected) => {
  if (connected) {
    loadSummoner();
  } else {
    summoner.value = null;
    matches.value = [];
    rankedQueues.value = [];
  }
}, { immediate: true });

// 提取排位队列
const soloQueue = computed(() => {
  return rankedQueues.value.find(q => q.queueType === "RANKED_SOLO_5x5") || null;
});

const flexQueue = computed(() => {
  return rankedQueues.value.find(q => q.queueType === "RANKED_FLEX_SR") || null;
});

// 计算近期 20 场对局统计
const statsSummary = computed(() => {
  if (matches.value.length === 0) return null;
  let wins = 0;
  let losses = 0;
  let kills = 0;
  let deaths = 0;
  let assists = 0;
  const champMap: Record<number, { id: number; icon: string; count: number }> = {};

  for (const m of matches.value) {
    if (m.win) wins++; else losses++;
    kills += m.kills;
    deaths += m.deaths;
    assists += m.assists;

    if (!champMap[m.championId]) {
      champMap[m.championId] = { id: m.championId, icon: m.championIconUrl, count: 0 };
    }
    champMap[m.championId].count++;
  }

  const topChamps = Object.values(champMap)
    .sort((a, b) => b.count - a.count)
    .slice(0, 6);

  const kdaRatio = deaths === 0 ? "Perfect" : ((kills + assists) / deaths).toFixed(1);

  return {
    wins,
    losses,
    kills,
    deaths,
    assists,
    kda: kdaRatio,
    topChamps
  };
});
</script>

<template>
  <div class="career">
    <div v-if="!store.isConnected" class="tip-container">
      <div class="offline-logo">🎮</div>
      <p class="tip">请先启动英雄联盟客户端</p>
    </div>

    <div v-else class="career-content">
      <div v-if="error" class="error">{{ error }}</div>

      <!-- 召唤师信息卡片 -->
      <div v-if="summoner" class="summoner-header">
        <div class="profile-icon-wrapper">
          <!-- 外层进度条圆环 -->
          <div class="gauge-ring" :style="{ '--progress': summoner.percentCompleteForNextLevel }"></div>
          <div class="avatar-container">
            <LcuImage :src="summoner.profileIconUrl" class="profile-avatar" alt="avatar" />
          </div>
          <!-- 等级角标 -->
          <div class="level-badge">{{ summoner.summonerLevel }}</div>
        </div>

        <div class="summoner-info">
          <div class="name-row">
            <h1 class="display-name">{{ summoner.gameName || summoner.displayName }}</h1>
            <button class="copy-riot-id-btn" @click="copyRiotId" :title="`复制: ${summoner.gameName || summoner.displayName}#${summoner.tagLine}`">
              <span v-if="copied" class="copied-text">✓ 已复制</span>
              <svg v-else viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="copy-icon">
                <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
                <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
              </svg>
            </button>
          </div>
          <span class="tagline"># {{ summoner.tagLine }}</span>
        </div>

        <div class="header-actions">
          <button class="action-btn" @click="loadSummoner" :disabled="loading">刷新</button>
          <button class="action-btn" @click="goToHistory" :disabled="loading">历史战绩</button>
        </div>
      </div>

      <!-- 排位段位信息表 -->
      <div class="rank-table-wrapper">
        <table class="rank-table">
          <thead>
            <tr>
              <th>类型</th>
              <th>总场次</th>
              <th>胜率</th>
              <th>胜场</th>
              <th>负场</th>
              <th>段位</th>
              <th>胜点</th>
              <th>赛季最高</th>
              <th>上赛季结算</th>
            </tr>
          </thead>
          <tbody>
            <!-- 单双排 -->
            <tr>
              <td class="type-name">单 / 双排</td>
              <td>{{ soloQueue ? soloQueue.wins + soloQueue.losses : 0 }}</td>
              <td>
                {{ soloQueue && soloQueue.wins + soloQueue.losses > 0 
                  ? ((soloQueue.wins / (soloQueue.wins + soloQueue.losses)) * 100).toFixed(0) + '%' 
                  : '--' 
                }}
              </td>
              <td>{{ soloQueue ? soloQueue.wins : 0 }}</td>
              <td>{{ soloQueue ? soloQueue.losses : 0 }}</td>
              <td class="rank-name">{{ soloQueue ? formatRank(soloQueue) : '--' }}</td>
              <td>{{ soloQueue ? soloQueue.leaguePoints : 0 }}</td>
              <td>{{ soloQueue ? formatHighestRank(soloQueue) : '--' }}</td>
              <td>{{ soloQueue ? formatPrevSeasonRank(soloQueue) : '--' }}</td>
            </tr>
            <!-- 灵活排位 -->
            <tr>
              <td class="type-name">灵活排位</td>
              <td>{{ flexQueue ? flexQueue.wins + flexQueue.losses : 0 }}</td>
              <td>
                {{ flexQueue && flexQueue.wins + flexQueue.losses > 0 
                  ? ((flexQueue.wins / (flexQueue.wins + flexQueue.losses)) * 100).toFixed(0) + '%' 
                  : '--' 
                }}
              </td>
              <td>{{ flexQueue ? flexQueue.wins : 0 }}</td>
              <td>{{ flexQueue ? flexQueue.losses : 0 }}</td>
              <td class="rank-name">{{ flexQueue ? formatRank(flexQueue) : '--' }}</td>
              <td>{{ flexQueue ? flexQueue.leaguePoints : 0 }}</td>
              <td>{{ flexQueue ? formatHighestRank(flexQueue) : '--' }}</td>
              <td>{{ flexQueue ? formatPrevSeasonRank(flexQueue) : '--' }}</td>
            </tr>
          </tbody>
        </table>
      </div>

      <!-- 近期数据看板 & 常用英雄 -->
      <div v-if="statsSummary" class="recent-summary-bar">
        <div class="summary-text">
          <span class="summary-title">近期对局（最近 20 场）</span>
          <span class="win-color">胜: {{ statsSummary.wins }}</span>
          <span class="lose-color">负: {{ statsSummary.losses }}</span>
          <span class="kda-label">KDA:</span>
          <span class="kda-values">
            {{ statsSummary.kills }} / <span class="death-red">{{ statsSummary.deaths }}</span> / {{ statsSummary.assists }}
          </span>
          <span class="kda-ratio">({{ statsSummary.kda }})</span>
        </div>

        <div class="recent-champs">
          <div v-for="c in statsSummary.topChamps" :key="c.id" class="recent-champ-icon" :title="`近期使用 ${c.count} 场`">
            <LcuImage :src="c.icon" alt="champ" />
          </div>
        </div>

        <div class="summary-actions">
          <button class="summary-action-btn">最近队友</button>
          <div class="dropdown-trigger">
            <span>全部</span>
            <svg class="arrow-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
          </div>
        </div>
      </div>

      <!-- 战绩对局历史列表 -->
      <div v-if="matches.length > 0" class="match-history-list">
        <div
          v-for="m in matches"
          :key="m.gameId"
          :class="['match-card', m.win ? 'win' : 'lose']"
          @click="goToMatchDetail(m.gameId)"
          style="cursor: pointer;"
        >
          <!-- 1. 英雄头像、等级、技能、符文 -->
          <div class="champ-panel">
            <div class="champ-avatar-box">
              <LcuImage :src="m.championIconUrl" class="champ-avatar" alt="champ" />
              <div class="level-overlay">{{ m.champLevel }}</div>
            </div>
            <div class="spells-runes">
              <div class="spell-slot">
                <LcuImage :src="getSpellIcon(m, 1)" class="mini-icon" alt="s1" />
              </div>
              <div class="spell-slot">
                <LcuImage :src="getSpellIcon(m, 2)" class="mini-icon" alt="s2" />
              </div>
              <div class="rune-slot">
                <LcuImage :src="m.runeIconUrl" class="mini-icon circular" alt="rune" />
              </div>
            </div>
          </div>

          <!-- 2. 胜负状态与游戏模式 -->
          <div class="result-panel">
            <span :class="['result-text', m.win ? 'win-text' : 'lose-text']">
              {{ m.win ? '胜利' : '失败' }}
            </span>
            <span class="queue-mode">{{ m.name }}</span>
          </div>

          <!-- 3. KDA 数字与文字 -->
          <div class="kda-panel">
            <div class="kda-numbers">
              <span class="bold">{{ m.kills }}</span> / 
              <span class="bold death-red">{{ m.deaths }}</span> / 
              <span class="bold">{{ m.assists }}</span>
            </div>
            <div class="kda-desc">
              <span class="kda-ratio" :class="getKdaClass(m.kda)">{{ m.kda }} KDA</span>
            </div>
          </div>

          <!-- 4. 补刀补兵数 -->
          <div class="cs-panel">
            <span class="cs-count">{{ m.cs }}</span>
            <svg class="cs-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          </div>

          <!-- 5. 装备栏 (前 6 件常规装备 + 第 7 件饰品) -->
          <div class="items-panel">
            <div class="items-grid">
              <div v-for="idx in 6" :key="idx" class="item-slot">
                <LcuImage v-if="m.itemIconUrls[idx-1]" :src="m.itemIconUrls[idx-1]" class="item-img" alt="item" />
              </div>
            </div>
            <!-- 饰品独立显示 -->
            <div class="ward-slot">
              <LcuImage v-if="m.itemIconUrls[6]" :src="m.itemIconUrls[6]" class="item-img" alt="ward" />
            </div>
          </div>

          <!-- 6. 获得金币 -->
          <div class="gold-panel">
            <span class="gold-count">{{ m.gold.toLocaleString() }}</span>
            <svg class="gold-icon" viewBox="0 0 24 24" fill="currentColor">
              <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="2" fill="none"/>
              <path d="M12 6v12M15 9H11.5a1.5 1.5 0 0 0 0 3h1a1.5 1.5 0 0 1 0 3H9" stroke="currentColor" stroke-width="2" stroke-linecap="round" fill="none"/>
            </svg>
          </div>

          <!-- 7. 地图模式与时长/日期 -->
          <div class="time-panel">
            <span class="map-name">{{ m.map }}</span>
            <span class="match-time">{{ m.duration }} · {{ m.shortTime }}</span>
          </div>
        </div>
      </div>
      <div v-else-if="!loading" class="tip-container">
        <p class="tip">暂无战绩记录</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.career {
  padding: 1.5rem 1.5rem 1.5rem 0.6rem;
  background-color: #fafbfc;
  min-height: 100%;
}

.tip-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 8rem 2rem;
  color: #909399;
}

.offline-logo {
  font-size: 3rem;
  margin-bottom: 1rem;
}

.tip {
  font-size: 1rem;
  color: #8c8c8c;
}

.error {
  color: #f56c6c;
  background: #fef0f0;
  border: 1px solid #fde2e2;
  padding: 10px 16px;
  border-radius: 6px;
  margin-bottom: 1rem;
}

.career-content {
  max-width: 1000px;
  margin: 0 auto;
  animation: fadeIn 0.3s ease-out;
}

/* 召唤师头部卡片 */
.summoner-header {
  display: flex;
  align-items: center;
  gap: 1.2rem;
  padding: 1.5rem;
  background: white;
  border: 1px solid #ebeef5;
  border-radius: 12px;
  margin-bottom: 1.5rem;
  box-shadow: 0 2px 8px rgba(0,0,0,0.02);
}

.profile-icon-wrapper {
  position: relative;
  width: 80px;
  height: 80px;
  flex-shrink: 0;
}

.gauge-ring {
  position: absolute;
  inset: 0;
  border-radius: 50%;
  background: radial-gradient(circle, white 33px, transparent 34px),
              conic-gradient(#2ecc71 calc(var(--progress) * 1%), #e2e5e9 0);
}

.avatar-container {
  position: absolute;
  inset: 5px;
  border-radius: 50%;
  overflow: hidden;
}

.profile-avatar {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.level-badge {
  position: absolute;
  bottom: -4px;
  left: 50%;
  transform: translateX(-50%);
  background-color: #2ecc71;
  color: white;
  font-size: 0.72rem;
  font-weight: bold;
  padding: 1px 8px;
  border-radius: 10px;
  box-shadow: 0 1px 3px rgba(0,0,0,0.2);
  z-index: 1;
  white-space: nowrap;
}

.summoner-info {
  display: flex;
  flex-direction: column;
  flex: 1;
}

.name-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.display-name {
  font-size: 1.6rem;
  font-weight: 800;
  color: #303133;
  margin: 0;
}

.tagline {
  font-size: 0.9rem;
  color: #909399;
  margin-top: 4px;
}

.copy-riot-id-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: 1px solid #dcdfe6;
  border-radius: 6px;
  background: white;
  cursor: pointer;
  color: #909399;
  transition: all 0.2s;
  flex-shrink: 0;
  padding: 0;
}

.copy-riot-id-btn:hover {
  background: #f5f7fa;
  color: #606266;
  border-color: #c0c4cc;
}

.copy-icon {
  width: 14px;
  height: 14px;
}

.copied-text {
  font-size: 0.65rem;
  color: #2ecc71;
  font-weight: 600;
  white-space: nowrap;
}

.header-actions {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-left: auto;
  flex-shrink: 0;
}

.action-btn {
  background: white;
  border: 1px solid #dcdfe6;
  color: #606266;
  padding: 6px 16px;
  border-radius: 6px;
  font-size: 0.85rem;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.action-btn:hover {
  background: #f5f7fa;
  color: #1a73e8;
  border-color: #c0c4cc;
}

/* 排位数据表 */
.rank-table-wrapper {
  background: white;
  border: 1px solid #ebeef5;
  border-radius: 12px;
  overflow: hidden;
  margin-bottom: 1.5rem;
  box-shadow: 0 2px 8px rgba(0,0,0,0.02);
}

.rank-table {
  width: 100%;
  border-collapse: collapse;
  text-align: left;
}

.rank-table th, .rank-table td {
  padding: 12px 16px;
  font-size: 0.88rem;
  border-bottom: 1px solid #f0f0f0;
}

.rank-table th {
  background-color: #fdfdfd;
  color: #909399;
  font-weight: 500;
  border-bottom: 1.5px solid #ebeef5;
}

.rank-table tr:last-child td {
  border-bottom: none;
}

.type-name {
  font-weight: 500;
  color: #606266;
}

.rank-name {
  font-weight: bold;
  color: #303133;
}

/* 近期数据概览栏 */
.recent-summary-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 16px;
  background-color: white;
  border: 1px solid #ebeef5;
  border-radius: 8px;
  margin-bottom: 1rem;
  box-shadow: 0 2px 6px rgba(0,0,0,0.01);
}

.summary-text {
  font-size: 0.88rem;
  color: #606266;
  display: flex;
  align-items: center;
  gap: 8px;
}

.summary-title {
  font-weight: bold;
  color: #303133;
  margin-right: 4px;
}

.win-color {
  color: #2ecc71;
  font-weight: 500;
}

.lose-color {
  color: #e74c3c;
  font-weight: 500;
}

.kda-label {
  color: #909399;
  margin-left: 8px;
}

.kda-values {
  color: #303133;
  font-weight: 500;
}

.death-red {
  color: #e74c3c;
}

.kda-ratio {
  color: #909399;
  font-size: 0.82rem;
}

.recent-champs {
  display: flex;
  gap: 4px;
}

.recent-champ-icon {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  overflow: hidden;
  border: 1px solid #e1e4e8;
}

.summary-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.summary-action-btn {
  background: white;
  border: 1px solid #dcdfe6;
  color: #606266;
  padding: 4px 12px;
  border-radius: 4px;
  font-size: 0.8rem;
  font-weight: 500;
  cursor: pointer;
}

.summary-action-btn:hover {
  background: #f5f7fa;
}

.dropdown-trigger {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background: white;
  border: 1px solid #dcdfe6;
  padding: 4px 10px;
  border-radius: 4px;
  font-size: 0.8rem;
  color: #606266;
  cursor: pointer;
}

.arrow-icon {
  width: 12px;
  height: 12px;
}

/* 战绩对局历史卡片 */
.match-history-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.match-card {
  display: flex;
  align-items: center;
  padding: 10px 16px;
  border-radius: 8px;
  border-left: 4px solid;
  transition: transform 0.15s, box-shadow 0.2s;
  cursor: pointer;
}

.match-card:hover {
  transform: translateY(-1px);
  box-shadow: 0 2px 8px rgba(0,0,0,0.06);
}

.match-card.win {
  background-color: #f0f9eb;
  border-color: #2ecc71;
  border-top: 1px solid #e1f3d8;
  border-right: 1px solid #e1f3d8;
  border-bottom: 1px solid #e1f3d8;
}

.match-card.lose {
  background-color: #fef0f0;
  border-color: #f56c6c;
  border-top: 1px solid #fde2e2;
  border-right: 1px solid #fde2e2;
  border-bottom: 1px solid #fde2e2;
}

/* 1. 英雄面板 */
.champ-panel {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 100px;
}

.champ-avatar-box {
  position: relative;
  width: 44px;
  height: 44px;
}

.champ-avatar {
  width: 44px;
  height: 44px;
  border-radius: 50%;
  overflow: hidden;
  border: 1px solid #dcdfe6;
}

.level-overlay {
  position: absolute;
  bottom: -2px;
  right: -2px;
  width: 16px;
  height: 16px;
  line-height: 14px;
  background-color: #202124;
  color: white;
  border-radius: 50%;
  font-size: 0.65rem;
  font-weight: 700;
  text-align: center;
  border: 1px solid #fff;
}

.spells-runes {
  display: flex;
  gap: 2px;
}

.spell-slot, .rune-slot {
  width: 18px;
  height: 18px;
  border-radius: 2px;
  overflow: hidden;
  border: 1px solid #eee;
}

.mini-icon {
  width: 100%;
  height: 100%;
  display: block;
}

.mini-icon.circular {
  border-radius: 50%;
}

/* 2. 胜负面板 */
.result-panel {
  display: flex;
  flex-direction: column;
  justify-content: center;
  min-width: 90px;
}

.result-text {
  font-size: 0.95rem;
  font-weight: 800;
}

.win-text {
  color: #2ecc71;
}

.lose-text {
  color: #f56c6c;
}

.queue-mode {
  font-size: 0.78rem;
  color: #909399;
  margin-top: 2px;
}

/* 3. KDA面板 */
.kda-panel {
  display: flex;
  flex-direction: column;
  justify-content: center;
  min-width: 110px;
}

.kda-numbers {
  font-size: 0.95rem;
  color: #606266;
}

.bold {
  font-weight: bold;
  color: #303133;
}

.kda-desc {
  margin-top: 2px;
}

.kda-ratio {
  font-size: 0.78rem;
  font-weight: 600;
}

.kda-perfect { color: #ff9800; }
.kda-great { color: #e91e63; }
.kda-good { color: #2196f3; }
.kda-normal { color: #909399; }

/* 4. 补刀面板 */
.cs-panel {
  display: flex;
  align-items: center;
  gap: 4px;
  min-width: 70px;
}

.cs-count {
  font-size: 0.88rem;
  font-weight: bold;
  color: #606266;
}

.cs-icon {
  width: 16px;
  height: 16px;
  color: #909399;
}

/* 5. 装备面板 */
.items-panel {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: 1;
}

.items-grid {
  display: flex;
  gap: 2px;
}

.item-slot, .ward-slot {
  width: 24px;
  height: 24px;
  background-color: rgba(0, 0, 0, 0.05);
  border-radius: 4px;
  overflow: hidden;
  border: 1px solid rgba(0,0,0,0.08);
}

.item-img {
  width: 100%;
  height: 100%;
  display: block;
}

.ward-slot {
  border-color: #e6a23c;
  background-color: rgba(230, 162, 60, 0.05);
  margin-left: 2px;
}

/* 6. 金币面板 */
.gold-panel {
  display: flex;
  align-items: center;
  gap: 4px;
  min-width: 80px;
  justify-content: flex-end;
}

.gold-count {
  font-size: 0.88rem;
  font-weight: bold;
  color: #606266;
}

.gold-icon {
  width: 14px;
  height: 14px;
  color: #e6a23c;
}

/* 7. 时间面板 */
.time-panel {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  min-width: 160px;
  font-size: 0.78rem;
  color: #909399;
}

.map-name {
  font-weight: 500;
  color: #606266;
}

.match-time {
  margin-top: 4px;
}

@keyframes fadeIn {
  from { opacity: 0; transform: translateY(6px); }
  to { opacity: 1; transform: translateY(0); }
}
</style>
