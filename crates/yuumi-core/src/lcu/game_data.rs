use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tokio::time::{sleep, Duration};

use super::client::{build_auth_header, LcuClient};

/// 游戏资源加载重试次数和间隔
const GAME_DATA_RETRIES: u32 = 3;
const GAME_DATA_RETRY_DELAY: Duration = Duration::from_secs(2);

/// 海克斯强化详情（来自 LCU cherry-augments.json）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CherryAugmentDetail {
    pub id: i32,
    pub name: String,
    pub icon_path: String,
}

/// LCU 连接后预加载的游戏资源路径映射（ID → iconPath / name）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GameDataAssets {
    /// 物品 ID → iconPath
    pub items: HashMap<i32, String>,
    /// 召唤师技能 ID → iconPath
    pub spells: HashMap<i32, String>,
    /// 符文 (perk) ID → iconPath
    pub runes: HashMap<i32, String>,
    /// 英雄 ID → 英雄名称
    pub champions: HashMap<i32, String>,
    /// 海克斯强化 ID → 详情（名称、描述、图标路径）
    pub augments: HashMap<i32, CherryAugmentDetail>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct IdEntry {
    id: Option<i32>,
    #[serde(rename = "iconPath")]
    icon_path: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

/// 从 LCU 预加载所有游戏资源路径（带重试）。
pub async fn fetch_game_data_assets(lcu: &LcuClient) -> GameDataAssets {
    let mut last_assets = GameDataAssets::default();

    for attempt in 1..=GAME_DATA_RETRIES {
        last_assets = fetch_game_data_assets_inner(lcu).await;

        let is_empty = last_assets.items.is_empty()
            && last_assets.spells.is_empty()
            && last_assets.runes.is_empty()
            && last_assets.champions.is_empty();

        if !is_empty {
            return last_assets;
        }

        if attempt < GAME_DATA_RETRIES {
            log::warn!(
                "游戏资源加载为空 ({}/{})，{}秒后重试...",
                attempt,
                GAME_DATA_RETRIES,
                GAME_DATA_RETRY_DELAY.as_secs()
            );
            sleep(GAME_DATA_RETRY_DELAY).await;
        }
    }

    log::warn!("游戏资源在 {} 次尝试后仍为空", GAME_DATA_RETRIES);
    last_assets
}

/// 从 LCU 预加载所有游戏资源路径（单次尝试）。
async fn fetch_game_data_assets_inner(lcu: &LcuClient) -> GameDataAssets {
    let auth = build_auth_header(&lcu.token);
    let base = format!("https://127.0.0.1:{}", lcu.port);

    let (items, spells, runes, champions, augments) = tokio::join!(
        fetch_id_map(
            &lcu.http_client,
            &base,
            &auth,
            "/lol-game-data/assets/v1/items.json"
        ),
        fetch_id_map(
            &lcu.http_client,
            &base,
            &auth,
            "/lol-game-data/assets/v1/summoner-spells.json"
        ),
        fetch_id_map(
            &lcu.http_client,
            &base,
            &auth,
            "/lol-game-data/assets/v1/perks.json"
        ),
        fetch_champion_map(&lcu.http_client, &base, &auth),
        fetch_cherry_augment_map(
            &lcu.http_client,
            &base,
            &auth,
            "/lol-game-data/assets/v1/cherry-augments.json",
        ),
    );

    let items = if items.is_empty() {
        fetch_id_map_flexible(
            &lcu.http_client,
            &base,
            &auth,
            "/lol-game-data/assets/v1/items.json",
        )
        .await
    } else {
        items
    };

    let spells = if spells.is_empty() {
        fetch_id_map_flexible(
            &lcu.http_client,
            &base,
            &auth,
            "/lol-game-data/assets/v1/summoner-spells.json",
        )
        .await
    } else {
        spells
    };

    let runes = if runes.is_empty() {
        fetch_id_map_flexible(
            &lcu.http_client,
            &base,
            &auth,
            "/lol-game-data/assets/v1/perks.json",
        )
        .await
    } else {
        runes
    };

    GameDataAssets {
        items,
        spells,
        runes,
        champions,
        augments,
    }
}

async fn fetch_id_map(
    http: &reqwest::Client,
    base: &str,
    auth: &str,
    path: &str,
) -> HashMap<i32, String> {
    let url = format!("{}{}", base, path);

    match http.get(&url).header("Authorization", auth).send().await {
        Ok(resp) if resp.status().is_success() => match resp.json::<Vec<IdEntry>>().await {
            Ok(entries) => entries
                .into_iter()
                .filter_map(|e| Some((e.id?, e.icon_path?)))
                .collect(),
            Err(_) => HashMap::new(),
        },
        _ => HashMap::new(),
    }
}

async fn fetch_champion_map(
    http: &reqwest::Client,
    base: &str,
    auth: &str,
) -> HashMap<i32, String> {
    let url = format!("{}/lol-game-data/assets/v1/champion-summary.json", base);
    match http.get(&url).header("Authorization", auth).send().await {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(arr) = resp.json::<Vec<serde_json::Value>>().await {
                return arr
                    .iter()
                    .filter_map(|entry| {
                        let id = entry.get("id")?.as_i64()? as i32;
                        let name = entry.get("name")?.as_str()?.to_string();
                        if name.is_empty() {
                            None
                        } else {
                            Some((id, name))
                        }
                    })
                    .collect();
            }
        }
        _ => {}
    }
    HashMap::new()
}

