import type { MatchDisplay } from "../api/lcu";
import { getChampionNameById } from "./championCache";

export type PositionKey = "TOP" | "JUNGLE" | "MIDDLE" | "BOTTOM" | "UTILITY";

export const POSITION_KEYS: PositionKey[] = [
  "TOP",
  "JUNGLE",
  "MIDDLE",
  "BOTTOM",
  "UTILITY",
];

export const POSITION_COLORS: Record<PositionKey, string> = {
  TOP: "#e06b6b",
  JUNGLE: "#55a879",
  MIDDLE: "#d7a84d",
  BOTTOM: "#5d8ed6",
  UTILITY: "#9b79c6",
};

export const POSITION_NAMES: Record<PositionKey, string> = {
  TOP: "上路",
  JUNGLE: "打野",
  MIDDLE: "中路",
  BOTTOM: "下路",
  UTILITY: "辅助",
};

export const POSITION_SHORT_NAMES: Record<PositionKey, string> = {
  TOP: "上",
  JUNGLE: "野",
  MIDDLE: "中",
  BOTTOM: "下",
  UTILITY: "辅",
};

export interface PositionCountEntry {
  position: PositionKey;
  name: string;
  count: number;
  percentage: number;
  color: string;
}

export interface AggregatedPositions {
  counts: Record<PositionKey, number>;
  percentages: Record<PositionKey, number>;
  totalValid: number;
  primaryPosition: PositionKey | null;
  primaryPercentage: number;
  secondaryPosition: PositionKey | null;
  secondaryPercentage?: number;
  entries: PositionCountEntry[];
}

export interface AggregatedChampion {
  championId: number;
  championName?: string;
  championIconUrl: string;
  games: number;
  wins: number;
  losses: number;
  winRate: number;
  kills: number;
  deaths: number;
  assists: number;
  kda: string;
  primaryPosition: PositionKey | null;
  positions: Partial<Record<PositionKey, number>>;
}

/**
 * 标准化位置名称（支持别名和大小写容错）
 */
export function normalizePosition(pos?: string | null): PositionKey | null {
  if (!pos) return null;
  const upper = pos.trim().toUpperCase();
  switch (upper) {
    case "TOP":
      return "TOP";
    case "JUNGLE":
    case "JUG":
      return "JUNGLE";
    case "MIDDLE":
    case "MID":
      return "MIDDLE";
    case "BOTTOM":
    case "BOT":
    case "ADC":
      return "BOTTOM";
    case "UTILITY":
    case "SUPPORT":
    case "SUP":
      return "UTILITY";
    default:
      return null;
  }
}

/**
 * 获取位置中文名称（如 上路、打野、中路、下路、辅助）
 */
export function getPositionName(pos?: string | null): string {
  const norm = normalizePosition(pos);
  if (!norm) return pos || "";
  return POSITION_NAMES[norm] || pos || "";
}

/**
 * 获取位置单字简称（如 上、野、中、下、辅）
 */
export function getShortPositionName(pos?: string | null): string {
  const norm = normalizePosition(pos);
  if (!norm) return pos || "";
  return POSITION_SHORT_NAMES[norm] || pos || "";
}

/**
 * 统计近期对局中的位置分布占比
 */
