use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tokio::time::{sleep, Duration};

use super::client::{build_auth_header, LcuClient};

/// 游戏资源加载重试次数和间隔（覆盖客户端启动阶段的端点就绪窗口期）
const GAME_DATA_RETRIES: u32 = 8;
const GAME_DATA_RETRY_DELAY: Duration = Duration::from_millis(1500);

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

impl GameDataAssets {
    /// 检查核心游戏资产是否均已就绪（装备、召唤师技能、英雄均非空）
    pub fn is_complete(&self) -> bool {
        !self.items.is_empty() && !self.spells.is_empty() && !self.champions.is_empty()
    }

    /// 检查是否完全为空
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
            && self.spells.is_empty()
            && self.runes.is_empty()
            && self.champions.is_empty()
            && self.augments.is_empty()
    }

    /// 用另一个 assets 补充当前缺失的部分
    pub fn merge_missing(&mut self, fallback: &GameDataAssets) {
        if self.items.is_empty() {
            self.items = fallback.items.clone();
        }
        if self.spells.is_empty() {
            self.spells = fallback.spells.clone();
        }
        if self.runes.is_empty() {
            self.runes = fallback.runes.clone();
        }
        if self.champions.is_empty() {
            self.champions = fallback.champions.clone();
        }
        if self.augments.is_empty() {
            self.augments = fallback.augments.clone();
        }
    }
}

