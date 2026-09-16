/**
 * 英雄列表缓存模块。
 * 使用模块级 Map + Promise 去重，确保整个应用生命周期内只请求一次。
 * 独立于 Vue SFC 编译，不受组件销毁/重建影响。
 */

import i18n from "../i18n";

export interface ChampionEntry {
  id: number;
  name: string;
  alias?: string;
  title?: string;
  iconPath: string;
}

/** LCU champion-summary.json / champions.json 的原始条目（仅消费字段子集） */
interface RawChampionEntry {
  id: number;
  name?: string;
  alias?: string;
  title?: string;
  squarePortraitPath?: string;
}

// ─── 纯 JS 模块级缓存（不依赖 Vue ref，不会被 HMR 重置）───
let cachedChampions: ChampionEntry[] | null = null;
let cachedKeywords: Record<number, string> | null = null;
let loadChampionsPromise: Promise<ChampionEntry[]> | null = null;
let loadKeywordsPromise: Promise<Record<number, string>> | null = null;

/** 获取缓存的英雄列表，缓存命中时同步返回 */
export function getCachedChampions(): ChampionEntry[] | null {
  return cachedChampions;
}

/** 获取缓存的腾讯关键词，缓存命中时同步返回 */
export function getCachedKeywords(): Record<number, string> | null {
  return cachedKeywords;
}

/** 请求英雄列表（有缓存直接返回，多调用共享同一个 promise） */
export async function fetchChampions(): Promise<ChampionEntry[]> {
  if (cachedChampions) return cachedChampions;
  if (loadChampionsPromise) return loadChampionsPromise;

  loadChampionsPromise = doFetchChampions();
  return loadChampionsPromise;
}

async function doFetchChampions(): Promise<ChampionEntry[]> {
  // 动态 import 避免循环依赖
  const { lcuRequest } = await import("../api/lcu");

  let success = false;
  let rawData: RawChampionEntry[] | Record<string, RawChampionEntry> | null = null;

  // 尝试 1: champion-summary.json
  try {
    const resp = await lcuRequest<RawChampionEntry[] | Record<string, RawChampionEntry>>(
      "GET",
      "/lol-game-data/assets/v1/champion-summary.json",
    );
    if (resp.success && resp.data) {
      rawData = resp.data;
      success = true;
      console.log(
        "Yuumi - Successfully fetched champion list via champion-summary.json",
      );
    }
  } catch (e) {
    console.error("Yuumi - Failed to fetch champion-summary.json:", e);
  }

  // 尝试 2: champions.json
  if (!success) {
    try {
      const resp = await lcuRequest<RawChampionEntry[] | Record<string, RawChampionEntry>>(
        "GET",
        "/lol-game-data/assets/v1/champions.json",
      );
      if (resp.success && resp.data) {
        rawData = resp.data;
        success = true;
        console.log(
          "Yuumi - Successfully fetched champion list via champions.json",
        );
      }
    } catch (e) {
      console.error("Yuumi - Failed to fetch champions.json:", e);
    }
  }

  if (success && rawData) {
    let list: RawChampionEntry[] = [];
    if (Array.isArray(rawData)) {
      list = rawData;
    } else if (typeof rawData === "object" && rawData !== null) {
      list = Object.values(rawData);
    }

    if (list.length > 0) {
      const isEnglish =
        (i18n.global.locale as unknown as { value: string }).value === "en_US";
      cachedChampions = list
        .filter((c) => c && c.id > 0)
        .map((c) => ({
          id: c.id,
          name:
            isEnglish && c.alias ? c.alias : c.name || c.alias || `#${c.id}`,
          alias: c.alias,
          title: c.title,
          iconPath:
            c.squarePortraitPath ||
            `/lol-game-data/assets/v1/champion-icons/${c.id}.png`,
        }))
        .sort((a: ChampionEntry, b: ChampionEntry) =>
          a.name.localeCompare(b.name, "zh"),
        );
      console.log(
        `Yuumi - Successfully rendered ${cachedChampions.length} champions`,
      );
    } else {
      console.error("Yuumi - Extracted champion list is empty");
    }
  } else {
    console.error("Yuumi - All champion endpoints failed");
  }

  return cachedChampions || [];
}

/** 请求腾讯英雄别称/拼音检索库（有缓存直接返回，多调用共享同一个 promise） */
export async function fetchKeywords(): Promise<Record<number, string>> {
  if (cachedKeywords) return cachedKeywords;
  if (loadKeywordsPromise) return loadKeywordsPromise;

  loadKeywordsPromise = doFetchKeywords();
  return loadKeywordsPromise;
}