export function computeAggregatedPositions(
  matches: MatchDisplay[],
): AggregatedPositions | null {
  if (!matches || matches.length === 0) return null;

  const counts: Record<PositionKey, number> = {
    TOP: 0,
    JUNGLE: 0,
    MIDDLE: 0,
    BOTTOM: 0,
    UTILITY: 0,
  };

  let totalValid = 0;

  for (const m of matches) {
    // 忽略重开局
    if (m.remake) continue;
    const pos = normalizePosition(m.position);
    if (pos && counts[pos] !== undefined) {
      counts[pos]++;
      totalValid++;
    }
  }

  if (totalValid === 0) {
    return null;
  }

  const percentages: Record<PositionKey, number> = {
    TOP: Math.round((counts.TOP / totalValid) * 100),
    JUNGLE: Math.round((counts.JUNGLE / totalValid) * 100),
    MIDDLE: Math.round((counts.MIDDLE / totalValid) * 100),
    BOTTOM: Math.round((counts.BOTTOM / totalValid) * 100),
    UTILITY: Math.round((counts.UTILITY / totalValid) * 100),
  };

  // 排序生成有效 entry 列表
  const sortedKeys = [...POSITION_KEYS].sort(
    (a, b) => counts[b] - counts[a],
  );

  const primaryPosition = counts[sortedKeys[0]] > 0 ? sortedKeys[0] : null;
  const primaryPercentage = primaryPosition ? percentages[primaryPosition] : 0;
  const secondaryPosition =
    sortedKeys[1] && counts[sortedKeys[1]] > 0 ? sortedKeys[1] : null;
  const secondaryPercentage = secondaryPosition
    ? percentages[secondaryPosition]
    : 0;

  const entries: PositionCountEntry[] = sortedKeys
    .filter((k) => counts[k] > 0)
    .map((k) => ({
      position: k,
      name: POSITION_NAMES[k],
      count: counts[k],
      percentage: percentages[k],
      color: POSITION_COLORS[k],
    }));

  return {
    counts,
    percentages,
    totalValid,
    primaryPosition,
    primaryPercentage,
    secondaryPosition,
    secondaryPercentage,
    entries,
  };
}

/**
 * 统计近期对局中常用英雄（场次、胜率、KDA、常用位置）
 */
export function computeAggregatedChampions(
  matches: MatchDisplay[],
  limit = 5,
): AggregatedChampion[] {
  if (!matches || matches.length === 0) return [];

  const champMap = new Map<
    number,
    {
      championId: number;
      championIconUrl: string;
      games: number;
      wins: number;
      losses: number;
      kills: number;
      deaths: number;
      assists: number;
      positions: Record<PositionKey, number>;
    }
  >();

  for (const m of matches) {
    if (m.remake || !m.championId || m.championId <= 0) continue;

    let entry = champMap.get(m.championId);
    if (!entry) {
      entry = {
        championId: m.championId,
        championIconUrl: m.championIconUrl || "",
        games: 0,
        wins: 0,
        losses: 0,
        kills: 0,
        deaths: 0,
        assists: 0,
        positions: {
          TOP: 0,
          JUNGLE: 0,
          MIDDLE: 0,
          BOTTOM: 0,
          UTILITY: 0,
        },
      };
      champMap.set(m.championId, entry);
    }

    entry.games++;
    if (m.win) {
      entry.wins++;
    } else {
      entry.losses++;
    }
    entry.kills += m.kills ?? 0;
    entry.deaths += m.deaths ?? 0;
    entry.assists += m.assists ?? 0;

    const pos = normalizePosition(m.position);
    if (pos && entry.positions[pos] !== undefined) {
      entry.positions[pos]++;
    }
  }

  const list: AggregatedChampion[] = Array.from(champMap.values()).map(
    (item) => {
      const winRate =
        item.games > 0 ? Math.round((item.wins / item.games) * 100) : 0;
      const kda =
        item.deaths === 0
          ? "Perfect"
          : ((item.kills + item.assists) / item.deaths).toFixed(1);

      // 计算该英雄的主玩位置
      let primaryPos: PositionKey | null = null;
      let maxPosCount = 0;
      for (const key of POSITION_KEYS) {
        if (item.positions[key] > maxPosCount) {
          maxPosCount = item.positions[key];
          primaryPos = key;
        }
      }

      return {
        championId: item.championId,
        championName: getChampionNameById(item.championId),
        championIconUrl: item.championIconUrl,
        games: item.games,
        wins: item.wins,
        losses: item.losses,
        winRate,
        kills: item.kills,
        deaths: item.deaths,
        assists: item.assists,
        kda,
        primaryPosition: primaryPos,
        positions: item.positions,
      };
    },
  );

  // 排序：场次多的在前；场次相同则胜率高的在前；胜率相同则 KDA 高的在前
  list.sort((a, b) => {
    if (b.games !== a.games) return b.games - a.games;
    if (b.winRate !== a.winRate) return b.winRate - a.winRate;
    const kdaA = a.kda === "Perfect" ? 999 : parseFloat(a.kda) || 0;
    const kdaB = b.kda === "Perfect" ? 999 : parseFloat(b.kda) || 0;
    return kdaB - kdaA;
  });

  return list.slice(0, limit);
}
