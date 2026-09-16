import { inject, type Ref } from "vue";
import type { PlayerData } from "../types/gameInfo";
import type { SummonerDisplay } from "../api/lcu";

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
  const navigateCareerPayload = inject<
    Ref<{ puuid: string; summoner?: SummonerDisplay } | null>
  >("navigateCareerPayload");
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

  function handleCareerClick(
    e: MouseEvent,
    player: PlayerNameLike & { puuid?: string },
    playerData?: PlayerData,
  ) {
    e.stopPropagation();
    const puuid = playerData?.info?.puuid || player?.puuid;
    if (puuid && puuid !== "00000000-0000-0000-0000-000000000000") {
      if (navigateCareerPayload) {
        const info = playerData?.info;
        const displayName =
          info?.gameName ||
          info?.displayName ||
          player?.displayName ||
          player?.summonerName ||
          "召唤师";
        const gameName = info?.gameName || player?.displayName || "";
        const tagLine = info?.tagLine || player?.tagLine || "";
        const iconId = info?.profileIconId ?? 29;
        navigateCareerPayload.value = {
          puuid,
          summoner: {
            accountId: info?.accountId ?? 0,
            displayName,
            gameName,
            tagLine,
            percentCompleteForNextLevel: 0,
            profileIconId: iconId,
            puuid,
            summonerId: info?.summonerId ?? 0,
            summonerLevel: info?.summonerLevel ?? 0,
            xpSinceLastLevel: 0,
            xpUntilNextLevel: 0,
            profileIconUrl: `/lol-game-data/assets/v1/profile-icons/${iconId}.jpg`,
          },
        };
      }
      if (navigateTo) {
        navigateTo("career");
      }
      return;
    }
    handleNameClick(e, player, playerData);
  }

  return {
    getPlayerSearchName,
    handleNameClick,
    handleCareerClick,
  };
}
