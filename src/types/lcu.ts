/**
 * LCU 原始 API 响应类型定义（仅声明项目实际消费的字段子集）。
 * 数据经 Rust call_lcu_api 透传，字段以 LCU 实际返回为准。
 */

import type { SummonerDisplay } from "../api/lcu";

/** 海克斯强化详情（Rust CherryAugmentDetail 对应） */
export interface CherryAugmentDetail {
  id: number;
  name: string;
  iconPath: string;
}

/** /lol-ranked/v1/ranked-stats/{puuid} 的队列条目 */
export interface RankedQueueEntry {
  queueType: string;
  tier: string;
  rank: string;
  leaguePoints?: number;
  wins: number;
  losses: number;
}

/** /lol-ranked/v1/ranked-stats/{puuid} 响应 */
export interface RankedStats {
  queues: RankedQueueEntry[];
  /** 赛季最高段位（响应顶层字段，非队列条目内） */
  highestTier?: string;
  highestRank?: string;
  previousSeasonEndTier?: string;
  previousSeasonEndRank?: string;
}

/** 段位展示格式化的入参：队列条目 + 可选的赛季段位扩展字段 */
export type RankDisplaySource = RankedQueueEntry & {
  highestTier?: string;
  highestRank?: string;
  previousSeasonEndTier?: string;
  previousSeasonEndRank?: string;
};

/** gameflow session 中的对局参与者 */
export interface GameflowParticipant {
  summonerId?: number;
  puuid?: string;
  displayName?: string;
  /** 游戏内昵称（进游戏后与 displayName 可能不同） */
  summonerName?: string;
  championId?: number;
  cellId?: number;
  teamParticipantId?: number | string | null;
  partyId?: number | string | null;
  /** 分路：top / jungle / middle / bottom / utility */
  assignedPosition?: string;
}

/** /lol-gameflow/v1/session 的队列信息 */
export interface GameflowQueue {
  id: number;
  /** 游戏模式标识，如 MATCHED_GAME / TFT / ARAM */
  gameMode?: string;
}

/** /lol-gameflow/v1/session 的游戏数据 */
export interface GameflowGameData {
  queue: GameflowQueue;
  teamOne?: GameflowParticipant[];
  teamTwo?: GameflowParticipant[];
}

/** /lol-gameflow/v1/session 响应 */
export interface GameflowSession {
  gameData: GameflowGameData;
}

// ─── 对局详情（/lol-match-history/v1/games/{gameId}，仅消费字段子集） ───

/** 海克斯强化 ID 字段（stats 与 participant 上均可能携带） */
export interface MatchDetailAugments {
  augments?: number[];
  playerAugment1?: number;
  playerAugment2?: number;
  playerAugment3?: number;
  playerAugment4?: number;
  playerAugment5?: number;
}

export interface MatchDetailStats extends MatchDetailAugments {
  win?: boolean;
  kills?: number;
  deaths?: number;
  assists?: number;
  champLevel?: number;
  item0?: number;
  item1?: number;
  item2?: number;
  item3?: number;
  item4?: number;
  item5?: number;
  item6?: number;
  perk0?: number;
  totalMinionsKilled?: number;
  neutralMinionsKilled?: number;
  goldEarned?: number;
  totalDamageDealtToChampions?: number;
}

export interface MatchDetailParticipant
  extends MatchDetailAugments {
  participantId: number;
  teamId: number;
  championId: number;
  spell1Id?: number;
  spell2Id?: number;
  stats?: MatchDetailStats;
}

export interface MatchDetailIdentityPlayer {
  gameName?: string;
  summonerName?: string;
  tagLine?: string;
  puuid?: string;
  summonerId?: number;
}

export interface MatchDetailIdentity {
  participantId: number;
  player?: MatchDetailIdentityPlayer;
}

export interface MatchDetailTeam {
  teamId: number;
  towerKills?: number;
  inhibitorKills?: number;
  baronKills?: number;
  dragonKills?: number;
  riftHeraldKills?: number;
}

export interface MatchDetail {
  gameId: number;
  queueId: number;
  mapId: number;
  gameDuration: number;
  gameCreation: number;
  participants?: MatchDetailParticipant[];
  participantIdentities?: MatchDetailIdentity[];
  teams?: MatchDetailTeam[];
}

/** Rust get_game_data_assets 预加载的游戏资源映射 */
export interface GameDataAssets {
  items: Record<number, string>;
  spells: Record<number, string>;
  runes: Record<number, string>;
  /** 英雄 ID → 图标路径 */
  champions: Record<number, string>;
  augments: Record<number, CherryAugmentDetail>;
}

/** 召唤师查询类接口的原始响应（各端点字段覆盖不一，统一按可缺失处理） */
export type RawSummoner = Partial<SummonerDisplay>;
