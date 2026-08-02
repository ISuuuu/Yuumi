import { invoke } from "@tauri-apps/api/core";

export interface LcuApiResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}

/**
 * 净化错误消息，若包含 JSON 则尝试解析提取 message，若有“LCU 返回错误”前缀则过滤
 */
export function cleanError(error: unknown): string {
  let msg = error instanceof Error ? error.message : String(error);

  // 1. 优先使用正则表达式从可能含有 JSON (甚至是残破 JSON) 的文本中，安全且精准地提取核心 message 字段
  const jsonMessageMatch = msg.match(/"(?:error)?Message"\s*:\s*"([^"]+)"/i);
  if (jsonMessageMatch && jsonMessageMatch[1]) {
    let cleanMsg = jsonMessageMatch[1];
    if (cleanMsg.includes("Error response for ") && cleanMsg.includes(":")) {
      const parts = cleanMsg.split(":");
      cleanMsg = parts[parts.length - 1].trim();
    }
    msg = cleanMsg;
  } else if (msg.includes("{") && msg.includes("}")) {
    // 降级：如果正则未匹配上但含有完整大括号，依然尝试标准 JSON 序列化解析
    try {
      const start = msg.indexOf("{");
      const end = msg.lastIndexOf("}");
      const jsonStr = msg.slice(start, end + 1);
      const obj = JSON.parse(jsonStr);
      let cleanMsg = obj.message || obj.errorMessage || obj.description;
      if (cleanMsg) {
        if (
          cleanMsg.includes("Error response for ") &&
          cleanMsg.includes(":")
        ) {
          const parts = cleanMsg.split(":");
          cleanMsg = parts[parts.length - 1].trim();
        }
        msg = cleanMsg;
      }
    } catch {
      /* ignore */
    }
  }

  // 2. 如果包含“LCU 返回错误”前缀，提取后面的纯消息
  if (msg.startsWith("LCU 返回错误")) {
    const parts = msg.split("]:");
    if (parts.length > 1) {
      msg = parts[1].trim();
    }
  }

  // 3. 仅安全剥离最外层可能残留的包围单双引号，不损伤任何合法的 {} 符号
  msg = msg.trim().replace(/^["']+|["']+$/g, "");

  // 4. 友好汉化经典的观战业务报错
  if (msg.includes("Already in gameflow")) {
    return "你当前已处于对局中，无法重复观战";
  }
  if (msg.includes("Cannot spectate game because spectator key is missing")) {
    return "该召唤师当前不在游戏中（观战密钥缺失）";
  }

  return msg;
}

/**
 * 统一的 LCU API 调用封装。
 * 所有请求通过 Tauri IPC 转发到 Rust 侧的 call_lcu_api 命令。
 */
export async function lcuRequest<T>(
  method: "GET" | "POST" | "PUT" | "PATCH" | "DELETE",
  path: string,
  body?: unknown,
): Promise<LcuApiResponse<T>> {
  try {
    const data = await invoke<T>("call_lcu_api", { method, path, body });
    return { success: true, data };
  } catch (error: unknown) {
    return { success: false, error: cleanError(error) };
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

export interface CherryAugmentDetail {
  id: number;
  name: string;
  iconPath: string;
  description: string;
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
  placement?: number;
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
  augmentIds: number[];
  augmentIconUrls: string[];
  augmentNames: string[];
  augmentDescs: string[];
}

/** 获取当前召唤师信息（Rust 解析层清洗后，404 时自动重试） */
export async function fetchCurrentSummoner(
  maxRetries = 15,
): Promise<SummonerDisplay> {
  for (let i = 0; i <= maxRetries; i++) {
    try {
      return await invoke<SummonerDisplay>("get_current_summoner");
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      if (msg.includes("404") && i < maxRetries) {
        await new Promise((r) => setTimeout(r, 2000));
        continue;
      }
      if (msg.includes("404")) {
        throw new Error("未登录：请在英雄联盟客户端中登录您的账号");
      }
      throw e;
    }
  }
  throw new Error("获取召唤师信息失败");
}

/** 获取战绩列表（Rust 解析层清洗后） */
export const fetchMatchHistory = (
  puuid: string,
  begIndex?: number,
  endIndex?: number,
) => invoke<MatchDisplay[]>("get_match_history", { puuid, begIndex, endIndex });

/** 通过 SGP 接口获取战绩列表（支持分页，仅腾讯国服可用） */
export const fetchMatchHistorySgp = (
  puuid: string,
  begIndex: number,
  endIndex: number,
) =>
  invoke<MatchDisplay[]>("get_match_history_sgp", {
    puuid,
    begIndex,
    endIndex,
  });

export interface RecentTeammate {
  name: string;
  puuid: string;
  icon: string;
  total: number;
  wins: number;
  losses: number;
  lastPlayTime: number;
}

export interface RecentTeammatesResponse {
  puuid: string;
  summoners: RecentTeammate[];
}

export interface PlayerFateInfo {
  fateFlag: "ally" | "enemy" | null;
  recentlyChampionName: string | null;
}

/** 获取最近队友统计 */
export const fetchRecentTeammates = (gameIds: number[], puuid: string) =>
  invoke<RecentTeammatesResponse>("get_recent_teammates", { gameIds, puuid });

/** 获取单个玩家上一局与自己的宿命关系及英雄名 */
export const fetchPlayerFateInfo = (
  gameId: number,
  targetPuuid: string,
  currentSummonerId: number,
) =>
  invoke<PlayerFateInfo>("get_player_fate_info", {
    gameId,
    targetPuuid,
    currentSummonerId,
  });

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
    { championId, completed: true },
  );

/** 在选人阶段禁用英雄 */
export const banChampion = (actionId: number, championId: number) =>
  lcuRequest<void>(
    "PATCH",
    `/lol-champ-select/v1/session/actions/${actionId}`,
    { championId, completed: true },
  );

/** 设置召唤师技能 */
export const setSummonerSpells = (spell1Id: number, spell2Id: number) =>
  lcuRequest<void>("PATCH", "/lol-champ-select/v1/session/my-selection", {
    spell1Id,
    spell2Id,
  });

// ─── 应用配置（读写）───
// 字段名使用 PascalCase，与 Rust serde(rename_all = "PascalCase") 一致

export interface GeneralConfig {
  LolPath: string[];
  EnableStartLolWithApp: boolean;
  EnableCloseToTray: boolean | null;
  EnableGameStartMinimize: boolean;
  EnableCheckUpdate: boolean;
  LogLevel: number;
  EnableGithubProxy: boolean;
  GithubProxyAddr: string;
  EnableOpggProxy: boolean;
  OpggProxyAddr: string;
  EnableSignalrHub: boolean;
  SignalrServerUrl: string;
  SignalrUserId: string;
  UploadApiUrl: string;
}

export interface PersonalizationConfig {
  MicaEnabled: boolean;
  DpiScale: string;
  Language: string;
  ThemeMode: string;
  WinCardColor: string;
  LoseCardColor: string;
  RemakeCardColor: string;
  LightDeathsNumberColor: string;
  DarkDeathsNumberColor: string;
  ThemeColor: string;
}

export interface FunctionsConfig {
  CareerGamesNumber: number;
  ApiConcurrencyNumber: number;
  GameInfoFilter: boolean;
  ShowTierInGameInfo: boolean;
  AutoShowOpgg: boolean;
  EnableOpggOnTop: boolean;
  EnableAutoAcceptMatching: boolean;
  EnableAutoReconnect: boolean;
  EnableAutoCreateLobby: boolean;
  DefaultGameMode: number;
  AutoAcceptMatchingDelay: number;
  EnableAutoHoverChampion: boolean;
  AutoSelectConfirmOnTimeout: boolean;
  EnableRandomSkin: boolean;
  EnableAutoSelectChampion: boolean;
  AutoSelectChampion: number[];
  AutoSelectChampionTop: number[];
  AutoSelectChampionJug: number[];
  AutoSelectChampionMid: number[];
  AutoSelectChampionBot: number[];
  AutoSelectChampionSup: number[];
  EnableAutoBanChampion: boolean;
  AutoBanChampion: number[];
  AutoBanChampionTop: number[];
  AutoBanChampionJug: number[];
  AutoBanChampionMid: number[];
  AutoBanChampionBot: number[];
  AutoBanChampionSup: number[];
  AutoBanDelay: number;
  PretendBan: boolean;
  AutoAcceptCeilSwap: boolean;
  AutoAcceptChampTrade: boolean;
  EnableAutoSetSpells: boolean;
  AutoSetSummonerSpell: number[];
  AutoSetSummonerSpellTop: number[];
  AutoSetSummonerSpellJug: number[];
  AutoSetSummonerSpellMid: number[];
  AutoSetSummonerSpellBot: number[];
  AutoSetSummonerSpellSup: number[];
  EnableReserveGameinfo: boolean;
  LcuRealtimeEnabled: boolean;
  LcuUserId: string;
  UploadEnabled: boolean;
  HideTft: boolean;
  HideSavedPlayers: boolean;
  EnableBenchOverlay: boolean;
  EnableScreenshotOnMultikill: boolean;
  ScreenshotOnMultikillLevels: number[];
  ScreenshotSavePath: string;
  EnableAutoHandleInvite: boolean;
  EnableAutoHonor: boolean;
  EnableAutoPlayAgain: boolean;
  EnableAutoAramTeamSide: boolean;
  AramTeamSideVisibleToTeam: boolean;
  EnableAutoTagReminder: boolean;
}

export interface OtherConfig {
  LastNoticeSha: string;
  SearchHistory: string;
}

export interface AppConfig {
  Version?: number;
  General: GeneralConfig;
  Personalization: PersonalizationConfig;
  Functions: FunctionsConfig;
  Other: OtherConfig;
}

/**
 * 获取完整应用配置。
 * 启动早期窗口 JS 可能抢在 Rust 侧 app.manage(AppState) 之前调用，
 * 此时后端会报 "state not managed"，这里做短暂自愈重试，其余错误直接抛出。
 */
export const fetchConfig = () => {
  const attempt = (left: number): Promise<AppConfig> =>
    invoke<AppConfig>("get_config").catch((err) => {
      const errStr = String(err || "");
      const isStateNotReady =
        errStr.includes("state not managed") || errStr.includes("must call .manage()");
      if (isStateNotReady && left > 0) {
        const delay = 200 * (5 - left + 1);
        return new Promise<void>((resolve) => setTimeout(resolve, delay)).then(() =>
          attempt(left - 1),
        );
      }
      throw err;
    });
  return attempt(5);
};

/** 更新完整应用配置 */
export const updateConfig = (config: AppConfig) =>
  invoke<void>("update_config", { newConfig: config });

// ─── 对局上传 ───

export interface BatchUploadResult {
  successCount: number;
  failedCount: number;
  error: string | null;
}

/** 单场上传（推入后台队列） */
export const uploadSingleMatch = (gameId: number) =>
  invoke<string>("upload_single_match", { gameId });

/** 批量上传对局（直接 POST 到外部 API） */
export const batchUploadMatches = (gameIds: number[]) =>
  invoke<BatchUploadResult>("batch_upload_matches", { gameIds });

// ─── 保存的玩家 ───

export interface SavedPlayer {
  puuid: string;
  selfPuuid: string;
  region: string;
  rsoPlatformId: string;
  tag: string | null;
  summonerName: string;
  profileIconId: number;
  /** Riot ID 的 tagLine（如 NA1），自动相遇记录时采集 */
  tagLine: string | null;
  /** 上次相遇对局使用的英雄 id，0 表示未知 */
  championId: number;
  updateAt: number;
  lastMetAt: number | null;
  lastQueueType: string | null;
  encounterCount: number;
}

export interface EncounteredGame {
  id: number;
  gameId: number;
  puuid: string;
  selfPuuid: string;
  region: string;
  rsoPlatformId: string;
  queueType: string;
  updateAt: number;
}

export interface PageResult<T> {
  data: T[];
  count: number;
}

export interface SaveSavedPlayerInput {
  puuid: string;
  selfPuuid: string;
  rsoPlatformId?: string;
  region?: string;
  tag?: string | null;
  summonerName?: string;
  profileIconId?: number;
  encountered?: boolean;
}

/** 分页查询保存的玩家 */
export const queryAllSavedPlayers = (
  selfPuuid: string,
  page?: number,
  pageSize?: number
) =>
  invoke<PageResult<SavedPlayer>>("query_all_saved_players", {
    selfPuuid,
    page,
    pageSize,
  });

/** 分页查询相遇记录 */
export const queryEncounteredGames = (
  selfPuuid: string,
  puuid: string,
  queueType?: string,
  page?: number,
  pageSize?: number
) =>
  invoke<PageResult<EncounteredGame>>("query_encountered_games", {
    selfPuuid,
    puuid,
    queueType,
    page,
    pageSize,
  });

/** 保存玩家 / 更新 tag */
export const saveSavedPlayer = (dto: SaveSavedPlayerInput) =>
  invoke<void>("save_saved_player", { dto });

/** 回填保存玩家的召唤师 ID（tagLine），返回更新的数量 */
export const backfillSavedPlayerIdentity = () =>
  invoke<number>("backfill_saved_player_identity");

/** 删除保存的玩家 */
export const deleteSavedPlayer = (puuid: string, selfPuuid: string) =>
  invoke<void>("delete_saved_player", { puuid, selfPuuid });

/** 导出带标记玩家为 JSON 文件，取消时返回 null */
export const exportTaggedPlayersToJsonFile = () =>
  invoke<string | null>("export_tagged_players_to_json_file");

/** 从 JSON 文件导入带标记玩家，返回导入数量 */
export const importTaggedPlayersFromJsonFile = () =>
  invoke<number>("import_tagged_players_from_json_file");
