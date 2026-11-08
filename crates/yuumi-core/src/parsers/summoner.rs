//! 召唤师信息清洗与获取

use crate::lcu::client::LcuClient;
use serde::{Deserialize, Serialize};

/// `/lol-summoner/v1/current-summoner` 响应结构体
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummonerInfo {
    pub account_id: Option<u64>,
    pub display_name: Option<String>,
    pub game_name: Option<String>,
    pub tag_line: Option<String>,
    pub profile_icon_id: Option<i32>,
    pub puuid: String,
    pub summoner_id: Option<u64>,
    pub summoner_level: Option<u32>,
    pub xp_since_last_level: Option<u64>,
    pub xp_until_next_level: Option<u64>,
}

impl SummonerInfo {
    /// 完整显示名（例如: "UZI#001" 或 "UZI"）
    pub fn full_name(&self) -> String {
        if let (Some(name), Some(tag)) = (&self.game_name, &self.tag_line) {
            if !name.is_empty() && !tag.is_empty() {
                return format!("{}#{}", name, tag);
            }
        }
        self.display_name
            .clone()
            .unwrap_or_else(|| self.game_name.clone().unwrap_or_else(|| "召唤师".to_string()))
    }

    /// 头像资源路径
    pub fn icon_path(&self) -> String {
        format!(
            "/lol-game-data/assets/v1/profile-icons/{}.jpg",
            self.profile_icon_id.unwrap_or(29)
        )
    }
}

/// 获取当前登录的召唤师信息
pub async fn fetch_current_summoner(client: &LcuClient) -> Result<SummonerInfo, String> {
    let val = client.get("/lol-summoner/v1/current-summoner").await?;
    serde_json::from_value(val).map_err(|e| format!("解析召唤师失败: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_summoner_info_display() {
        let json = r#"{
            "accountId": 10001,
            "displayName": "Hide on bush",
            "gameName": "Faker",
            "tagLine": "KR1",
            "profileIconId": 501,
            "puuid": "mock-puuid-1234",
            "summonerId": 20002,
            "summonerLevel": 500
        }"#;

        let s: SummonerInfo = serde_json::from_str(json).unwrap();
        assert_eq!(s.full_name(), "Faker#KR1");
        assert_eq!(s.icon_path(), "/lol-game-data/assets/v1/profile-icons/501.jpg");
    }
}
