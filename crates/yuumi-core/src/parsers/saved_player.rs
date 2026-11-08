//! 路人集 (SavedPlayers) 数据结构与解析模型
//!
//! 支持记录同局玩家、打标、相遇对局历史及查询统计。

use serde::{Deserialize, Serialize};

/// 常用推荐标签集合常量
pub const RECOMMENDED_TAGS: &[&str] = &["全部", "置顶", "大腿", "避坑", "绝活", "演员/挂机"];

/// 单个相遇对局历史摘要
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncounteredGameRecord {
    pub game_id: u64,
    pub queue_type: String,
    pub champion_id: i32,
    pub timestamp: i64,
    pub result: Option<String>,
}

/// 已保存的路人记录
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedPlayer {
    /// 唯一标识符 PUUID
    pub puuid: String,
    /// 召唤师名称
    pub summoner_name: String,
    /// 标签号 (如 "CN1", "KR1", "001")
    pub tag_line: String,
    /// 自定义或系统标签 (如 "大腿", "避坑", "绝活", "演员/挂机")
    pub tag: Option<String>,
    /// 玩家当前或最近使用的英雄 ID
    pub champion_id: i32,
    /// 头像图标 ID
    pub profile_icon_id: i32,
    /// 累计同局相遇次数
    pub encountered_count: u32,
    /// 上次同局时间戳 (毫秒)
    pub last_met_timestamp: i64,
    /// 是否置顶高亮
    pub is_pinned: bool,
    /// 用户备忘录 / 笔记
    pub notes: String,
    /// 相遇对局历史简讯
    #[serde(default)]
    pub encounter_history: Vec<EncounteredGameRecord>,
}

impl SavedPlayer {
    /// 创建新路人记录
    pub fn new(
        puuid: impl Into<String>,
        summoner_name: impl Into<String>,
        tag_line: impl Into<String>,
    ) -> Self {
        Self {
            puuid: puuid.into(),
            summoner_name: summoner_name.into(),
            tag_line: tag_line.into(),
            tag: None,
            champion_id: 0,
            profile_icon_id: 29,
            encountered_count: 1,
            last_met_timestamp: chrono::Utc::now().timestamp_millis(),
            is_pinned: false,
            notes: String::new(),
            encounter_history: Vec::new(),
        }
    }

    /// 完整全名 (如 "Faker#KR1")
    pub fn full_name(&self) -> String {
        if self.tag_line.is_empty() {
            self.summoner_name.clone()
        } else {
            format!("{}#{}", self.summoner_name, self.tag_line)
        }
    }

    /// 头像资源路径
    pub fn profile_icon_url(&self) -> String {
        let icon_id = if self.profile_icon_id > 0 {
            self.profile_icon_id
        } else {
            29
        };
        format!("/lol-game-data/assets/v1/profile-icons/{}.jpg", icon_id)
    }

    /// 英雄头像资源路径
    pub fn champion_icon_url(&self) -> Option<String> {
        if self.champion_id > 0 {
            Some(format!(
                "/lol-game-data/assets/v1/champion-icons/{}.png",
                self.champion_id
            ))
        } else {
            None
        }
    }

    /// 相对相遇时间格式化 (如 "3小时前", "2天前")
    pub fn formatted_last_met(&self) -> String {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let diff_ms = now_ms - self.last_met_timestamp;
        if diff_ms < 0 {
            return "刚刚".to_string();
        }
        let diff_secs = diff_ms / 1000;
        let diff_mins = diff_secs / 60;
        let diff_hours = diff_mins / 60;
        let diff_days = diff_hours / 24;

        if diff_mins < 1 {
            "刚刚".to_string()
        } else if diff_mins < 60 {
            format!("{}分钟前", diff_mins)
        } else if diff_hours < 24 {
            format!("{}小时前", diff_hours)
        } else if diff_days < 30 {
            format!("{}天前", diff_days)
        } else {
            let months = diff_days / 30;
            format!("{}个月前", months)
        }
    }
}

