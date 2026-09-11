import { computed, type Ref } from "vue";
import {
  PREMADE_COLORS,
  type PlayerData,
  type PremadeGroup,
  type PremadeMember,
  type PremadePlayerLike,
  type PremadeTarget,
} from "../types/gameInfo";

/** 根据 teamParticipantId / partyId 分组，计算组队颜色映射 */
export function computePremadeColors(
  team: PremadePlayerLike[],
): Record<number, number> {
  if (!team || team.length === 0) return {};
  const tIdToMembers: Record<string | number, PremadePlayerLike[]> = {};

  for (const p of team) {
    const tpid = p.teamParticipantId ?? p.partyId;
    if (tpid === undefined || tpid === null || tpid === "") continue;
    if (!tIdToMembers[tpid]) tIdToMembers[tpid] = [];
    tIdToMembers[tpid].push(p);
  }

  const result: Record<number, number> = {};
  let currentColor = 0;

  for (const members of Object.values(tIdToMembers)) {
    if (members.length === 1) {
      const p = members[0];
      if (p.summonerId) result[p.summonerId] = -1;
      if (p.cellId !== undefined) result[p.cellId] = -1;
    } else {
      for (const p of members) {
        if (p.summonerId) result[p.summonerId] = currentColor;
        if (p.cellId !== undefined) result[p.cellId] = currentColor;
      }
      currentColor++;
    }
  }
  return result;
}

export function buildPremadeGroups(
  team: PremadePlayerLike[],
  colors: Record<number, number>,
  playerDataMap: Record<string | number, PlayerData>,
): PremadeGroup[] {
  if (!team || team.length === 0 || !colors) return [];
  const map: Record<number, PremadeMember[]> = {};

  for (const p of team) {
    const sid = p.summonerId;
    const cid = p.cellId;
    const cIdx =
      sid && colors[sid] !== undefined
        ? colors[sid]
        : cid !== undefined
          ? colors[cid]
          : undefined;
    if (cIdx === undefined || cIdx < 0) continue;

    if (!map[cIdx]) map[cIdx] = [];
    const champId = p.championId || p.championPickIntent || 0;
    const pData =
      cid !== undefined ? playerDataMap[cid] : sid ? playerDataMap[sid] : undefined;
    const name =
      pData?.info?.gameName ||
      pData?.info?.displayName ||
      p.displayName ||
      p.summonerName ||
      "";

    map[cIdx].push({
      summonerId: sid || cid || 0,
      displayName: name,
      championId: champId,
    });
  }

  return Object.entries(map)
    .map(([cIdxStr, members]) => ({
      colorIdx: Number(cIdxStr),
      members,
    }))
    .sort((a, b) => a.colorIdx - b.colorIdx);
}

export function usePremadeGroup(
  myTeam: Ref<PremadePlayerLike[]>,
  theirTeam: Ref<PremadePlayerLike[]>,
  sessionAllyTeam: Ref<PremadePlayerLike[]>,
  sessionEnemyTeam: Ref<PremadePlayerLike[]>,
  playerData: Ref<Record<string | number, PlayerData>>,
  premadeColorsMy: Ref<Record<number, number>>,
  premadeColorsTheir: Ref<Record<number, number>>,
) {
  /** 获取玩家组队颜色索引 */
  function getPremadeIdx(
    target: PremadeTarget | null | undefined,
    side: "my" | "their" = "my",
  ): number {
    if (target === undefined || target === null) return -1;
    const colors =
      side === "my" ? premadeColorsMy.value : premadeColorsTheir.value;
    if (!colors) return -1;

    if (typeof target === "object") {
      if (
        target.summonerId !== undefined &&
        target.summonerId !== 0 &&
        colors[target.summonerId] !== undefined
      ) {
        return colors[target.summonerId];
      }
      if (target.cellId !== undefined && colors[target.cellId] !== undefined) {
        return colors[target.cellId];
      }
      return -1;
    }

    return colors[target] ?? -1;
  }

  /** 左侧玩家卡片组队样式 */
  function getPremadeCardStyle(
    target: PremadeTarget | null | undefined,
    side: "my" | "their" = "my",
  ): Record<string, string> {
    const idx = getPremadeIdx(target, side);
    if (idx < 0) return {};
    const c = PREMADE_COLORS[idx % PREMADE_COLORS.length];
    return {
      backgroundColor: c.bg,
      borderColor: c.border,
    };
  }

  // 己方组队列表
  const myPremadeGroups = computed(() => {
    const teamList =
      myTeam.value.length > 0 ? myTeam.value : sessionAllyTeam.value;
    return buildPremadeGroups(
      teamList,
      premadeColorsMy.value,
      playerData.value,
    );
  });

  // 敌方组队列表
  const theirPremadeGroups = computed(() => {
    const teamList =
      theirTeam.value.length > 0 ? theirTeam.value : sessionEnemyTeam.value;
    return buildPremadeGroups(
      teamList,
      premadeColorsTheir.value,
      playerData.value,
    );
  });

  return {
    premadeColorsMy,
    premadeColorsTheir,
    getPremadeIdx,
    getPremadeCardStyle,
    myPremadeGroups,
    theirPremadeGroups,
  };
}
