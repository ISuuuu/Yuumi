/**
 * 英雄列表缓存模块。
 * 使用模块级 Map + Promise 去重，确保整个应用生命周期内只请求一次。
 * 独立于 Vue SFC 编译，不受组件销毁/重建影响。
 */

import i18n from "../i18n";

export interface ChampionEntry {
  id: number;
  name: string;
  iconPath: string;
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
  let rawData: any = null;

  // 尝试 1: champion-summary.json
  try {
    const resp = await lcuRequest<any>(
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
      const resp = await lcuRequest<any>(
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
    let list: any[] = [];
    if (Array.isArray(rawData)) {
      list = rawData;
    } else if (typeof rawData === "object" && rawData !== null) {
      list = Object.values(rawData);
    }

    if (list.length > 0) {
      const isEnglish = (i18n.global.locale as any).value === "en_US";
      cachedChampions = list
        .filter((c: any) => c && c.id > 0)
        .map((c: any) => ({
          id: c.id,
          name:
            isEnglish && c.alias ? c.alias : c.name || c.alias || `#${c.id}`,
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
