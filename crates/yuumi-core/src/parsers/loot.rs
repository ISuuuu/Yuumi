//! 战利品与精粹资产解析

use crate::lcu::client::LcuClient;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EssenceBalances {
    pub blue_essence: i32,
    pub orange_essence: i32,
    pub mythic_essence: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    pub is_openable: bool,
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

    for item in arr {
        let loot_id = item
            .get("lootId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let count = item.get("count").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

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

        if count <= 0 {
            continue;
        }

        let loot_type = item
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let name = item
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

        let item_desc = item
            .get("itemDesc")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let value = item.get("value").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let disenchant_value = item
            .get("disenchantValue")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;

        let mut tile_path = item
            .get("tilePath")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        if tile_path.is_none() {
            tile_path = item
                .get("imagePath")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
        }

        let is_openable = loot_type.contains("CHEST")
            || loot_type.contains("ORB")
            || loot_type.contains("CAPSULE")
            || loot_id.contains("CHEST")
            || loot_id.contains("ORB");

        items.push(LootInventoryItem {
            loot_id,
            name,
            item_desc,
            loot_type,
            count,
            value,
            disenchant_value,
            tile_path,
            is_openable,
        });
    }

    Ok((essences, items))
}
