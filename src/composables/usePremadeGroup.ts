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
): Record<string | number, number> {
  if (!team || team.length === 0) return {};
  const tIdToMembers: Record<string | number, PremadePlayerLike[]> = {};

  for (const p of team) {
    const tpid = p.teamParticipantId ?? p.partyId;
    if (
      tpid === undefined ||
      tpid === null ||
      tpid === "" ||
      tpid === 0 ||
      tpid === "0"
    )
      continue;
    if (!tIdToMembers[tpid]) tIdToMembers[tpid] = [];
    tIdToMembers[tpid].push(p);
  }

  const result: Record<string | number, number> = {};
  let currentColor = 0;

  for (const members of Object.values(tIdToMembers)) {
    if (members.length === 1) {
      const p = members[0];
      if (p.puuid) result[p.puuid] = -1;
      if (p.summonerId) result[p.summonerId] = -1;
      if (p.cellId !== undefined) result[p.cellId] = -1;
    } else {
      for (const p of members) {
        if (p.puuid) result[p.puuid] = currentColor;
        if (p.summonerId) result[p.summonerId] = currentColor;
        if (p.cellId !== undefined) result[p.cellId] = currentColor;
      }
      currentColor++;
    }
  }
  return result;
}

/** 判断颜色映射中是否包含有效开黑组队（colorIdx >= 0） */
export function hasPremadeGroup(
  colors: Record<string | number, number> | null | undefined,
): boolean {
  if (!colors) return false;
  return Object.values(colors).some((v) => typeof v === "number" && v >= 0);
}

export function buildPremadeGroups(
  team: PremadePlayerLike[],
  colors: Record<string | number, number>,
  playerDataMap: Record<string | number, PlayerData>,
): PremadeGroup[] {
  if (!team || team.length === 0 || !colors) return [];
  const map: Record<number, PremadeMember[]> = {};

  for (const p of team) {
    const puuid = p.puuid;
    const sid = p.summonerId;
    const cid = p.cellId;
    const cIdx =
      puuid && colors[puuid] !== undefined
        ? colors[puuid]
        : sid && colors[sid] !== undefined
          ? colors[sid]
          : cid !== undefined
            ? colors[cid]
            : undefined;
    if (cIdx === undefined || cIdx < 0) continue;

    if (!map[cIdx]) map[cIdx] = [];
    const pData =
      puuid && playerDataMap[puuid]
        ? playerDataMap[puuid]
        : cid !== undefined && playerDataMap[cid]
          ? playerDataMap[cid]
          : sid && playerDataMap[sid]
            ? playerDataMap[sid]
            : undefined;
    const champId =
      p.championId || p.championPickIntent || pData?.championId || 0;
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
  premadeColorsMy: Ref<Record<string | number, number>>,
  premadeColorsTheir: Ref<Record<string | number, number>>,
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
      if (target.puuid && colors[target.puuid] !== undefined) {
        return colors[target.puuid];
      }
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
