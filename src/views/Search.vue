<script setup lang="ts">
import { ref, computed, onMounted, inject, watch, type Ref } from "vue";
import { useLcuStore } from "../store/lcuStore";
import { fetchMatchHistory, lcuRequest } from "../api/lcu";
import type { SummonerDisplay, MatchDisplay } from "../api/lcu";
import LcuImage from "../components/LcuImage.vue";
import { invoke } from "@tauri-apps/api/core";

const store = useLcuStore();
const searchName = ref("");
const searching = ref(false);
const error = ref("");
const summoner = ref<SummonerDisplay | null>(null);
const matches = ref<MatchDisplay[]>([]);

// 对局详情相关
const selectedGameId = ref<number | null>(null);
const selectedGame = ref<any | null>(null);
const gameLoading = ref(false);
const gameDataAssets = ref<any>(null);

// 分页相关
const currentPageNum = ref(1);
const matchesPerPage = 10;

// 搜索历史
const searchHistory = ref<string[]>([]);
const showHistory = ref(false);

function loadSearchHistory() {
  try {
    const saved = localStorage.getItem("yuumi_search_history");
    if (saved) searchHistory.value = JSON.parse(saved);
  } catch { /* ignore */ }
}

function saveToHistory(name: string) {
  const trimmed = name.trim();
  if (!trimmed) return;
  // 去重，最新的放最前面
  searchHistory.value = [trimmed, ...searchHistory.value.filter((h: string) => h !== trimmed)].slice(0, 20);
  localStorage.setItem("yuumi_search_history", JSON.stringify(searchHistory.value));
}

function removeFromHistory(name: string) {
  searchHistory.value = searchHistory.value.filter((h: string) => h !== name);
  localStorage.setItem("yuumi_search_history", JSON.stringify(searchHistory.value));
}

function selectHistory(name: string) {
  searchName.value = name;
  showHistory.value = false;
  doSearch();
}

function hideHistoryDelayed() {
  setTimeout(() => { showHistory.value = false; }, 200);
}

// 过滤后的历史记录（根据当前输入）
const filteredHistory = computed(() => {
  const q = searchName.value.trim().toLowerCase();
  if (!q) return searchHistory.value;
  return searchHistory.value.filter((h: string) => h.toLowerCase().includes(q));
});

// 从 App.vue 注入 Career → Search 跳转状态
const navigateSearchPayload = inject<Ref<{ name: string; gameId: number | null } | null>>("navigateSearchPayload")!;

onMounted(async () => {
  loadSearchHistory();
  try {
    gameDataAssets.value = await invoke("get_game_data_assets");
  } catch (e) {
    console.error("加载静态资源数据映射失败:", e);
  }
});

// 监听 Career → Search 跳转：自动填入名称并搜索，然后选中指定对局
watch(navigateSearchPayload, async (payload) => {
  if (!payload || !payload.name || payload.gameId === null) return;
  searchName.value = payload.name;
  await doSearch();
  // doSearch 完成后自动选中 Career 传来的对局（-1 表示只搜索不选中）
  if (payload.gameId > 0 && matches.value.length > 0) {
    selectMatch(payload.gameId);
  }
  // 清除跳转状态，避免后续重复触发
  navigateSearchPayload.value = null;
}, { immediate: true });

