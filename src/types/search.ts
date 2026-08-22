/** Search 页对局详情的视图模型（gameDetails 计算产物） */

/** 详情页单名玩家的展示数据 */
export interface GameDetailPlayer {
  participantId: number;
  teamId: number;
  championId: number;
  championIconUrl: string;
  spell1Url: string;
  spell2Url: string;
  runeUrl: string;
  name: string;
  puuid: string;
  summonerId: number;
  level?: number;
  kills: number;
  deaths: number;
  assists: number;
  cs: number;
  gold: number;
  damage: number;
  items: string[];
  ward?: string;
  win?: boolean;
  augmentIconUrls: string[];
  augmentNames: string[];
}

/** 详情页单支队伍的展示数据 */
export interface GameDetailTeam {
  teamId: number;
  players: GameDetailPlayer[];
  kills: number;
  win: boolean;
  towerKills: number;
  inhibitorKills: number;
  baronKills: number;
  dragonKills: number;
  riftHeraldKills: number;
}

/** 详情页整场对局的展示数据 */
export interface GameDetail {
  gameId: number;
  queueId: number;
  mapId: number;
  duration: string;
  date: string;
  queueName: string;
  mapName: string;
  win: boolean;
  queriedPlayerChampionIconUrl: string;
  mapIconUrl: string;
  blue: GameDetailTeam;
  red: GameDetailTeam;
}
