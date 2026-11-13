//! 战利品与精粹资产解析
//!
//! 对照 Vue 版 `src-tauri/src/loot.rs` 的 `get_loot_inventory`：
//! - 过滤空 lootId、TFT 系列与 count<=0 的脏数据；
//! - 名称兜底归一（钥匙 / 钥匙碎片 / 杰作钥匙 / 魔法引擎）；
//! - 升级精粹消耗读取 `upgradeEssenceValue`（`upgradeEssenceCost` 在真实载荷中不存在）；
//! - LCU 的 `itemStatus` 常年为 NONE，需用已拥有皮肤/图标/表情/守卫集合交叉校验。

use crate::lcu::client::LcuClient;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EssenceBalances {
    pub blue_essence: i32,
    pub orange_essence: i32,
    pub mythic_essence: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LootInventoryItem {
    pub loot_id: String,
    pub name: String,
    pub item_desc: String,
    pub loot_type: String,
    pub count: i32,
    pub value: i32,
    pub disenchant_value: i32,
    pub tile_path: Option<String>,
    /// 展示分类（CHAMPION / SKIN / EMOTE / MATERIAL ...），筛选与卡片精粹色依赖它
    pub display_categories: String,
    /// 拥有状态（OWNED / NONE ...），拥有筛选与升级前置校验依赖它
    pub item_status: String,
    /// 皮肤对应英雄的拥有状态
    pub parent_item_status: String,
    /// 升级配方名（为空时动作层按关键字兜底查找）
    pub upgrade_recipe_name: Option<String>,
    /// 升级所需精粹（`upgradeEssenceValue`）
    pub upgrade_essence_cost: i32,
    pub rarity: String,
}

/// 已拥有道具的商店 ID 集合（按 displayCategories 区分）
#[derive(Debug, Default)]
struct OwnedStoreIds {
    skins: HashSet<i32>,
    icons: HashSet<i32>,
    emotes: HashSet<i32>,
    ward_skins: HashSet<i32>,
}

fn collect_ids(value: &serde_json::Value, id_keys: &[&str], out: &mut HashSet<i32>) {
    let arrays: Vec<&Vec<serde_json::Value>> = match value {
        serde_json::Value::Array(arr) => vec![arr],
        serde_json::Value::Object(obj) => obj
            .values()
            .filter_map(|v| v.as_array())
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    for arr in arrays {
        for item in arr {
            for key in id_keys {
                if let Some(id) = item.get(*key).and_then(|v| v.as_i64()) {
                    out.insert(id as i32);
                    break;
                }
            }
        }
    }
}

fn collect_owned_skins(value: &serde_json::Value, out: &mut HashSet<i32>) {
    let Some(arr) = value.as_array() else {
        return;
    };
    for item in arr {
        let owned = item
            .get("ownership")
            .and_then(|o| o.get("owned"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !owned {
            continue;
        }
        if let Some(id) = item.get("id").and_then(|v| v.as_i64()) {
            out.insert(id as i32);
        }
    }
}

/// 并发拉取已拥有的皮肤 / 图标 / 表情 / 守卫皮肤 ID，用于修正 LCU 的 itemStatus
async fn fetch_owned_store_ids(client: &LcuClient) -> OwnedStoreIds {
    let mut owned = OwnedStoreIds::default();

    let summoner = match client.get("/lol-summoner/v1/current-summoner").await {
        Ok(v) => v,
        Err(e) => {
            log::warn!("[Loot] 获取当前召唤师失败，跳过拥有状态修正: {}", e);
            return owned;
        }
    };
    let Some(summoner_id) = summoner.get("summonerId").and_then(|v| v.as_i64()) else {
        return owned;
    };

    let skins_path = format!(
        "/lol-champions/v1/inventories/by-summoner/{}/skins",
        summoner_id
    );
    let icons_path = format!(
        "/lol-collections/v1/inventories/{}/summoner-icons",
        summoner_id
    );
    let wards_path = format!("/lol-collections/v1/inventories/{}/ward-skins", summoner_id);
    let emotes_path = "/lol-inventory/v1/inventory/emotes".to_string();

    let (skins, icons, emotes, wards) = tokio::join!(
        client.get(&skins_path),
        client.get(&icons_path),
        client.get(&emotes_path),
        client.get(&wards_path),
    );

    if let Ok(v) = skins {
        collect_owned_skins(&v, &mut owned.skins);
    }
    if let Ok(v) = icons {
        collect_ids(&v, &["id", "iconId"], &mut owned.icons);
    }
    if let Ok(v) = emotes {
        collect_ids(&v, &["itemId", "id"], &mut owned.emotes);
    }
    if let Ok(v) = wards {
        collect_owned_skins(&v, &mut owned.ward_skins);
    }

    owned
}

/// 名称归一（Vue `getLootDisplayName` / `get_loot_inventory` 的兜底文案）
pub(crate) fn normalize_loot_name(loot_id: &str, name: &str) -> String {
    let trimmed = name.trim();
    if trimmed == "MATERIAL_key_fragment" || trimmed.eq_ignore_ascii_case("MATERIAL_key_fragment") {
        return "钥匙碎片".to_string();
    }
    if trimmed == "MATERIAL_key" || trimmed.eq_ignore_ascii_case("MATERIAL_key") {
        return "海克斯科技钥匙".to_string();
    }
    if trimmed == "MATERIAL_key_premium" || trimmed.eq_ignore_ascii_case("MATERIAL_key_premium") {
        return "杰作钥匙".to_string();
    }
    if trimmed.eq_ignore_ascii_case("chest_128") {
        return "英雄魔法引擎".to_string();
    }
    if trimmed.eq_ignore_ascii_case("chest_129") {
        return "荣耀英雄魔法引擎".to_string();
    }
    if trimmed.is_empty() {
        return loot_id.to_string();
    }
    trimmed.to_string()
}

/// 获取战利品库存与精粹资产
pub async fn fetch_loot_summary(
    client: &LcuClient,
) -> Result<(EssenceBalances, Vec<LootInventoryItem>), String> {
    let raw_val = client.get("/lol-loot/v1/player-loot").await?;
    let arr = raw_val
        .as_array()
        .ok_or_else(|| "战利品数据格式错误，期望数组".to_string())?;

    let mut essences = EssenceBalances::default();
    let mut items = Vec::new();

    // 先收集原始条目，随后再决定是否需要交叉校验拥有状态（只有碎片/皮肤类才需要）
    let mut raw_items: Vec<&serde_json::Value> = Vec::new();
    for item in arr {
        let loot_id = item
            .get("lootId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let count = item.get("count").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

        // 3 类精粹单独取出，不作为碎片条目
        if loot_id == "CURRENCY_champion" {
            essences.blue_essence = count;
            continue;
        } else if loot_id == "CURRENCY_cosmetic" {
            essences.orange_essence = count;
            continue;
        } else if loot_id == "CURRENCY_mythic" {
            essences.mythic_essence = count;
            continue;
        }

        // 对照 Vue：空 lootId（真实载荷中确实存在）与 TFT 系列直接跳过
        if loot_id.is_empty() || loot_id.to_uppercase().starts_with("TFT") || count <= 0 {
            continue;
        }
        raw_items.push(item);
    }

    let owned = fetch_owned_store_ids(client).await;

    for item in raw_items {
        let loot_id = item
            .get("lootId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let count = item.get("count").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

        let loot_type = item
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let raw_name = item
            .get("localizedName")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                item.get("itemDesc")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
            })
            .unwrap_or(&loot_id)
            .to_string();
        let item_desc = normalize_loot_name(&loot_id, &raw_name);
        if item_desc.trim().is_empty() {
            continue;
        }

        let value = item.get("value").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let disenchant_value = item
            .get("disenchantValue")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;

        let mut tile_path = item
            .get("tilePath")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        if tile_path.is_none() {
            tile_path = item
                .get("imagePath")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
        }

        let str_field = |key: &str| {
            item.get(key)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };
        let display_categories = {
            let raw = str_field("displayCategories");
            if raw.is_empty() {
                loot_type.clone()
            } else {
                raw
            }
        };

        // 对照 Vue：LCU 的 itemStatus 常年为 NONE，用已拥有集合交叉校验
        let mut item_status = str_field("itemStatus");
        let store_item_id = item
            .get("storeItemId")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        if store_item_id > 0 {
            let cat = display_categories.to_uppercase();
            let is_owned = match cat.as_str() {
                "SKIN" => owned.skins.contains(&store_item_id),
                "SUMMONERICON" => owned.icons.contains(&store_item_id),
                "EMOTE" => owned.emotes.contains(&store_item_id),
                "WARDSKIN" => owned.ward_skins.contains(&store_item_id),
                _ => false,
            };
            if is_owned {
                item_status = "OWNED".to_string();
            }
        }

        let parent_item_status = str_field("parentItemStatus");
        let upgrade_recipe_name = item
            .get("upgradeRecipeName")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        // 真实载荷只提供 upgradeEssenceValue，旧字段 upgradeEssenceCost 仅作兼容兜底
        let upgrade_essence_cost = item
            .get("upgradeEssenceValue")
            .and_then(|v| v.as_i64())
            .or_else(|| item.get("upgradeEssenceCost").and_then(|v| v.as_i64()))
            .unwrap_or(0) as i32;
        let rarity = str_field("rarity");

        items.push(LootInventoryItem {
            loot_id,
            name: item_desc.clone(),
            item_desc,
            loot_type,
            count,
            value,
            disenchant_value,
            tile_path,
            display_categories,
            item_status,
            parent_item_status,
            upgrade_recipe_name,
            upgrade_essence_cost,
            rarity,
        });
    }

    Ok((essences, items))
}
