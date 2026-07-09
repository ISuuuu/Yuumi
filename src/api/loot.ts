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
