//! 主页背景皮肤服务（工单 D09）
//!
//! 对照 Vue `ProfileBackgroundCard.vue` 与旧 Tauri `get_champion_skins`：
//! - 选英雄后 `GET /lol-game-data/assets/v1/champions/{id}.json` 拉真实皮肤列表
//! - 当前主页背景经 `GET /lol-chat/v1/me` 的 `lol.backgroundSkinId` 回读
//! - 确认后 `POST /lol-summoner/v1/current-summoner/summoner-profile` 写入并回读校验
//!
//! 所有 LCU 调用统一经 `LiveLcuService::get_client()` 取得的 `LcuClient`，
//! 视图层不直连 LCU 端口；皮肤立绘路径交给 `LcuImage`（`yuumi-asset://`）渲染，不走 IPC Base64。

use serde::Deserialize;
use serde_json::Value;
use yuumi_core::lcu::client::LcuClient;

/// 单个皮肤条目
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinInfo {
    pub id: i32,
    pub name: String,
    /// LCU 资源路径，例如 `/lol-game-data/assets/v1/champion-loadscreens/157/157003.jpg`
    pub load_screen_path: String,
}

#[derive(Deserialize)]
struct LcuSkin {
    id: i32,
    name: String,
    #[serde(rename = "loadScreenPath")]
    load_screen_path: Option<String>,
}

#[derive(Deserialize)]
struct LcuChampionDetails {
    skins: Vec<LcuSkin>,
}

/// 拉取指定英雄的真实皮肤列表（对齐旧版 `get_champion_skins`）
pub async fn load_skins(client: &LcuClient, champion_id: i32) -> Result<Vec<SkinInfo>, String> {
    let path = format!("/lol-game-data/assets/v1/champions/{}.json", champion_id);
    let value = client.request("GET", &path, None, false).await?;
    let details: LcuChampionDetails =
        serde_json::from_value(value).map_err(|e| format!("皮肤列表解析失败: {}", e))?;
    Ok(details
        .skins
        .into_iter()
        .map(|s| {
            let name = if s.name.eq_ignore_ascii_case("default") {
                "默认皮肤".to_string()
            } else {
                s.name
            };
            SkinInfo {
                load_screen_path: s.load_screen_path.unwrap_or_else(|| {
                    format!(
                        "/lol-game-data/assets/v1/champion-loadscreens/{}/{}.jpg",
                        champion_id, s.id
                    )
                }),
                id: s.id,
                name,
            }
        })
        .collect())
}

/// 回读当前主页背景皮肤 ID（`/lol-chat/v1/me` 的 `lol.backgroundSkinId`）
pub async fn read_current_background(client: &LcuClient) -> Result<Option<i32>, String> {
    let me = client
        .request("GET", "/lol-chat/v1/me", None, false)
        .await?;
    let id = match me.pointer("/lol/backgroundSkinId") {
        Some(Value::Number(n)) => n.as_i64().map(|v| v as i32),
        Some(Value::String(s)) => s.parse::<i32>().ok(),
        _ => None,
    };
    Ok(id)
}

/// 确认写入主页背景并立即回读校验；返回回读到的皮肤 ID（可能为空）。
/// 写入或回读任一失败均返回真实错误文案，供视图层保留预览态并重试。
pub async fn apply_skin(client: &LcuClient, skin_id: i32) -> Result<Option<i32>, String> {
    let body = serde_json::json!({ "key": "backgroundSkinId", "value": skin_id });
    client
        .request(
            "POST",
            "/lol-summoner/v1/current-summoner/summoner-profile",
            Some(body),
            false,
        )
        .await?;
    read_current_background(client).await
}