async function doSearch() {
  if (!searchName.value.trim()) return;
  searching.value = true;
  error.value = "";
  summoner.value = null;
  matches.value = [];
  selectedGame.value = null;
  selectedGameId.value = null;
  currentPageNum.value = 1;

  try {
    const name = searchName.value.trim();
    let resp;
    const summonerId = pendingSummonerId.value;
    pendingSummonerId.value = 0;

    if (summonerId) {
      // 通过数字 summonerId 直接查询（从对局详情点击其他玩家时使用）
      resp = await lcuRequest<any>("GET", `/lol-summoner/v1/summoners/${summonerId}`);
    } else if (name.includes("#")) {
      const hashIndex = name.indexOf("#");
      const gameName = name.slice(0, hashIndex);
      const tagLine = name.slice(hashIndex + 1);
      resp = await lcuRequest<any>(
        "GET",
        `/lol-summoner/v1/alias/lookup?gameName=${encodeURIComponent(gameName)}&tagLine=${encodeURIComponent(tagLine)}`
      );
    } else {
      resp = await lcuRequest<any>("GET", `/lol-summoner/v1/summoners?name=${encodeURIComponent(name)}`);
    }

    if (!resp.success || !resp.data) {
      error.value = resp.error || "未找到该召唤师";
      return;
    }
    const data = resp.data;
    summoner.value = {
      accountId: data.accountId ?? 0,
      displayName: data.displayName ?? name,
      gameName: data.gameName ?? "",
      tagLine: data.tagLine ?? "",
      percentCompleteForNextLevel: data.percentCompleteForNextLevel ?? 0,
      profileIconId: data.profileIconId ?? 29,
      puuid: data.puuid ?? "",
      summonerId: data.summonerId ?? 0,
      summonerLevel: data.summonerLevel ?? 0,
      xpSinceLastLevel: data.xpSinceLastLevel ?? 0,
      xpUntilNextLevel: data.xpUntilNextLevel ?? 0,
      profileIconUrl: `/lol-game-data/assets/v1/profile-icons/${data.profileIconId ?? 29}.jpg`,
    };

    // 搜索成功后保存到历史记录
    saveToHistory(name);

    if (summoner.value.puuid) {
      await loadMatchHistoryList();
    }
  } catch (e: any) {
    error.value = e.toString();
  } finally {
    searching.value = false;
  }
}

async function loadMatchHistoryList() {
  if (!summoner.value) return;
  try {
    const beg = (currentPageNum.value - 1) * matchesPerPage;
    const end = beg + matchesPerPage;
    matches.value = await fetchMatchHistory(summoner.value.puuid, beg, end);
    
    // 默认载入第一局对局的详情
    if (matches.value.length > 0) {
      selectMatch(matches.value[0].gameId);
    }
  } catch (e) {
    console.error("抓取战绩列表失败:", e);
  }
}

async function selectMatch(gameId: number) {
  selectedGameId.value = gameId;
  gameLoading.value = true;
  selectedGame.value = null;
  try {
    const resp = await lcuRequest<any>("GET", `/lol-match-history/v1/games/${gameId}`);
    if (resp.success && resp.data) {
      selectedGame.value = resp.data;
    }
  } catch (e) {
    console.error("拉取对局详细信息失败:", e);
  } finally {
    gameLoading.value = false;
  }
}

async function handlePrevPage() {
  if (currentPageNum.value > 1) {
    currentPageNum.value--;
    await loadMatchHistoryList();
  }
}

async function handleNextPage() {
  currentPageNum.value++;
  await loadMatchHistoryList();
}

// 静态映射查找
function getSpellUrl(spellId: number) {
  return gameDataAssets.value?.spells?.[spellId] || "";
}

function getRuneUrl(runeId: number) {
  const path = gameDataAssets.value?.runes?.[runeId];
  return path || "";
}

function getItemUrl(itemId: number) {
  if (!itemId) return "";
  return gameDataAssets.value?.items?.[itemId] || "";
}

function copyGameId(gameId: number) {
  navigator.clipboard.writeText(String(gameId));
  alert(`📋 游戏 ID: ${gameId} 已复制到剪贴板`);
}

// 点击对局中的其他召唤师名称 → 在当前页面搜索（用 summonerId 避免 400/404/422 错误）
const pendingSummonerId = ref<number>(0);

function searchPlayerBySummonerId(summonerId: number, displayName: string) {
  if (!summonerId) return;
  pendingSummonerId.value = summonerId;
  searchName.value = displayName || String(summonerId);
  doSearch();
}

