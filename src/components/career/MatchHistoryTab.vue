<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted, inject, type Ref } from "vue";
import { useLcuStore } from "../../store/lcuStore";
import { useI18n } from "vue-i18n";
import { fetchRecentTeammates, fetchConfig } from "../../api/lcu";
import type { SummonerDisplay, MatchDisplay, RecentTeammate } from "../../api/lcu";
import LcuImage from "../LcuImage.vue";
import { NPopover, NSpin } from "naive-ui";

const store = useLcuStore();
const { t, te } = useI18n();

// 从 Career.vue inject 共享的 composable 状态（单一实例）
const mh = inject<{
  summoner: Ref<SummonerDisplay | null>;
  matches: Ref<MatchDisplay[]>;
  recentMatches: Ref<MatchDisplay[]>;
  rankedQueues: Ref<any[]>;
  loading: Ref<boolean>;
  loadSummoner: (force?: boolean) => Promise<void>;
  loadMatches: (puuid: string, sync?: boolean) => Promise<void>;
  loadRecentMatches: (puuid: string, sync?: boolean) => Promise<void>;
  loadRankedStats: (puuid: string) => Promise<void>;
  clearCache: () => void;
  fetchMatchHistoryWithFallback: (puuid: string, beg: number, end: number, sync?: boolean) => Promise<MatchDisplay[]>;
}>("matchHistoryState")!;

const { summoner, matches, recentMatches, rankedQueues, loading } = mh;

// ─── 本地 UI 状态（仅本组件使用）───
const careerGamesNumber = ref(20);
const selectedQueue = ref<number | null>(null);
const showQueueDropdown = ref(false);
const loadingTeammates = ref(false);
const recentTeammates = ref<RecentTeammate[]>([]);
let currentTeammatePuuid = "";

const QUEUE_OPTIONS = [
  { id: null, label: "全部" },
  { id: 2400, label: "海克斯大乱斗" },
  { id: 450, label: "极地大乱斗" },
  { id: 430, label: "匹配模式" },
  { id: 420, label: "单双排位" },
  { id: 440, label: "灵活排位" },
];

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

// ─── 计算属性 ───
const filteredMatches = computed(() => {
  if (selectedQueue.value === null) return matches.value;
  return matches.value.filter((m: MatchDisplay) => m.queueId === selectedQueue.value);
});

const soloQueue = computed(() =>
  rankedQueues.value.find((q) => q.queueType === "RANKED_SOLO_5x5") || null,
);

const flexQueue = computed(() =>
  rankedQueues.value.find((q) => q.queueType === "RANKED_FLEX_SR") || null,
);

