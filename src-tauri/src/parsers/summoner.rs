use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{build_auth_header, AppState};

// ─── LCU 原始响应结构体 ───

/// `/lol-summoner/v1/current-summoner` 的原始返回
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LcuSummoner {
    pub account_id: Option<u64>,
    pub display_name: Option<String>,
    pub game_name: Option<String>,
    pub tag_line: Option<String>,
    pub percent_complete_for_next_level: Option<u32>,
    pub profile_icon_id: Option<i32>,
    pub puuid: Option<String>,
    pub summoner_id: Option<u64>,
    pub summoner_level: Option<u32>,
    pub xp_since_last_level: Option<u64>,
    pub xp_until_next_level: Option<u64>,
}

// ─── 前端展示用的清洗结构体 ───

/// 清洗后的召唤师数据，直接返回给前端
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummonerDisplay {
    pub account_id: u64,
    pub display_name: String,
    pub game_name: String,
    pub tag_line: String,
    pub percent_complete_for_next_level: u32,
    pub profile_icon_id: i32,
    pub puuid: String,
    pub summoner_id: u64,
    pub summoner_level: u32,
    pub xp_since_last_level: u64,
    pub xp_until_next_level: u64,
    // 计算字段：前端可直接拼接 URL
    pub profile_icon_url: String,
}

impl LcuSummoner {
    /// 转换为前端展示结构
    pub fn to_display(&self) -> Option<SummonerDisplay> {
        Some(SummonerDisplay {
            account_id: self.account_id?,
            display_name: self.display_name.clone()?,
            game_name: self.game_name.clone().unwrap_or_default(),
            tag_line: self.tag_line.clone().unwrap_or_default(),
            percent_complete_for_next_level: self.percent_complete_for_next_level.unwrap_or(0),
            profile_icon_id: self.profile_icon_id.unwrap_or(0),
            puuid: self.puuid.clone()?,
            summoner_id: self.summoner_id?,
            summoner_level: self.summoner_level.unwrap_or(0),
            xp_since_last_level: self.xp_since_last_level.unwrap_or(0),
            xp_until_next_level: self.xp_until_next_level.unwrap_or(0),
            profile_icon_url: format!(
                "/lol-game-data/assets/v1/profile-icons/{}.jpg",
                self.profile_icon_id.unwrap_or(29)
            ),
        })
    }
}

// ─── Tauri 命令 ───

/// 获取当前召唤师信息（清洗后）
#[tauri::command]
pub async fn get_current_summoner(
    app_state: State<'_, AppState>,
) -> Result<SummonerDisplay, String> {
    let lock = app_state.lcu().await?;
    let lcu = lock.as_ref().unwrap();

    let url = format!(
        "https://127.0.0.1:{}/lol-summoner/v1/current-summoner",
        lcu.port
    );
    let auth = build_auth_header(&lcu.token);

    let resp = lcu
        .http_client
        .get(&url)
        .header("Authorization", auth)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("获取召唤师信息失败: HTTP {}", resp.status()));
    }

    let summoner: LcuSummoner = resp.json().await.map_err(|e| e.to_string())?;

    summoner.to_display().ok_or("召唤师数据不完整".to_string())
}
