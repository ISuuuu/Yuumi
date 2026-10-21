import { getMapSide } from "../api/lcu";
import {
  computePremadeColors,
  hasPremadeGroup,
} from "../composables/usePremadeGroup";
import type { PremadePlayerLike, PlayerData } from "../types/gameInfo";
import type { GameflowParticipant } from "../types/lcu";
import {
  getValidTpid,
  inheritPlaceholderChampion,
  isIdentityCompatible,
} from "./playerIdentity";
import {
  createPlaceholderPlayerData,
  createSummonerDisplay,
} from "./playerPlaceholder";

export type MapSide = "blue" | "red" | null;

/** 构建对局队伍所需的只读上下文（全部由调用方以当前值传入，函数内不持有闭包） */
export interface TeamBuildContext {
  currentSummonerId: number;
  currentSummonerPuuid: string;
  /** 调用方当前的 store.mapSide */
  mapSide: MapSide;
  champSelectTeamSnapshot: PremadePlayerLike[];
  champSelectTheirTeamSnapshot: PremadePlayerLike[];
  sessionAllyTeam: PremadePlayerLike[];
  sessionEnemyTeam: PremadePlayerLike[];
  gameflowMyTeam: PremadePlayerLike[];
  gameflowTheirTeam: PremadePlayerLike[];
  playerData: Record<string | number, PlayerData>;
}

/**
 * 按 puuid / summonerId / gameName 强标识在选人快照中寻找同一玩家。
 * `includeDisplayName` 仅队伍映射阶段启用（阵营判定阶段不比较 displayName，保持既有语义）。
 */
function findInSnapshot(
  p: GameflowParticipant,
  snapshot: PremadePlayerLike[],
  includeDisplayName: boolean,
): PremadePlayerLike | undefined {
  return snapshot.find(
    (cs) =>
      (p.puuid && cs.puuid && cs.puuid === p.puuid) ||
      (p.summonerId && cs.summonerId && cs.summonerId === p.summonerId) ||
      (p.summonerName && cs.displayName && cs.displayName === p.summonerName) ||
      (p.gameName && cs.gameName && cs.gameName === p.gameName) ||
      (includeDisplayName &&
        p.displayName &&
        cs.displayName &&
        cs.displayName === p.displayName),
  );
}

/**
 * 判定 teamOne / teamTwo 哪边是我方。
 * 返回的 side 非空时，调用方需同步写入 store.setMapSide(side)。
 */
export async function resolveTeamSide(
  teamOne: GameflowParticipant[],
  teamTwo: GameflowParticipant[],
  ctx: TeamBuildContext,
): Promise<{
  allyTeam: GameflowParticipant[];
  enemyTeam: GameflowParticipant[];
  side: MapSide;
}> {
  const checkMatches = (p: GameflowParticipant) => {
    const matchLocal =
      (ctx.currentSummonerId && p.summonerId === ctx.currentSummonerId) ||
      (ctx.currentSummonerPuuid && p.puuid === ctx.currentSummonerPuuid);
    if (matchLocal) return true;
    // 结合选人阶段我方队员快照进行队伍识别
    return ctx.champSelectTeamSnapshot.some(
      (cs) =>
        (p.puuid && cs.puuid && cs.puuid === p.puuid) ||
        (p.summonerId && cs.summonerId && cs.summonerId === p.summonerId) ||
        (p.summonerName && cs.displayName && cs.displayName === p.summonerName) ||
        (p.gameName && cs.gameName && cs.gameName === p.gameName),
    );
  };

  const isTeamOne = teamOne.some(checkMatches);
  const isTeamTwo = teamTwo.some(checkMatches);

  if (isTeamOne) {
    console.info(
      "[GamePlayerData] checkMatches 确定队伍: 我方为蓝方 (teamOne)",
    );
    return { allyTeam: teamOne, enemyTeam: teamTwo, side: "blue" };
  }
  if (isTeamTwo) {
    console.info("[GamePlayerData] checkMatches 确定队伍: 我方为红方 (teamTwo)");
    return { allyTeam: teamTwo, enemyTeam: teamOne, side: "red" };
  }

  // 脱敏对局（排位赛等）：未直接匹配到当前玩家，结合多重信息推断阵营
  let determinedSide: MapSide = ctx.mapSide;

  // 1. 若当前 store.mapSide 未设置，从选人阶段快照 cellId 推断（蓝方 0..4，红方 5..9）
  if (!determinedSide && ctx.champSelectTeamSnapshot.length > 0) {
    const hasRedCell = ctx.champSelectTeamSnapshot.some(
      (cs) => cs.cellId !== undefined && cs.cellId >= 5,
    );
    const hasBlueCell = ctx.champSelectTeamSnapshot.some(
      (cs) => cs.cellId !== undefined && cs.cellId < 5,
    );
    if (hasRedCell && !hasBlueCell) {
      determinedSide = "red";
    } else if (hasBlueCell && !hasRedCell) {
      determinedSide = "blue";
    }
  }

  // 2. 若仍未确定，异步向 Rust 端获取（含 LCU session 及 2999 端口 LiveClientData 探测）
  if (!determinedSide) {
    try {
      const side = await getMapSide();
      if (side === "blue" || side === "red") {
        determinedSide = side;
      }
    } catch {
      /* ignore */
    }
  }

  // 3. 根据最终推断的阵营分配队伍：LOL 中 teamOne 永远为蓝方，teamTwo 永远为红方
  if (determinedSide === "red") {
    console.info(
      "[GamePlayerData] 脱敏队伍推断成功: 我方为红方 (teamTwo), 敌方为蓝方 (teamOne)",
    );
    return {
      allyTeam: teamTwo.length > 0 ? teamTwo : teamOne,
      enemyTeam: teamTwo.length > 0 ? teamOne : teamTwo,
      side: "red",
    };
  }
  if (determinedSide === "blue") {
    console.info(
      "[GamePlayerData] 脱敏队伍推断成功: 我方为蓝方 (teamOne), 敌方为红方 (teamTwo)",
    );
    return {
      allyTeam: teamOne.length > 0 ? teamOne : teamTwo,
      enemyTeam: teamOne.length > 0 ? teamTwo : teamOne,
      side: "blue",
    };
  }

  // 最终兜底：非空队伍优先作为 allyTeam
  console.warn("[GamePlayerData] 脱敏队伍推断兜底: 默认分配 teamOne 为我方");
  return {
    allyTeam: teamOne.length > 0 ? teamOne : teamTwo,
    enemyTeam: teamOne.length > 0 ? teamTwo : teamOne,
    side: teamOne.length > 0 ? "blue" : teamTwo.length > 0 ? "red" : null,
  };
}