/// 从本地磁盘缓存读取已持久化的 GameDataAssets（兜底初次连接）
pub fn load_cached_assets() -> Option<GameDataAssets> {
    let path = crate::runtime::app_data_dir()
        .join("cache")
        .join("gamedata_assets.json");
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// 将已成功且完整的 GameDataAssets 原子写入本地磁盘缓存
pub fn save_cached_assets(assets: &GameDataAssets) {
    if !assets.is_complete() {
        return;
    }
    let dir = crate::runtime::app_data_dir().join("cache");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("gamedata_assets.json");
    let temp_path = dir.join(format!("gamedata_assets.{}.tmp", std::process::id()));
    if let Ok(bytes) = serde_json::to_vec(assets) {
        if std::fs::write(&temp_path, &bytes).is_ok() {
            let _ = std::fs::rename(temp_path, path);
        }
    }
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

/// 从 LCU 预加载所有游戏资源路径（带重试与本地磁盘缓存兜底）。
pub async fn fetch_game_data_assets(lcu: &LcuClient) -> GameDataAssets {
    let mut assets = GameDataAssets::default();

    // 优先尝试读取本地磁盘缓存作为兜底底座
    let cached_opt = load_cached_assets();

    for attempt in 1..=GAME_DATA_RETRIES {
        let auth = build_auth_header(&lcu.token);
        let base = format!("https://127.0.0.1:{}", lcu.port);

        // 增量式并发获取：只请求当前尚未获取成功的项
        let fetch_items = assets.items.is_empty();
        let fetch_spells = assets.spells.is_empty();
        let fetch_runes = assets.runes.is_empty();
        let fetch_champions = assets.champions.is_empty();
        let fetch_augments = assets.augments.is_empty();

        let (new_items, new_spells, new_runes, new_champions, new_augments) = tokio::join!(
            async {
                if fetch_items {
                    fetch_id_map(
                        &lcu.http_client,
                        &base,
                        &auth,
                        "/lol-game-data/assets/v1/items.json",
                    )
                    .await
                } else {
                    HashMap::new()
                }
            },
            async {
                if fetch_spells {
                    fetch_id_map(
                        &lcu.http_client,
                        &base,
                        &auth,
                        "/lol-game-data/assets/v1/summoner-spells.json",
                    )
                    .await
                } else {
                    HashMap::new()
                }
            },
            async {
                if fetch_runes {
                    fetch_id_map(
                        &lcu.http_client,
                        &base,
                        &auth,
                        "/lol-game-data/assets/v1/perks.json",
                    )
                    .await
                } else {
                    HashMap::new()
                }
            },
            async {
                if fetch_champions {
                    fetch_champion_map(&lcu.http_client, &base, &auth).await
                } else {
                    HashMap::new()
                }
            },
            async {
                if fetch_augments {
                    fetch_cherry_augment_map(
                        &lcu.http_client,
                        &base,
                        &auth,
                        "/lol-game-data/assets/v1/cherry-augments.json",
                    )
                    .await
                } else {
                    HashMap::new()
                }
            },
        );

        if !new_items.is_empty() {
            assets.items = new_items;
        }
        if !new_spells.is_empty() {
            assets.spells = new_spells;
        }
        if !new_runes.is_empty() {
            assets.runes = new_runes;
        }
        if !new_champions.is_empty() {
            assets.champions = new_champions;
        }
        if !new_augments.is_empty() {
            assets.augments = new_augments;
        }

        if assets.is_complete() {
            log::info!(
                "[GameData] 游戏资源字典加载完整 (第 {}/{} 次成功): items={}, spells={}, runes={}, champions={}, augments={}",
                attempt,
                GAME_DATA_RETRIES,
                assets.items.len(),
                assets.spells.len(),
                assets.runes.len(),
                assets.champions.len(),
                assets.augments.len()
            );
            let assets_clone = assets.clone();
            crate::runtime::spawn_background(async move {
                save_cached_assets(&assets_clone);
            });
            return assets;
        }

        if attempt < GAME_DATA_RETRIES {
            log::debug!(
                "[GameData] 游戏资源部分未就绪 (尝试 {}/{}): items={}, spells={}, runes={}, champions={}, {}ms 后重试...",
                attempt,
                GAME_DATA_RETRIES,
                assets.items.len(),
                assets.spells.len(),
                assets.runes.len(),
                assets.champions.len(),
                GAME_DATA_RETRY_DELAY.as_millis()
            );
            sleep(GAME_DATA_RETRY_DELAY).await;
        }
    }

    // 最终尝试用本地磁盘缓存补齐缺失部分
    if let Some(cached) = cached_opt {
        log::warn!(
            "[GameData] LCU 游戏资源在 {} 次尝试后仍未完整，使用本地磁盘缓存补充",
            GAME_DATA_RETRIES
        );
        assets.merge_missing(&cached);
    } else {
        log::warn!(
            "[GameData] 游戏资源在 {} 次尝试后仍不完整且无本地缓存",
            GAME_DATA_RETRIES
        );
    }

    assets
}

async fn fetch_id_map(
    http: &reqwest::Client,
    base: &str,
    auth: &str,
    path: &str,
) -> HashMap<i32, String> {
    let url = format!("{}{}", base, path);

    let resp = match http.get(&url).header("Authorization", auth).send().await {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            log::debug!("[GameData] 端点 {} 状态码异常: {}", path, r.status());
            return HashMap::new();
        }
        Err(e) => {
            log::debug!("[GameData] 请求端点 {} 失败: {}", path, e);
            return HashMap::new();
        }
    };

    let text = match resp.text().await {
        Ok(t) if !t.is_empty() => t,
        _ => return HashMap::new(),
    };

    // Fast path: 直接反序列化 Vec<IdEntry>
    if let Ok(entries) = serde_json::from_str::<Vec<IdEntry>>(&text) {
        let map: HashMap<i32, String> = entries
            .into_iter()
            .filter_map(|e| Some((e.id?, e.icon_path?)))
            .collect();
        if !map.is_empty() {
            return map;
        }
    }

    // Flexible path: 支持以字典对象或松散结构组织的载荷
    if let Ok(val) = serde_json::from_str::<Value>(&text) {
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
        return map;
    }

    HashMap::new()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_data_assets_is_complete_and_empty() {
        let mut assets = GameDataAssets::default();
        assert!(assets.is_empty());
        assert!(!assets.is_complete());

        assets.items.insert(1001, "boot.png".to_string());
        assert!(!assets.is_empty());
        assert!(!assets.is_complete());

        assets.spells.insert(4, "flash.png".to_string());
        assert!(!assets.is_complete());

        assets.champions.insert(1, "Annie".to_string());
        assert!(assets.is_complete());
    }

    #[test]
    fn test_game_data_assets_merge_missing() {
        let mut partial = GameDataAssets::default();
        partial.runes.insert(8000, "precision.png".to_string());

        let mut fallback = GameDataAssets::default();
        fallback.items.insert(1001, "boot.png".to_string());
        fallback.spells.insert(4, "flash.png".to_string());
        fallback.champions.insert(1, "Annie".to_string());

        partial.merge_missing(&fallback);
        assert!(partial.is_complete());
        assert_eq!(partial.items.len(), 1);
        assert_eq!(partial.runes.len(), 1);
    }
}
