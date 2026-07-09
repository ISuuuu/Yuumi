import { invoke } from "@tauri-apps/api/core";

export interface OpenableLoot {
  lootId: string;
  name: string;
  count: number;
  recipeName: string;
  needKey: boolean;
  keyLootId: string | null;
  keyCount?: number;
  keyName?: string | null;
  tilePath?: string | null;
}

export interface LootProgressEvent {
  current: number;
  total: number;
  success: boolean;
  rewardName: string;
  errorMsg: string | null;
  itemName?: string | null;
}

export interface OpenBatchItem {
  lootId: string;
  name: string;
  count: number;
  recipeName: string;
  ingredients: string[];
}

/** 获取所有可开启的战利品（箱子、法球等） */
export const fetchOpenableLoots = () =>
  invoke<OpenableLoot[]>("get_openable_loots");

/** 后台批量开启战利品 */
export const batchOpenLoots = (
  recipeName: string,
  ingredients: string[],
  repeatCount: number,
) =>
  invoke<string>("batch_open_loots", {
    recipeName,
    ingredients,
    repeatCount,
  });

/** 智能一键开启：按优先级自动分配钥匙 */
export const smartOpenAllLoots = (items: OpenBatchItem[]) =>
  invoke<string>("smart_open_all_loots", { items });

// ─── 碎片库存管理（分解 / 三合一重随）───

export interface LootItem {
  lootId: string;
  lootName: string;
  itemDesc: string;
  displayCategories: string; // CHAMPION, SKIN, EMOTE, WARD_SKIN, SUMMONERICON
  lootType: string;
  rarity: string;
  count: number;
  value: number;
  disenchantValue: number;
  itemStatus: string; // OWNED, NONE, RENTAL
  tilePath: string | null;
  upgradeRecipeName: string;
  upgradeEssenceCost: number;
}

export interface DisenchantItem {
  lootId: string;
  count: number;
  upgradeRecipeName?: string;
}

export interface ActionProgressEvent {
  current: number;
  total: number;
  success: boolean;
  lootName: string;
  rewardDesc: string;
  errorMsg: string | null;
}

/** 获取玩家所有的碎片库存（排除了材料） */
export const fetchLootInventory = () =>
  invoke<LootItem[]>("get_loot_inventory");

/** 批量分解选中的碎片 */
export const disenchantLoot = (items: DisenchantItem[]) =>
  invoke<string>("disenchant_loot", { items });

/** 批量三合一重随选中的碎片 */
export const rerollLoot = (lootIds: string[]) =>
  invoke<string>("reroll_loot", { lootIds });

/** 批量升级选中的碎片为永久 */
export const upgradeLoot = (items: DisenchantItem[]) =>
  invoke<string>("upgrade_loot", { items });

export interface EssenceBalances {
  blueEssence: number;
  orangeEssence: number;
}

/** 获取玩家蓝/橙精粹余额 */
export const fetchEssenceBalances = () =>
  invoke<EssenceBalances>("get_essence_balances");