// 整理后的对局详情
const gameDetails = computed(() => {
  if (!selectedGame.value) return null;
  const g = selectedGame.value;

  const playerMap: Record<number, { name: string; puuid: string; summonerId: number }> = {};
  if (g.participantIdentities) {
    for (const identity of g.participantIdentities) {
      const pId = identity.participantId;
      const player = identity.player;
      playerMap[pId] = {
        name: player?.summonerName || player?.gameName || "未知",
        puuid: player?.puuid || "",
        summonerId: player?.summonerId ?? 0
      };
    }
  }

  const bluePlayers: any[] = [];
  const redPlayers: any[] = [];

  if (g.participants) {
    for (const p of g.participants) {
      const pId = p.participantId;
      const nameInfo = playerMap[pId] || { name: "未知", puuid: "", summonerId: 0 };
      const stats = p.stats || {};
      
      const itemUrls = [
        getItemUrl(stats.item0),
        getItemUrl(stats.item1),
        getItemUrl(stats.item2),
        getItemUrl(stats.item3),
        getItemUrl(stats.item4),
        getItemUrl(stats.item5),
        getItemUrl(stats.item6)
      ];

      const pData = {
        participantId: pId,
        teamId: p.teamId,
        championId: p.championId,
        championIconUrl: `/lol-game-data/assets/v1/champion-icons/${p.championId}.png`,
        spell1Url: getSpellUrl(p.spell1Id),
        spell2Url: getSpellUrl(p.spell2Id),
        runeUrl: getRuneUrl(stats.perk0),
        name: nameInfo.name,
        puuid: nameInfo.puuid,
        summonerId: nameInfo.summonerId,
        level: stats.champLevel,
        kills: stats.kills ?? 0,
        deaths: stats.deaths ?? 0,
        assists: stats.assists ?? 0,
        cs: (stats.totalMinionsKilled ?? 0) + (stats.neutralMinionsKilled ?? 0),
        gold: stats.goldEarned ?? 0,
        damage: stats.totalDamageDealtToChampions ?? 0,
        items: itemUrls.slice(0, 6),
        ward: itemUrls[6],
        win: stats.win
      };

      if (p.teamId === 100) {
        bluePlayers.push(pData);
      } else {
        redPlayers.push(pData);
      }
    }
  }

  const isBlueWin = bluePlayers[0]?.win ?? false;

  const blueKills = bluePlayers.reduce((sum, p) => sum + p.kills, 0);
  const redKills = redPlayers.reduce((sum, p) => sum + p.kills, 0);

  // 从 teams 数据中提取团队目标统计
  const teamsData: any[] = g.teams || [];
  const blueTeamRaw = teamsData.find((t: any) => t.teamId === 100) || {};
  const redTeamRaw = teamsData.find((t: any) => t.teamId === 200) || {};

  const queueNames: Record<number, string> = {
    420: "排位单双排", 430: "匹配模式", 440: "排位灵活组排", 450: "极地大乱斗",
    900: "无限火力", 1020: "克隆模式", 1300: "极限闪击", 1700: "斗魂竞技场", 0: "自定义"
  };

  const mapNames: Record<number, string> = {
    11: "召唤师峡谷", 12: "嚎哭深渊", 21: "极限闪击", 22: "对战大厅"
  };

  const mins = Math.floor(g.gameDuration / 60);
  const secs = g.gameDuration % 60;
  const durationStr = `${mins}:${secs < 10 ? '0' + secs : secs}`;

  const date = new Date(g.gameCreation);
  const dateStr = `${date.getFullYear()}/${(date.getMonth()+1).toString().padStart(2, '0')}/${date.getDate().toString().padStart(2, '0')} ${date.getHours().toString().padStart(2, '0')}:${date.getMinutes().toString().padStart(2, '0')}`;

  // 判定当前选中的对局是胜利还是失败（当前搜索的召唤师是哪一方）
  let isQueriedPlayerWin = false;
  if (summoner.value) {
    const queriedPuuid = summoner.value.puuid;
    const allPlayers = [...bluePlayers, ...redPlayers];
    const found = allPlayers.find(p => p.puuid === queriedPuuid);
    if (found) {
      isQueriedPlayerWin = found.win;
    }
  }

  return {
    gameId: g.gameId,
    duration: durationStr,
    date: dateStr,
    queueName: queueNames[g.queueId] || "其他模式",
    mapName: mapNames[g.mapId] || "未知地图",
    win: isQueriedPlayerWin,
    blue: {
      players: bluePlayers,
      kills: blueKills,
      win: isBlueWin,
      towerKills: blueTeamRaw.towerKills ?? 0,
      inhibitorKills: blueTeamRaw.inhibitorKills ?? 0,
      baronKills: blueTeamRaw.baronKills ?? 0,
      dragonKills: blueTeamRaw.dragonKills ?? 0,
      riftHeraldKills: blueTeamRaw.riftHeraldKills ?? 0,
    },
    red: {
      players: redPlayers,
      kills: redKills,
      win: !isBlueWin,
      towerKills: redTeamRaw.towerKills ?? 0,
      inhibitorKills: redTeamRaw.inhibitorKills ?? 0,
      baronKills: redTeamRaw.baronKills ?? 0,
      dragonKills: redTeamRaw.dragonKills ?? 0,
      riftHeraldKills: redTeamRaw.riftHeraldKills ?? 0,
    }
  };
});
</script>

