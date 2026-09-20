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
    pub privacy: Option<String>,
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
    pub privacy: Option<String>,
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
            privacy: self.privacy.clone(),
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
    // 锁内只提取连接参数（http_client 克隆是 Arc 浅拷贝），立即释放读锁，
    // 避免跨 HTTP await 持有锁阻塞 monitor 重连写锁
    let (port, token, http_client) = {
        let lock = app_state.lcu().await?;
        let lcu = lock.as_ref().unwrap();
        (lcu.port, lcu.token.clone(), lcu.http_client.clone())
    };

    let url = format!(
        "https://127.0.0.1:{}/lol-summoner/v1/current-summoner",
        port
    );
    let auth = build_auth_header(&token);

    let resp = http_client
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

// ─── 英雄熟练度与赛段里程碑 ───

/// 英雄熟练度与里程碑清洗结构体
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampionMasteryInfo {
    pub champion_id: i64,
    pub champion_name: String,
    pub champion_title: String,
    pub champion_icon_url: String,
    pub champion_level: i32,
    pub champion_points: i64,
    pub champion_points_since_last_level: i64,
    pub champion_points_until_next_level: i64,
    pub mark_required_for_next_level: i32,
    pub tokens_earned: i32,
    pub champion_season_milestone: i32,
    pub milestone_grades: Vec<String>,
    pub chest_granted: bool,
    pub eligible_for_chest: bool,
    pub next_level_points: i64,
    pub last_play_time: i64,
}

/// 获取当前玩家全英雄熟练度列表与里程碑进度
#[tauri::command]
pub async fn get_champion_mastery_list(
    app_state: State<'_, AppState>,
) -> Result<Vec<ChampionMasteryInfo>, String> {
    use crate::lcu::client::lcu_request;

    // 1. 获取全英雄熟练度基础数据
    let mastery_val = lcu_request(
        app_state.inner(),
        "GET",
        "/lol-champion-mastery/v1/local-player/champion-mastery",
        None,
    )
    .await?;

    // 2. 尝试获取赛段里程碑进度（容错）
    let milestone_val = lcu_request(
        app_state.inner(),
        "GET",
        "/lol-champion-mastery/v1/local-player/champion-milestone-progress",
        None,
    )
    .await
    .ok();

    // 映射：championId -> (eligibleForChest, milestoneGrades)
    let mut milestone_map: std::collections::HashMap<i64, (Option<bool>, Vec<String>)> =
        std::collections::HashMap::new();

    if let Some(val) = milestone_val {
        if let Some(arr) = val.as_array() {
            for item in arr {
                if let Some(cid) = item.get("championId").and_then(|v| v.as_i64()) {
                    let eligible = item.get("eligibleForChest").and_then(|v| v.as_bool());
                    let grades = item
                        .get("milestoneGrades")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|g| g.as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default();
                    milestone_map.insert(cid, (eligible, grades));
                }
            }
        }
    }

    let items = mastery_val
        .as_array()
        .ok_or_else(|| "熟练度数据格式错误，期望数组".to_string())?;

    let game_data = app_state.game_data.read().await;

    let mut result = Vec::with_capacity(items.len());
    for item in items {
        let champion_id = item
            .get("championId")
            .and_then(|v| v.as_i64())
            .unwrap_or_default();
        if champion_id <= 0 {
            continue;
        }

        let champion_level = item
            .get("championLevel")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let champion_points = item
            .get("championPoints")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let champion_points_since_last_level = item
            .get("championPointsSinceLastLevel")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let champion_points_until_next_level = item
            .get("championPointsUntilNextLevel")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let mark_required_for_next_level = item
            .get("markRequiredForNextLevel")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let tokens_earned = item
            .get("tokensEarned")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let champion_season_milestone = item
            .get("championSeasonMilestone")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let chest_granted = item
            .get("chestGranted")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let last_play_time = item
            .get("lastPlayTime")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        let mut milestone_grades: Vec<String> = item
            .get("milestoneGrades")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|g| g.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let mut eligible_for_chest = !chest_granted;
        if let Some((prog_eligible, prog_grades)) = milestone_map.get(&champion_id) {
            if let Some(el) = prog_eligible {
                eligible_for_chest = *el;
            }
            if milestone_grades.is_empty() && !prog_grades.is_empty() {
                milestone_grades = prog_grades.clone();
            }
        }

        let next_level_points = if champion_points_until_next_level > 0 {
            champion_points + champion_points_until_next_level
        } else {
            champion_points
        };

        let champion_name = game_data
            .champions
            .get(&(champion_id as i32))
            .cloned()
            .unwrap_or_else(|| format!("英雄 {}", champion_id));

        let champion_icon_url = format!(
            "/lol-game-data/assets/v1/champion-icons/{}.png",
            champion_id
        );

        result.push(ChampionMasteryInfo {
            champion_id,
            champion_name,
            champion_title: String::new(),
            champion_icon_url,
            champion_level,
            champion_points,
            champion_points_since_last_level,
            champion_points_until_next_level,
            mark_required_for_next_level,
            tokens_earned,
            champion_season_milestone,
            milestone_grades,
            chest_granted,
            eligible_for_chest,
            next_level_points,
            last_play_time,
        });
    }

    Ok(result)
}
