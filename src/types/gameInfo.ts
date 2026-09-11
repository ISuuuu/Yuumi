import type { MatchDisplay, SummonerDisplay } from "../api/lcu";
import type { RankedQueueEntry } from "./lcu";

export interface PlayerData {
  info: SummonerDisplay | null;
  matches: MatchDisplay[];
  ranked: { solo: RankedQueueEntry | null; flex: RankedQueueEntry | null };
  loading: boolean;
  matchHistoryHidden?: boolean;
  championId?: number;
  avgKda?: number;
  winRate?: number;
  winCount?: number;
  lossesCount?: number;
  fateFlag?: "ally" | "enemy" | null;
  recentlyChampionName?: string;
  masteries?: ChampionMasteryItem[];
  streak?: StreakInfo | null;
}

export interface ChampionMasteryItem {
  championId: number;
  championLevel: number;
  championPoints: number;
  highestGrade?: string;
  championPointsSinceLastLevel?: number;
  championPointsUntilNextLevel?: number;
  tokensEarned?: number;
}

export interface StreakInfo {
  type: "win" | "loss";
  count: number;
}

export interface PremadeMember {
  summonerId: number;
  displayName: string;
  championId: number;
}

export interface PremadePlayerLike {
  summonerId?: number;
  cellId?: number;
  teamParticipantId?: number | string | null;
  partyId?: number | string | null;
  championId?: number;
  championPickIntent?: number;
  displayName?: string;
  summonerName?: string;
  gameName?: string;
  tagLine?: string;
  profileIconId?: number;
  puuid?: string;
  bot?: boolean;
  isBot?: boolean;
  botChampionId?: number;
  botName?: string;
}

export type PremadeTarget = number | PremadePlayerLike;

export interface PremadeGroup {
  colorIdx: number;
  members: PremadeMember[];
}

export interface PremadeColor {
  border: string;
  bg: string;
  dot: string;
}

export function getChampionIcon(id: number): string {
  return id > 0 ? `/lol-game-data/assets/v1/champion-icons/${id}.png` : "";
}

export interface ChampSelectActionLike {
  actorCellId: number;
  championId: number;
  completed?: boolean;
  isInProgress?: boolean;
  type: string;
}

export interface ChampSelectSessionLike {
  actions?: ChampSelectActionLike[][];
  myTeam?: Array<{ cellId: number; championId?: number; championPickIntent?: number; puuid?: string; summonerId?: number }>;
  theirTeam?: Array<{ cellId: number; championId?: number; championPickIntent?: number; puuid?: string; summonerId?: number }>;
}

/**
 * 解析玩家当前选择/锁定的英雄 ID（选人阶段动作与预选兜底）
 */
export function resolvePlayerChampionId(
  player?: PremadePlayerLike,
  session?: ChampSelectSessionLike | null,
): number {
  if (!player) return 0;
  if (player.championId && player.championId > 0) return player.championId;
  if (player.botChampionId && player.botChampionId > 0) return player.botChampionId;
  if (player.championPickIntent && player.championPickIntent > 0) return player.championPickIntent;

  // 从 actions 中查找该玩家的 pick（cellId 直接用原值查，仅适用于 session 原始 cellId 0..N）
  // 注意：未轮到该玩家或该动作未完成(completed: false)且不在进行中时，
  // session actions 预填的 championId 可能残留默认值或上一次意图，只有 completed 或 isInProgress 才代表真实选择
  if (session?.actions && player.cellId !== undefined) {
    for (const group of session.actions) {
      for (const act of group) {
        if (
          act.actorCellId === player.cellId &&
          act.type === "pick" &&
          act.championId > 0 &&
          (act.completed || act.isInProgress)
        ) {
          return act.championId;
        }
      }
    }
  }

  // 兜底从 session 成员中查找（puuid/summonerId 优先，cellId 辅助）
  if (session) {
    const allMembers = [...(session.myTeam || []), ...(session.theirTeam || [])];
    const match = allMembers.find(
      (m) =>
        (player.puuid && m.puuid === player.puuid) ||
        (player.summonerId && m.summonerId === player.summonerId) ||
        (player.cellId !== undefined && m.cellId === player.cellId),
    );
    if (match) {
      if (match.championId && match.championId > 0) return match.championId;
      if (match.championPickIntent && match.championPickIntent > 0) return match.championPickIntent;
    }
  }

  return 0;
}

// 预组队颜色方案（鲜明优雅的半透明组队背景色）
export const PREMADE_COLORS: PremadeColor[] = [
  {
    border: "rgba(245, 158, 11, 0.70)",
    bg: "rgba(245, 158, 11, 0.22)",
    dot: "#f59e0b",
  }, // 暖橙 / 金色
  {
    border: "rgba(236, 72, 153, 0.70)",
    bg: "rgba(236, 72, 153, 0.22)",
    dot: "#ec4899",
  }, // 柔粉 / 玫瑰
  {
    border: "rgba(59, 130, 246, 0.70)",
    bg: "rgba(59, 130, 246, 0.22)",
    dot: "#3b82f6",
  }, // 蔚蓝 / 晴空
  {
    border: "rgba(16, 185, 129, 0.70)",
    bg: "rgba(16, 185, 129, 0.22)",
    dot: "#10b981",
  }, // 翡翠 / 绿光
  {
    border: "rgba(168, 85, 247, 0.70)",
    bg: "rgba(168, 85, 247, 0.22)",
    dot: "#a855f7",
  }, // 幽紫 / 水晶
];

export function formatMasteryPoints(points: number): string {
  if (points >= 1000000) {
    return `${(points / 1000000).toFixed(1)}M`;
  }
  if (points >= 10000) {
    return `${(points / 10000).toFixed(1)}w`;
  }
  if (points >= 1000) {
    return `${(points / 1000).toFixed(1)}k`;
  }
  return points.toString();
}

export interface ChampionMasteryDetail {
  level: number;
  points: number;
  formattedPoints: string;
  highestGrade?: string;
  isTopChampion: boolean;
  isPracticing: boolean;
  topChampions: ChampionMasteryItem[];
}

export function getPlayerMasteryDetail(
  playerData?: PlayerData,
  targetChampId?: number,
): ChampionMasteryDetail | null {
  if (!playerData?.masteries || !Array.isArray(playerData.masteries) || !targetChampId || targetChampId <= 0) {
    return null;
  }
  // 如果当前玩家的数据还未返回或列表为空，不展示假的 0 级
  if (playerData.masteries.length === 0) {
    return null;
  }

  const topChampions = playerData.masteries.slice(0, 3);
  const current = playerData.masteries.find((m) => Number(m.championId) === Number(targetChampId));
  
  // 如果该玩家的熟练度列表中根本没有该英雄，说明该英雄完全没碰过（0场 0点）
  const level = Number(current?.championLevel ?? 0);
  const points = Number(current?.championPoints ?? 0);
  const isTopChampion =
    (topChampions.some((t) => Number(t.championId) === Number(targetChampId)) && points >= 10000) ||
    points >= 100000;
  const isPracticing = !isTopChampion && (level <= 1 || points < 5000);

  return {
    level,
    points,
    formattedPoints: formatMasteryPoints(points),
    highestGrade: current?.highestGrade,
    isTopChampion,
    isPracticing,
    topChampions,
  };
}