<template>
  <div class="search-view">
    <div v-if="!store.isConnected" class="tip-container">
      <div class="offline-logo">🔍</div>
      <p class="tip">请先启动英雄联盟客户端</p>
    </div>

    <div v-else class="search-container">
      <!-- 顶部搜索工具栏 -->
      <div class="search-bar">
        <div class="search-input-wrapper">
          <input
            v-model="searchName"
            placeholder="输入召唤师名称（如 你好#5201）"
            @keyup.enter="doSearch"
            @focus="showHistory = true"
            @blur="hideHistoryDelayed"
            :disabled="searching"
            class="search-input"
          />
          <button class="search-trigger-btn" @click="doSearch" :disabled="searching || !searchName.trim()">
            <svg class="search-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="11" cy="11" r="8"/>
              <line x1="21" y1="21" x2="16.65" y2="16.65"/>
            </svg>
          </button>
          <!-- 搜索历史下拉框 -->
          <div v-if="showHistory && filteredHistory.length > 0" class="history-dropdown">
            <div
              v-for="item in filteredHistory"
              :key="item"
              class="history-item"
              @mousedown.prevent="selectHistory(item)"
            >
              <span class="history-icon">🕐</span>
              <span class="history-text">{{ item }}</span>
              <span class="history-delete" @mousedown.prevent.stop="removeFromHistory(item)" title="删除">✕</span>
            </div>
          </div>
        </div>
        
        <button class="tab-btn active">生涯</button>
        
        <div class="dropdown-select">
          <span>全部</span>
          <svg class="arrow-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="6 9 12 15 18 9"/>
          </svg>
        </div>

        <label class="checkbox-wrapper">
          <input type="checkbox" checked />
          <span>Upload matches</span>
        </label>
      </div>

      <div v-if="error" class="error">{{ error }}</div>

      <!-- 分栏对局面板 -->
      <div v-if="summoner && matches.length > 0" class="panel-layout">
        <!-- 左侧：迷你对局卡片列表 -->
        <div class="left-match-list-panel">
          <div class="mini-match-list">
            <div
              v-for="m in matches"
              :key="m.gameId"
              :class="['mini-match-card', m.win ? 'win' : 'lose', { selected: selectedGameId === m.gameId }]"
              @click="selectMatch(m.gameId)"
            >
              <div class="mini-avatar">
                <LcuImage :src="m.championIconUrl" alt="champ" />
              </div>
              <div class="mini-info">
                <span class="mini-mode">{{ m.name }}</span>
                <span class="mini-time">{{ m.shortTime.split(' ')[0] }}</span>
              </div>
              <div class="mini-kda">
                {{ m.kills }}/<span class="death-red">{{ m.deaths }}</span>/{{ m.assists }}
              </div>
            </div>
          </div>

          <!-- 翻页控制 -->
          <div class="pagination">
            <button class="page-btn" @click="handlePrevPage" :disabled="currentPageNum <= 1">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="15 18 9 12 15 6"/></svg>
            </button>
            <span class="page-num">{{ currentPageNum }}</span>
            <button class="page-btn" @click="handleNextPage" :disabled="matches.length < matchesPerPage">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="9 18 15 12 9 6"/></svg>
            </button>
          </div>
        </div>

        <!-- 右侧：对局详情 -->
        <div class="right-detail-panel">
          <div v-if="gameLoading" class="detail-loading">
            <div class="loading-spinner"></div>
            <span>拉取对局详情数据中...</span>
          </div>

          <div v-else-if="gameDetails" class="detail-content">
            <!-- 头部大 Banner -->
            <div :class="['detail-banner', gameDetails.win ? 'win' : 'lose']">
              <div class="banner-left">
                <h2 :class="['banner-result', gameDetails.win ? 'win' : 'lose']">
                  {{ gameDetails.win ? '胜利' : '失败' }}
                </h2>
                <span class="banner-subtext">
                  {{ gameDetails.mapName }} · {{ gameDetails.queueName }} · {{ gameDetails.duration }} · {{ gameDetails.date }} · 游戏 ID: {{ gameDetails.gameId }}
                </span>
              </div>
              <button class="copy-btn" @click="copyGameId(gameDetails.gameId)">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
                  <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
                </svg>
              </button>
            </div>

            <!-- 队伍详细数据 -->
            <div class="teams-container">
              <!-- 胜方 / 败方 -->
              <div v-for="team in [gameDetails.blue, gameDetails.red]" :key="team.win ? 'win' : 'lose'" class="team-block">
                <!-- 队头资源概览 -->
                <div :class="['team-header-bar', team.win ? 'win-bar' : 'lose-bar']">
                  <span :class="['team-result-label', team.win ? 'win-text' : 'lose-text']">
                    {{ team.win ? '胜方' : '败方' }}
                  </span>

                  <div class="team-objectives">
                    <span class="obj-item"><span class="obj-icon">⚔️</span> {{ team.kills }}</span>
                    <span class="obj-item"><span class="obj-icon">🏰</span> {{ team.towerKills }}</span>
                    <span class="obj-item"><span class="obj-icon">💎</span> {{ team.inhibitorKills }}</span>
                    <span class="obj-item"><span class="obj-icon">👾</span> {{ team.baronKills }}</span>
                    <span class="obj-item"><span class="obj-icon">🐉</span> {{ team.dragonKills }}</span>
                    <span class="obj-item"><span class="obj-icon">🦀</span> {{ team.riftHeraldKills }}</span>
                  </div>
                </div>

                <!-- 玩家列表 -->
                <div class="players-table">
                  <div v-for="p in team.players" :key="p.participantId" class="player-row">
                    <!-- 头像及技能/符文 -->
                    <div class="player-avatar-col">
                      <div class="row-avatar-box">
                        <LcuImage :src="p.championIconUrl" class="row-avatar" alt="champ" />
                        <span class="row-level-overlay">{{ p.level }}</span>
                      </div>
                      <div class="row-spell-rune-row">
                        <div class="row-spell-col">
                          <LcuImage :src="p.spell1Url" class="row-spell" alt="s1" />
                          <LcuImage :src="p.spell2Url" class="row-spell" alt="s2" />
                        </div>
                        <div class="row-rune">
                          <LcuImage :src="p.runeUrl" class="row-rune-img" alt="rune" />
                        </div>
                      </div>
                    </div>

                    <!-- 名字（可点击搜索） -->
                    <div class="player-name-col">
                      <span
                        :class="['row-name', { 'highlight-user': summoner && p.puuid === summoner.puuid }]"
                        @click="searchPlayerBySummonerId(p.summonerId, p.name)"
                        :title="`搜索 ${p.name}`"
                      >
                        {{ p.name }}
                      </span>
                    </div>

                    <!-- 装备栏 -->
                    <div class="player-items-col">
                      <div class="row-items-grid">
                        <div v-for="idx in 6" :key="idx" class="row-item-slot">
                          <LcuImage v-if="p.items[idx-1]" :src="p.items[idx-1]" class="row-item-img" alt="item" />
                        </div>
                      </div>
                      <div class="row-ward-slot">
                        <LcuImage v-if="p.ward" :src="p.ward" class="row-item-img" alt="ward" />
                      </div>
                    </div>

                    <!-- KDA -->
                    <div class="player-kda-col">
                      <span class="row-kda-text">
                        {{ p.kills }}/<span class="death-red">{{ p.deaths }}</span>/{{ p.assists }}
                      </span>
                    </div>

                    <!-- 补兵 -->
                    <div class="player-cs-col">
                      <span class="row-cs-text">{{ p.cs }}</span>
                    </div>

                    <!-- 金币 -->
                    <div class="player-gold-col">
                      <span class="row-gold-text">{{ p.gold.toLocaleString() }}</span>
                    </div>

                    <!-- 伤害 -->
                    <div class="player-damage-col">
                      <span class="row-damage-text">{{ p.damage.toLocaleString() }}</span>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
          <div v-else class="detail-empty">
            <p>请在左侧选择一局对局查看详情</p>
          </div>
        </div>
      </div>

      <div v-else-if="summoner && !searching" class="detail-empty">
        <p>暂无战绩记录</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.search-view {
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
  padding: 8px 16px;
  border-radius: 6px;
  margin-bottom: 1rem;
  font-size: 0.88rem;
}

