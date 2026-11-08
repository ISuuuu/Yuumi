//! 英雄熟练度与赛段里程碑解析

use crate::lcu::client::LcuClient;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampionMasteryItem {
    pub champion_id: i64,
    pub champion_name: String,
    pub champion_title: String,
    pub champion_icon_url: String,
    pub champion_level: i32,
    pub champion_points: i64,
    pub champion_points_since_last_level: i64,
    pub champion_points_until_next_level: i64,
    pub champion_season_milestone: i32,
    pub milestone_grades: Vec<String>,
    pub chest_granted: bool,
    pub eligible_for_chest: bool,
    pub last_play_time: i64,
}

/// 解析熟练度与里程碑数据
pub async fn fetch_champion_mastery_list(client: &LcuClient) -> Result<Vec<ChampionMasteryItem>, String> {
    let mastery_val = client
        .get("/lol-champion-mastery/v1/local-player/champion-mastery")
        .await?;

    let milestone_val = client
        .get("/lol-champion-mastery/v1/local-player/champion-milestone-progress")
        .await
        .ok();

    let mut milestone_map = std::collections::HashMap::new();
    if let Some(val) = milestone_val {
        if let Some(arr) = val.as_array() {
            for item in arr {
                if let Some(cid) = item.get("championId").and_then(|v| v.as_i64()) {
                    let eligible = item.get("eligibleForChest").and_then(|v| v.as_bool()).unwrap_or(false);
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

        let (eligible_for_chest, milestone_grades) = milestone_map
            .remove(&champion_id)
            .unwrap_or((false, Vec::new()));

        result.push(ChampionMasteryItem {
            champion_id,
            champion_name: format!("英雄 {}", champion_id),
            champion_title: String::new(),
            champion_icon_url: format!("/lol-game-data/assets/v1/champion-icons/{}.png", champion_id),
            champion_level,
            champion_points,
            champion_points_since_last_level,
            champion_points_until_next_level,
            champion_season_milestone,
            milestone_grades,
            chest_granted,
            eligible_for_chest,
            last_play_time,
        });
    }

    Ok(result)
}
