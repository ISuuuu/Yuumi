<script setup lang="ts">
import { ref, watch, computed, inject, onMounted, onUnmounted, type Ref } from "vue";
import { useLcuStore } from "../store/lcuStore";
import { fetchCurrentSummoner, fetchMatchHistory, lcuRequest, fetchConfig } from "../api/lcu";
import type { SummonerDisplay, MatchDisplay } from "../api/lcu";
import LcuImage from "../components/LcuImage.vue";

const store = useLcuStore();
const summoner = ref<SummonerDisplay | null>(null);
const matches = ref<MatchDisplay[]>([]);
const recentMatches = ref<MatchDisplay[]>([]);
const rankedQueues = ref<any[]>([]);
const loading = ref(false);
const error = ref("");
const copied = ref(false);
const careerGamesNumber = ref(20); // 默认值，启动时从配置读取

// 游戏模式筛选
const selectedQueue = ref<number | null>(null);
const QUEUE_OPTIONS = [
  { id: null, label: '全部' },
  { id: 2400, label: '海克斯大乱斗' },
  { id: 450, label: '极地大乱斗' },
  { id: 430, label: '匹配模式' },
  { id: 420, label: '单双排位' },
  { id: 440, label: '灵活排位' },
];
const showQueueDropdown = ref(false);

const filteredMatches = computed(() => {
  if (selectedQueue.value === null) return matches.value;
  return matches.value.filter((m: MatchDisplay) => m.queueId === selectedQueue.value);
});

function selectQueue(id: number | null) {
  selectedQueue.value = id;
  showQueueDropdown.value = false;
}

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
        loadMatches(summoner.value.puuid),
      ]);
      // 异步加载近期统计缓存（不阻塞页面，不 await 避免覆盖翻页数据）
      loadRecentMatches(summoner.value.puuid);
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

const MATCHES_CACHE_KEY = (puuid: string) => `yuumi_matches_cache_${puuid}`;

async function loadMatches(puuid: string) {
  try {
    loading.value = true;
    const targetCount = careerGamesNumber.value;
    // endIndex 是 inclusive 的，所以传 targetCount - 1
    // 使用稳定的 /v1/matchlists/by-puuid 接口，begIndex/endIndex 分页正常工作
    matches.value = await fetchMatchHistory(puuid, 0, targetCount - 1);
  } catch (e) {
    console.error("获取战绩历史失败:", e);
  } finally {
    loading.value = false;
  }
}

async function loadRecentMatches(puuid: string) {
  try {
    const fresh = await fetchMatchHistory(puuid, 0, careerGamesNumber.value);
    updateRecentMatchesCache(puuid, fresh);
  } catch (e) {
    console.error("获取近期战绩统计失败:", e);
  }
}