.search-container {
  max-width: 1080px;
  margin: 0 auto;
  animation: fadeIn 0.3s ease-out;
}

/* 顶部搜索栏 */
.search-bar {
  display: flex;
  align-items: center;
  gap: 12px;
  background: white;
  border: 1px solid #ebeef5;
  padding: 10px 16px;
  border-radius: 12px;
  margin-bottom: 1.5rem;
  box-shadow: 0 2px 8px rgba(0,0,0,0.02);
}

.search-input-wrapper {
  position: relative;
  display: flex;
  flex: 1;
  max-width: 400px;
}

/* 搜索历史下拉框 */
.history-dropdown {
  position: absolute;
  top: 100%;
  left: 0;
  right: 0;
  background: white;
  border: 1px solid #dcdfe6;
  border-top: none;
  border-radius: 0 0 8px 8px;
  box-shadow: 0 4px 12px rgba(0,0,0,0.08);
  z-index: 100;
  max-height: 260px;
  overflow-y: auto;
}

.history-item {
  display: flex;
  align-items: center;
  padding: 8px 12px;
  cursor: pointer;
  transition: background 0.15s;
  gap: 8px;
}

.history-item:hover {
  background: #f5f7fa;
}

.history-icon {
  font-size: 0.75rem;
  flex-shrink: 0;
  opacity: 0.5;
}

