import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { SavedPlayerMarker } from "../api/lcu";
import type { PlayerData } from "../types/gameInfo";

/**
 * 宿命/曾同局徽标共享逻辑（PlayerCard 与 PlayerMatchColumn 公用）。
 * 两组件推断玩家 puuid 的兜底来源不同，故通过 getter 传入。
 */
export function useFateBadge(
  getPuuid: () => string | undefined,
  getSavedMap: () => Record<string, SavedPlayerMarker> | undefined,
  getSelfPuuid: () => string | undefined,
) {
  const { t } = useI18n();

  // 该玩家是否为"保存的玩家"（曾同局/打了标签），取 tag 与相遇次数；
  // 当前登录召唤师自身不展示曾同局徽章
  const savedInfo = computed<SavedPlayerMarker | undefined>(() => {
    const puuid = getPuuid();
    const savedMap = getSavedMap();
    if (!puuid || !savedMap) return undefined;
    const selfPuuid = getSelfPuuid();
    if (selfPuuid && puuid === selfPuuid) return undefined;
    return savedMap[puuid];
  });

  function savedBadgeTitle(info: SavedPlayerMarker): string {
    return info.tag
      ? t("gameInfo.savedPlayerTip", {
          count: info.encounterCount,
          tag: info.tag,
        })
      : t("gameInfo.savedPlayerTipNoTag", { count: info.encounterCount });
  }

  function getFateBadgeText(data: PlayerData | undefined): string {
    if (!data?.fateFlag) return "";
    const isLast = data.fateIsLastGame ?? true;
    if (data.fateFlag === "ally") {
      return isLast ? t("gameInfo.fateAllyText") : t("gameInfo.fateRecentAllyText");
    }
    return isLast ? t("gameInfo.fateEnemyText") : t("gameInfo.fateRecentEnemyText");
  }

  function getFateTitle(data: PlayerData | undefined): string {
    if (!data?.fateFlag) return "";
    const isLast = data.fateIsLastGame ?? true;
    const baseTitle =
      data.fateFlag === "ally"
        ? isLast
          ? t("gameInfo.fateAllyTitle")
          : t("gameInfo.fateRecentAllyTitle")
        : isLast
          ? t("gameInfo.fateEnemyTitle")
          : t("gameInfo.fateRecentEnemyTitle");
    const champPart = data.recentlyChampionName
      ? ` (${t("gameInfo.fateUsedChampion", { champ: data.recentlyChampionName })})`
      : "";
    let fullTitle = `${baseTitle}${champPart}`;

    // 若多次交手且既有队友又有对手，在末尾补充详细交手统计
    const allyCount = data.fateAllyCount ?? 0;
    const enemyCount = data.fateEnemyCount ?? 0;
    const totalCount = allyCount + enemyCount;
    if (totalCount >= 2 && allyCount > 0 && enemyCount > 0) {
      const statsPart = t("gameInfo.fateMultiEncounter", {
        total: totalCount,
        ally: allyCount,
        enemy: enemyCount,
      });
      fullTitle += `\n${statsPart}`;
    }

    return fullTitle;
  }

  return { savedInfo, savedBadgeTitle, getFateBadgeText, getFateTitle };
}
