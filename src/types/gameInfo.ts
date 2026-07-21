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

// 预组队颜色方案（降低饱和度与透明度，非常柔和剔透，不刺眼）
export const PREMADE_COLORS: PremadeColor[] = [
  {
    border: "rgba(255, 140, 0, 0.6)",
    bg: "rgba(255, 140, 0, 0.15)",
    dot: "#ff9010",
  }, // 亮橙 / 橘色
  {
    border: "rgba(236, 72, 153, 0.4)",
    bg: "rgba(236, 72, 153, 0.10)",
    dot: "#ec4899",
  }, // 柔粉
  {
    border: "rgba(59, 130, 246, 0.4)",
    bg: "rgba(59, 130, 246, 0.10)",
    dot: "#3b82f6",
  }, // 柔蓝
  {
    border: "rgba(16, 185, 129, 0.4)",
    bg: "rgba(16, 185, 129, 0.10)",
    dot: "#10b981",
  }, // 柔绿
  {
    border: "rgba(168, 85, 247, 0.4)",
    bg: "rgba(168, 85, 247, 0.10)",
    dot: "#a855f7",
  }, // 柔紫
];