.history-text {
  flex: 1;
  font-size: 0.85rem;
  color: #303133;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.history-delete {
  font-size: 0.72rem;
  color: #c0c4cc;
  cursor: pointer;
  padding: 2px 4px;
  border-radius: 3px;
  transition: all 0.15s;
  flex-shrink: 0;
}

.history-delete:hover {
  color: #f56c6c;
  background: #fef0f0;
}

.search-input {
  width: 100%;
  padding: 8px 40px 8px 16px;
  border: 1px solid #dcdfe6;
  border-radius: 6px;
  font-size: 0.88rem;
  color: #606266;
  outline: none;
  transition: border-color 0.2s;
}

.search-input:focus {
  border-color: #6c5ce7;
}

.search-trigger-btn {
  position: absolute;
  right: 4px;
  top: 50%;
  transform: translateY(-50%);
  background: transparent;
  border: none;
  cursor: pointer;
  color: #909399;
  display: flex;
  align-items: center;
  padding: 4px;
}

.search-icon {
  width: 18px;
  height: 18px;
}

.tab-btn {
  background: white;
  border: 1px solid #dcdfe6;
  color: #606266;
  padding: 8px 20px;
  border-radius: 6px;
  font-size: 0.88rem;
  font-weight: 500;
  cursor: pointer;
}

.tab-btn.active {
  background-color: #6c5ce7;
  color: white;
  border-color: #6c5ce7;
}

.dropdown-select {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  background: white;
  border: 1px solid #dcdfe6;
  padding: 8px 16px;
  border-radius: 6px;
  font-size: 0.88rem;
  color: #606266;
  cursor: pointer;
}

