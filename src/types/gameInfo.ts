import type { MatchDisplay, SummonerDisplay } from "../api/lcu";
import type { RankedQueueEntry } from "./lcu";

export interface PlayerData {
  info: SummonerDisplay | null;
  matches: MatchDisplay[];
  ranked: { solo: RankedQueueEntry | null; flex: RankedQueueEntry | null };
  loading: boolean;
  matchHistoryHidden?: boolean;
  isProfilePrivate?: boolean;
  championId?: number;
  avgKda?: number;
  winRate?: number;
  winCount?: number;
  lossesCount?: number;
  fateFlag?: "ally" | "enemy" | null;
  recentlyChampionName?: string;
  fateGameCreation?: number;
  fateIsLastGame?: boolean;
  fateAllyCount?: number;
  fateEnemyCount?: number;
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
  isEnemy?: boolean;
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

  // 1. 若存在选人会话 session，优先从 session 实时推断（保证悬停选人、锁定、ARAM换英雄、板凳席互换实时刷新）
  if (session) {
    // 判断 player 归属阵营：优先使用明确的 isEnemy 字段；若无，通过 puuid/summonerId 在 theirTeam 匹配判定
    // 严禁使用 cellId >= 5 判定敌方（选人阶段我方若为红方，我方在 myTeam 中的真实 cellId 即为 5..9）
    const isEnemy =
      player.isEnemy !== undefined
        ? player.isEnemy
        : (session.theirTeam || []).some(
            (m) =>
              (player.puuid && m.puuid === player.puuid) ||
              (player.summonerId && m.summonerId === player.summonerId),
          );
    const searchPool = isEnemy ? (session.theirTeam || []) : (session.myTeam || []);

    let match = searchPool.find(
      (m) =>
        (player.puuid && m.puuid === player.puuid) ||
        (player.summonerId && m.summonerId === player.summonerId) ||
        (player.cellId !== undefined && m.cellId === player.cellId),
    );

    // 跨阵营身份兜底（仅限真实 puuid / summonerId，严禁跨阵营按 cellId 匹配）
    if (!match && (player.puuid || player.summonerId)) {
      const allMembers = [...(session.myTeam || []), ...(session.theirTeam || [])];
      match = allMembers.find(
        (m) =>
          (player.puuid && m.puuid === player.puuid) ||
          (player.summonerId && m.summonerId === player.summonerId),
      );
    }

    // 只有在选人会话中真正匹配到了玩家，且该玩家并非敌方匿名未选时，才使用会话动作与成员英雄
    // 严禁对敌方匿名槽位使用 session.actions（actions 中的 actorCellId 5..9 属于我方红方选人动作，会与敌方虚拟 cellId 5..9 碰撞）
    if (isEnemy && !match && !player.puuid && !player.summonerId) {
      // 敌方匿名槽位未匹配到真实 session 成员，不尝试 actions 兜底，防止误用我方选人动作
    } else {
      const actualCellId = match?.cellId;

      // 1.1 最高时效性：正在进行中的 Pick 动作（玩家在选人框中刚点击/悬停挑选英雄，尚未锁定）
      if (session.actions && actualCellId !== undefined) {
        for (const group of session.actions) {
          for (const act of group) {
            if (
              act.actorCellId === actualCellId &&
              act.type === "pick" &&
              act.isInProgress &&
              act.championId > 0
            ) {
              return act.championId;
            }
          }
        }
      }

      // 1.2 选人会话成员上的已确定/锁定/ARAM随机分配与换选英雄
      // ARAM 摇骰子（reroll）、板凳席互换（swap）、局末队友交换（trade）及排位锁定均实时更新 match.championId
      if (match?.championId && match.championId > 0) {
        return match.championId;
      }

      // 1.3 已完成的 Pick 动作（已锁定，但 member.championId 尚未同步到位的极短过渡微帧）
      if (session.actions && actualCellId !== undefined) {
        for (const group of session.actions) {
          for (const act of group) {
            if (
              act.actorCellId === actualCellId &&
              act.type === "pick" &&
              act.completed &&
              act.championId > 0
            ) {
              return act.championId;
            }
          }
        }
      }

      // 1.4 预选意图（Pick Intent：尚未轮到该玩家挑选时，展示其计划选择的英雄）
      if (match?.championPickIntent && match.championPickIntent > 0) {
        return match.championPickIntent;
      }
    }
  }

  // 2. 玩家对象自带预选意图兜底
  if (player.championPickIntent && player.championPickIntent > 0) {
    return player.championPickIntent;
  }

  // 3. 人机英雄兜底
  if (player.botChampionId && player.botChampionId > 0) {
    return player.botChampionId;
  }

  // 4. 玩家已有历史英雄兜底（会话外或无会话更新时使用）
  if (player.championId && player.championId > 0) {
    return player.championId;
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