async fn fetch_cherry_augment_map(
    http: &reqwest::Client,
    base: &str,
    auth: &str,
    path: &str,
) -> HashMap<i32, CherryAugmentDetail> {
    let url = format!("{}{}", base, path);
    let Ok(resp) = http.get(&url).header("Authorization", auth).send().await else {
        return HashMap::new();
    };

    if !resp.status().is_success() {
        return HashMap::new();
    }

    let Ok(text) = resp.text().await else {
        return HashMap::new();
    };

    let Ok(arr) = serde_json::from_str::<Vec<Value>>(&text) else {
        return HashMap::new();
    };

    fn extract_str(val: &Value) -> String {
        val.as_str().unwrap_or_default().to_string()
    }

    arr.into_iter()
        .filter_map(|entry| {
            let obj = entry.as_object()?;
            let id = obj.get("id")?.as_i64()? as i32;

            let icon_path = obj
                .get("augmentSmallIconPath")
                .or_else(|| obj.get("iconPath"))
                .or_else(|| obj.get("icon"))
                .map(extract_str)?;
            if icon_path.is_empty() {
                return None;
            }

            let name = obj
                .get("nameTRA")
                .or_else(|| obj.get("simpleNameTRA"))
                .or_else(|| obj.get("name"))
                .map(extract_str)
                .unwrap_or_default();

            Some((
                id,
                CherryAugmentDetail {
                    id,
                    name,
                    icon_path,
                },
            ))
        })
        .collect()
}

async fn fetch_id_map_flexible(
    http: &reqwest::Client,
    base: &str,
    auth: &str,
    path: &str,
) -> HashMap<i32, String> {
    let url = format!("{}{}", base, path);

    let Ok(resp) = http.get(&url).header("Authorization", auth).send().await else {
        return HashMap::new();
    };

    if !resp.status().is_success() {
        return HashMap::new();
    }

    let Ok(val) = resp.json::<Value>().await else {
        return HashMap::new();
    };

    let mut map = HashMap::new();

    if let Some(arr) = val.as_array() {
        for item in arr {
            if let (Some(id), Some(icon)) = (
                item.get("id").and_then(|v| v.as_i64()),
                item.get("iconPath").and_then(|v| v.as_str()),
            ) {
                map.insert(id as i32, icon.to_string());
            }
        }
    } else if let Some(obj) = val.as_object() {
        for (_k, v) in obj {
            if let (Some(id), Some(icon)) = (
                v.get("id").and_then(|i| i.as_i64()),
                v.get("iconPath").and_then(|i| i.as_str()),
            ) {
                map.insert(id as i32, icon.to_string());
            }
        }
    }

    map
}
