import {
  lcuRequest,
  querySavedPlayersMap,
  type LcuApiResponse,
  type SavedPlayerMarker,
} from "../api/lcu";
import type {
  ChampionMasteryItem,
  RawChampionMastery,
} from "../types/gameInfo";
import type { RankedStats } from "../types/lcu";

// 注意：本模块的缓存/in-flight 去重均为全局单例，多个对局页实例共享同一份数据。

// ── 排位数据缓存（puuid → { data, timestamp }），带 LRU / 容量上限保护，避免内存泄露
const rankCache = new Map<string, { data: RankedStats; timestamp: number }>();
const RANK_CACHE_TTL = 5 * 60 * 1000; // 5 分钟
const RANK_CACHE_MAX_SIZE = 100; // 最多缓存 100 个玩家的排位信息

function getRankFromCache(puuid: string) {
  const cached = rankCache.get(puuid);
  if (!cached) return null;
  if (Date.now() - cached.timestamp >= RANK_CACHE_TTL) {
    rankCache.delete(puuid);
    return null;
  }
  return cached.data;
}

function setRankToCache(puuid: string, data: RankedStats) {
  if (rankCache.size >= RANK_CACHE_MAX_SIZE) {
    const firstKey = rankCache.keys().next().value;
    if (firstKey) rankCache.delete(firstKey);
  }
  rankCache.set(puuid, { data, timestamp: Date.now() });
}

// ── 英雄熟练度数据缓存（puuid → { data, timestamp }）
const masteryCache = new Map<
  string,
  { data: ChampionMasteryItem[]; timestamp: number }
>();
const MASTERY_CACHE_TTL = 5 * 60 * 1000; // 5 分钟
const MASTERY_CACHE_MAX_SIZE = 100;

function getMasteryFromCache(puuid: string) {
  const cached = masteryCache.get(puuid);
  if (!cached) return null;
  if (Date.now() - cached.timestamp >= MASTERY_CACHE_TTL) {
    masteryCache.delete(puuid);
    return null;
  }
  return cached.data;
}

function setMasteryToCache(puuid: string, data: ChampionMasteryItem[]) {
  if (masteryCache.size >= MASTERY_CACHE_MAX_SIZE) {
    const firstKey = masteryCache.keys().next().value;
    if (firstKey) masteryCache.delete(firstKey);
  }
  masteryCache.set(puuid, { data, timestamp: Date.now() });
}

// ── 路人集标记映射共享查询：GameInfo 视图与数据层共用，同 puuid 并发只发一次
let savedPlayersMapInflight: {
  puuid: string;
  promise: Promise<Record<string, SavedPlayerMarker>>;
} | null = null;

export function querySavedPlayersMapDedup(
  puuid: string,
): Promise<Record<string, SavedPlayerMarker>> {
  if (savedPlayersMapInflight?.puuid === puuid) {
    return savedPlayersMapInflight.promise;
  }
  const promise = querySavedPlayersMap(puuid).finally(() => {
    if (savedPlayersMapInflight?.promise === promise) {
      savedPlayersMapInflight = null;
    }
  });
  savedPlayersMapInflight = { puuid, promise };
  return promise;
}

/** 排位信息：带 5 分钟缓存，缓存命中直接返回 */
export async function fetchRankedStatsCached(
  puuid: string,
): Promise<LcuApiResponse<RankedStats>> {
  const cached = getRankFromCache(puuid);
  if (cached) {
    return { success: true, data: cached };
  }
  return lcuRequest<RankedStats>("GET", `/lol-ranked/v1/ranked-stats/${puuid}`)
    .then((rResp) => {
      if (rResp.success && rResp.data) {
        setRankToCache(puuid, rResp.data);
      }
      return rResp;
    })
    .catch(() => ({ success: false as const }));
}

export async function fetchPlayerMastery(
  puuid?: string,
  summonerId?: number,
  isMe?: boolean,
): Promise<ChampionMasteryItem[]> {
  if (!puuid && !summonerId) return [];

  if (puuid) {
    const cached = getMasteryFromCache(puuid);
    if (cached && cached.length > 0) {
      return cached;
    }
  }

  // 1. 优先按 puuid 查询
  let mResp: LcuApiResponse<RawChampionMastery[]> = { success: false };
  if (puuid) {
    mResp = await lcuRequest<RawChampionMastery[]>(
      "GET",
      `/lol-champion-mastery/v1/${puuid}/champion-mastery`,
    );
  }

  // 2. 若是当前玩家且按 puuid 失败（或无 puuid），降级到 local-player
  if ((!mResp.success || !mResp.data) && isMe) {
    mResp = await lcuRequest<RawChampionMastery[]>(
      "GET",
      "/lol-champion-mastery/v1/local-player/champion-mastery",
    );
  }

  // 3. 如果仍未成功，尝试按 summonerId 查询
  if ((!mResp.success || !mResp.data) && summonerId) {
    mResp = await lcuRequest<RawChampionMastery[]>(
      "GET",
      `/lol-champion-mastery/v1/summoners/${summonerId}/champion-mastery`,
    );
  }

  if (mResp.success && Array.isArray(mResp.data)) {
    const normalized: ChampionMasteryItem[] = mResp.data.map(
      (item: RawChampionMastery): ChampionMasteryItem => ({
        championId: Number(item.championId ?? item.champion_id ?? 0),
        championLevel: Number(
          item.championLevel ?? item.masteryLevel ?? item.level ?? 0,
        ),
        championPoints: Number(
          item.championPoints ?? item.points ?? item.score ?? 0,
        ),
        highestGrade: item.highestGrade ?? item.highest_grade,
        championPointsSinceLastLevel: item.championPointsSinceLastLevel,
        championPointsUntilNextLevel: item.championPointsUntilNextLevel,
        tokensEarned: item.tokensEarned,
      }),
    );
    if (puuid) {
      setMasteryToCache(puuid, normalized);
    }
    return normalized;
  }

  return [];
}
