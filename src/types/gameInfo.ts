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
  if (session?.actions && player.cellId !== undefined) {
    for (const group of session.actions) {
      for (const act of group) {
        if (act.actorCellId === player.cellId && act.type === "pick" && act.championId > 0) {
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