/// 默认本地与测试 Mock 数据构造器
pub fn create_mock_saved_players() -> Vec<SavedPlayer> {
    let now = chrono::Utc::now().timestamp_millis();
    let hour = 3600 * 1000;
    let day = 24 * hour;

    vec![
        SavedPlayer {
            puuid: "mock-puuid-faker".to_string(),
            summoner_name: "Hide on bush".to_string(),
            tag_line: "KR1".to_string(),
            tag: Some("绝活".to_string()),
            champion_id: 157, // 亚索 Yasuo
            profile_icon_id: 501,
            encountered_count: 7,
            last_met_timestamp: now - (2 * hour),
            is_pinned: true,
            notes: "中路极强对线压制力，支援速度极快，听其打点节奏走。".to_string(),
            encounter_history: vec![
                EncounteredGameRecord {
                    game_id: 70001,
                    queue_type: "420".to_string(),
                    champion_id: 157,
                    timestamp: now - (2 * hour),
                    result: Some("Win".to_string()),
                },
                EncounteredGameRecord {
                    game_id: 70002,
                    queue_type: "420".to_string(),
                    champion_id: 103, // 阿狸
                    timestamp: now - day,
                    result: Some("Win".to_string()),
                },
                EncounteredGameRecord {
                    game_id: 70003,
                    queue_type: "440".to_string(),
                    champion_id: 134, // 辛德拉
                    timestamp: now - (3 * day),
                    result: Some("Win".to_string()),
                },
            ],
        },
        SavedPlayer {
            puuid: "mock-puuid-jungle".to_string(),
            summoner_name: "峡谷巡航机".to_string(),
            tag_line: "CN1".to_string(),
            tag: Some("大腿".to_string()),
            champion_id: 64, // 盲僧 Lee Sin
            profile_icon_id: 4208,
            encountered_count: 5,
            last_met_timestamp: now - (5 * hour),
            is_pinned: true,
            notes: "野区规划清晰，控龙率极高，反蹲极为及时。".to_string(),
            encounter_history: vec![
                EncounteredGameRecord {
                    game_id: 70010,
                    queue_type: "420".to_string(),
                    champion_id: 64,
                    timestamp: now - (5 * hour),
                    result: Some("Win".to_string()),
                },
                EncounteredGameRecord {
                    game_id: 70011,
                    queue_type: "420".to_string(),
                    champion_id: 121, // 卡兹克
                    timestamp: now - (2 * day),
                    result: Some("Win".to_string()),
                },
            ],
        },
        SavedPlayer {
            puuid: "mock-puuid-top-troll".to_string(),
            summoner_name: "绝不下路支援".to_string(),
            tag_line: "CN2".to_string(),
            tag: Some("避坑".to_string()),
            champion_id: 23, // 蛮王 Tryndamere
            profile_icon_id: 742,
            encountered_count: 4,
            last_met_timestamp: now - day,
            is_pinned: false,
            notes: "劣势易红温点投降，拒绝配合团战打龙，建议静音避让。".to_string(),
            encounter_history: vec![
                EncounteredGameRecord {
                    game_id: 70020,
                    queue_type: "420".to_string(),
                    champion_id: 23,
                    timestamp: now - day,
                    result: Some("Loss".to_string()),
                },
                EncounteredGameRecord {
                    game_id: 70021,
                    queue_type: "430".to_string(),
                    champion_id: 23,
                    timestamp: now - (4 * day),
                    result: Some("Loss".to_string()),
                },
            ],
        },
        SavedPlayer {
            puuid: "mock-puuid-afk".to_string(),
            summoner_name: "网络波动中".to_string(),
            tag_line: "CN1".to_string(),
            tag: Some("演员/挂机".to_string()),
            champion_id: 35, // 萨科 Shaco
            profile_icon_id: 588,
            encountered_count: 3,
            last_met_timestamp: now - (2 * day),
            is_pinned: false,
            notes: "开局死两次直接泉水挂机，发言恶劣，务必注意。".to_string(),
            encounter_history: vec![EncounteredGameRecord {
                game_id: 70030,
                queue_type: "420".to_string(),
                champion_id: 35,
                timestamp: now - (2 * day),
                result: Some("Loss".to_string()),
            }],
        },
        SavedPlayer {
            puuid: "mock-puuid-support".to_string(),
            summoner_name: "温和猫猫酱".to_string(),
            tag_line: "CN3".to_string(),
            tag: Some("大腿".to_string()),
            champion_id: 350, // 悠米 Yuumi
            profile_icon_id: 4892,
            encountered_count: 6,
            last_met_timestamp: now - (3 * day),
            is_pinned: false,
            notes: "主玩软辅，视野得分极高，开麦报技能温柔且清晰。".to_string(),
            encounter_history: vec![
                EncounteredGameRecord {
                    game_id: 70040,
                    queue_type: "450".to_string(), // 大乱斗
                    champion_id: 350,
                    timestamp: now - (3 * day),
                    result: Some("Win".to_string()),
                },
                EncounteredGameRecord {
                    game_id: 70041,
                    queue_type: "420".to_string(),
                    champion_id: 40, // 风女
                    timestamp: now - (5 * day),
                    result: Some("Win".to_string()),
                },
            ],
        },
        SavedPlayer {
            puuid: "mock-puuid-adc-master".to_string(),
            summoner_name: "狂暴滑板鞋".to_string(),
            tag_line: "001".to_string(),
            tag: Some("绝活".to_string()),
            champion_id: 429, // 卡莉丝塔
            profile_icon_id: 3150,
            encountered_count: 2,
            last_met_timestamp: now - (5 * day),
            is_pinned: false,
            notes: "操作手法极其纯熟，走砍拉扯一流。".to_string(),
            encounter_history: vec![EncounteredGameRecord {
                game_id: 70050,
                queue_type: "420".to_string(),
                champion_id: 429,
                timestamp: now - (5 * day),
                result: Some("Win".to_string()),
            }],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_saved_player_format_and_helpers() {
        let mut p = SavedPlayer::new("puuid-1", "测试玩家", "CN1");
        assert_eq!(p.full_name(), "测试玩家#CN1");
        assert_eq!(
            p.profile_icon_url(),
            "/lol-game-data/assets/v1/profile-icons/29.jpg"
        );
        assert_eq!(p.champion_icon_url(), None);

        p.champion_id = 157;
        assert_eq!(
            p.champion_icon_url(),
            Some("/lol-game-data/assets/v1/champion-icons/157.png".to_string())
        );

        p.last_met_timestamp = chrono::Utc::now().timestamp_millis() - 60_000 * 5;
        assert_eq!(p.formatted_last_met(), "5分钟前");
    }

    #[test]
    fn test_mock_saved_players() {
        let players = create_mock_saved_players();
        assert!(players.len() >= 5);
        assert!(players.iter().any(|p| p.is_pinned));
        assert!(players.iter().any(|p| p.tag.as_deref() == Some("大腿")));
        assert!(players.iter().any(|p| p.tag.as_deref() == Some("避坑")));
        assert!(players.iter().any(|p| p.tag.as_deref() == Some("绝活")));
    }
}
