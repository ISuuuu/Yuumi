import { computed, type ComputedRef } from "vue";
import { useLcuStore } from "../store/lcuStore";
import {
  resolvePlayerChampionId,
  type PlayerData,
  type PremadePlayerLike,
} from "../types/gameInfo";

/**
 * 玩家当前英雄 ID 解析（PlayerCard 与 PlayerMatchColumn 共用）。
 * 选人阶段始终优先从选人会话实时推断（保证挑选悬停、锁定、ARAM 换英雄、板凳席互换实时响应）；
 * 其余阶段（游戏中/加载中/对局结束）使用数据层与玩家对象上的明确 championId，
 * 严禁使用选人残留 session 盲目赋值。
 */
export function usePlayerChampionId(
  getPlayer: () => PremadePlayerLike | undefined,
  getPlayerData: () => PlayerData | undefined,
): ComputedRef<number> {
  const store = useLcuStore();

  return computed(() => {
    // 1. 选人阶段：从选人会话实时推断
    if (store.gamePhase === "ChampSelect" && store.champSelectSession) {
      const fromResolver = resolvePlayerChampionId(
        getPlayer(),
        store.champSelectSession,
      );
      if (fromResolver > 0) return fromResolver;
    }
    // 2. 选人外或会话未推断出时：优先使用已有明确 championId
    const dataChampId = getPlayerData()?.championId;
    if (dataChampId && dataChampId > 0) return dataChampId;
    const player = getPlayer();
    if (player?.championId && player.championId > 0) return player.championId;
    if (player?.botChampionId && player.botChampionId > 0) {
      return player.botChampionId;
    }
    return 0;
  });
}
