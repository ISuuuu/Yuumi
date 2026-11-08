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
        self.display_name.clone().unwrap_or_else(|| {
            self.game_name
                .clone()
                .unwrap_or_else(|| "召唤师".to_string())
        })
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

/// 根据召唤师名称（纯名或 游戏名#标号）查询召唤师信息
pub async fn fetch_summoner_by_name(
    client: &LcuClient,
    name: &str,
) -> Result<SummonerInfo, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("召唤师名称不能为空".to_string());
    }

    let val = if let Some((game_name, tag_line)) = trimmed.split_once('#') {
        let path = format!(
            "/lol-summoner/v1/alias/lookup?gameName={}&tagLine={}",
            urlencoding_encode(game_name),
            urlencoding_encode(tag_line)
        );
        client.get(&path).await?
    } else {
        let path = format!(
            "/lol-summoner/v1/summoners?name={}",
            urlencoding_encode(trimmed)
        );
        client.get(&path).await?
    };

    serde_json::from_value(val).map_err(|e| format!("解析召唤师失败: {}", e))
}

/// 辅助 URL 编码函数
fn urlencoding_encode(s: &str) -> String {
    let mut encoded = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
            encoded.push(b as char);
        } else {
            encoded.push_str(&format!("%{:02X}", b));
        }
    }
    encoded
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
        assert_eq!(
            s.icon_path(),
            "/lol-game-data/assets/v1/profile-icons/501.jpg"
        );
    }

    #[test]
    fn test_urlencoding_encode() {
        assert_eq!(urlencoding_encode("玩家#123"), "%E7%8E%A9%E5%AE%B6%23123");
        assert_eq!(urlencoding_encode("Faker"), "Faker");
        assert_eq!(urlencoding_encode("a b"), "a%20b");
    }
}
