import { inject, type Ref } from "vue";
import type { PlayerData } from "../types/gameInfo";

export function usePlayerSearch() {
  const navigateSearchPayload = inject<
    Ref<{ name: string; gameId: number | null } | null>
  >("navigateSearchPayload");
  const navigateTo = inject<(page: string) => void>("navigateTo");

  function getPlayerSearchName(player: any, playerData?: PlayerData): string {
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
    player: any,
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