function updateRecentMatchesCache(puuid: string, fresh: MatchDisplay[]) {
  let cached: MatchDisplay[] = [];
  try {
    const raw = localStorage.getItem(MATCHES_CACHE_KEY(puuid));
    if (raw) cached = JSON.parse(raw);
  } catch { /* ignore */ }

  const merged = [...fresh, ...cached]
    .filter((m, idx, arr) => arr.findIndex(x => x.gameId === m.gameId) === idx)
    .sort((a, b) => b.timeStamp - a.timeStamp)
    .slice(0, careerGamesNumber.value);

  recentMatches.value = merged;

  try {
    localStorage.setItem(MATCHES_CACHE_KEY(puuid), JSON.stringify(merged));
  } catch { /* ignore */ }
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

// 读取配置中的对局数量
onMounted(async () => {
  try {
    const cfg = await fetchConfig();
    careerGamesNumber.value = cfg.Functions?.CareerGamesNumber ?? 20;
  } catch (e) {
    console.warn("加载 CareerGamesNumber 配置失败，使用默认值 20:", e);
  }
  document.addEventListener("click", onDocClick);
});

onUnmounted(() => {
  document.removeEventListener("click", onDocClick);
});

function onDocClick() {
  showQueueDropdown.value = false;
}

// 自动加载逻辑
watch(() => store.isConnected, (connected) => {
  if (connected) {
    loadSummoner();
  } else {
    summoner.value = null;
    matches.value = [];
    recentMatches.value = [];
    rankedQueues.value = [];
  }
}, { immediate: true });

// 对局结束后自动刷新战绩
// 参考 Seraphine: 进入结算/大厅状态后等 2 秒，让 LCU 把对局数据落盘；
// 然后重试 5 次 × 3 秒，因为 lol-match-history 同步新对局通常有几秒到十几秒延迟
watch(() => store.gamePhase, async (phase: string, oldPhase: string | undefined) => {
  if (!summoner.value?.puuid) return;
  const gamePhases = ["InProgress", "GameStart", "ChampSelect", "ReadyCheck", "PreEndOfGame"];
  const endPhases = ["EndOfGame", "Lobby", "None"];
  if (gamePhases.includes(oldPhase ?? "") && endPhases.includes(phase ?? "")) {
    const puuid = summoner.value.puuid;
    const prevLatestId = recentMatches.value[0]?.gameId ?? matches.value[0]?.gameId ?? null;
    console.log(`[Career] 对局结束 (${oldPhase} → ${phase})，等待 LCU 同步并重试刷新`);

    // 第一次延迟：让结算界面走完
    await new Promise(r => setTimeout(r, 2000));

    // 最多重试 5 次，每次间隔 3 秒；只要发现新对局 ID 就提前停止
    for (let attempt = 0; attempt < 5; attempt++) {
      if (attempt > 0) {
        await new Promise(r => setTimeout(r, 3000));
      }
      try {
        await loadMatches(puuid);
        // 异步更新近期统计缓存
        loadRecentMatches(puuid);
        const latestId = matches.value[0]?.gameId ?? null;
        if (latestId && latestId !== prevLatestId) {
          console.log(`[Career] 第 ${attempt + 1} 次重试时已发现新对局 ${latestId}`);
          return;
        }
        console.log(`[Career] 第 ${attempt + 1} 次刷新：尚未发现新对局，继续等待`);
      } catch (e) {
        console.warn(`[Career] 第 ${attempt + 1} 次刷新失败:`, e);
      }
    }
    // 兜底刷新召唤师信息（段位变化等）
    await loadRankedStats(puuid);
  }
});

// 提取排位队列
const soloQueue = computed(() => {
  return rankedQueues.value.find(q => q.queueType === "RANKED_SOLO_5x5") || null;
});

const flexQueue = computed(() => {
  return rankedQueues.value.find(q => q.queueType === "RANKED_FLEX_SR") || null;
});

// 计算近期对局统计
const statsSummary = computed(() => {
  if (recentMatches.value.length === 0) return null;
  let wins = 0;
  let losses = 0;
  let kills = 0;
  let deaths = 0;
  let assists = 0;
  const champMap: Record<number, { id: number; icon: string; count: number }> = {};

  for (const m of recentMatches.value) {
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

function formatTime(ts: number): string {
  const d = new Date(ts);
  const pad = (n: number) => n.toString().padStart(2, '0');
  return `${d.getFullYear()}/${pad(d.getMonth() + 1)}/${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}
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
        <div class="profile-center">
          <div class="profile-icon-wrapper">
            <!-- 等级进度环形条（缺口在底部） -->
            <svg class="gauge-ring-svg" viewBox="0 0 100 100">
              <circle class="gauge-track" cx="50" cy="50" r="45" />
              <circle class="gauge-progress" cx="50" cy="50" r="45"
                      :style="{ '--progress': summoner.percentCompleteForNextLevel }" />
            </svg>
            <div class="avatar-container">
              <LcuImage :src="summoner.profileIconUrl" class="profile-avatar" alt="avatar" />
            </div>
            <!-- 等级数字（底部缺口处） -->
            <div class="level-badge">{{ summoner.summonerLevel }}</div>
          </div>

          <div class="summoner-info">
            <h1 class="display-name">{{ summoner.gameName || summoner.displayName }}</h1>
            <div class="copy-wrapper">
              <button class="copy-riot-id-btn" @click="copyRiotId" :title="`复制: ${summoner.gameName || summoner.displayName}#${summoner.tagLine}`">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="copy-icon">
                  <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
                  <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
                </svg>
              </button>
              <span v-if="copied" class="copied-text">✓ 已复制</span>
            </div>
            <span class="tagline"># {{ summoner.tagLine }}</span>
          </div>
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
          <span class="summary-title">近期对局（最近 {{ recentMatches.length }} 场）</span>
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
          <div class="dropdown-trigger" @click.stop="showQueueDropdown = !showQueueDropdown">
            <span>{{ QUEUE_OPTIONS.find(q => q.id === selectedQueue)?.label || '全部' }}</span>
            <svg :class="['arrow-icon', { expanded: showQueueDropdown }]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="6 9 12 15 18 9"/>
            </svg>
            <div v-if="showQueueDropdown" class="queue-dropdown-menu" @click.stop>
              <div
                v-for="q in QUEUE_OPTIONS"
                :key="q.id ?? -1"
                :class="['queue-dropdown-item', { active: selectedQueue === q.id }]"
                @click="selectQueue(q.id)"
              >
                {{ q.label }}
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- 战绩对局历史列表 -->
      <div v-if="filteredMatches.length > 0" class="match-history-list">
        <div
          v-for="m in filteredMatches"
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
              <div class="spells-col">
                <div class="spell-slot">
                  <LcuImage :src="getSpellIcon(m, 1)" class="mini-icon" alt="s1" />
                </div>
                <div class="spell-slot">
                  <LcuImage :src="getSpellIcon(m, 2)" class="mini-icon" alt="s2" />
                </div>
              </div>
              <div v-if="m.queueId !== 2400" class="rune-slot">
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
            <span class="match-time">{{ m.duration }} · {{ formatTime(m.timeStamp) }}</span>
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
  background-color: transparent;
  min-height: 100%;
}

.tip-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 8rem 2rem;
  color: var(--text-muted);
}

.offline-logo {
  font-size: 3rem;
  margin-bottom: 1rem;
}

.tip {
  font-size: 0.95rem;
  color: var(--text-dimmed);
}

.error {
  color: var(--loss-color);
  background: var(--loss-bg);
  border: 1px solid var(--loss-border);
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
  display: grid;
  grid-template-columns: 1fr auto 1fr;
  align-items: center;
  padding: 1.5rem;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  margin-bottom: 1.5rem;
  box-shadow: var(--shadow-sm);
  transition: all 0.25s ease;
}

.profile-center {
  display: flex;
  align-items: center;
  gap: 1.2rem;
  justify-content: center;
  grid-column: 2;
}

.summoner-header:hover {
  border-color: var(--primary-color-alpha-30);
  box-shadow: var(--shadow-md);
  background-color: var(--card-bg-hover);
}

.profile-icon-wrapper {
  position: relative;
  width: 104px;
  height: 104px;
  flex-shrink: 0;
}

.gauge-ring-svg {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  overflow: visible;
}

/* 环形轨道（灰色）与进度（主题色）共享参数 */
/* r=45, 周长=2π×45≈282.74, 缺口60°以容纳3位等级, 可见弧≈235.62 */
/* 旋转120°使缺口居中于底部(90°) */
.gauge-track,
.gauge-progress {
  fill: none;
  stroke-width: 5;
  stroke-linecap: round;
  stroke-dasharray: 235.62 282.74;
  transform: rotate(120deg);
  transform-origin: center;
}

.gauge-track {
  stroke: #d9d9d9;
}

.gauge-progress {
  stroke: var(--primary-color);
  stroke-dashoffset: calc(235.62px * (1 - var(--progress) / 100));
  transition: stroke-dashoffset 0.8s ease;
}

.avatar-container {
  position: absolute;
  inset: 13px;
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
  bottom: 0;
  left: 50%;
  transform: translateX(-50%);
  color: var(--text-color);
  font-size: 0.72rem;
  font-weight: 700;
  z-index: 2;
  white-space: nowrap;
  line-height: 1;
}

[data-theme="dark"] .level-badge {
  color: var(--text-color);
}

.summoner-info {
  display: grid;
  grid-template-columns: 1fr auto;
  align-items: center;
  gap: 4px 8px;
  flex: 1;
}

.display-name {
  grid-column: 1;
  grid-row: 1;
  font-size: 1.6rem;
  font-weight: 800;
  color: var(--text-color);
  margin: 0;
  text-align: center;
}

.tagline {
  grid-column: 1;
  grid-row: 2;
  font-size: 0.85rem;
  color: var(--text-muted);
  text-align: center;
}

.copy-wrapper {
  grid-column: 2;
  grid-row: 1 / 3;
  align-self: center;
  justify-self: end;
  position: relative;
  display: flex;
  align-items: center;
}

.copy-riot-id-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: 1px solid var(--border-color);
  border-radius: 6px;
  background: rgba(0, 0, 0, 0.02);
  cursor: pointer;
  color: var(--text-muted);
  transition: all 0.2s;
  padding: 0;
}

.copy-riot-id-btn:hover {
  background: rgba(0, 0, 0, 0.05);
  color: var(--text-color);
  border-color: var(--primary-color);
}

.copy-icon {
  width: 14px;
  height: 14px;
}

.copied-text {
  position: absolute;
  left: calc(100% + 6px);
  top: 50%;
  transform: translateY(-50%);
  font-size: 0.75rem;
  color: var(--win-color);
  font-weight: 600;
  white-space: nowrap;
}

.header-actions {
  display: flex;
  flex-direction: column;
  gap: 8px;
  grid-column: 3;
  justify-self: end;
}

.action-btn {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  color: var(--text-color);
  padding: 6px 16px;
  border-radius: 6px;
  font-size: 0.82rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
}

.action-btn:hover {
  background: var(--card-bg-hover);
  color: var(--text-color);
  border-color: var(--primary-color);
}

/* 排位数据表 */
.rank-table-wrapper {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  overflow: hidden;
  margin-bottom: 1.5rem;
  box-shadow: var(--shadow-sm);
  transition: all 0.25s ease;
}

.rank-table-wrapper:hover {
  background-color: var(--card-bg-hover);
  box-shadow: var(--shadow-md);
  border-color: var(--primary-color-alpha-30);
}

.rank-table {
  width: 100%;
  border-collapse: collapse;
  text-align: left;
}

.rank-table th, .rank-table td {
  padding: 12px 16px;
  font-size: 0.82rem;
  border-bottom: 1px solid var(--border-color);
}

.rank-table th {
  background-color: rgba(0, 0, 0, 0.01);
  color: var(--text-muted);
  font-weight: 600;
  border-bottom: 1.5px solid var(--border-color);
}

.rank-table tr:last-child td {
  border-bottom: none;
}

.type-name {
  font-weight: 600;
  color: var(--text-color);
}

.rank-name {
  font-weight: bold;
  color: var(--primary-color);
}

/* 近期数据概览栏 */
.recent-summary-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 16px;
  background-color: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  margin-bottom: 1rem;
  box-shadow: var(--shadow-sm);
  transition: all 0.25s ease;
  position: relative;
  z-index: 10;
}

.recent-summary-bar:hover {
  background-color: var(--card-bg-hover);
  box-shadow: var(--shadow-md);
  border-color: var(--primary-color-alpha-30);
}

.summary-text {
  font-size: 0.82rem;
  color: var(--text-muted);
  display: flex;
  align-items: center;
  gap: 8px;
}

.summary-title {
  font-weight: bold;
  color: var(--text-color);
  margin-right: 4px;
}

.win-color {
  color: var(--win-color);
  font-weight: 600;
}

.lose-color {
  color: var(--loss-color);
  font-weight: 600;
}

.kda-label {
  color: var(--text-dimmed);
  margin-left: 8px;
}

.kda-values {
  color: var(--text-color);
  font-weight: 600;
}

.death-red {
  color: var(--death-color, var(--loss-color));
}

.kda-ratio {
  color: var(--text-muted);
  font-size: 0.8rem;
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
  border: 1px solid var(--border-color);
}

.summary-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.summary-action-btn {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  color: var(--text-color);
  padding: 4px 12px;
  border-radius: 4px;
  font-size: 0.78rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
}

.summary-action-btn:hover {
  background: var(--card-bg-hover);
  border-color: var(--primary-color);
}

.dropdown-trigger {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  padding: 4px 10px;
  border-radius: 4px;
  font-size: 0.78rem;
  color: var(--text-color);
  cursor: pointer;
  position: relative;
  transition: all 0.2s;
}

.dropdown-trigger:hover {
  background: var(--card-bg-hover);
  border-color: var(--primary-color);
}

.dropdown-trigger .arrow-icon {
  width: 12px;
  height: 12px;
  transition: transform 0.2s;
}

.dropdown-trigger .arrow-icon.expanded {
  transform: rotate(180deg);
}

/* 模式筛选下拉菜单 */
.queue-dropdown-menu {
  position: absolute;
  top: calc(100% + 4px);
  right: 0;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  box-shadow: var(--shadow-lg);
  z-index: 100;
  min-width: 130px;
  padding: 4px 0;
  backdrop-filter: var(--glass-filter);
  -webkit-backdrop-filter: var(--glass-filter);
}

.queue-dropdown-item {
  padding: 6px 14px;
  font-size: 0.78rem;
  color: var(--text-muted);
  cursor: pointer;
  transition: all 0.2s;
}

.queue-dropdown-item:hover {
  background: var(--hover-bg);
  color: var(--text-color);
}

.queue-dropdown-item.active {
  color: var(--primary-color);
  font-weight: 600;
  background: var(--primary-color-alpha-15);
}

.arrow-icon {
  width: 12px;
  height: 12px;
}

/* 战绩对局历史卡片 */
.match-history-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.match-card {
  display: flex;
  align-items: center;
  padding: 14px 20px;
  border-radius: 10px;
  border: 1px solid var(--border-color);
  transition: all 0.25s cubic-bezier(0.25, 0.8, 0.25, 1);
  cursor: pointer;
}

.match-card:hover {
  box-shadow: var(--shadow-md);
}

.match-card.win {
  background-color: var(--win-bg);
  border-color: var(--win-border);
}

.match-card.win:hover {
  background-color: rgba(16, 185, 129, 0.12);
}

.match-card.lose {
  background-color: var(--loss-bg);
  border-color: var(--loss-border);
}

.match-card.lose:hover {
  background-color: rgba(239, 68, 68, 0.11);
}

/* 1. 英雄面板 */
.champ-panel {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 120px;
}

.champ-avatar-box {
  position: relative;
  width: 52px;
  height: 52px;
}

.champ-avatar {
  width: 52px;
  height: 52px;
  border-radius: 50%;
  overflow: hidden;
  border: 1.5px solid var(--border-color);
}

.level-overlay {
  position: absolute;
  bottom: -2px;
  right: -2px;
  width: 18px;
  height: 18px;
  line-height: 16px;
  background-color: var(--card-bg);
  color: var(--text-color);
  border-radius: 50%;
  font-size: 0.7rem;
  font-weight: 700;
  text-align: center;
  border: 1px solid var(--border-color);
}

[data-theme="dark"] .level-overlay {
  background-color: var(--card-bg);
  color: var(--text-color);
  border-color: rgba(255, 255, 255, 0.15);
}

.spells-runes {
  display: flex;
  gap: 3px;
  align-items: center;
}

.spells-col {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.spell-slot, .rune-slot {
  width: 22px;
  height: 22px;
  border-radius: 3px;
  overflow: hidden;
  border: 1px solid var(--border-color);
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
  min-width: 100px;
}

.result-text {
  font-size: 1rem;
  font-weight: 800;
}

.win-text {
  color: var(--win-color);
}

.lose-text {
  color: var(--loss-color);
}

.queue-mode {
  font-size: 0.8rem;
  color: var(--text-muted);
  margin-top: 2px;
}

/* 3. KDA面板 */
.kda-panel {
  display: flex;
  flex-direction: column;
  justify-content: center;
  min-width: 130px;
}

.kda-numbers {
  font-size: 1rem;
  color: var(--text-muted);
}

.bold {
  font-weight: 700;
  color: var(--text-color);
}

.kda-desc {
  margin-top: 2px;
}

.kda-ratio {
  font-size: 0.82rem;
  font-weight: 700;
}

.kda-perfect { color: #d97706; }
.kda-great { color: #db2777; }
.kda-good { color: #2563eb; }
.kda-normal { color: var(--text-dimmed); }

/* 4. 补刀面板 */
.cs-panel {
  display: flex;
  align-items: center;
  gap: 5px;
  min-width: 75px;
}

.cs-count {
  font-size: 0.9rem;
  font-weight: 700;
  color: var(--text-muted);
}

.cs-icon {
  width: 16px;
  height: 16px;
  color: var(--text-dimmed);
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
  width: 28px;
  height: 28px;
  background-color: rgba(0, 0, 0, 0.04);
  border-radius: 4px;
  overflow: hidden;
  border: 1px solid rgba(0, 0, 0, 0.05);
}

.item-img {
  width: 100%;
  height: 100%;
  display: block;
}

.ward-slot {
  border-color: rgba(245, 158, 11, 0.3);
  background-color: rgba(245, 158, 11, 0.05);
  margin-left: 2px;
}

/* 6. 金币面板 */
.gold-panel {
  display: flex;
  align-items: center;
  gap: 5px;
  min-width: 85px;
  justify-content: flex-end;
}

.gold-count {
  font-size: 0.9rem;
  font-weight: 700;
  color: var(--text-muted);
}

.gold-icon {
  width: 14px;
  height: 14px;
  color: #fbbf24;
}

/* 7. 时间面板 */
.time-panel {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  min-width: 170px;
  font-size: 0.82rem;
  color: var(--text-dimmed);
}

.map-name {
  font-weight: 600;
  color: var(--text-muted);
}

.match-time {
  margin-top: 4px;
}

@keyframes fadeIn {
  from { opacity: 0; transform: translateY(6px); }
  to { opacity: 1; transform: translateY(0); }
}
</style>