.arrow-icon {
  width: 14px;
  height: 14px;
}

.checkbox-wrapper {
  margin-left: auto;
  display: inline-flex;
  align-items: center;
  gap: 8px;
  font-size: 0.85rem;
  color: #606266;
  cursor: pointer;
}

/* 分栏大布局 */
.panel-layout {
  display: grid;
  grid-template-columns: 260px 1fr;
  gap: 16px;
  align-items: stretch;
}

/* 左侧迷你列表 */
.left-match-list-panel {
  display: flex;
  flex-direction: column;
  gap: 10px;
  height: 100%;
}

.mini-match-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.mini-match-card {
  display: flex;
  align-items: center;
  padding: 10px 12px;
  border-radius: 8px;
  border-left: 4px solid;
  cursor: pointer;
  transition: all 0.2s;
  background: white;
  border-top: 1px solid #ebeef5;
  border-right: 1px solid #ebeef5;
  border-bottom: 1px solid #ebeef5;
}

.mini-match-card.win {
  border-left-color: #2ecc71;
}

.mini-match-card.win:hover {
  background: #f0f9eb;
}

.mini-match-card.lose {
  border-left-color: #f56c6c;
}

.mini-match-card.lose:hover {
  background: #fef0f0;
}

.mini-match-card.selected {
  background: #f0f2f5 !important;
  border-top-color: #dcdfe6;
  border-right-color: #dcdfe6;
  border-bottom-color: #dcdfe6;
}

.mini-avatar {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  overflow: hidden;
  border: 1px solid #dcdfe6;
  margin-right: 10px;
  flex-shrink: 0;
}

.mini-info {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
}

.mini-mode {
  font-size: 0.8rem;
  font-weight: bold;
  color: #303133;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.mini-time {
  font-size: 0.72rem;
  color: #909399;
  margin-top: 2px;
}

.mini-kda {
  font-size: 0.8rem;
  font-weight: 600;
  color: #606266;
  text-align: right;
  margin-left: 8px;
}

.death-red {
  color: #f56c6c;
}

/* 分页 */
.pagination {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 16px;
  margin-top: 8px;
}

.page-btn {
  background: white;
  border: 1px solid #dcdfe6;
  border-radius: 6px;
  width: 30px;
  height: 30px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  color: #606266;
}

.page-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.page-btn svg {
  width: 16px;
  height: 16px;
}

.page-num {
  font-size: 0.9rem;
  font-weight: bold;
  color: #303133;
}

/* 右侧详情面板 */
.right-detail-panel {
  background: white;
  border: 1px solid #ebeef5;
  border-radius: 12px;
  box-shadow: 0 2px 10px rgba(0,0,0,0.02);
  min-height: 480px;
  overflow: hidden;
}

.detail-loading, .detail-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 480px;
  color: #909399;
  font-size: 0.9rem;
}

.loading-spinner {
  width: 36px;
  height: 36px;
  border: 3px solid #e2e5e9;
  border-top-color: #6c5ce7;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
  margin-bottom: 12px;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

/* 详情 Banner */
.detail-banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 24px;
  border-bottom: 1px solid;
}

.detail-banner.win {
  background-color: #f0f9eb;
  border-color: #e1f3d8;
}

.detail-banner.lose {
  background-color: #fef0f0;
  border-color: #fde2e2;
}

.banner-result {
  font-size: 1.6rem;
  font-weight: 800;
  margin: 0 0 4px;
}

