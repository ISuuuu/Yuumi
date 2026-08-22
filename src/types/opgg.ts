/** OP.GG 数据弹窗（OpggModal 及其子组件）共享的类型定义 */

// ─── OP.GG 返回数据结构（fetch_opgg_data）───
export interface OpggStats {
  win_rate?: number;
  pick_rate?: number;
  ban_rate?: number;
  winRate?: number;
  pickRate?: number;
  banRate?: number;
  kda?: number;
  tier?: string | number;
  rank?: number;
  tier_data?: { tier?: string; rank?: number };
}

export interface OpggPositionEntry {
  name: string;
  stats?: OpggStats;
  counters?: { champion_id: number }[];
}

export interface OpggChampionEntry {
  id: number;
  name: string;
  positions?: OpggPositionEntry[];
  average_stats?: OpggStats;
}

export interface OpggTierPayload {
  meta?: { version?: string };
  data?: OpggChampionEntry[];
}

// 梯度列表行（由 OpggChampionEntry 归一化而来）
export interface TierListItem {
  id: number;
  name: string;
  win_rate?: number;
  pick_rate?: number;
  ban_rate?: number;
  kda?: number;
  tier?: string | number;
  rank?: number;
  position?: string;
  counters?: number[];
}

export interface OpggBuildSummary {
  id?: number;
  name?: string;
  positions?: OpggPositionEntry[];
  average_stats?: OpggStats | null;
}

export interface OpggCounter {
  champion_id: number;
  name?: string;
  play: number;
  win: number;
}

export interface OpggRunePreset {
  primary_page_id: number;
  secondary_page_id: number;
  primary_rune_ids?: number[];
  secondary_rune_ids?: number[];
  stat_mod_ids?: number[];
  win: number;
  play: number;
  pick_rate?: number;
}

export interface OpggIdWinRate {
  ids: number[];
  win: number;
  play: number;
  pick_rate?: number;
}

export interface OpggSkillOrder {
  order: string[];
  win: number;
  play: number;
  pick_rate?: number;
}

export interface OpggSkillMastery {
  ids: string[];
}

export interface OpggBuildData {
  summary?: OpggBuildSummary;
  counters?: OpggCounter[];
  runes: OpggRunePreset[];
  skills?: OpggSkillOrder[];
  skill_masteries?: OpggSkillMastery[];
  summoner_spells: OpggIdWinRate[];
  starter_items?: OpggIdWinRate[];
  core_items?: OpggIdWinRate[];
  boots?: OpggIdWinRate[];
  last_items: OpggIdWinRate[];
}

export interface OpggBuildPayload {
  meta?: { version?: string };
  data?: OpggBuildData;
}

// ─── LCU 符文静态资源数据结构（符文树渲染共用）───
export interface PerkStyle {
  id: number;
  name: string;
  iconPath: string;
  slots: { perks: number[] }[];
}

export interface Perk {
  id: number;
  name: string;
  shortDesc?: string;
}
