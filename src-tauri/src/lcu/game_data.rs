use std::collections::HashMap;

use serde::Deserialize;

use crate::{build_auth_header, LcuClient};

/// LCU 连接后预加载的游戏资源路径映射（ID → iconPath）。
/// 仅用于图标 URL 查找，不存图片二进制。
#[derive(Debug, Clone, Default)]
pub struct GameDataAssets {
    /// 物品 ID → iconPath
    pub items: HashMap<i32, String>,
    /// 召唤师技能 ID → iconPath
    pub spells: HashMap<i32, String>,
    /// 符文 (perk) ID → iconPath
    pub runes: HashMap<i32, String>,
}

#[derive(Deserialize)]
struct IdEntry {
    id: Option<i32>,
    #[serde(rename = "iconPath")]
    icon_path: Option<String>,
}

/// 从 LCU 预加载所有游戏资源路径。
/// 在 LCU 连接成功后、写入 AppState.game_data 之前调用。
pub async fn fetch_game_data_assets(lcu: &LcuClient) -> GameDataAssets {
    let auth = build_auth_header(&lcu.token);
    let base = format!("https://127.0.0.1:{}", lcu.port);

    let (items, spells, runes) = tokio::join!(
        fetch_id_map(&lcu.http_client, &base, &auth, "/lol-game-data/assets/v1/items.json"),
        fetch_id_map(&lcu.http_client, &base, &auth, "/lol-game-data/assets/v1/summoner-spells.json"),
        fetch_id_map(&lcu.http_client, &base, &auth, "/lol-game-data/assets/v1/perks.json"),
    );

    log::info!(
        "游戏资源加载完成: 物品={}, 技能={}, 符文={}",
        items.len(),
        spells.len(),
        runes.len()
    );

    GameDataAssets { items, spells, runes }
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
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<Vec<IdEntry>>().await {
                Ok(entries) => entries
                    .into_iter()
                    .filter_map(|e| Some((e.id?, e.icon_path?)))
                    .collect(),
                Err(e) => {
                    log::warn!("解析 {} 失败: {}", path, e);
                    HashMap::new()
                }
            }
        }
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