.banner-result.win { color: #2ecc71; }
.banner-result.lose { color: #f56c6c; }

.banner-subtext {
  font-size: 0.8rem;
  color: #909399;
}

.copy-btn {
  background: white;
  border: 1px solid #dcdfe6;
  border-radius: 6px;
  width: 32px;
  height: 32px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  color: #606266;
  transition: all 0.2s;
}

.copy-btn:hover {
  background-color: #f5f7fa;
  border-color: #c0c4cc;
}

.copy-btn svg {
  width: 16px;
  height: 16px;
}

/* 队伍 block */
.teams-container {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 16px;
}

.team-block {
  border: 1px solid #ebeef5;
  border-radius: 8px;
  overflow: hidden;
}

.team-header-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 16px;
  font-size: 0.85rem;
}

.team-header-bar.win-bar {
  background-color: #f0f9eb;
  border-bottom: 1px solid #e1f3d8;
}

.team-header-bar.lose-bar {
  background-color: #fef0f0;
  border-bottom: 1px solid #fde2e2;
}

.team-result-label {
  font-weight: bold;
}

.team-objectives {
  display: flex;
  align-items: center;
  gap: 10px;
  color: #606266;
  font-weight: 500;
  font-size: 0.8rem;
}

.obj-item {
  display: inline-flex;
  align-items: center;
  gap: 3px;
}

.obj-icon {
  font-size: 0.85rem;
}

/* 玩家列表 Table 行 */
.players-table {
  display: flex;
  flex-direction: column;
}

.player-row {
  display: flex;
  align-items: center;
  padding: 8px 16px;
  border-bottom: 1px solid #f0f2f5;
  font-size: 0.82rem;
  color: #333;
}

.player-row:last-child {
  border-bottom: none;
}

/* 1. 头像区 */
.player-avatar-col {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 130px;
}

.row-avatar-box {
  position: relative;
  width: 46px;
  height: 46px;
  flex-shrink: 0;
}

.row-avatar {
  width: 46px;
  height: 46px;
  border-radius: 50%;
  overflow: hidden;
  border: 1px solid #dcdfe6;
}

.row-level-overlay {
  position: absolute;
  bottom: -2px;
  right: -2px;
  width: 15px;
  height: 15px;
  line-height: 13px;
  background: #202124;
  color: white;
  border-radius: 50%;
  font-size: 0.62rem;
  font-weight: bold;
  text-align: center;
  border: 1px solid #fff;
}

.row-spell-rune-row {
  display: flex;
  align-items: center;
  gap: 3px;
}

.row-spell-col {
  display: flex;
  flex-direction: row;
  gap: 2px;
}

.row-spell {
  width: 18px;
  height: 18px;
  border-radius: 2px;
  border: 1px solid rgba(0,0,0,0.08);
}

.row-rune {
  width: 20px;
  height: 20px;
  flex-shrink: 0;
}

.row-rune-img {
  width: 20px;
  height: 20px;
  border-radius: 50%;
}

/* 2. 名字区 */
.player-name-col {
  width: 140px;
  min-width: 0;
  padding-right: 8px;
}

.row-name {
  display: block;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  color: #555;
  cursor: pointer;
  transition: color 0.15s;
}

.row-name:hover {
  color: #6c5ce7;
}

.highlight-user {
  color: #2ecc71 !important;
  font-weight: 800;
}

.highlight-user:hover {
  color: #27ae60 !important;
}

/* 3. 装备区 */
.player-items-col {
  display: flex;
  align-items: center;
  gap: 4px;
  flex: 1;
}

.row-items-grid {
  display: flex;
  gap: 1px;
}

.row-item-slot {
  width: 28px;
  height: 28px;
  background: rgba(0,0,0,0.03);
  border-radius: 3px;
  overflow: hidden;
  border: 1px solid rgba(0,0,0,0.05);
}

.row-item-img {
  width: 100%;
  height: 100%;
  display: block;
}

.row-ward-slot {
  width: 28px;
  height: 28px;
  border-radius: 3px;
  overflow: hidden;
  border: 1px solid #e6a23c;
  background-color: rgba(230,162,60,0.03);
}

/* 4. KDA */
.player-kda-col {
  width: 75px;
  text-align: center;
  font-weight: 600;
}

/* 5. 补兵 */
.player-cs-col {
  width: 45px;
  text-align: center;
  color: #606266;
}

/* 6. 金币 */
.player-gold-col {
  width: 60px;
  text-align: right;
  color: #606266;
}

/* 7. 伤害 */
.player-damage-col {
  width: 65px;
  text-align: right;
  font-weight: 700;
  color: #2c3e50;
}

@keyframes fadeIn {
  from { opacity: 0; transform: translateY(6px); }
  to { opacity: 1; transform: translateY(0); }
}
</style>