/** 把上一轮已加载的玩家数据重新挂到当前 team 的各个身份键上（cellId / summonerId / puuid） */
export function rekeyExistingPlayerData(
  players: PremadePlayerLike[],
  playerData: Record<string | number, PlayerData>,
): void {
  for (const p of players) {
    const sourceData =
      (p.cellId !== undefined ? playerData[p.cellId] : undefined) ||
      (p.summonerId ? playerData[p.summonerId] : undefined) ||
      (p.puuid ? playerData[p.puuid] : undefined);
    if (sourceData) {
      if (p.cellId !== undefined) playerData[p.cellId] = sourceData;
      if (p.summonerId) playerData[p.summonerId] = sourceData;
      if (p.puuid) playerData[p.puuid] = sourceData;
    }
  }
}

/** gameflow 参与者 → 队伍成员（英雄 / 头像 / 组队标识在各级快照间继承，cellId 稳定化） */
function mapParticipant(
  p: GameflowParticipant,
  idx: number,
  offset: number,
  isEnemy: boolean,
  ctx: TeamBuildContext,
): PremadePlayerLike {
  const resolvedName = p.gameName
    ? p.tagLine
      ? `${p.gameName}#${p.tagLine}`
      : p.gameName
    : p.summonerName ||
      p.displayName ||
      p.botName ||
      (p.bot || p.isBot ? "电脑" : "");
  // 为敌我双方生成互不碰撞的稳定 cellId：我方 0..4，敌方 5..9
  const stableCellId = offset + idx;

  // 尝试从选人快照中继承已选英雄和已有数据
  let resolvedChampId = p.championId ?? p.botChampionId ?? 0;
  let profileIconId = p.profileIconId;
  const targetSnapshot = isEnemy
    ? ctx.champSelectTheirTeamSnapshot
    : ctx.champSelectTeamSnapshot;
  // 严格匹配：必须满足以下强标识之一，绝不使用没有强身份关联的 cellId 盲目兜底
  const snap = findInSnapshot(p, targetSnapshot, true);
  if (snap) {
    if (!resolvedChampId || resolvedChampId <= 0) {
      resolvedChampId = snap.championId ?? snap.championPickIntent ?? 0;
    }
    if (!profileIconId && snap.profileIconId) {
      profileIconId = snap.profileIconId;
    }
  }
  // 最终兜底：从当前已渲染的队伍数据中找同一玩家的 championId
  // 这解决 InProgress 阶段 gameflow session 不再携带 championId 导致覆盖 GameStart 时有效值的问题
  if (!resolvedChampId || resolvedChampId <= 0) {
    const prevTeam = isEnemy ? ctx.gameflowTheirTeam : ctx.gameflowMyTeam;
    const prevPlayer = prevTeam.find(
      (pp) =>
        (p.puuid && pp.puuid && pp.puuid === p.puuid) ||
        (p.summonerId && pp.summonerId && pp.summonerId === p.summonerId) ||
        pp.cellId === stableCellId,
    );
    if (prevPlayer?.championId && prevPlayer.championId > 0) {
      resolvedChampId = prevPlayer.championId;
    }
  }
  // 同样从 playerData 里找已有的 championId
  if (!resolvedChampId || resolvedChampId <= 0) {
    const pd =
      (p.puuid ? ctx.playerData[p.puuid] : undefined) ||
      (p.summonerId ? ctx.playerData[p.summonerId] : undefined) ||
      ctx.playerData[stableCellId];
    if (pd?.championId && pd.championId > 0) {
      resolvedChampId = pd.championId;
    }
  }

  // 继承组队标识：优先自身已有，若缺失则从 snap、sessionAlly/EnemyTeam 或 prevTeam 继承
  let resolvedTeamParticipantId = getValidTpid(p);

  if (!resolvedTeamParticipantId && snap) {
    resolvedTeamParticipantId = getValidTpid(snap);
  }

  if (!resolvedTeamParticipantId) {
    const sessionTeam = isEnemy ? ctx.sessionEnemyTeam : ctx.sessionAllyTeam;
    const matchedSession = sessionTeam.find(
      (sp) =>
        (p.puuid && sp.puuid && p.puuid === sp.puuid) ||
        (p.summonerId && sp.summonerId && p.summonerId === sp.summonerId) ||
        (p.summonerName &&
          sp.displayName &&
          p.summonerName === sp.displayName) ||
        (p.gameName && sp.gameName && p.gameName === sp.gameName),
    );
    if (matchedSession) {
      resolvedTeamParticipantId = getValidTpid(matchedSession);
    }
  }

  if (!resolvedTeamParticipantId) {
    const prevTeam = isEnemy ? ctx.gameflowTheirTeam : ctx.gameflowMyTeam;
    const prevPlayer = prevTeam.find(
      (pp) =>
        (p.puuid && pp.puuid && pp.puuid === p.puuid) ||
        (p.summonerId && pp.summonerId && pp.summonerId === p.summonerId) ||
        pp.cellId === stableCellId,
    );
    if (prevPlayer) {
      resolvedTeamParticipantId = getValidTpid(prevPlayer);
    }
  }

  return {
    ...p,
    cellId: stableCellId,
    isEnemy,
    championId: resolvedChampId,
    teamParticipantId: resolvedTeamParticipantId ?? p.teamParticipantId,
    partyId: resolvedTeamParticipantId ?? p.partyId,
    summonerId: p.summonerId || snap?.summonerId,
    puuid: p.puuid || snap?.puuid,
    gameName: p.gameName || snap?.gameName,
    tagLine: p.tagLine || snap?.tagLine,
    profileIconId,
    displayName: resolvedName,
    bot: Boolean(p.bot || p.isBot),
    isBot: Boolean(p.bot || p.isBot),
  };
}

