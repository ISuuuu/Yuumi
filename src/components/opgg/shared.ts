/**
 * OP.GG 组件群共享的格式化与图标路径解析工具。
 * 图标解析依赖预加载资源映射（gameDataAssets）与符文体系列表，由调用方传入。
 */
import type { GameDataAssets } from "../../types/lcu";
import type { PerkStyle } from "../../types/opgg";

/** 百分比格式化：0.5234 -> "52.3%"，空值显示 "-" */
export function pct(v: number | null | undefined): string {
  return v != null ? (v * 100).toFixed(1) + "%" : "-";
}

/** KDA 格式化：保留两位小数，空值显示 "-" */
export function fmtKda(k: number | null | undefined): string {
  return k != null ? Number(k).toFixed(2) : "-";
}

/** 英雄图标路径 */
export function champIcon(id: number): string {
  return `/lol-game-data/assets/v1/champion-icons/${id}.png`;
}

/** 物品图标：优先预加载映射，兜底 LCU 路径 */
export function resolveItemIcon(
  assets: GameDataAssets | null | undefined,
  id: number | undefined,
): string {
  if (!id) return "";
  const preloadedPath = assets?.items?.[id];
  if (preloadedPath) {
    return preloadedPath;
  }
  return `/lol-game-data/assets/v1/items/${id}.png`;
}

/** 召唤师技能图标：优先预加载映射，兜底 LCU 路径 */
export function resolveSpellIcon(
  assets: GameDataAssets | null | undefined,
  id: number,
): string {
  const preloadedPath = assets?.spells?.[id];
  if (preloadedPath) {
    return preloadedPath;
  }
  return `/lol-game-data/assets/v1/summoner-spells/${id}.png`;
}

/** 符文图标：优先预加载映射，其次符文体系图标，兜底 LCU 路径 */
export function resolveRuneIcon(
  assets: GameDataAssets | null | undefined,
  perkStyles: PerkStyle[] | null | undefined,
  id: number,
): string {
  const preloadedPath = assets?.runes?.[id];
  if (preloadedPath) {
    return preloadedPath;
  }
  const style = perkStyles?.find((s) => s.id === id);
  if (style?.iconPath) {
    return style.iconPath;
  }
  return `/lol-game-data/assets/v1/perks/${id}.png`;
}
