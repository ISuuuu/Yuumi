//! 称号与挑战勋章服务（工单 D11）
//!
//! 对照 Vue `BadgesPreferencesCard.vue`、`ClearBadgesCard.vue`、`ClearBorderCard.vue`
//! 与旧 Tauri `get_player_challenges` / `set_player_preferences`：
//! - `GET /lol-challenges/v1/challenges/local-player` 已拥有挑战
//! - `GET /lol-challenges/v1/summary-player-data/local-player` 称号列表与当前称号
//! - `GET /lol-challenges/v1/preferences/local-player` 当前佩戴偏好（回读）
//! - `POST /lol-challenges/v1/update-player-preferences/` 写入（带 `bannerAccent` 保护）
//! - `PUT /lol-regalia/v2/current-regalia {preferredBorderType:"NONE"}` 只清头像边框
//!
//! 所有 LCU 调用统一经 `LiveLcuService::get_client()` 取得的 `LcuClient`。

use serde_json::Value;
use yuumi_core::lcu::client::LcuClient;

/// 已拥有的挑战勋章
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChallengeItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub level: String,
}

/// 已解锁的称号
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TitleItem {
    pub id: String,
    pub name: String,
}

/// 打开装配弹层所需的真实数据快照
#[derive(Debug, Clone, Default)]
pub struct ChallengesData {
    pub badges: Vec<ChallengeItem>,
    pub titles: Vec<TitleItem>,
    pub current_title: String,
    pub current_badges: Vec<String>,
}

fn as_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn challenge_level(item: &Value) -> String {
    item.get("currentLevel")
        .or_else(|| item.get("level"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn valid_challenge_id(id: &str) -> bool {
    !id.is_empty() && id != "0" && id != "-1"
}

/// 读取当前头像框（`/lol-chat/v1/me` 的 `lol.bannerIdSelected`），用于写入时保护边框
async fn read_banner_accent(client: &LcuClient) -> Option<String> {
    let me = client
        .request("GET", "/lol-chat/v1/me", None, false)
        .await
        .ok()?;
    let banner = me.pointer("/lol/bannerIdSelected")?;
    let s = match banner {
        Value::String(v) => v.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    };
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// 回读当前佩戴偏好（从 summary-player-data/local-player 获取称号与已佩戴勋章）
pub async fn read_preferences(client: &LcuClient) -> Result<(String, Vec<String>), String> {
    let summary = client
        .request(
            "GET",
            "/lol-challenges/v1/summary-player-data/local-player",
            None,
            false,
        )
        .await?;
    let title = summary
        .get("title")
        .and_then(|t| {
            if t.is_object() {
                t.get("itemId").or_else(|| t.get("id")).and_then(as_string)
            } else {
                as_string(t)
            }
        })
        .map(|s| {
            if s == "-1" || s == "0" {
                String::new()
            } else {
                s
            }
        })
        .unwrap_or_default();
    let ids: Vec<String> = summary
        .get("topChallenges")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.get("id").and_then(as_string))
                .filter(|id| valid_challenge_id(id))
                .collect()
        })
        .unwrap_or_default();
    Ok((title, ids))
}

/// 拉取真实拥有的挑战、称号与当前佩戴偏好
pub async fn load(client: &LcuClient) -> Result<ChallengesData, String> {
    // 1. 已拥有挑战：兼容数组与「以 id 为键」的对象两种载荷（对齐 Vue Object.values 兜底）
    let raw = client
        .request(
            "GET",
            "/lol-challenges/v1/challenges/local-player",
            None,
            false,
        )
        .await?;
    let entries: Vec<Value> = match raw {
        Value::Array(arr) => arr,
        Value::Object(map) => map.into_values().collect(),
        _ => Vec::new(),
    };
    let mut badges = Vec::new();
    for item in entries {
        let id = item.get("id").and_then(as_string).unwrap_or_default();
        let level = challenge_level(&item);
        let name = item
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        // 只保留已获得的有效勋章（对齐 Vue availableBadges：排除 NONE / 空等级）
        if !valid_challenge_id(&id)
            || name.is_empty()
            || level.is_empty()
            || level.eq_ignore_ascii_case("NONE")
        {
            continue;
        }
        let description = item
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        badges.push(ChallengeItem {
            id,
            name,
            description,
            level,
        });
    }

    // 2. 称号列表、当前称号与佩戴勋章（由 summary-player-data 统一切片返回）
    let summary = client
        .request(
            "GET",
            "/lol-challenges/v1/summary-player-data/local-player",
            None,
            false,
        )
        .await?;
    let mut titles = Vec::new();
    if let Some(arr) = summary.get("titles").and_then(|v| v.as_array()) {
        for t in arr {
            let id = t
                .get("itemId")
                .or_else(|| t.get("id"))
                .and_then(as_string)
                .unwrap_or_default();
            let name = t
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if !id.is_empty() && !name.is_empty() {
                titles.push(TitleItem { id, name });
            }
        }
    }
    let current_title = summary
        .get("title")
        .and_then(|t| {
            if t.is_object() {
                t.get("itemId").or_else(|| t.get("id")).and_then(as_string)
            } else {
                as_string(t)
            }
        })
        .map(|s| {
            if s == "-1" || s == "0" {
                String::new()
            } else {
                s
            }
        })
        .unwrap_or_default();

    // 从 summary-player-data 提取当前佩戴勋章 (topChallenges)
    let mut current_badges = Vec::new();
    if let Some(arr) = summary.get("topChallenges").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(id) = item.get("id").and_then(as_string) {
                if valid_challenge_id(&id) {
                    current_badges.push(id);
                }
            }
        }
    }

    Ok(ChallengesData {
        badges,
        titles,
        current_title,
        current_badges,
    })
}