/**
 * 按阵营把 gameflow 双方参与者映射为队伍成员。
 * 严谨对应英雄联盟底层阵营机制：
 * 蓝方 (ORDER / 100) 对应 cellId 0..4；红方 (CHAOS / 200) 对应 cellId 5..9。
 * 我方是红方还是蓝方，决定了 0..4 与 5..9 属于我方还是敌方！
 */
export function buildGameflowTeams(
  allyTeam: GameflowParticipant[],
  enemyTeam: GameflowParticipant[],
  mapSide: MapSide,
  ctx: TeamBuildContext,
): { mappedMy: PremadePlayerLike[]; mappedTheir: PremadePlayerLike[] } {
  const isAllyRed = mapSide === "red";
  const allyOffset = isAllyRed ? 5 : 0;
  const enemyOffset = isAllyRed ? 0 : 5;

  return {
    mappedMy: allyTeam.map((p, idx) =>
      mapParticipant(p, idx, allyOffset, false, ctx),
    ),
    mappedTheir: enemyTeam.map((p, idx) =>
      mapParticipant(p, idx, enemyOffset, true, ctx),
    ),
  };
}

/** 为队伍成员预填充基础占位，避免后台异步请求完成前界面出现空白 */
export function seedInitialPlayerData(
  players: PremadePlayerLike[],
  playerData: Record<string | number, PlayerData>,
): void {
  for (const p of players) {
    const key = p.cellId ?? 0;
    const isBot = Boolean(p.bot || p.isBot || p.botChampionId);
    // 如果已有选人阶段完整加载的数据，保留并继承。
    // 身份优先查找：puuid → summonerId → cellId。选人/对局顺序变化时同 cell 槽
    // 可能残留异队旧数据，身份键优先才能命中同一玩家的已加载项，避免误杀重拉
    const byPuuid = p.puuid ? playerData[p.puuid] : undefined;
    const bySid = p.summonerId ? playerData[p.summonerId] : undefined;
    const byCell = playerData[key];
    const incoming = {
      puuid: p.puuid,
      summonerId: p.summonerId,
      cellId: key,
    };
    const existing = [byPuuid, bySid, byCell].find((e) =>
      Boolean(e) && isIdentityCompatible(e, incoming),
    );
    if (existing) {
      // 若 gameflow 带来了新的 championId（加载中阶段可获取），始终更新
      if (p.championId && p.championId > 0) {
        existing.championId = p.championId;
      }
      playerData[key] = existing;
      if (p.summonerId) playerData[p.summonerId] = existing;
      if (p.puuid) playerData[p.puuid] = existing;
      continue;
    }
    // cell 槽被异队旧数据占据：清槽后按新身份重建，避免把我方 info/matches 挂到敌方 key 上。
    // 若被清的是无身份占位且带有英雄 ID，先继承给 p，保证加载中英雄不断档
    if (byCell) {
      if (!p.championId || p.championId <= 0) {
        const carried = inheritPlaceholderChampion(byCell, key, p);
        if (carried > 0) p.championId = carried;
      }
      delete playerData[key];
    }
    if (!playerData[key]) {
      const fallbackName =
        p.displayName ||
        p.gameName ||
        p.summonerName ||
        (isBot ? "电脑" : `玩家${key + 1}`);
      playerData[key] = createPlaceholderPlayerData({
        info: createSummonerDisplay({
          summonerId: p.summonerId,
          puuid: p.puuid,
          displayName: fallbackName,
          gameName: p.gameName,
          tagLine: p.tagLine,
          profileIconId: p.profileIconId,
        }),
        loading: !isBot,
        matchHistoryHidden: isBot,
        championId: p.championId,
      });
    }
    if (
      p.summonerId &&
      p.summonerId !== key &&
      !playerData[p.summonerId]
    ) {
      // 防止选人阶段占位数据（cellId 充当 summonerId）污染真实 summonerId 键
      const existingSid = playerData[key]?.info?.summonerId;
      if (!existingSid || existingSid === p.summonerId || existingSid === key) {
        playerData[p.summonerId] = playerData[key];
      }
    }
    if (p.puuid && !playerData[p.puuid]) {
      // 防止选人阶段空 puuid 占位数据污染真实 puuid 键
      const existingPuuid = playerData[key]?.info?.puuid;
      if (!existingPuuid || existingPuuid === p.puuid) {
        playerData[p.puuid] = playerData[key];
      }
    }
  }
}

