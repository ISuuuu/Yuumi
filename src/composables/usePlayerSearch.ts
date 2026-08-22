import { inject, type Ref } from "vue";
import type { PlayerData } from "../types/gameInfo";

/** 可点击跳转搜索的玩家名称载体（PremadePlayerLike / SummonerDisplay 等的结构子集） */
export interface PlayerNameLike {
  displayName?: string;
  summonerName?: string;
  tagLine?: string;
}

export function usePlayerSearch() {
  const navigateSearchPayload = inject<
    Ref<{ name: string; gameId: number | null } | null>
  >("navigateSearchPayload");
  const navigateTo = inject<(page: string) => void>("navigateTo");

  function getPlayerSearchName(
    player: PlayerNameLike,
    playerData?: PlayerData,
  ): string {
    const info = playerData?.info;
    const gameName =
      info?.gameName ||
      info?.displayName ||
      player?.displayName ||
      player?.summonerName ||
      "";
    if (!gameName || gameName.startsWith("玩家") || gameName === "未知") {
      return "";
    }
    if (gameName.includes("#")) return gameName;
    const tagLine = info?.tagLine || player?.tagLine || "";
    return tagLine ? `${gameName}#${tagLine}` : gameName;
  }

  function handleNameClick(
    e: MouseEvent,
    player: PlayerNameLike,
    playerData?: PlayerData,
  ) {
    e.stopPropagation();
    const searchName = getPlayerSearchName(player, playerData);
    if (!searchName) return;
    if (navigateSearchPayload) {
      navigateSearchPayload.value = { name: searchName, gameId: -1 };
    }
    if (navigateTo) {
      navigateTo("search");
    }
  }

  return {
    getPlayerSearchName,
    handleNameClick,
  };
}
