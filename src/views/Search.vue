<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, inject, watch, type Ref } from "vue";
import { useLcuStore } from "../store/lcuStore";
import { fetchMatchHistory, lcuRequest, batchUploadMatches, fetchConfig, updateConfig } from "../api/lcu";
import type { SummonerDisplay, MatchDisplay } from "../api/lcu";
import LcuImage from "../components/LcuImage.vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const store = useLcuStore();
const navigateTo = inject<(page: string) => void>("navigateTo")!;
const searchName = ref("");

// Toast 通知
const toast = ref<{ message: string; type: 'success' | 'error'; visible: boolean }>({
  message: '', type: 'success', visible: false
});
let toastTimer: ReturnType<typeof setTimeout> | null = null;
function showToast(message: string, type: 'success' | 'error' = 'success') {
  if (toastTimer) clearTimeout(toastTimer);
  toast.value = { message, type, visible: true };
  toastTimer = setTimeout(() => { toast.value.visible = false; }, 3000);
}
const searching = ref(false);
const error = ref("");
const summoner = ref<SummonerDisplay | null>(null);
const matches = ref<MatchDisplay[]>([]);

// 游戏模式筛选
const selectedQueue = ref<number | null>(null); // null = 全部
const showQueueDropdown = ref(false);
const QUEUE_OPTIONS = [
  { id: null, label: '全部' },
  { id: 2400, label: '海克斯大乱斗' },
  { id: 450, label: '极地大乱斗' },
  { id: 430, label: '匹配模式' },
  { id: 420, label: '单双排位' },
  { id: 440, label: '灵活排位' },
];

// 上传相关
const uploadEnabled = ref(true);

async function onUploadToggle() {
  uploadEnabled.value = !uploadEnabled.value;
  try {
    const cfg = await fetchConfig();
    cfg.Functions.UploadEnabled = uploadEnabled.value;
    await updateConfig(cfg);
  } catch (e) {
    console.error("保存上传配置失败:", e);
  }
}
const uploadedGameIds = ref(new Set<number>());

const filteredMatches = computed(() => {
  if (selectedQueue.value === null) return matches.value;
  return matches.value.filter((m: MatchDisplay) => m.queueId === selectedQueue.value);
});

function selectQueue(id: number | null) {
  selectedQueue.value = id;
}

// 对局详情相关
const selectedGameId = ref<number | null>(null);
const selectedGame = ref<any | null>(null);
const gameLoading = ref(false);
const participantRanks = ref<Record<string, string>>({});

const appConfig = ref<any>(null);

const TIER_MAP: Record<string, string> = {
  NONE: "", IRON: "黑铁", BRONZE: "黄铜", SILVER: "白银", GOLD: "黄金",
  PLATINUM: "铂金", EMERALD: "翡翠", DIAMOND: "钻石",
  MASTER: "大师", GRANDMASTER: "宗师", CHALLENGER: "王者",
};
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

  // 如果输入框内容和当前展示的召唤师姓名完全一致（代表没有进行新的输入，仅仅是点击聚焦），则展示全部历史记录
  if (summoner.value) {
    const gn = summoner.value.gameName || summoner.value.displayName;
    const tl = summoner.value.tagLine;
    const currentRiotId = tl ? `${gn}#${tl}` : gn;
    if (q === currentRiotId.toLowerCase()) {
      return searchHistory.value;
    }
  }

  return searchHistory.value.filter((h: string) => h.toLowerCase().includes(q));
});

// 从 App.vue 注入 Career → Search 跳转状态
const navigateSearchPayload = inject<Ref<{ name: string; gameId: number | null } | null>>("navigateSearchPayload")!;

let unlistenGameDataReady: (() => void) | null = null;

onMounted(async () => {
  loadSearchHistory();
  document.addEventListener("click", onDocClick);
  try {
    const cfg = await fetchConfig();
    appConfig.value = cfg;
    uploadEnabled.value = cfg.Functions?.UploadEnabled ?? true;
  } catch (e) {
    console.warn("加载上传配置失败，使用默认值:", e);
  }
  try {
    unlistenGameDataReady = await listen("game-data-ready", async () => {
      try {
        gameDataAssets.value = await invoke("get_game_data_assets");
      } catch (e) {
        console.error("收到就绪事件后加载静态资源映射失败:", e);
      }
    });
  } catch (e) {
    console.error("订阅 game-data-ready 事件失败:", e);
  }
});

