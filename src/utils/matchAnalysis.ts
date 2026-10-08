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
 * 判断是否为辅助装备
 */
export function isSupportItemId(itemId: number): boolean {
  return (itemId >= 3850 && itemId <= 3871) || (itemId >= 3876 && itemId <= 3884);
}

/**
 * 判断是否为打野装备
 */
export function isJungleItemId(itemId: number): boolean {
  return itemId >= 1101 && itemId <= 1103;
}

/**
 * 判断是否为具备分路的召唤师峡谷类 5v5 模式
 */
export function isLanedQueue(queueId?: number | null): boolean {
  if (queueId === undefined || queueId === null) return true;
  return [400, 420, 430, 440, 480, 490, 700, 800, 810, 820, 830, 840, 850, 1300, 1400, 1900].includes(queueId);
}

/**
 * 官方分路任务/专属装备 ID 解析分路（TOP / MIDDLE / BOTTOM / UTILITY / JUNGLE）
 */
export function resolveRoleBoundItemId(itemId: number): PositionKey | null {
  switch (itemId) {
    // 上路任务与奖励道具（1090 任务: 上路, 1200 上路任务, 1220 解封的传送/上路奖励, 1221, 1222）
    case 1090:
    case 1200:
    case 1220:
    case 1221:
    case 1222:
      return "TOP";
    // 中路任务与奖励道具（1091 任务: 中路, 1201 中路任务, 1206 中路奖励）
    case 1091:
    case 1201:
    case 1206:
      return "MIDDLE";
    // 下路任务与奖励道具（1092 任务: 下路, 1202 下路任务, 1207 下路奖励）
    case 1092:
    case 1202:
    case 1207:
      return "BOTTOM";
    // 辅助任务与奖励道具（1093 任务: 辅助, 1203 辅助任务, 1208 辅助奖励）
    case 1093:
    case 1203:
    case 1208:
      return "UTILITY";
    // 打野任务与奖励道具（1094 任务: 打野, 1204 打野任务, 1205/1209/1210/1211 打野升级幼体）
    case 1094:
    case 1204:
    case 1205:
    case 1209:
    case 1210:
    case 1211:
      return "JUNGLE";
    default:
      return null;
  }
}

/**
 * 兜底从 MatchDisplay 尝试推导/规范化位置（针对旧快照或缺少 position 字段的对局）
 */
export function inferMatchPosition(m: MatchDisplay): PositionKey | null {
  // 1. 大乱斗（450 / 2400 / 2450）、斗魂竞技场（1700）等娱乐模式全员无分路，绝不分配位置
  if (!isLanedQueue(m.queueId)) {
    return null;
  }

  // 2. 官方分路专属装备字段 (roleBoundItem) - 最权威直接匹配
  if (m.roleBoundItem) {
    const rolePos = resolveRoleBoundItemId(m.roleBoundItem);
    if (rolePos) return rolePos;
  }

  // 3. 装备栏中若包含专属任务/奖励装备
  if (m.itemIds && m.itemIds.length > 0) {
    for (const id of m.itemIds) {
      const pos = resolveRoleBoundItemId(id);
      if (pos) return pos;
    }
  }

  // 4. 已有规范化位置
  if (m.position) {
    const norm = normalizePosition(m.position);
    if (norm) return norm;
  }

  // 5. 召唤师技能惩戒 (Smite, ID 11) -> 必定打野
  if (m.spell1Id === 11 || m.spell2Id === 11) {
    return "JUNGLE";
  }
  // 6. 装备中存在辅助装备 -> 辅助
  if (m.itemIds && m.itemIds.some(isSupportItemId)) {
    return "UTILITY";
  }
  // 7. 装备中存在打野装备 -> 打野
  if (m.itemIds && m.itemIds.some(isJungleItemId)) {
    return "JUNGLE";
  }

  return null;
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
    const pos = inferMatchPosition(m);
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