async function doFetchKeywords(): Promise<Record<number, string>> {
  const map: Record<number, string> = {};
  try {
    const res = await fetch(
      "https://game.gtimg.cn/images/lol/act/img/js/heroList/hero_list.js",
    );
    if (res.ok) {
      const data = await res.json();
      if (data && Array.isArray(data.hero)) {
        for (const h of data.hero) {
          const id = Number(h.heroId);
          if (id > 0) {
            map[id] =
              (h.keywords || "") +
              "," +
              (h.title || "") +
              "," +
              (h.alias || "");
          }
        }
        if (map[901]) map[901] += ",小火龙";
        if (map[950]) map[950] += ",狗,那亚菲利";
        if (map[902]) map[902] += ",丁真,米利欧";
        if (map[897]) map[897] += ",黑龙,奎桑提";
        cachedKeywords = map;
        console.log(
          "Yuumi - Successfully loaded Tencent champion alias/pinyin library",
        );
      }
    }
  } catch (e) {
    console.warn("Yuumi - Failed to load Tencent champion alias endpoint:", e);
  }
  return map;
}

/**
 * 将 LiveClientData 返回的 championName 或 rawChampionName 匹配为 championId
 * LiveClientData 返回的 championName 通常为英文 Alias（如 "Ahri", "MonkeyKing", "LeeSin", "MissFortune"）
 * 或带有特例（如 "FiddleSticks", "DrMundo", "TwistedFate"）
 */
const LIVECLIENT_SPECIAL_ALIAS_MAP: Record<string, number> = {
  // 特例英雄 ID 映射
  wukong: 62,
  monkeyking: 62,
  drmundo: 36,
  fiddlesticks: 9,
  twistedfate: 4,
  xinzhao: 5,
  missfortune: 21,
  jarvaniv: 59,
  masteryi: 11,
  tahmkench: 223,
  aurelionsol: 136,
  kogmaw: 96,
  reksai: 421,
  ksante: 897,
  leblanc: 7,
  nunu: 20,
  nunuiwillump: 20,
  renataglasc: 888,
  belveth: 200,
};

export function findChampionIdByLiveClientName(
  champList: ChampionEntry[],
  champName?: string,
  rawChampName?: string,
): number {
  if (!champName && !rawChampName) return 0;

  const rawTrimmed = (rawChampName || "").trim();
  const nameTrimmed = (champName || "").trim();

  // 1. 直接全等匹配中文名或称号（例如 "放逐之刃", "影流之主", "锐雯", "劫"）
  if (nameTrimmed) {
    for (const c of champList) {
      if (
        c.name === nameTrimmed ||
        c.title === nameTrimmed ||
        c.alias === nameTrimmed
      ) {
        return c.id;
      }
    }
  }
  if (rawTrimmed) {
    for (const c of champList) {
      if (
        c.name === rawTrimmed ||
        c.title === rawTrimmed ||
        c.alias === rawTrimmed
      ) {
        return c.id;
      }
    }
  }

  // 2. 尝试从 rawChampionName 提取英文（例如 "game_character_displayname_Ahri" 或 "Ahri"）
  const cleanRaw = (rawChampName || "")
    .replace(/^game_character_displayname_/i, "")
    .replace(/[^a-zA-Z0-9]/g, "")
    .toLowerCase();
  const cleanName = (champName || "").replace(/[^a-zA-Z0-9]/g, "").toLowerCase();

  // 3. 特例表命中
  if (cleanName && LIVECLIENT_SPECIAL_ALIAS_MAP[cleanName]) {
    return LIVECLIENT_SPECIAL_ALIAS_MAP[cleanName];
  }
  if (cleanRaw && LIVECLIENT_SPECIAL_ALIAS_MAP[cleanRaw]) {
    return LIVECLIENT_SPECIAL_ALIAS_MAP[cleanRaw];
  }

  // 4. 在已缓存的英雄列表项中比对 alias 或 clean name
  for (const c of champList) {
    const cAliasClean = (c.alias || "").replace(/[^a-zA-Z0-9]/g, "").toLowerCase();
    const cNameClean = (c.name || "").replace(/[^a-zA-Z0-9]/g, "").toLowerCase();

    if (cleanName && (cAliasClean === cleanName || cNameClean === cleanName)) {
      return c.id;
    }
    if (cleanRaw && (cAliasClean === cleanRaw || cNameClean === cleanRaw)) {
      return c.id;
    }
  }

  // 5. 模糊包含匹配（中文称号/名字，如 "放逐之刃" 包含在 "放逐之刃锐雯" 中，或反之；限制长度>=2避免单字误伤）
  if (nameTrimmed && nameTrimmed.length >= 2) {
    for (const c of champList) {
      if (
        (c.name && (c.name.includes(nameTrimmed) || nameTrimmed.includes(c.name))) ||
        (c.title && (c.title.includes(nameTrimmed) || nameTrimmed.includes(c.title)))
      ) {
        return c.id;
      }
    }
  }

  // 6. 比对 iconPath 中的英雄名字段
  for (const c of champList) {
    const iconLower = c.iconPath.toLowerCase();
    if (cleanName && iconLower.includes(`/${cleanName}.png`)) {
      return c.id;
    }
    if (cleanRaw && iconLower.includes(`/${cleanRaw}.png`)) {
      return c.id;
    }
  }

  return 0;
}