onUnmounted(() => {
  document.removeEventListener("click", onDocClick);
  if (unlistenGameDataReady) {
    unlistenGameDataReady();
  }
});

function onDocClick() {
  showQueueDropdown.value = false;
}

// 监听 LCU 连接状态，当连接成功后重新拉取静态资源映射
watch(
  () => store.isConnected,
  async (connected) => {
    if (connected) {
      try {
        gameDataAssets.value = await invoke("get_game_data_assets");
      } catch (e) {
        console.error("加载静态资源数据映射失败:", e);
      }
    } else {
      gameDataAssets.value = null;
    }
  },
  { immediate: true }
);

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
  uploadedGameIds.value = new Set();

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

    // 搜索成功后保存到历史记录（用 gameName#tagLine 格式，方便下次直接查询）
    const gn = summoner.value.gameName || summoner.value.displayName || name;
    const tl = summoner.value.tagLine;
    const historyKey = tl ? `${gn}#${tl}` : gn;
    saveToHistory(historyKey);

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

    // 自动批量上传当前页对局（去重 + fire-and-forget）
    if (uploadEnabled.value && matches.value.length > 0) {
      const newIds = matches.value
        .map((m: MatchDisplay) => m.gameId)
        .filter((id: number) => !uploadedGameIds.value.has(id));
      if (newIds.length > 0) {
        newIds.forEach((id: number) => uploadedGameIds.value.add(id));
        console.log(`[upload] 开始批量上传 ${newIds.length} 场对局:`, newIds);
        batchUploadMatches(newIds).then((result: { successCount: number; failedCount: number; error: string | null }) => {
          console.log(`[upload] 批量上传结果: 成功=${result.successCount}, 失败=${result.failedCount}, error=${result.error}`);
          if (result.error && result.successCount === 0) {
            showToast(`上传失败: ${result.error}`, 'error');
          } else if (result.successCount > 0) {
            showToast(`已上传 ${result.successCount} 场对局`);
          } else {
            showToast('所有对局已存在，无需上传');
          }
        }).catch((e: any) => {
          console.error("[upload] 批量上传异常:", e);
          showToast(`上传异常: ${e?.message || e}`, 'error');
        });
      } else {
        console.log("[upload] 当前页对局均已上传过，跳过");
      }
    }
  } catch (e) {
    console.error("抓取战绩列表失败:", e);
  }
}

