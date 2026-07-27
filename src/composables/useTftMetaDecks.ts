import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

export interface TftMetaUnit {
  cell: { x: number; y: number };
  /** OP.GG MCP 返回字段；兼容历史 characterId */
  key?: string;
  characterId: string;
  items: (string | null)[];
  iconUrl?: string;
  tier?: number;
  isCore?: boolean;
  priority?: number | null;
}

export interface TftMetaDeckTrait {
  key: string;
  numUnits: number;
  style: number;
}

export interface TftMetaPhase {
  deckId: string;
  finalDeckId: string;
  level: string;
  lose: number;
  play: number;
  traits: TftMetaDeckTrait[];
  units: TftMetaUnit[];
  win: number;
}

export interface TftMetaDeckBadge {
  key: string;
  value: unknown;
}

export interface TftMetaDeckStat {
  win_rate?: number;
  pick_rate?: number;
  avg_placement?: number;
  top4_rate?: number;
  play_count?: number;
  [key: string]: unknown;
}

export interface TftMetaDeck {
  id: string;
  name: Record<string, string>;
  cost: number;
  teamCode: string;
  badge: TftMetaDeckBadge[];
  stat: TftMetaDeckStat;
  traits: TftMetaDeckTrait[];
  units: TftMetaUnit[];
  early?: TftMetaPhase | null;
  middle?: TftMetaPhase | null;
}

let cachedDecks: TftMetaDeck[] | null = null;
let cachedMetadata: {
  gameStatCounts?: number;
  gameStatDateTime?: string;
} | null = null;
let traitNameMap: Record<string, string> = {};
let championIconMap: Record<string, string> = {};
let championNameMap: Record<string, string> = {};
let itemNameMap: Record<string, string> = {};
let itemIconMap: Record<string, string> = {};

function normalizeTraitKey(key: string): string {
  const parts = key.split("_");
  let last = parts[parts.length - 1] ?? key;
  // 下划线分隔的 _Trait 后缀
  if (last === "Trait" && parts.length > 1) {
    last = parts[parts.length - 2];
  }
  // 无下划线直接贴末尾的 "Trait"（如 RhaastUniqueTrait → RhaastUnique）
  if (last.endsWith("Trait") && last.length > 5) {
    last = last.slice(0, -5);
  }
  return last;
}

export function getChampionIconUrl(
  characterId: string | null | undefined,
): string | undefined {
  if (!characterId) return undefined;
  const url =
    championIconMap[characterId] ?? championIconMap[characterId.toLowerCase()];
  if (url) return url;

  const baseName = characterId.split("_").pop()?.toLowerCase();
  if (baseName && baseName !== characterId.toLowerCase()) {
    const found = Object.entries(championIconMap).find(([key]) =>
      key.toLowerCase().endsWith(baseName),
    );
    if (found) return found[1];
  }

  console.log(
    "[TFT Meta] 未找到图标:",
    characterId,
    "映射键样例:",
    Object.keys(championIconMap).slice(0, 5),
  );
  return undefined;
}

export function getItemIconUrl(
  item: string | null | undefined,
): string | undefined {
  if (!item) return undefined;
  let url = itemIconMap[item] ?? itemIconMap[item.toLowerCase()];
  if (url) return url;

  const cleaned = item.replace(/^TFT_Item_/i, "");
  url =
    itemIconMap[cleaned] ??
    itemIconMap[cleaned.toLowerCase()] ??
    itemIconMap[`tft_item_${cleaned.toLowerCase()}`];
  if (url) return url;

  const found = Object.entries(itemIconMap).find(([k]) =>
    k.toLowerCase().endsWith(cleaned.toLowerCase()),
  );
  if (found) return found[1];

  return undefined;
}

function formatTraitName(raw: string): string {
  let s = raw
    .replace(/trait$/i, "")
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .replace(/^[a-z]/, (c) => c.toUpperCase());
  return s || raw;
}

export function getDeckDisplayName(deck: TftMetaDeck): string {
  return (
    deck.name?.zh_CN ??
    deck.name?.en_US ??
    Object.values(deck.name ?? {})[0] ??
    ""
  );
}

export function getChampionDisplayName(
  characterId: string | null | undefined,
): string {
  if (!characterId) return "?";
  const name =
    championNameMap[characterId] ?? championNameMap[characterId.toLowerCase()];
  if (name) return name;
  const parts = characterId.split("_");
  return parts[parts.length - 1] ?? characterId;
}

export function getTraitDisplayName(key: string): string {
  const normalized = normalizeTraitKey(key).toLowerCase();
  const result =
    traitNameMap[normalized] ?? traitNameMap[key.toLowerCase()] ?? null;
  if (result) return result;
  console.log(
    "[TFT Traits] 未命中映射 → key:",
    key,
    "| normalized:",
    normalized,
    "| 映射键样例:",
    Object.keys(traitNameMap).slice(0, 15),
  );
  return formatTraitName(key);
}

