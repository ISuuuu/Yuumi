use std::collections::HashMap;

use serde::Deserialize;
use tokio::time::{sleep, Duration};

use crate::{build_auth_header, LcuClient};

/// 游戏资源加载重试次数和间隔
const GAME_DATA_RETRIES: u32 = 3;
const GAME_DATA_RETRY_DELAY: Duration = Duration::from_secs(2);

/// LCU 连接后预加载的游戏资源路径映射（ID → iconPath / name）。
#[derive(Debug, Clone, Default)]
pub struct GameDataAssets {
    /// 物品 ID → iconPath
    pub items: HashMap<i32, String>,
    /// 召唤师技能 ID → iconPath
    pub spells: HashMap<i32, String>,
    /// 符文 (perk) ID → iconPath
    pub runes: HashMap<i32, String>,
    /// 英雄 ID → 英雄名称
    pub champions: HashMap<i32, String>,
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
/// 在 LCU 连接成功后、写入 AppState.game_data 之前调用。
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

    // 先尝试标准数组解析
    let (items, spells, runes, champions) = tokio::join!(
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
    );

    // 对任何为空的资源，用灵活解析器重试
    let items = if items.is_empty() {
        log::warn!("物品数据为空({}条)，使用灵活解析器重试...", items.len());
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
        log::warn!("技能数据为空({}条)，使用灵活解析器重试...", spells.len());
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
        log::warn!("符文数据为空({}条)，使用灵活解析器重试...", runes.len());
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

    log::info!(
        "游戏资源加载完成: 物品={}, 技能={}, 符文={}, 英雄={}",
        items.len(),
        spells.len(),
        runes.len(),
        champions.len(),
    );
    if champions.is_empty() {
        log::warn!("英雄名称映射为空！上传时 championName 将为 Unknown");
    }

    GameDataAssets {
        items,
        spells,
        runes,
        champions,
    }
}

/// GET JSON 数组 → 提取每个元素的 (id, iconPath) → HashMap
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
            Err(e) => {
                log::warn!("解析 {} 失败: {}", path, e);
                HashMap::new()
            }
        },
        Ok(resp) => {
            log::warn!("获取 {} 失败: HTTP {}", path, resp.status());
            HashMap::new()
        }
        Err(e) => {
            log::warn!("请求 {} 失败: {}", path, e);
            HashMap::new()
        }
    }
}

/// 从 LCU 获取英雄 ID → 名称映射。
/// 对齐 Python: champion-summary.json 返回数组 [{id, name, ...}, ...]
async fn fetch_champion_map(
    http: &reqwest::Client,
    base: &str,
    auth: &str,
) -> HashMap<i32, String> {
    // 对齐 Python: /lol-game-data/assets/v1/champion-summary.json
    let url = format!("{}/lol-game-data/assets/v1/champion-summary.json", base);
    match http.get(&url).header("Authorization", auth).send().await {
        Ok(resp) if resp.status().is_success() => {
            // champion-summary.json 返回数组: [{id: 1, name: "Annie", ...}, ...]
            if let Ok(arr) = resp.json::<Vec<serde_json::Value>>().await {
                let map: HashMap<i32, String> = arr
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
                if !map.is_empty() {
                    log::info!("英雄名称映射加载成功: {} 条", map.len());
                    return map;
                }
                log::warn!("champion-summary.json 解析后为空");
            }
        }
        Ok(resp) => {
            log::warn!("获取 champion-summary.json 失败: HTTP {}", resp.status());
        }
        Err(e) => {
            log::warn!("请求 champion-summary.json 失败: {}", e);
        }
    }

    HashMap::new()
}

/// 灵活解析：先尝试数组格式，失败则解析为 JSON Value 处理对象/嵌套格式。
/// 召唤师技能等资源的 JSON 结构可能与物品不同（如按技能名分组的对象格式）。
async fn fetch_id_map_flexible(
    http: &reqwest::Client,
    base: &str,
    auth: &str,
    path: &str,
) -> HashMap<i32, String> {
    let url = format!("{}{}", base, path);

    let resp = match http.get(&url).header("Authorization", auth).send().await {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            log::warn!("[flexible] 获取 {} 失败: HTTP {}", path, r.status());
            return HashMap::new();
        }
        Err(e) => {
            log::warn!("[flexible] 请求 {} 失败: {}", path, e);
            return HashMap::new();
        }
    };

    let text = match resp.text().await {
        Ok(t) => t,
        Err(e) => {
            log::warn!("[flexible] 读取 {} 响应体失败: {}", path, e);
            return HashMap::new();
        }
    };

    let value: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("[flexible] 解析 {} JSON 失败: {}", path, e);
            return HashMap::new();
        }
    };

    let mut map = HashMap::new();

    // 情况 1: 顶层是数组
    if let Some(arr) = value.as_array() {
        log::info!("[flexible] {} 是数组格式, 元素数: {}", path, arr.len());
        for entry in arr {
            extract_id_icon(entry, &mut map);
        }
        return map;
    }

    // 情况 2: 顶层是对象 → 遍历所有值，查找数组子元素
    if let Some(obj) = value.as_object() {
        log::info!("[flexible] {} 是对象格式, 顶层 key 数: {}", path, obj.len());
        for (_key, val) in obj {
            if let Some(arr) = val.as_array() {
                // 值是数组: 提取每个元素
                for entry in arr {
                    extract_id_icon(entry, &mut map);
                }
            } else if val.is_object() {
                // 值是对象: 直接尝试提取 id + iconPath
                extract_id_icon(val, &mut map);
            }
            // 跳过其他类型
        }
        // 如果对象有 id/iconPath 字段本身（单个条目）
        if map.is_empty() {
            extract_id_icon(&value, &mut map);
        }
        return map;
    }

    log::warn!("[flexible] {} 格式无法识别: {:?}", path, value);
    map
}

/// 从单个 JSON 对象中提取 (id, iconPath) 插入 map
fn extract_id_icon(entry: &serde_json::Value, map: &mut HashMap<i32, String>) {
    if let Some(obj) = entry.as_object() {
        let id = obj.get("id").and_then(|v| v.as_i64()).map(|v| v as i32);
        let icon_path = obj
            .get("iconPath")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        if let (Some(id), Some(path)) = (id, icon_path) {
            map.insert(id, path);
        }
    }
}