const statsSummary = computed(() => {
  if (recentMatches.value.length === 0) return null;
  let wins = 0, losses = 0, kills = 0, deaths = 0, assists = 0;
  const champMap: Record<number, { id: number; icon: string; count: number }> = {};

  for (const m of recentMatches.value) {
    if (m.win) wins++;
    else losses++;
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
  return { wins, losses, kills, deaths, assists, kda: kdaRatio, topChamps };
});

// ─── 辅助函数 ───
function selectQueue(id: number | null) {
  selectedQueue.value = id;
  showQueueDropdown.value = false;
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

function getSpellIcon(m: MatchDisplay, slot: 1 | 2): string {
  return slot === 1 ? m.spell1IconUrl : m.spell2IconUrl;
}

function formatTime(ts: number): string {
  const d = new Date(ts);
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${d.getFullYear()}/${pad(d.getMonth() + 1)}/${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function translateMapName(name: string): string {
  if (!name) return "";
  if (name.includes("峡谷") || name.includes("Rift")) return t("maps.11");
  if (name.includes("深渊") || name.includes("Abyss")) return t("maps.12");
  if (name.includes("闪击") || name.includes("Blitz")) return t("maps.21");
  if (name.includes("大厅") || name.includes("Lobby")) return t("maps.22");
  return name;
}

function getQueueName(m: MatchDisplay): string {
  const key = `gameModes.${m.queueId}`;
  if (te(key)) {
    const translation = t(key);
    if (
      (translation.includes("云顶") || translation.includes("TFT")) &&
      !m.name.includes("云顶") && !m.name.includes("TFT")
    ) {
      return m.name;
    }
    return translation;
  }
  return m.name;
}

function getKdaClass(kda: string): string {
  const val = parseFloat(kda);
  if (isNaN(val)) return "kda-perfect";
  if (val >= 5) return "kda-great";
  if (val >= 3) return "kda-good";
  return "kda-normal";
}

async function calculateRecentTeammates() {
  if (!summoner.value?.puuid || recentMatches.value.length === 0) {
    recentTeammates.value = [];
    return;
  }
  const targetPuuid = summoner.value.puuid;
  currentTeammatePuuid = targetPuuid;
  loadingTeammates.value = true;
  try {
    const gameIds = recentMatches.value.map((m) => m.gameId);
    const resp = await fetchRecentTeammates(gameIds, targetPuuid);
    if (currentTeammatePuuid === targetPuuid) {
      recentTeammates.value = resp.summoners || [];
    }
  } catch (err) {
    console.error("计算最近队友失败:", err);
    if (currentTeammatePuuid === targetPuuid) {
      recentTeammates.value = [];
    }
  } finally {
    if (currentTeammatePuuid === targetPuuid) {
      loadingTeammates.value = false;
    }
  }
}

// ─── 导航函数 ───
const navigateSearchPayload = inject<
  Ref<{ name: string; gameId: number | null } | null>
>("navigateSearchPayload")!;
const navigateTo = inject<(page: string) => void>("navigateTo");

function goToMatchDetail(gameId: number) {
  if (!summoner.value) return;
  const name = summoner.value.gameName || summoner.value.displayName;
  const fullName = summoner.value.tagLine
    ? `${name}#${summoner.value.tagLine}`
    : name;
  navigateSearchPayload.value = { name: fullName, gameId };
}

function handleClickTeammate(name: string) {
  if (navigateSearchPayload) {
    navigateSearchPayload.value = { name, gameId: -1 };
  }
  if (navigateTo) {
    navigateTo("search");
  }
}

// 点击外部关闭下拉菜单
function onDocClick() {
  showQueueDropdown.value = false;
}

// ─── 生命周期与监听 ───

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

// 对局结束后自动刷新战绩列表（召唤师数据由 Career.vue 统一刷新）
watch(
  () => store.gamePhase,
  async (phase: string, oldPhase: string | undefined) => {
    if (!summoner.value?.puuid) return;
    const gamePhases = [
      "InProgress", "GameStart", "ChampSelect", "ReadyCheck", "PreEndOfGame",
    ];
    const endPhases = ["EndOfGame", "Lobby", "None"];
    if (
      gamePhases.includes(oldPhase ?? "") &&
      endPhases.includes(phase ?? "")
    ) {
      const puuid = summoner.value.puuid;
      const prevLatestId =
        recentMatches.value[0]?.gameId ?? matches.value[0]?.gameId ?? null;
      console.log(
        `[Career] 对局结束 (${oldPhase} → ${phase})，等待 LCU 同步并重试刷新`,
      );

      await new Promise((r) => setTimeout(r, 2000));

      for (let attempt = 0; attempt < 5; attempt++) {
        if (attempt > 0) {
          await new Promise((r) => setTimeout(r, 3000));
        }
        try {
          await mh.loadMatches(puuid, true);
          mh.loadRecentMatches(puuid, true);
          const latestId = matches.value[0]?.gameId ?? null;
          if (latestId && latestId !== prevLatestId) {
            console.log(
              `[Career] 第 ${attempt + 1} 次重试时已发现新对局 ${latestId}`,
            );
            return;
          }
          console.log(
            `[Career] 第 ${attempt + 1} 次刷新：尚未发现新对局，继续等待`,
          );
        } catch (e) {
          console.warn(`[Career] 第 ${attempt + 1} 次刷新失败:`, e);
        }
      }
      await mh.loadRankedStats(puuid);
      await calculateRecentTeammates();
    }
  },
);
</script>

<template>
  <div class="match-history-tab">
    <!-- 加载中：召唤师数据尚未就绪 -->
    <div v-if="!summoner && loading" class="tip-container">
      <n-spin size="large" />
      <p class="tip" style="margin-top: 1rem;">{{ $t("career.loading") || "加载中..." }}</p>
    </div>

    <!-- 空态：加载完成但无数据 -->
    <div v-else-if="!summoner" class="tip-container">
      <p class="tip">{{ $t("career.empty") }}</p>
    </div>

    <template v-else>
    <!-- 排位段位信息表 -->
    <div class="rank-table-wrapper">
      <table class="rank-table">
        <thead>
          <tr>
            <th>{{ $t("career.type") }}</th>
            <th>{{ $t("career.totalGames") }}</th>
            <th>{{ $t("career.winRate") }}</th>
            <th>{{ $t("career.winsLabel") }}</th>
            <th>{{ $t("career.lossesLabel") }}</th>
            <th>{{ $t("career.tier") }}</th>
            <th>{{ $t("career.lp") }}</th>
            <th>{{ $t("career.highest") }}</th>
            <th>{{ $t("career.prevSeason") }}</th>
          </tr>
        </thead>
        <tbody>
          <!-- 单双排 -->
          <tr>
            <td class="type-name">{{ $t("gameModes.420") }}</td>
            <td>{{ soloQueue ? soloQueue.wins + soloQueue.losses : 0 }}</td>
            <td>
              {{
                soloQueue && soloQueue.wins + soloQueue.losses > 0
                  ? (
                      (soloQueue.wins / (soloQueue.wins + soloQueue.losses)) *
                      100
                    ).toFixed(0) + "%"
                  : "--"
              }}
            </td>
            <td>{{ soloQueue ? soloQueue.wins : 0 }}</td>
            <td>{{ soloQueue ? soloQueue.losses : 0 }}</td>
            <td class="rank-name">
              {{ soloQueue ? formatRank(soloQueue) : "--" }}
            </td>
            <td>{{ soloQueue ? soloQueue.leaguePoints : 0 }}</td>
            <td>{{ soloQueue ? formatHighestRank(soloQueue) : "--" }}</td>
            <td>{{ soloQueue ? formatPrevSeasonRank(soloQueue) : "--" }}</td>
          </tr>
          <!-- 灵活排位 -->
          <tr>
            <td class="type-name">{{ $t("gameModes.440") }}</td>
            <td>{{ flexQueue ? flexQueue.wins + flexQueue.losses : 0 }}</td>
            <td>
              {{
                flexQueue && flexQueue.wins + flexQueue.losses > 0
                  ? (
                      (flexQueue.wins / (flexQueue.wins + flexQueue.losses)) *
                      100
                    ).toFixed(0) + "%"
                  : "--"
              }}
            </td>
            <td>{{ flexQueue ? flexQueue.wins : 0 }}</td>
            <td>{{ flexQueue ? flexQueue.losses : 0 }}</td>
            <td class="rank-name">
              {{ flexQueue ? formatRank(flexQueue) : "--" }}
            </td>
            <td>{{ flexQueue ? flexQueue.leaguePoints : 0 }}</td>
            <td>{{ flexQueue ? formatHighestRank(flexQueue) : "--" }}</td>
            <td>{{ flexQueue ? formatPrevSeasonRank(flexQueue) : "--" }}</td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- 近期数据看板 & 常用英雄 -->
    <div v-if="statsSummary" class="recent-summary-bar">
      <div class="summary-text">
        <span class="summary-title">{{
          $t("career.recentGamesTitle", { count: recentMatches.length })
        }}</span>
        <span class="win-color"
          >{{ $t("career.win") }}: {{ statsSummary.wins }}</span
        >
        <span class="lose-color"
          >{{ $t("career.lose") }}: {{ statsSummary.losses }}</span
        >
        <span class="kda-label">KDA:</span>
        <span class="kda-values">
          {{ statsSummary.kills }} /
          <span class="death-red">{{ statsSummary.deaths }}</span> /
          {{ statsSummary.assists }}
        </span>
        <span class="kda-ratio">({{ statsSummary.kda }})</span>
      </div>

      <div class="recent-champs">
        <div
          v-for="c in statsSummary.topChamps"
          :key="c.id"
          class="recent-champ-icon"
          :title="$t('career.gamesCount', { count: c.count })"
        >
          <LcuImage :src="c.icon" alt="champ" />
        </div>
      </div>

      <div class="summary-actions">
        <n-popover
          trigger="click"
          placement="bottom-start"
          scrollable
          class="recent-teammates-popover"
          style="
            max-height: 400px;
            width: 420px;
            border-radius: 12px;
            padding: 12px;
          "
        >
          <template #trigger>
            <button class="summary-action-btn">
              {{ $t("career.recentTeammates") }}
            </button>
          </template>
          <div class="teammates-flyout">
            <div v-if="loadingTeammates" class="loading-container">
              <n-spin size="medium" />
            </div>
            <div v-else-if="recentTeammates.length === 0" class="empty-text">
              暂无最近队友数据
            </div>
            <div v-else class="teammates-list">
              <div
                v-for="tm in recentTeammates"
                :key="tm.puuid"
                class="teammate-card"
                @click="handleClickTeammate(tm.name)"
              >
                <div class="teammate-avatar">
                  <LcuImage :src="tm.icon" />
                </div>
                <div class="teammate-info-col">
                  <div class="teammate-name">
                    {{ tm.name }}
                  </div>
                  <div class="teammate-last-time" v-if="tm.lastPlayTime">
                    {{ formatTime(tm.lastPlayTime) }}
                  </div>
                </div>
                <div class="teammate-stats">
                  <span class="stat-item">
                    {{ $t("career.total") || "总" }}: <span class="stat-value total">{{ tm.total }}</span>
                  </span>
                  <span class="stat-item">
                    {{ $t("career.win") || "胜" }}: <span class="stat-value win">{{ tm.wins }}</span>
                  </span>
                  <span class="stat-item">
                    {{ $t("career.lose") || "负" }}: <span class="stat-value lose">{{ tm.losses }}</span>
                  </span>
                </div>
              </div>
            </div>
          </div>
        </n-popover>
        <div
          class="dropdown-trigger"
          @click.stop="showQueueDropdown = !showQueueDropdown"
        >
          <span>{{
            selectedQueue === null
              ? $t("career.all")
              : $t("gameModes." + selectedQueue)
          }}</span>
          <svg
            :class="['arrow-icon', { expanded: showQueueDropdown }]"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
          >
            <polyline points="6 9 12 15 18 9" />
          </svg>
          <div
            v-if="showQueueDropdown"
            class="queue-dropdown-menu"
            @click.stop
          >
            <div
              v-for="q in QUEUE_OPTIONS"
              :key="q.id ?? -1"
              :class="[
                'queue-dropdown-item',
                { active: selectedQueue === q.id },
              ]"
              @click="selectQueue(q.id)"
            >
              {{ q.id === null ? $t("career.all") : $t("gameModes." + q.id) }}
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- 局部滚动包裹区域：保留头像、排位与近期对局看板，仅滚动对局战绩列表 -->
    <div class="career-scroll-area">
      <!-- 战绩对局历史列表 -->
      <div v-if="filteredMatches.length > 0" class="match-history-list">
        <div
          v-for="m in filteredMatches"
          :key="m.gameId"
          :class="['match-card', m.win ? 'win' : 'lose']"
          @click="goToMatchDetail(m.gameId)"
          style="cursor: pointer"
        >
          <!-- 1. 英雄头像、等级、技能、符文 -->
          <div class="champ-panel">
            <div class="champ-avatar-box">
              <LcuImage
                :src="m.championIconUrl"
                class="champ-avatar"
                alt="champ"
              />
              <div class="level-overlay">{{ m.champLevel }}</div>
            </div>
            <div class="spells-runes">
              <div class="spells-col">
                <div class="spell-slot">
                  <LcuImage
                    :src="getSpellIcon(m, 1)"
                    class="mini-icon"
                    alt="s1"
                  />
                </div>
                <div class="spell-slot">
                  <LcuImage
                    :src="getSpellIcon(m, 2)"
                    class="mini-icon"
                    alt="s2"
                  />
                </div>
              </div>
              <div v-if="m.queueId !== 2400" class="rune-slot">
                <LcuImage
                  :src="m.runeIconUrl"
                  class="mini-icon circular"
                  alt="rune"
                />
              </div>
            </div>
          </div>

          <!-- 2. 胜负状态与游戏模式 -->
          <div class="result-panel">
            <span :class="['result-text', m.win ? 'win-text' : 'lose-text']">
              {{ m.win ? $t("career.victory") : $t("career.defeat") }}
            </span>
            <span class="queue-mode">{{ getQueueName(m) }}</span>
          </div>

          <!-- 3. KDA 数字与文字 -->
          <div class="kda-panel">
            <div class="kda-numbers">
              <span class="bold">{{ m.kills }}</span> /
              <span class="bold death-red">{{ m.deaths }}</span> /
              <span class="bold">{{ m.assists }}</span>
            </div>
            <div class="kda-desc">
              <span class="kda-ratio" :class="getKdaClass(m.kda)"
                >{{ m.kda }} KDA</span
              >
            </div>
          </div>

          <!-- 4. 补刀补兵数 -->
          <div class="cs-panel">
            <span class="cs-count">{{ m.cs }}</span>
            <svg
              class="cs-icon"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
            >
              <path
                d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
            </svg>
          </div>

          <!-- 5. 装备栏 (前 6 件常规装备 + 第 7 件饰品) -->
          <div class="items-panel">
            <div class="items-grid">
              <div v-for="idx in 6" :key="idx" class="item-slot">
                <LcuImage
                  v-if="m.itemIconUrls[idx - 1]"
                  :src="m.itemIconUrls[idx - 1]"
                  class="item-img"
                  alt="item"
                />
              </div>
            </div>
            <!-- 饰品独立显示 -->
            <div class="ward-slot">
              <LcuImage
                v-if="m.itemIconUrls[6]"
                :src="m.itemIconUrls[6]"
                class="item-img"
                alt="ward"
              />
            </div>
          </div>

          <!-- 6. 获得金币 -->
          <div class="gold-panel">
            <span class="gold-count">{{ m.gold.toLocaleString() }}</span>
            <svg class="gold-icon" viewBox="0 0 24 24" fill="currentColor">
              <circle
                cx="12"
                cy="12"
                r="10"
                stroke="currentColor"
                stroke-width="2"
                fill="none"
              />
              <path
                d="M12 6v12M15 9H11.5a1.5 1.5 0 0 0 0 3h1a1.5 1.5 0 0 1 0 3H9"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                fill="none"
              />
            </svg>
          </div>

          <!-- 7. 地图模式与时长/日期 -->
          <div class="time-panel">
            <span class="game-map">{{ translateMapName(m.map) }}</span>
            <span class="match-time"
              >{{ m.duration }} · {{ formatTime(m.timeStamp) }}</span
            >
          </div>
        </div>
      </div>
      <div v-else-if="!loading" class="tip-container">
        <p class="tip">{{ $t("career.empty") }}</p>
      </div>
    </div>
    </template>
  </div>
</template>

<style scoped>
.match-history-tab {
  display: flex;
  flex-direction: column;
  flex: 1;
  overflow: hidden;
  min-height: 0;
}

.tip-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 6rem 2rem;
  color: var(--text-muted);
  flex: 1;
}

.tip {
  font-size: 0.95rem;
  color: var(--text-dimmed);
  margin: 0;
}

/* 排位数据表 */
.rank-table-wrapper {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  overflow: hidden;
  margin-bottom: 1rem;
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

.rank-table th,
.rank-table td {
  padding: 6px 16px;
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
  background-color: var(--settings-collapse-bg, var(--card-bg)) !important;
  backdrop-filter: blur(15px) !important;
  border: 1px solid var(--border-color);
  border-radius: 8px;
  margin-bottom: 1rem;
  box-shadow: var(--shadow-sm);
  transition: all 0.25s ease;
  position: sticky;
  top: 0;
  z-index: 100 !important;
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
  background: #ffffff;
  border: 1px solid var(--border-color, rgba(0, 0, 0, 0.08));
  border-radius: 8px;
  box-shadow: var(--shadow-lg);
  z-index: 100;
  min-width: 130px;
  padding: 4px 0;
}

[data-theme="dark"] .queue-dropdown-menu {
  background: #18181c;
  border-color: rgba(255, 255, 255, 0.08);
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

/* 战绩及看板局部滚动区域 */
.career-scroll-area {
  flex: 1;
  overflow-y: auto;
  padding-right: 4px;
  margin-top: 1rem;
  scroll-behavior: smooth;
  /* 顶部与底部边缘 16px 渐变淡出羽化，消除滑动硬切裂感，极致顺滑 */
  -webkit-mask-image: linear-gradient(
    to bottom,
    transparent 0%,
    black 16px,
    black calc(100% - 16px),
    transparent 100%
  );
  mask-image: linear-gradient(
    to bottom,
    transparent 0%,
    black 16px,
    black calc(100% - 16px),
    transparent 100%
  );
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

.spell-slot,
.rune-slot {
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
  color: var(--text-muted);
  font-size: 0.82rem;
  font-weight: 700;
}

.kda-perfect {
  color: #d97706;
}
.kda-great {
  color: #db2777;
}
.kda-good {
  color: #2563eb;
}
.kda-normal {
  color: var(--text-dimmed);
}

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

.item-slot,
.ward-slot {
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

/* 队友弹窗内加载容器 */
.loading-container {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 2rem;
}

.empty-text {
  text-align: center;
  padding: 1rem;
  color: var(--text-dimmed);
  font-size: 0.85rem;
}

/* 队友卡片 */
.teammates-flyout {
  min-width: 380px;
}

.teammates-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.teammate-card {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 12px;
  border-radius: 8px;
  cursor: pointer;
  transition: background-color 0.2s;
}

.teammate-card:hover {
  background-color: var(--hover-bg);
}

.teammate-avatar {
  width: 36px;
  height: 36px;
  border-radius: 50%;
  overflow: hidden;
  flex-shrink: 0;
}

.teammate-info-col {
  flex: 1;
  min-width: 0;
}

.teammate-name {
  font-size: 0.85rem;
  font-weight: 600;
  color: var(--text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.teammate-last-time {
  font-size: 0.72rem;
  color: var(--text-dimmed);
  margin-top: 2px;
}

.teammate-stats {
  display: flex;
  gap: 10px;
  font-size: 0.78rem;
  color: var(--text-muted);
  flex-shrink: 0;
}

.stat-item {
  white-space: nowrap;
}

.stat-value {
  font-weight: 700;
}

.stat-value.total {
  color: var(--text-color);
}

.stat-value.win {
  color: var(--win-color);
}

.stat-value.lose {
  color: var(--loss-color);
}

@keyframes fadeIn {
  from {
    opacity: 0;
    transform: translateY(6px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>
