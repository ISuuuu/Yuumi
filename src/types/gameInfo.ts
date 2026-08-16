import type { MatchDisplay } from "../api/lcu";

export interface PlayerData {
  info: any;
  matches: MatchDisplay[];
  ranked: { solo: any; flex: any };
  loading: boolean;
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
  puuid?: string;
}

export type PremadeTarget = number | PremadePlayerLike;

export interface PremadeGroup {
  colorIdx: number;
  members: PremadeMember[];
}

export interface PremadeRow {
  ally?: PremadeGroup;
  enemy?: PremadeGroup;
}

export interface PremadeColor {
  border: string;
  bg: string;
  dot: string;
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
