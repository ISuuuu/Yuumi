import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useLcuStore } from "../store/lcuStore";

export interface TftRankDisplay {
  soloTier: string;
  soloDivision: string;
  soloLp: number;
  soloWins: number;
  soloLosses: number;
  turboTier: string;
  turboRating: number;
  turboWins: number;
  doubleTier: string;
  doubleDivision: string;
  doubleLp: number;
  doubleWins: number;
  doubleLosses: number;
}

export interface TftUnitDisplay {
  characterId: string;
  name: string;
  iconUrl: string;
  rarity: number;
  tier: number;
  itemNames: string[];
  itemIconUrls: string[];
}

export interface TftTraitDisplay {
  name: string;
  numUnits: number;
  tierCurrent: number;
  iconUrl: string;
}

export interface TftParticipantDisplay {
  puuid: string;
  summonerName: string;
  isSelf: boolean;
  placement: number;
  level: number;
  goldLeft: number;
  totalDamageToPlayers: number;
  companionIconUrl: string;
  traits: TftTraitDisplay[];
  units: TftUnitDisplay[];
  augments: string[];
}

export interface TftMatchDisplay {
  gameId: number;
  queueId: number;
  queueName: string;
  gameCreation: number;
  gameDuration: number;
  timeStr: string;
  durationStr: string;
  placement: number;
  level: number;
  goldLeft: number;
  totalDamageToPlayers: number;
  companionIconUrl: string;
  traits: TftTraitDisplay[];
  units: TftUnitDisplay[];
  augments: string[];
  participants: TftParticipantDisplay[];
}

export interface TftMatchSummary {
  totalGames: number;
  winCount: number;
  top4Count: number;
  top4Rate: number;
  winRate: number;
  avgPlacement: number;
  matches: TftMatchDisplay[];
}

// 模块作用域内存缓存
let cachedRankedStats: TftRankDisplay | null = null;
let cachedSummary: TftMatchSummary | null = null;

export function useTftData() {
  const store = useLcuStore();

  const loading = ref(false);
  const error = ref("");
  const rankedStats = ref<TftRankDisplay | null>(cachedRankedStats);
  const summary = ref<TftMatchSummary | null>(cachedSummary);

  const matches = computed(() => summary.value?.matches || []);

  async function loadTftData(forceRefresh = false) {
    if (!store.isConnected) return;
    if (!forceRefresh && cachedRankedStats && cachedSummary) {
      rankedStats.value = cachedRankedStats;
      summary.value = cachedSummary;
      return;
    }

    loading.value = true;
    error.value = "";

    try {
      // 1. 获取当前登录召唤师 PUUID
      const summoner = await invoke<{ puuid: string }>("get_current_summoner");
      if (!summoner || !summoner.puuid) {
        throw new Error("无法获取召唤师信息");
      }

      // 2. 并行获取云顶段位和战绩列表
      const [rankData, historyData] = await Promise.all([
        invoke<TftRankDisplay>("get_tft_ranked_stats", {
          puuid: summoner.puuid,
        }),
        invoke<TftMatchSummary>("get_tft_match_history", {
          puuid: summoner.puuid,
          begIndex: 0,
          endIndex: 20,
        }),
      ]);

      cachedRankedStats = rankData;
      cachedSummary = historyData;

      rankedStats.value = rankData;
      summary.value = historyData;
    } catch (e: any) {
      console.error("加载 TFT 数据失败:", e);
      error.value = e?.toString() || "加载 TFT 数据失败";
    } finally {
      loading.value = false;
    }
  }

  function refresh() {
    return loadTftData(true);
  }

  return {
    loading,
    error,
    rankedStats,
    summary,
    matches,
    loadTftData,
    refresh,
  };
}