async function selectMatch(gameId: number) {
  selectedGameId.value = gameId;
  gameLoading.value = true;
  try {
    const resp = await lcuRequest<any>("GET", `/lol-match-history/v1/games/${gameId}`);
    if (resp.success && resp.data) {
      selectedGame.value = resp.data;

      // 清空上次对局玩家的段位缓存
      participantRanks.value = {};

      const g = resp.data;
      const participants = g.participants || [];
      const identities = g.participantIdentities || [];

      // 如果开启了显示段位选项，则并发拉取所有玩家的段位
      const showTier = appConfig.value?.Functions?.ShowTierInGameInfo ?? false;
      if (showTier && participants.length > 0) {
        const playerPuuids: string[] = [];
        for (const identity of identities) {
          if (identity.player?.puuid && identity.player.summonerId) { // 排除机器人
            playerPuuids.push(identity.player.puuid);
          }
        }

        // 并发拉取段位数据
        const rankPromises = playerPuuids.map(async (puuid) => {
          try {
            const rResp = await lcuRequest<any>("GET", `/lol-ranked/v1/ranked-stats/${puuid}`);
            if (rResp.success && rResp.data?.queues) {
              const queues = rResp.data.queues;
              // 优先单双排，其次灵活排位
              const solo = queues.find((q: any) => q.queueType === "RANKED_SOLO_5x5");
              const flex = queues.find((q: any) => q.queueType === "RANKED_FLEX_SR");
              const activeQueue = solo || flex;
              if (activeQueue && activeQueue.tier && activeQueue.tier !== "NONE") {
                const tier = TIER_MAP[activeQueue.tier] || activeQueue.tier;
                const div = activeQueue.rank && activeQueue.rank !== "NA" ? activeQueue.rank : "";
                return { puuid, rankStr: `${tier}${div}` };
              }
            }
          } catch (e) {
            console.error(`拉取 PUUID 为 ${puuid} 的段位失败:`, e);
          }
          return { puuid, rankStr: "" };
        });

        const rankResults = await Promise.all(rankPromises);
        for (const res of rankResults) {
          if (res.rankStr) {
            participantRanks.value[res.puuid] = res.rankStr;
          }
        }
      }
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
  const prevPage = currentPageNum.value;
  currentPageNum.value++;
  const prevMatches = matches.value;
  await loadMatchHistoryList();
  // 如果新页没有数据，回退页码并保留原有数据
  if (matches.value.length === 0) {
    currentPageNum.value = prevPage;
    matches.value = prevMatches;
  }
}

// 静态映射查找
function getSpellUrl(spellId: number) {
  const path = gameDataAssets.value?.spells?.[spellId];
  if (!path) return "";
  return path.startsWith("/") ? path : "/" + path;
}

function getRuneUrl(runeId: number) {
  const path = gameDataAssets.value?.runes?.[runeId];
  if (!path) return "";
  return path.startsWith("/") ? path : "/" + path;
}

function getItemUrl(itemId: number) {
  if (!itemId) return "";
  const mapped = gameDataAssets.value?.items?.[itemId];
  if (mapped) {
    return mapped.startsWith("/") ? mapped : "/" + mapped;
  }
  // 回退：LCU 标准物品图标路径（小写）
  return `/lol-game-data/assets/v1/items/icons2d/${itemId}.png`;
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
      const baseName = player?.gameName || player?.summonerName || "未知";
      const tag = player?.tagLine;
      playerMap[pId] = {
        name: tag ? `${baseName}#${tag}` : baseName,
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
    400: "征召模式", 420: "排位单双排", 430: "匹配模式", 440: "排位灵活组排", 490: "快速游戏",
    450: "极地大乱斗", 2400: "海克斯大乱斗",
    900: "无限火力", 1010: "随机无限火力", 1020: "克隆模式",
    1300: "极限闪击", 1700: "斗魂竞技场", 1710: "斗魂竞技场",
    1810: "捉鬼模式", 1820: "捉鬼模式", 1830: "捉鬼模式", 1840: "捉鬼模式",
    0: "自定义模式"
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
  let queriedPlayerChampionIconUrl = "";
  if (summoner.value) {
    const queriedPuuid = summoner.value.puuid;
    const allPlayers = [...bluePlayers, ...redPlayers];
    const found = allPlayers.find(p => p.puuid === queriedPuuid);
    if (found) {
      isQueriedPlayerWin = found.win;
      queriedPlayerChampionIconUrl = found.championIconUrl;
    }
  }

  // 计算地图状态图标名（根据 mapId 和 win）
  const resultStr = isQueriedPlayerWin ? "victory" : "defeat";
  let mapKey = "other";
  if (g.mapId === 11) {
    mapKey = "sr";
  } else if (g.mapId === 12) {
    mapKey = "ha";
  } else if (g.mapId === 30 || g.queueId === 1700) {
    mapKey = "arena";
  }
  const mapIconUrl = `/images/${mapKey}-${resultStr}.png`;

  return {
    gameId: g.gameId,
    duration: durationStr,
    date: dateStr,
    queueName: queueNames[g.queueId] || "自定义模式",
    mapName: mapNames[g.mapId] || "未知地图",
    win: isQueriedPlayerWin,
    queriedPlayerChampionIconUrl,
    mapIconUrl,
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
    <!-- Toast -->
    <Transition name="toast">
      <div v-if="toast.visible" :class="['toast', `toast-${toast.type}`]">{{ toast.message }}</div>
    </Transition>

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
            @click="showHistory = true"
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
            <div class="history-header">
              <span class="history-title">🕐 最近搜索</span>
            </div>
            <div class="history-tags-container">
              <div
                v-for="item in filteredHistory"
                :key="item"
                class="history-tag"
                @mousedown.prevent="selectHistory(item)"
              >
                <span class="history-text" :title="item">{{ item }}</span>
                <span class="history-delete" @mousedown.prevent.stop="removeFromHistory(item)" title="删除">✕</span>
              </div>
            </div>
          </div>
        </div>
        
        <button class="tab-btn active" @click="navigateTo('career')">生涯</button>

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
              @click="selectQueue(q.id); showQueueDropdown = false"
            >
              {{ q.label }}
            </div>
          </div>
        </div>

        <label class="checkbox-wrapper">
          <input type="checkbox" :checked="uploadEnabled" @change="onUploadToggle" />
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
              v-for="m in filteredMatches"
              :key="m.gameId"
              :class="['mini-match-card', m.win ? 'win' : 'lose', { selected: selectedGameId === m.gameId }]"
              @click="selectMatch(m.gameId)"
            >
              <div class="mini-avatar">
                <LcuImage :src="m.championIconUrl" alt="champ" />
              </div>
              <div class="mini-info">
                <span class="mini-mode">{{ m.name }}</span>
                <span class="mini-time-kda">
                  {{ m.shortTime.split(' ')[0] }} &nbsp;&nbsp;
                  {{ m.kills }}/<span class="death-red">{{ m.deaths }}</span>/{{ m.assists }}
                </span>
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
          <div v-if="gameLoading && !gameDetails" class="detail-loading">
            <div class="loading-spinner"></div>
            <span>拉取对局详情数据中...</span>
          </div>

          <div v-show="gameDetails" class="detail-content">
            <!-- 头部大 Banner -->
            <div v-if="gameDetails" :class="['detail-banner', gameDetails.win ? 'win' : 'lose']">
              <div class="banner-main">
                <div class="banner-map-icon">
                  <img :src="gameDetails.mapIconUrl" alt="map" />
                </div>
                <div class="banner-left">
                  <h2 :class="['banner-result', gameDetails.win ? 'win' : 'lose']">
                    {{ gameDetails.win ? '胜利' : '失败' }}
                  </h2>
                  <span class="banner-subtext">
                    {{ gameDetails.mapName }} · {{ gameDetails.queueName }} · {{ gameDetails.duration }} · {{ gameDetails.date }} · 游戏 ID: {{ gameDetails.gameId }}
                  </span>
                </div>
              </div>
              <button class="copy-btn" @click="copyGameId(gameDetails.gameId)" title="复制游戏 ID">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
                  <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
                </svg>
              </button>
            </div>

            <!-- 队伍详细数据 -->
            <div v-if="gameDetails" class="teams-container">
              <!-- 胜方 / 败方 -->
              <div v-for="team in [gameDetails.blue, gameDetails.red]" :key="team.win ? 'win' : 'lose'" :class="['team-block', team.win ? 'win-block' : 'lose-block']">
                <!-- 队头资源概览 -->
                <div :class="['team-header-bar', team.win ? 'win-bar' : 'lose-bar']">
                  <span :class="['team-result-label', team.win ? 'win-text' : 'lose-text']">
                    {{ team.win ? '胜方' : '败方' }}
                  </span>

                  <div class="team-objectives">
                    <span class="obj-item" title="击杀"><span class="obj-icon">⚔️</span> {{ team.kills }}</span>
                    <span class="obj-item" title="摧毁防御塔"><span class="obj-icon">🏰</span> {{ team.towerKills }}</span>
                    <span class="obj-item" title="摧毁水晶"><span class="obj-icon">💎</span> {{ team.inhibitorKills }}</span>
                    <span class="obj-item" title="击杀纳什男爵"><span class="obj-icon">👾</span> {{ team.baronKills }}</span>
                    <span class="obj-item" title="击杀巨龙"><span class="obj-icon">🐉</span> {{ team.dragonKills }}</span>
                    <span class="obj-item" title="击杀峡谷先锋 / 虚空巢虫"><span class="obj-icon">🦀</span> {{ team.riftHeraldKills }}</span>
                  </div>

                  <div class="team-header-spacer"></div>

                  <div class="team-header-right">
                    <span class="header-items">装备</span>
                    <span class="header-kda">KDA</span>
                    <span class="header-cs">补CS</span>
                    <span class="header-gold">金币</span>
                    <span class="header-damage">伤害</span>
                  </div>
                </div>

                <!-- 玩家列表 -->
                <div class="players-table">
                  <div
                    v-for="p in team.players"
                    :key="p.participantId"
                    :class="['player-row', { 'highlight-row': summoner && p.puuid === summoner.puuid, 'win-row': team.win, 'lose-row': !team.win }]"
                  >
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
                        <div v-if="selectedGame?.queueId !== 2400" class="row-rune">
                          <LcuImage :src="p.runeUrl" class="row-rune-img" alt="rune" />
                        </div>
                      </div>
                    </div>

                    <!-- 名字（可点击搜索，机器人除外） -->
                    <div class="player-name-col">
                      <span
                        :class="['row-name', { 'highlight-user': summoner && p.puuid === summoner.puuid, 'bot-player': !p.summonerId }]"
                        @click="p.summonerId && searchPlayerBySummonerId(p.summonerId, p.name)"
                        :title="p.summonerId ? `搜索 ${p.name}` : '机器人'"
                      >
                        {{ p.name }}
                      </span>
                      <span v-if="participantRanks[p.puuid]" class="row-rank-badge" :title="`段位: ${participantRanks[p.puuid]}`">
                        {{ participantRanks[p.puuid] }}
                      </span>
                    </div>

                    <div class="player-spacer"></div>

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
          <div v-if="!gameDetails" class="detail-empty">
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
  padding: 1rem 1.5rem 1.5rem 0.6rem;
  background-color: transparent;
  min-height: 100%;
  font-family: "Segoe UI", "Microsoft YaHei", -apple-system, BlinkMacSystemFont, sans-serif;
  font-variant-numeric: tabular-nums;
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
  padding: 8px 16px;
  border-radius: 6px;
  margin-bottom: 1rem;
  font-size: 0.82rem;
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
  background: var(--card-bg);
  border: 1px solid rgba(0, 0, 0, 0.05);
  padding: 10px 16px;
  border-radius: 8px;
  margin-bottom: 1.2rem;
  box-shadow: var(--shadow-sm);
  position: sticky;
  top: 0;
  z-index: 50;
}

.search-input-wrapper {
  position: relative;
  display: flex;
  flex: 1;
  max-width: 600px;
}

/* 搜索历史下拉框 */
.history-dropdown {
  position: absolute;
  top: 100%;
  left: 0;
  right: 0;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-top: none;
  border-radius: 0 0 8px 8px;
  box-shadow: var(--shadow-lg);
  z-index: 200;
  max-height: 260px;
  overflow-y: auto;
  padding: 12px;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.history-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px dashed var(--border-color);
  padding-bottom: 6px;
  user-select: none;
}

.history-title {
  font-size: 0.72rem;
  font-weight: 700;
  color: var(--text-muted);
  letter-spacing: 0.5px;
}

.history-tags-container {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.history-tag {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  background: rgba(0, 0, 0, 0.02);
  border: 1px solid var(--border-color);
  border-radius: 14px;
  cursor: pointer;
  transition: all 0.2s ease-in-out;
  max-width: 160px;
  box-sizing: border-box;
}

.history-tag:hover {
  background: var(--primary-color-alpha-10);
  border-color: rgba(var(--primary-color-rgb, 59, 130, 246), 0.3);
  transform: translateY(-1px);
}

.history-tag:hover .history-text {
  color: var(--primary-color);
}

.history-text {
  font-size: 0.78rem;
  color: var(--text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  font-weight: 600;
  transition: color 0.2s ease-in-out;
}

.history-delete {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 14px;
  height: 14px;
  font-size: 0.62rem;
  color: var(--text-dimmed);
  border-radius: 50%;
  transition: all 0.15s ease-in-out;
  flex-shrink: 0;
}

.history-delete:hover {
  background: var(--loss-bg);
  color: var(--loss-color);
  transform: scale(1.1);
}

.search-input {
  width: 100%;
  padding: 6px 36px 6px 12px;
  border: 1px solid var(--border-color);
  background: var(--card-bg);
  border-radius: 6px;
  font-size: 0.85rem;
  color: var(--text-color);
  outline: none;
  transition: all 0.2s;
  text-align: center;
  height: 32px;
}

.search-input:focus {
  border-color: var(--primary-color);
  box-shadow: 0 0 8px var(--primary-color-alpha-15);
  background: var(--card-bg);
}

.search-trigger-btn {
  position: absolute;
  right: 8px;
  top: 50%;
  transform: translateY(-50%);
  background: transparent;
  border: none;
  cursor: pointer;
  color: var(--text-muted);
  display: flex;
  align-items: center;
  padding: 4px;
}

.search-icon {
  width: 16px;
  height: 16px;
}

.tab-btn {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  color: var(--text-color);
  padding: 0 16px;
  border-radius: 6px;
  font-size: 0.82rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
  height: 32px;
  display: inline-flex;
  align-items: center;
}

.tab-btn:hover {
  background: var(--card-bg);
  color: var(--text-color);
  border-color: var(--primary-color);
}

.tab-btn.active {
  background-color: var(--card-bg);
  color: var(--text-color);
  border-color: var(--border-color);
  font-weight: 600;
  box-shadow: none;
}

.tab-btn.active:hover {
  background: var(--card-bg);
  border-color: var(--primary-color);
}

.dropdown-trigger {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  padding: 4px 10px;
  border-radius: 6px;
  font-size: 0.82rem;
  color: var(--text-color);
  cursor: pointer;
  position: relative;
  transition: all 0.2s;
  height: 32px;
}

.dropdown-trigger:hover {
  background: var(--card-bg);
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
  background: rgba(0, 0, 0, 0.02);
  color: var(--text-color);
}

.queue-dropdown-item.active {
  color: var(--primary-color);
  font-weight: 600;
  background: var(--primary-color-alpha-15);
}

.checkbox-wrapper {
  margin-left: auto;
  display: inline-flex;
  align-items: center;
  gap: 8px;
  font-size: 0.82rem;
  color: var(--text-muted);
  cursor: pointer;
}

/* 分栏大布局 */
.panel-layout {
  display: grid;
  grid-template-columns: 180px 1fr;
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
  gap: 8px;
  flex: 1;
  overflow-y: auto;
  padding-right: 2px;
}

.mini-match-card {
  display: flex;
  align-items: center;
  padding: 10px 12px;
  border-radius: 6px;
  border: 1px solid rgba(0, 0, 0, 0.05);
  cursor: pointer;
  transition: all 0.2s ease;
  background: var(--card-bg);
}

.mini-match-card.win {
  background-color: var(--win-bg);
  border-color: var(--win-border);
}

.mini-match-card.win:hover {
  background-color: var(--win-bg);
}

[data-theme="dark"] .mini-match-card.win:hover {
  background-color: rgba(34, 197, 94, 0.15);
}

.mini-match-card.lose {
  background-color: var(--loss-bg);
  border-color: var(--loss-border);
}

.mini-match-card.lose:hover {
  background-color: var(--loss-bg);
}

[data-theme="dark"] .mini-match-card.lose:hover {
  background-color: rgba(239, 68, 68, 0.15);
}

.mini-match-card.selected.win {
  border: 2px solid #22c55e;
}

.mini-match-card.selected.lose {
  border: 2px solid #ef4444;
}

.mini-avatar {
  width: 36px;
  height: 36px;
  border-radius: 50%;
  overflow: hidden;
  border: 1px solid rgba(0, 0, 0, 0.05);
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
  font-size: 0.78rem;
  font-weight: 700;
  color: var(--text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.mini-time-kda {
  font-size: 0.72rem;
  color: var(--text-muted);
  margin-top: 2px;
}

.death-red {
  color: var(--death-color, var(--loss-color));
  font-weight: 600;
}

/* 分页 */
.pagination {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 16px;
  margin-top: 8px;
  background: var(--card-bg);
  border: 1px solid rgba(0, 0, 0, 0.05);
  padding: 6px;
  border-radius: 6px;
  box-shadow: var(--shadow-sm);
}

.page-btn {
  background: transparent;
  border: none;
  border-radius: 4px;
  width: 28px;
  height: 28px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  color: var(--text-color);
  transition: all 0.2s;
}

.page-btn:hover:not(:disabled) {
  background: rgba(0, 0, 0, 0.03);
  color: var(--primary-color);
}

.page-btn:disabled {
  opacity: 0.3;
  cursor: not-allowed;
}

.page-btn svg {
  width: 14px;
  height: 14px;
}

.page-num {
  font-size: 0.82rem;
  font-weight: 700;
  color: var(--text-color);
}

/* 右侧详情面板 */
.right-detail-panel {
  background: transparent;
  border: none;
  box-shadow: none;
  min-height: 640px;
}

.detail-content {
  position: relative;
}

.detail-loading, .detail-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 640px;
  color: var(--text-muted);
  font-size: 0.85rem;
  background: var(--card-bg);
  border: 1px solid rgba(0, 0, 0, 0.05);
  border-radius: 8px;
  box-shadow: var(--shadow-sm);
}

.loading-spinner {
  width: 36px;
  height: 36px;
  border: 3px solid rgba(0, 0, 0, 0.05);
  border-top-color: var(--primary-color);
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
  padding: 12px 16px;
  margin: 12px 12px 0 12px;
  border: 1px solid rgba(0, 0, 0, 0.05);
  border-radius: 8px;
  box-shadow: var(--shadow-sm);
}

.detail-banner.win {
  background-color: var(--win-bg);
  border-color: var(--win-border);
}

.detail-banner.lose {
  background-color: var(--loss-bg);
  border-color: var(--loss-border);
}

.banner-main {
  display: flex;
  align-items: center;
  gap: 12px;
}

.banner-map-icon {
  width: 54px;
  height: 54px;
  border-radius: 6px;
  overflow: hidden;
  border: 1px solid rgba(0, 0, 0, 0.05);
  flex-shrink: 0;
}

.banner-map-icon img {
  width: 100%;
  height: 100%;
  display: block;
}

.banner-left {
  display: flex;
  flex-direction: column;
}

.banner-result {
  font-size: 1.25rem;
  font-weight: 800;
  margin: 0 0 2px;
}

.banner-result.win { color: var(--win-color); }
.banner-result.lose { color: var(--loss-color); }

.banner-subtext {
  font-size: 0.75rem;
  color: var(--text-muted);
}

.copy-btn {
  background: var(--card-bg);
  border: 1px solid var(--border-color);
  border-radius: 6px;
  width: 30px;
  height: 30px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  color: var(--text-color);
  transition: all 0.2s;
}

.copy-btn:hover {
  background-color: var(--card-bg);
}

[data-theme="dark"] .copy-btn:hover {
  background-color: rgba(30, 41, 59, 0.9);
  border-color: var(--primary-color);
  color: var(--primary-color);
}

.copy-btn svg {
  width: 14px;
  height: 14px;
}

/* 队伍 block */
.teams-container {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 12px;
}

.team-block {
  border: 1px solid var(--border-color);
  border-radius: 8px;
  overflow: hidden;
  background: var(--card-bg);
  box-shadow: var(--shadow-sm);
}

.team-block.win-block {
  border-color: var(--win-border);
}

.team-block.lose-block {
  border-color: var(--loss-border);
}

.team-header-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 16px;
  font-size: 0.85rem;
}

.team-header-bar.win-bar {
  background-color: var(--win-bg);
  border-bottom: 1px solid var(--win-border);
}

.team-header-bar.lose-bar {
  background-color: var(--loss-bg);
  border-bottom: 1px solid var(--loss-border);
}

.team-result-label {
  font-weight: bold;
}
.win-text { color: var(--win-color); }
.lose-text { color: var(--loss-color); }

.team-objectives {
  display: flex;
  align-items: center;
  gap: 10px;
  color: var(--text-muted);
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
  padding: 6px 14px;
  border-bottom: 1px solid var(--border-color);
  font-size: 0.8rem;
  color: var(--text-color);
}

.player-row:last-child {
  border-bottom: none;
}

/* 玩家高亮行 */
.player-row.highlight-row.win-row {
  background-color: var(--win-bg) !important;
}

.player-row.highlight-row.lose-row {
  background-color: var(--loss-bg) !important;
}

.player-row.highlight-row.win-row .row-name,
.player-row.highlight-row.win-row .row-kda-text,
.player-row.highlight-row.win-row .row-kda-text .death-red,
.player-row.highlight-row.win-row .row-cs-text,
.player-row.highlight-row.win-row .row-gold-text,
.player-row.highlight-row.win-row .row-damage-text {
  color: var(--win-color) !important;
  font-weight: 800;
}

.player-row.highlight-row.lose-row .row-name,
.player-row.highlight-row.lose-row .row-kda-text,
.player-row.highlight-row.lose-row .row-kda-text .death-red,
.player-row.highlight-row.lose-row .row-cs-text,
.player-row.highlight-row.lose-row .row-gold-text,
.player-row.highlight-row.lose-row .row-damage-text {
  color: var(--loss-color) !important;
  font-weight: 800;
}

/* 1. 头像区 */
.player-avatar-col {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 120px;
  flex-shrink: 0;
}

.row-avatar-box {
  position: relative;
  width: 40px;
  height: 40px;
  flex-shrink: 0;
}

.row-avatar {
  width: 40px;
  height: 40px;
  border-radius: 50%;
  overflow: hidden;
  border: 1px solid var(--border-color);
}

.row-level-overlay {
  position: absolute;
  bottom: -2px;
  right: -2px;
  width: 14px;
  height: 14px;
  line-height: 12px;
  background: var(--text-color);
  color: var(--bg-color);
  border-radius: 50%;
  font-size: 0.58rem;
  font-weight: bold;
  text-align: center;
  border: 1px solid var(--card-bg);
}

[data-theme="dark"] .row-level-overlay {
  background: var(--card-bg);
  color: var(--text-color);
  border-color: rgba(255,255,255,0.2);
}

.row-spell-rune-row {
  display: flex;
  align-items: center;
  gap: 3px;
}

.row-spell-col {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.row-spell {
  width: 18px;
  height: 18px;
  border-radius: 2px;
  border: 1px solid var(--border-color);
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

.player-name-col {
  width: 120px;
  min-width: 0;
  padding-right: 6px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
}

.row-rank-badge {
  display: inline-block;
  font-size: 10px;
  color: var(--primary-color);
  background: rgba(142, 68, 173, 0.08);
  padding: 1px 4px;
  border-radius: 4px;
  margin-top: 2px;
  align-self: flex-start;
  font-weight: 500;
  max-width: 100%;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.row-name {
  display: block;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  color: var(--text-color);
  cursor: pointer;
  transition: color 0.15s;
}

.row-name:hover {
  color: var(--primary-color);
}

.bot-player {
  cursor: default;
  color: var(--text-dimmed);
}

.bot-player:hover {
  color: var(--text-dimmed);
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
  gap: 3px;
  width: 190px;
  flex-shrink: 0;
}

.row-items-grid {
  display: flex;
  gap: 1px;
}

.row-item-slot {
  width: 26px;
  height: 26px;
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
  width: 26px;
  height: 26px;
  border-radius: 3px;
  overflow: hidden;
  border: 1px solid #e6a23c;
  background-color: rgba(230,162,60,0.03);
}

/* 4. KDA */
.player-kda-col {
  width: 70px;
  text-align: center;
  font-weight: 600;
  flex-shrink: 0;
}

.row-kda-text {
  font-size: 0.8rem;
}

/* 5. 补兵 */
.player-cs-col {
  width: 42px;
  text-align: center;
  color: var(--text-muted);
  flex-shrink: 0;
}

.row-cs-text {
  font-size: 0.8rem;
}

/* 6. 金币 */
.player-gold-col {
  width: 55px;
  text-align: right;
  color: var(--text-muted);
  flex-shrink: 0;
}

.row-gold-text {
  font-size: 0.8rem;
}

/* 7. 伤害 */
.player-damage-col {
  width: 60px;
  text-align: right;
  font-weight: 700;
  color: var(--text-color);
  flex-shrink: 0;
}

.row-damage-text {
  font-size: 0.8rem;
}

.player-spacer,
.team-header-spacer {
  flex: 1;
}

.team-header-right {
  display: flex;
  align-items: center;
  font-size: 0.72rem;
  font-weight: 700;
  color: var(--text-muted);
  padding-right: 2px;
}

.header-items {
  width: 190px;
  text-align: center;
}

.header-kda {
  width: 70px;
  text-align: center;
}

.header-cs {
  width: 42px;
  text-align: center;
}

.header-gold {
  width: 55px;
  text-align: right;
}

.header-damage {
  width: 60px;
  text-align: right;
}

@keyframes fadeIn {
  from { opacity: 0; transform: translateY(6px); }
  to { opacity: 1; transform: translateY(0); }
}

/* Toast 通知 */
.toast {
  position: fixed; top: 20px; left: 50%; transform: translateX(-50%);
  padding: 10px 24px; border-radius: 8px; font-size: 0.82rem;
  font-weight: 600; color: white; z-index: 9999;
  box-shadow: var(--shadow-md); pointer-events: none;
}
.toast-success { background-color: var(--primary-color); }
.toast-error { background-color: var(--loss-color); }
.toast-enter-active { transition: all 0.25s ease-out; }
.toast-leave-active { transition: all 0.2s ease-in; }
.toast-enter-from { opacity: 0; transform: translateX(-50%) translateY(-12px); }
.toast-leave-to { opacity: 0; transform: translateX(-50%) translateY(-8px); }
</style>