/** 若当前 session 偶尔缺少组队 ID，利用已识别的旧组队（按 puuid / summonerId 强身份键），
 * 迁移绑定到带有新 stableCellId 的队伍成员上，防止组队颜色在游戏加载时丢失 */
function migratePremadeColors(
  team: PremadePlayerLike[],
  existingColors: Record<string | number, number>,
): Record<string | number, number> {
  const migrated: Record<string | number, number> = {};
  for (const p of team) {
    const cIdx =
      (p.puuid && existingColors[p.puuid] !== undefined
        ? existingColors[p.puuid]
        : undefined) ??
      (p.summonerId && existingColors[p.summonerId] !== undefined
        ? existingColors[p.summonerId]
        : undefined);
    if (cIdx !== undefined) {
      if (p.puuid) migrated[p.puuid] = cIdx;
      if (p.summonerId) migrated[p.summonerId] = cIdx;
      if (p.cellId !== undefined) migrated[p.cellId] = cIdx;
    }
  }
  return migrated;
}

/** 新组队颜色优先；新一轮缺失组队信息时沿用（迁移后）的旧颜色，避免配色闪烁 */
export function resolvePremadeColors(
  team: PremadePlayerLike[],
  existingColors: Record<string | number, number>,
): Record<string | number, number> {
  const newColors = computePremadeColors(team);
  if (hasPremadeGroup(newColors)) {
    return newColors;
  }
  if (!hasPremadeGroup(existingColors)) {
    return newColors;
  }
  const migrated = migratePremadeColors(team, existingColors);
  return hasPremadeGroup(migrated) ? migrated : existingColors;
}