/// 写入称号与勋章：带当前 `bannerAccent` 保护，避免装配时重置头像框
pub async fn save_preferences(
    client: &LcuClient,
    title: &str,
    challenge_ids: &[String],
) -> Result<(), String> {
    let clean_title = if title == "-1" || title == "0" {
        ""
    } else {
        title
    };
    let clean_badges: Vec<String> = challenge_ids
        .iter()
        .filter(|id| valid_challenge_id(id))
        .cloned()
        .collect();
    let mut body = serde_json::json!({
        "title": clean_title,
        "challengeIds": clean_badges,
    });
    if let Some(banner) = read_banner_accent(client).await {
        body["bannerAccent"] = Value::String(banner);
    }
    client
        .request(
            "POST",
            "/lol-challenges/v1/update-player-preferences/",
            Some(body),
            false,
        )
        .await?;
    Ok(())
}

/// 只卸下勋章：请求体带 `title` 与 `bannerAccent` 保护，保留称号与头像框
/// （对齐旧版 `ClearBadgesCard.vue:24-36`）
pub async fn clear_badges(client: &LcuClient) -> Result<(), String> {
    let mut body = serde_json::json!({ "challengeIds": [] });
    if let Ok((title, _)) = read_preferences(client).await {
        let clean = if title == "-1" || title == "0" {
            ""
        } else {
            &title
        };
        if !clean.is_empty() {
            body["title"] = Value::String(clean.to_string());
        }
    }
    // 若 preferences 回读为空，尝试从 summary 回读当前称号
    if !body.get("title").map(|v| v.is_string()).unwrap_or(false) {
        if let Ok(summary) = client
            .request(
                "GET",
                "/lol-challenges/v1/summary-player-data/local-player",
                None,
                false,
            )
            .await
        {
            if let Some(cur) = summary.get("title").and_then(|t| {
                if t.is_object() {
                    t.get("itemId").or_else(|| t.get("id")).and_then(as_string)
                } else {
                    as_string(t)
                }
            }) {
                let clean = if cur == "-1" || cur == "0" { "" } else { &cur };
                if !clean.is_empty() {
                    body["title"] = Value::String(clean.to_string());
                }
            }
        }
    }
    if let Some(banner) = read_banner_accent(client).await {
        body["bannerAccent"] = Value::String(banner);
    }
    client
        .request(
            "POST",
            "/lol-challenges/v1/update-player-preferences/",
            Some(body),
            false,
        )
        .await?;
    Ok(())
}

/// 只清除头像边框：对齐旧版 `ClearBorderCard.vue:24-30`。
/// `/lol-regalia/` 需在 `ALLOWED_API_PREFIXES` 白名单内，否则 `LcuClient::request` 会拒绝。
pub async fn clear_border(client: &LcuClient) -> Result<(), String> {
    let body = serde_json::json!({ "preferredBorderType": "NONE" });
    client
        .request("PUT", "/lol-regalia/v2/current-regalia", Some(body), false)
        .await?;
    Ok(())
}