export function getItemDisplayName(item: string | null | undefined): string {
  if (!item) return "";
  let name = itemNameMap[item] ?? itemNameMap[item.toLowerCase()];
  if (name) return name;

  const cleaned = item.replace(/^TFT_Item_/i, "");
  name =
    itemNameMap[cleaned] ??
    itemNameMap[cleaned.toLowerCase()] ??
    itemNameMap[`tft_item_${cleaned.toLowerCase()}`];
  if (name) return name;

  const found = Object.entries(itemNameMap).find(([k]) =>
    k.toLowerCase().endsWith(cleaned.toLowerCase()),
  );
  if (found) return found[1];

  // 兜底：按驼峰分词
  const formatted = cleaned.replace(/([a-z])([A-Z])/g, "$1 $2");
  return formatted || item;
}

export function useTftMetaDecks() {
  const loading = ref(false);
  const error = ref("");
  const decks = ref<TftMetaDeck[]>(cachedDecks ?? []);
  const metadata = ref<{
    gameStatCounts?: number;
    gameStatDateTime?: string;
  } | null>(cachedMetadata);

  async function loadDecks(forceRefresh = false) {
    if (!forceRefresh && cachedDecks) {
      decks.value = cachedDecks;
      metadata.value = cachedMetadata;
      return;
    }

    loading.value = true;
    error.value = "";

    try {
      const raw = await invoke<any>("fetch_tft_meta_decks");
      if (raw?.metadata) {
        cachedMetadata = raw.metadata;
        metadata.value = raw.metadata;
      }
      const list: TftMetaDeck[] =
        raw?.data ?? raw?.decks ?? (Array.isArray(raw) ? raw : []);
      if (raw?.trait_name_map && Object.keys(raw.trait_name_map).length > 0) {
        traitNameMap = raw.trait_name_map;
        console.log(
          "[TFT Meta] 羁绊名称映射:",
          Object.keys(traitNameMap).length,
          "条",
        );
      } else {
        console.warn("[TFT Meta] trait_name_map 为空");
      }
      if (
        raw?.champion_icon_map &&
        Object.keys(raw.champion_icon_map).length > 0
      ) {
        championIconMap = raw.champion_icon_map;
        console.log(
          "[TFT Meta] 英雄图标映射:",
          Object.keys(championIconMap).length,
          "条, 样例:",
          Object.entries(championIconMap).slice(0, 3),
        );
      } else {
        console.warn("[TFT Meta] champion_icon_map 为空");
      }
      if (
        raw?.champion_name_map &&
        Object.keys(raw.champion_name_map).length > 0
      ) {
        championNameMap = raw.champion_name_map;
        console.log(
          "[TFT Meta] 英雄名称映射:",
          Object.keys(championNameMap).length,
          "条, 样例:",
          Object.entries(championNameMap).slice(0, 3),
        );
      } else {
        console.warn("[TFT Meta] champion_name_map 为空");
      }
      if (raw?.item_name_map && Object.keys(raw.item_name_map).length > 0) {
        itemNameMap = raw.item_name_map;
        console.log(
          "[TFT Meta] 物品名称映射:",
          Object.keys(itemNameMap).length,
          "条, 样例:",
          Object.entries(itemNameMap).slice(0, 3),
        );
      } else {
        console.warn("[TFT Meta] item_name_map 为空");
      }
      if (raw?.item_icon_map && Object.keys(raw.item_icon_map).length > 0) {
        itemIconMap = raw.item_icon_map;
        console.log(
          "[TFT Meta] 物品图标映射:",
          Object.keys(itemIconMap).length,
          "条, 样例:",
          Object.entries(itemIconMap).slice(0, 3),
        );
      } else {
        console.warn("[TFT Meta] item_icon_map 为空");
      }
      // 归一化 unit：OP.GG MCP 用 key 表示棋子 ID，不是 characterId
      const normalizeUnit = (unit: TftMetaUnit) => {
        const raw = unit as TftMetaUnit & { character_id?: string };
        unit.characterId =
          unit.characterId || raw.key || raw.character_id || "";
        unit.iconUrl = getChampionIconUrl(unit.characterId);
      };
      for (const deck of list) {
        for (const unit of deck.units ?? []) {
          normalizeUnit(unit);
        }
        for (const phase of [deck.early, deck.middle].filter(Boolean)) {
          for (const unit of phase!.units ?? []) {
            normalizeUnit(unit);
          }
        }
      }
      // 打印第一个 deck 的 unit sample 用于调试
      if (list.length > 0) {
        const sample = list[0];
        console.log(
          "[TFT Meta] 首个 deck 样例:",
          sample.id,
          sample.units?.slice(0, 2),
        );
      }
      cachedDecks = list;
      decks.value = list;
    } catch (e: any) {
      console.error("加载 TFT 热门阵容失败:", e);
      error.value = e?.toString() || "加载阵容数据失败";
    } finally {
      loading.value = false;
    }
  }

  function refresh() {
    traitNameMap = {};
    championIconMap = {};
    championNameMap = {};
    itemNameMap = {};
    itemIconMap = {};
    return loadDecks(true);
  }

  return { loading, error, decks, metadata, loadDecks, refresh };
}
