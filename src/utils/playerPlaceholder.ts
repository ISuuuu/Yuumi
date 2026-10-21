import type { SummonerDisplay } from "../api/lcu";
import type { PlayerData } from "../types/gameInfo";

const DEFAULT_PROFILE_ICON_ID = 29;

export interface SummonerDisplaySeed {
  summonerId?: number;
  puuid?: string;
  displayName: string;
  gameName?: string;
  tagLine?: string;
  profileIconId?: number;
}

/** 构造前端兜底的召唤师信息（无 LCU 数据时的占位，等级/经验统一为 0） */
export function createSummonerDisplay(
  seed: SummonerDisplaySeed,
): SummonerDisplay {
  const iconId = seed.profileIconId ?? DEFAULT_PROFILE_ICON_ID;
  return {
    accountId: 0,
    summonerId: seed.summonerId ?? 0,
    puuid: seed.puuid ?? "",
    displayName: seed.displayName,
    gameName: seed.gameName || seed.displayName,
    tagLine: seed.tagLine || "",
    profileIconId: iconId,
    profileIconUrl: `/lol-game-data/assets/v1/profile-icons/${iconId}.jpg`,
    summonerLevel: 0,
    percentCompleteForNextLevel: 0,
    xpSinceLastLevel: 0,
    xpUntilNextLevel: 0,
  };
}

export interface PlaceholderPlayerOptions {
  info: SummonerDisplay | null;
  loading: boolean;
  matchHistoryHidden?: boolean;
  championId?: number;
}

/** 构造玩家占位数据（加载中 / 电脑 / 拉取失败 / 异常兜底共用） */
export function createPlaceholderPlayerData(
  opts: PlaceholderPlayerOptions,
): PlayerData {
  return {
    info: opts.info,
    matches: [],
    ranked: { solo: null, flex: null },
    loading: opts.loading,
    matchHistoryHidden: opts.matchHistoryHidden,
    championId: opts.championId,
  };
}
