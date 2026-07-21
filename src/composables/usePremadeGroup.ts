import { computed, type Ref } from "vue";
import {
  PREMADE_COLORS,
  type PlayerData,
  type PremadeGroup,
  type PremadeMember,
  type PremadeRow,
} from "../types/gameInfo";

/** 根据 teamParticipantId 分组，计算组队颜色映射 */
export function computePremadeColors(team: any[]): Record<number, number> {
  if (!team || team.length === 0) return {};
  const tIdToSIds: Record<number, number[]> = {};

  for (const p of team) {
    const sid = p.summonerId;
    const tpid = p.teamParticipantId;
    if (!sid || tpid === undefined || tpid === null) continue;
    if (!tIdToSIds[tpid]) tIdToSIds[tpid] = [];
    tIdToSIds[tpid].push(sid);
  }

  const result: Record<number, number> = {};
  let currentColor = 0;

  for (const ids of Object.values(tIdToSIds)) {
    if (ids.length === 1) {
      result[ids[0]] = -1; // 单排
    } else {
      for (const id of ids) result[id] = currentColor;
      currentColor++;
    }
  }
  return result;
}

export function buildPremadeGroups(
  team: any[],
  colors: Record<number, number>,
  playerDataMap: Record<number, PlayerData>,
): PremadeGroup[] {
  if (!team || team.length === 0 || !colors) return [];
  const map: Record<number, PremadeMember[]> = {};

  for (const p of team) {
    const sid = p.summonerId;
    if (!sid) continue;
    const cIdx = colors[sid];
    if (cIdx === undefined || cIdx < 0) continue;

    if (!map[cIdx]) map[cIdx] = [];
    const champId = p.championId || p.championPickIntent || 0;
    const pData = playerDataMap[p.cellId || sid];
    const name =
      pData?.info?.gameName ||
      pData?.info?.displayName ||
      p.displayName ||
      p.summonerName ||
      "";

    map[cIdx].push({
      summonerId: sid,
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
  myTeam: Ref<any[]>,
  theirTeam: Ref<any[]>,
  sessionAllyTeam: Ref<any[]>,
  sessionEnemyTeam: Ref<any[]>,
  playerData: Ref<Record<number, PlayerData>>,
  premadeColorsMy: Ref<Record<number, number>>,
  premadeColorsTheir: Ref<Record<number, number>>,
) {
  /** 获取玩家组队颜色索引 */
  function getPremadeIdx(summonerId: number, side: "my" | "their"): number {
    const colors =
      side === "my" ? premadeColorsMy.value : premadeColorsTheir.value;
    return colors[summonerId] ?? -1;
  }

  /** 左侧玩家卡片组队样式 */
  function getPremadeCardStyle(
    summonerId: number,
    side: "my" | "their",
  ): Record<string, string> {
    const idx = getPremadeIdx(summonerId, side);
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

  // 是否有任何组队信息
  const hasAnyPremadeInfo = computed(() => {
    return (
      myPremadeGroups.value.length > 0 || theirPremadeGroups.value.length > 0
    );
  });

  /** 逐行交错配对我方与敌方组队（优先保证友方显示在第一行，人数 >= 4 独占一行） */
  const premadeRows = computed<PremadeRow[]>(() => {
    const my = [...myPremadeGroups.value];
    const their = [...theirPremadeGroups.value];
    const rows: PremadeRow[] = [];

    let mIdx = 0;
    let tIdx = 0;

    while (mIdx < my.length || tIdx < their.length) {
      const mGroup = my[mIdx];
      const tGroup = their[tIdx];

      if (mGroup) {
        // 我方组队 >= 4 人时，我方独占一行
        if (mGroup.members.length >= 4) {
          rows.push({ ally: mGroup });
          mIdx++;
        } else {
          // 我方组队 < 4 人，如果敌方组队也 < 4 人，则同行并排展示
          if (tGroup && tGroup.members.length < 4) {
            rows.push({ ally: mGroup, enemy: tGroup });
            mIdx++;
            tIdx++;
          } else {
            // 如果敌方无组队或敌方组队 >= 4 人，优先将我方安排在当前行
            rows.push({ ally: mGroup });
            mIdx++;
          }
        }
      } else if (tGroup) {
        // 我方组队处理完毕，仅剩敌方组队
        rows.push({ enemy: tGroup });
        tIdx++;
      }
    }

    return rows;
  });

  return {
    premadeColorsMy,
    premadeColorsTheir,
    getPremadeIdx,
    getPremadeCardStyle,
    myPremadeGroups,
    theirPremadeGroups,
    hasAnyPremadeInfo,
    premadeRows,
  };
}
