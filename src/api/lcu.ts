import { invoke } from "@tauri-apps/api/core";

export interface LcuApiResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}

/**
 * 统一的 LCU API 调用封装。
 * 所有请求通过 Tauri IPC 转发到 Rust 侧的 call_lcu_api 命令。
 */
export async function lcuRequest<T>(
  method: "GET" | "POST" | "PUT" | "PATCH" | "DELETE",
  path: string,
  body?: unknown
): Promise<LcuApiResponse<T>> {
  try {
    const data = await invoke<T>("call_lcu_api", { method, path, body });
    return { success: true, data };
  } catch (error: unknown) {
    const msg = error instanceof Error ? error.message : String(error);
    return { success: false, error: msg };
  }
}

// ─── Rust 命令直接调用（数据解析层）───

export interface SummonerDisplay {
  accountId: number;
  displayName: string;
  gameName: string;
  tagLine: string;
  percentCompleteForNextLevel: number;
  profileIconId: number;
  puuid: string;
  summonerId: number;
  summonerLevel: number;
  xpSinceLastLevel: number;
  xpUntilNextLevel: number;
  profileIconUrl: string;
}

export interface MatchDisplay {
  queueId: number;
  gameId: number;
  time: string;
  shortTime: string;
  name: string;
  map: string;
  duration: string;
  remake: boolean;
  win: boolean;
  championId: number;
  spell1Id: number;
  spell2Id: number;
  champLevel: number;
  kills: number;
  deaths: number;
  assists: number;
  kda: string;
  itemIds: number[];
  runeId: number;
  cs: number;
  gold: number;
  timeStamp: number;
  totalDamage: number;
  totalHeal: number;
  championIconUrl: string;
  spell1IconUrl: string;
  spell2IconUrl: string;
  runeIconUrl: string;
  itemIconUrls: string[];
}

/** 获取当前召唤师信息（Rust 解析层清洗后） */
export const fetchCurrentSummoner = () =>
  invoke<SummonerDisplay>("get_current_summoner");

/** 获取战绩列表（Rust 解析层清洗后） */
export const fetchMatchHistory = (puuid: string, begIndex?: number, endIndex?: number) =>
  invoke<MatchDisplay[]>("get_match_history", { puuid, begIndex, endIndex });

/** 获取 LCU 静态资源（图片等），返回 data URL */
export const fetchLcuAsset = (path: string) =>
  invoke<string>("get_lcu_asset", { path });

// ─── LCU API 快捷方法（透传原始 JSON）───

/** 获取游戏阶段 */
export const getGameflowPhase = () =>
  lcuRequest<string>("GET", "/lol-gameflow/v1/gameflow-phase");

/** 获取选人会话 */
export const getChampSelectSession = () =>
  lcuRequest<any>("GET", "/lol-champ-select/v1/session");

/** 接受匹配 */
export const acceptMatch = () =>
  lcuRequest<void>("POST", "/lol-matchmaking/v1/ready-check/accept");

/** 修改生涯背景 */
export const setProfileBackground = (skinId: number) =>
  lcuRequest<void>("POST", "/lol-summoner/v1/current-summoner/background-id", {
    key: skinId,
  });

/** 修改头像图标 */
export const setProfileIcon = (iconId: number) =>
  lcuRequest<void>("PUT", "/lol-summoner/v1/current-summoner/icon", {
    profileIconId: iconId,
  });

/** 修改状态签名 */
export const setOnlineStatus = (status: string) =>
  lcuRequest<void>("PUT", "/lol-chat/v1/me", { statusMessage: status });

/** 设置在线状态 (online / away / offline) */
export const setOnlineAvailability = (availability: string) =>
  lcuRequest<void>("PUT", "/lol-chat/v1/me", { availability });

/** 在选人阶段选择英雄 */
export const selectChampion = (actionId: number, championId: number) =>
  lcuRequest<void>(
    "PATCH",
    `/lol-champ-select/v1/session/actions/${actionId}`,
    { championId, completed: true }
  );

/** 在选人阶段禁用英雄 */
export const banChampion = (actionId: number, championId: number) =>
  lcuRequest<void>(
    "PATCH",
    `/lol-champ-select/v1/session/actions/${actionId}`,
    { championId, completed: true }
  );

/** 设置召唤师技能 */
export const setSummonerSpells = (spell1Id: number, spell2Id: number) =>
  lcuRequest<void>(
    "PATCH",
    "/lol-champ-select/v1/session/my-selection",
    { spell1Id, spell2Id }
  );
