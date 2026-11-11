use serde::{Deserialize, Serialize};

use crate::lcu::client::{build_auth_header, LcuClient};
use crate::lcu::game_data::GameDataAssets;

// ─── LCU 原始响应结构体 ───

/// `/lol-match-history/v1/products/lol/{puuid}/matches` 的原始返回
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LcuMatchHistoryResponse {
    pub games: LcuMatchGamesContainer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LcuMatchGamesContainer {
    pub games: Vec<LcuMatchGame>,
    pub game_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LcuMatchGame {
    pub game_id: u64,
    pub game_creation: u64,
    pub game_duration: u64,
    pub queue_id: i32,
    pub map_id: Option<u32>,
    pub participants: Vec<LcuMatchParticipant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LcuMatchParticipant {
    pub champion_id: i32,
    pub spell1_id: i32,
    pub spell2_id: i32,
    pub stats: LcuMatchStats,
    #[serde(default)]
    pub timeline: Option<LcuParticipantTimeline>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LcuParticipantTimeline {
    #[serde(default)]
    pub lane: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LcuMatchStats {
    pub win: bool,
    pub kills: i32,
    pub deaths: i32,
    pub assists: i32,
    pub champ_level: i32,
    pub item0: i32,
    pub item1: i32,
    pub item2: i32,
    pub item3: i32,
    pub item4: i32,
    pub item5: i32,
    pub item6: i32,
    pub perk0: i32,
    pub total_minions_killed: Option<i32>,
    pub neutral_minions_killed: Option<i32>,
    pub gold_earned: Option<i32>,
    pub total_damage_dealt_to_champions: Option<i32>,
    pub total_heal: Option<i32>,
    #[serde(default)]
    pub game_ended_in_early_surrender: bool,
    #[serde(default)]
    pub subteam_placement: Option<u32>,
    // 海克斯强化（海克斯大乱斗 queueId 2400 / 经典海斗 2450）
    #[serde(default)]
    pub augments: Vec<i32>,
    #[serde(default)]
    pub player_augment1: i32,
    #[serde(default)]
    pub player_augment2: i32,
    #[serde(default)]
    pub player_augment3: i32,
    #[serde(default)]
    pub player_augment4: i32,
    #[serde(default)]
    pub player_augment5: i32,
    #[serde(default)]
    pub role_bound_item: Option<i32>,
}

// ─── 展示用的清洗结构体 ───

/// 清洗后的单局战绩数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchDisplay {
    pub queue_id: i32,
    pub game_id: u64,
    pub time: String,
    pub short_time: String,
    pub name: String,
    pub map: String,
    pub duration: String,
    pub remake: bool,
    pub win: bool,
    pub placement: Option<u32>,
    pub champion_id: i32,
    pub spell1_id: i32,
    pub spell2_id: i32,
    pub champ_level: i32,
    pub kills: i32,
    pub deaths: i32,
    pub assists: i32,
    pub kda: String,
    pub item_ids: Vec<i32>,
    pub rune_id: i32,
    pub cs: i32,
    pub gold: i32,
    pub time_stamp: u64,
    pub total_damage: i32,
    pub total_heal: i32,
    pub champion_icon_url: String,
    pub spell1_icon_url: String,
    pub spell2_icon_url: String,
    pub rune_icon_url: String,
    pub item_icon_urls: Vec<String>,
    pub augment_ids: Vec<i32>,
    pub augment_icon_urls: Vec<String>,
    pub augment_names: Vec<String>,
    #[serde(default)]
    pub position: Option<String>,
    #[serde(default)]
    pub role_bound_item: Option<i32>,
}

// ─── 数据清洗与辅助函数 ───

/// 判断是否为具备分路的召唤师峡谷类 5v5 模式
pub fn is_laned_mode(queue_id: i32) -> bool {
    matches!(
        queue_id,
        400 | 420
            | 430
            | 440
            | 480
            | 490
            | 700
            | 800
            | 810
            | 820
            | 830
            | 840
            | 850
            | 1300
            | 1400
            | 1900
    )
}

/// 根据官方分路任务/专属装备 ID 解析分路（TOP / MIDDLE / BOTTOM / UTILITY / JUNGLE）
pub fn resolve_role_bound_item_position(item_id: i32) -> Option<&'static str> {
    match item_id {
        1090 | 1200 | 1220 | 1221 | 1222 => Some("TOP"),
        1091 | 1201 | 1206 => Some("MIDDLE"),
        1092 | 1202 | 1207 => Some("BOTTOM"),
        1093 | 1203 | 1208 => Some("UTILITY"),
        1094 | 1204 | 1205 | 1209 | 1210 | 1211 => Some("JUNGLE"),
        _ => None,
    }
}

/// 判断是否为辅助装备
pub fn is_support_item(item_id: i32) -> bool {
    matches!(item_id, 3850..=3871 | 3876..=3884)
}

/// 判断是否为打野专属装备/幼体
pub fn is_jungle_item(item_id: i32) -> bool {
    matches!(item_id, 1101..=1103)
}

/// 将 LCU 的 lane 和 role 推导为标准 5 峡谷位置（TOP, JUNGLE, MIDDLE, BOTTOM, UTILITY）
pub fn resolve_position(lane: Option<&str>, role: Option<&str>) -> Option<String> {
    let lane_clean = lane.map(|l| l.trim()).unwrap_or("");
    let role_clean = role.map(|r| r.trim()).unwrap_or("");

    let lane_upper = lane_clean.to_ascii_uppercase();
    let role_upper = role_clean.to_ascii_uppercase();

    // 1. 如果角色明确为辅助
    if role_upper.contains("SUPPORT") {
        return Some("UTILITY".to_string());
    }

    // 2. 如果 lane 有具体值且不是 NONE
    if !lane_upper.is_empty() && lane_upper != "NONE" {
        match lane_upper.as_str() {
            "TOP" => return Some("TOP".to_string()),
            "JUNGLE" => return Some("JUNGLE".to_string()),
            "MIDDLE" | "MID" => return Some("MIDDLE".to_string()),
            "BOTTOM" | "BOT" => {
                if role_upper.contains("SUPPORT") {
                    return Some("UTILITY".to_string());
                } else {
                    return Some("BOTTOM".to_string());
                }
            }
            _ => {}
        }
    }

    // 3. 当 lane 为 NONE 或空时，从 role 进一步兜底
    if role_upper.contains("CARRY") || role_upper.contains("DUO") {
        return Some("BOTTOM".to_string());
    }

    None
}

/// 归一化 SGP teamPosition
pub fn normalize_sgp_position(val: Option<&str>) -> Option<String> {
    let val = val?.trim();
    if val.is_empty() || val.eq_ignore_ascii_case("NONE") {
        return None;
    }
    let upper = val.to_ascii_uppercase();
    match upper.as_str() {
        "TOP" => Some("TOP".to_string()),
        "JUNGLE" => Some("JUNGLE".to_string()),
        "MIDDLE" | "MID" => Some("MIDDLE".to_string()),
        "BOTTOM" | "BOT" => Some("BOTTOM".to_string()),
        "UTILITY" | "SUPPORT" => Some("UTILITY".to_string()),
        _ => None,
    }
}

/// 将候选海克斯强化 ID 去重、过滤 0、最多保留 5 个
pub fn dedupe_augment_ids<I: IntoIterator<Item = i32>>(ids: I) -> Vec<i32> {
    let mut seen = std::collections::HashSet::new();
    let mut ordered = Vec::new();
    for id in ids {
        if id != 0 && seen.insert(id) {
            ordered.push(id);
        }
    }
    ordered.truncate(5);
    ordered
}

fn extract_augment_ids(stats: &LcuMatchStats) -> Vec<i32> {
    dedupe_augment_ids(stats.augments.iter().copied().chain([
        stats.player_augment1,
        stats.player_augment2,
        stats.player_augment3,
        stats.player_augment4,
        stats.player_augment5,
    ]))
}

fn resolve_augment_details(ids: &[i32], assets: &GameDataAssets) -> (Vec<String>, Vec<String>) {
    let mut icon_urls = Vec::new();
    let mut names = Vec::new();
    for &id in ids {
        if let Some(detail) = assets.augments.get(&id) {
            icon_urls.push(detail.icon_path.clone());
            let name = if detail.name.trim().is_empty() {
                "海克斯强化".to_string()
            } else {
                detail.name.clone()
            };
            names.push(name);
        }
    }
    (icon_urls, names)
}

impl LcuMatchGame {
    /// 将 LCU 原始对局数据清洗为前端展示结构。
    pub fn to_display(&self, assets: &GameDataAssets) -> Option<MatchDisplay> {
        let participant = self.participants.first()?;
        let stats = &participant.stats;

        let cs =
            stats.total_minions_killed.unwrap_or(0) + stats.neutral_minions_killed.unwrap_or(0);
        let gold = stats.gold_earned.unwrap_or(0);
        let total_damage = stats.total_damage_dealt_to_champions.unwrap_or(0);
        let total_heal = stats.total_heal.unwrap_or(0);

        let item_ids = vec![
            stats.item0,
            stats.item1,
            stats.item2,
            stats.item3,
            stats.item4,
            stats.item5,
            stats.item6,
        ];

        let queue_info = get_queue_info(self.queue_id);
        let time = timestamp_to_str(self.game_creation);
        let short_time = timestamp_to_short_str(self.game_creation);
        let duration = secs_to_str(self.game_duration);

        let kda = if stats.deaths == 0 {
            "Perfect".to_string()
        } else {
            format!(
                "{:.2}",
                (stats.kills as f64 + stats.assists as f64) / stats.deaths as f64
            )
        };

        let champion_icon_url = format!(
            "/lol-game-data/assets/v1/champion-icons/{}.png",
            participant.champion_id
        );
        let spell1_icon_url = assets
            .spells
            .get(&participant.spell1_id)
            .cloned()
            .unwrap_or_default();
        let spell2_icon_url = assets
            .spells
            .get(&participant.spell2_id)
            .cloned()
            .unwrap_or_default();
        let rune_icon_url = assets.runes.get(&stats.perk0).cloned().unwrap_or_default();
        let item_icon_urls: Vec<String> = item_ids
            .iter()
            .filter(|&&id| id > 0)
            .filter_map(|id| assets.items.get(id).cloned())
            .collect();

        let augment_ids = extract_augment_ids(stats);
        let (augment_icon_urls, augment_names) = resolve_augment_details(&augment_ids, assets);

        let position = if !is_laned_mode(self.queue_id) {
            None
        } else {
            stats
                .role_bound_item
                .and_then(resolve_role_bound_item_position)
                .map(|pos| pos.to_string())
                .or_else(|| {
                    item_ids
                        .iter()
                        .find_map(|&id| resolve_role_bound_item_position(id))
                        .map(|pos| pos.to_string())
                })
                .or_else(|| {
                    if participant.spell1_id == 11 || participant.spell2_id == 11 {
                        Some("JUNGLE".to_string())
                    } else if item_ids.iter().any(|&id| is_support_item(id)) {
                        Some("UTILITY".to_string())
                    } else if item_ids.iter().any(|&id| is_jungle_item(id)) {
                        Some("JUNGLE".to_string())
                    } else {
                        None
                    }
                })
                .or_else(|| {
                    participant
                        .timeline
                        .as_ref()
                        .and_then(|t| resolve_position(t.lane.as_deref(), t.role.as_deref()))
                })
        };

        Some(MatchDisplay {
            queue_id: self.queue_id,
            game_id: self.game_id,
            time,
            short_time,
            name: queue_info.name.to_string(),
            map: queue_info.map.to_string(),
            duration,
            remake: stats.game_ended_in_early_surrender,
            win: stats.win,
            placement: stats.subteam_placement,
            champion_id: participant.champion_id,
            spell1_id: participant.spell1_id,
            spell2_id: participant.spell2_id,
            champ_level: stats.champ_level,
            kills: stats.kills,
            deaths: stats.deaths,
            assists: stats.assists,
            kda,
            item_ids,
            rune_id: stats.perk0,
            cs,
            gold,
            time_stamp: self.game_creation,
            total_damage,
            total_heal,
            champion_icon_url,
            spell1_icon_url,
            spell2_icon_url,
            rune_icon_url,
            item_icon_urls,
            augment_ids,
            augment_icon_urls,
            augment_names,
            position,
            role_bound_item: stats.role_bound_item,
        })
    }
}

/// 解析完整的对局列表为展示结构
pub fn parse_match_history_games(
    games: &[LcuMatchGame],
    assets: &GameDataAssets,
) -> Vec<MatchDisplay> {
    games.iter().filter_map(|g| g.to_display(assets)).collect()
}

/// 从 LCU HTTP 获取战绩列表并解析
pub async fn fetch_match_history(
    client: &LcuClient,
    puuid: &str,
    beg_index: Option<u32>,
    end_index: Option<u32>,
    assets: &GameDataAssets,
) -> Result<Vec<MatchDisplay>, String> {
    let mut url = format!(
        "https://127.0.0.1:{}/lol-match-history/v1/products/lol/{}/matches",
        client.port, puuid
    );

    let mut params = Vec::new();
    if let Some(b) = beg_index {
        params.push(format!("begIndex={}", b));
    }
    if let Some(e) = end_index {
        params.push(format!("endIndex={}", e));
    }
    if !params.is_empty() {
        url.push('?');
        url.push_str(&params.join("&"));
    }

    let auth = build_auth_header(&client.token);

    let resp = client
        .http_client
        .get(&url)
        .header("Authorization", auth)
        .send()
        .await
        .map_err(|e| format!("获取战绩网络错误: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("获取战绩失败: HTTP {}", resp.status()));
    }

    let history: LcuMatchHistoryResponse = resp
        .json()
        .await
        .map_err(|e| format!("解析战绩 JSON 失败: {}", e))?;

    Ok(parse_match_history_games(&history.games.games, assets))
}

// ─── 队列 ID 映射 ───

pub fn queue_id_to_opgg_mode(queue_id: i32) -> &'static str {
    match queue_id {
        450 | 2400 | 2450 => "aram",
        1700 | 1710 => "arena",
        1300 => "nexus_blitz",
        900 | 1900 => "urf",
        _ => "ranked",
    }
}

pub struct QueueInfo {
    pub name: &'static str,
    pub map: &'static str,
}

pub fn get_queue_info(queue_id: i32) -> QueueInfo {
    match queue_id {
        400 => QueueInfo {
            name: "征召模式",
            map: "召唤师峡谷",
        },
        420 => QueueInfo {
            name: "排位单双排",
            map: "召唤师峡谷",
        },
        430 => QueueInfo {
            name: "匹配模式",
            map: "召唤师峡谷",
        },
        440 => QueueInfo {
            name: "排位灵活组排",
            map: "召唤师峡谷",
        },
        480 | 490 => QueueInfo {
            name: "快速模式",
            map: "召唤师峡谷",
        },
        450 => QueueInfo {
            name: "极地大乱斗",
            map: "嚎哭深渊",
        },
        2400 => QueueInfo {
            name: "海克斯大乱斗",
            map: "嚎哭深渊",
        },
        2450 => QueueInfo {
            name: "经典海斗",
            map: "嚎哭深渊",
        },
        800..=850 => QueueInfo {
            name: "人机对战",
            map: "召唤师峡谷",
        },
        900 => QueueInfo {
            name: "无限火力",
            map: "召唤师峡谷",
        },
        1010 => QueueInfo {
            name: "随机无限火力",
            map: "嚎哭深渊",
        },
        1020 => QueueInfo {
            name: "克隆模式",
            map: "召唤师峡谷",
        },
        1300 => QueueInfo {
            name: "极限闪击",
            map: "极限闪击",
        },
        1700 | 1710 => QueueInfo {
            name: "斗魂竞技场",
            map: "斗魂竞技场",
        },
        1810..=1840 => QueueInfo {
            name: "捉鬼模式",
            map: "捉鬼模式",
        },
        4300 | 4310 => QueueInfo {
            name: "经典模式",
            map: "召唤师峡谷",
        },
        0 => QueueInfo {
            name: "自定义模式",
            map: "自定义",
        },
        _ => QueueInfo {
            name: "自定义模式",
            map: "自定义",
        },
    }
}

// ─── 时间工具函数 ───

pub fn timestamp_to_str(ms: u64) -> String {
    let secs = (ms / 1000) as i64;
    chrono::DateTime::from_timestamp(secs, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "1970-01-01 00:00".to_string())
}

pub fn timestamp_to_short_str(ms: u64) -> String {
    let secs = (ms / 1000) as i64;
    chrono::DateTime::from_timestamp(secs, 0)
        .map(|dt| dt.format("%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "01-01 00:00".to_string())
}

pub fn secs_to_str(total_secs: u64) -> String {
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{:02}:{:02}", mins, secs)
}

impl crate::gameflow::match_stats::MatchLike for MatchDisplay {
    fn is_remake(&self) -> bool {
        self.remake
    }

    fn is_win(&self) -> bool {
        self.win
    }

    fn kills(&self) -> u32 {
        self.kills.max(0) as u32
    }

    fn deaths(&self) -> u32 {
        self.deaths.max(0) as u32
    }

    fn assists(&self) -> u32 {
        self.assists.max(0) as u32
    }

    fn position(&self) -> Option<&str> {
        self.position.as_deref()
    }

    fn champion_id(&self) -> i64 {
        self.champion_id as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lcu::game_data::CherryAugmentDetail;
    use serde_json::json;

    #[test]
    fn dedupe_augment_ids_filters_zero_and_dedupes_in_order() {
        assert_eq!(dedupe_augment_ids([3, 0, 1, 3, 2, 1]), vec![3, 1, 2]);
        assert!(dedupe_augment_ids(std::iter::empty()).is_empty());
    }

    #[test]
    fn dedupe_augment_ids_truncates_to_five() {
        assert_eq!(
            dedupe_augment_ids([1, 2, 3, 4, 5, 6, 7]),
            vec![1, 2, 3, 4, 5]
        );
    }

    #[test]
    fn extract_augment_ids_merges_augments_and_player_slots() {
        let stats: LcuMatchStats = serde_json::from_value(json!({
            "win": true, "kills": 0, "deaths": 0, "assists": 0,
            "champLevel": 18,
            "item0": 0, "item1": 0, "item2": 0, "item3": 0,
            "item4": 0, "item5": 0, "item6": 0, "perk0": 0,
            "augments": [7010, 0],
            "playerAugment1": 7018,
            "playerAugment2": 7027,
            "playerAugment3": 7010,
            "playerAugment4": 0,
            "playerAugment5": 7022
        }))
        .unwrap();
        assert_eq!(extract_augment_ids(&stats), vec![7010, 7018, 7027, 7022]);
    }

    #[test]
    fn queue_id_to_opgg_mode_maps_known_queues() {
        assert_eq!(queue_id_to_opgg_mode(450), "aram");
        assert_eq!(queue_id_to_opgg_mode(2400), "aram");
        assert_eq!(queue_id_to_opgg_mode(1700), "arena");
        assert_eq!(queue_id_to_opgg_mode(1300), "nexus_blitz");
        assert_eq!(queue_id_to_opgg_mode(900), "urf");
        assert_eq!(queue_id_to_opgg_mode(420), "ranked");
        assert_eq!(queue_id_to_opgg_mode(-1), "ranked");
    }

    fn sample_game(queue_id: i32) -> LcuMatchGame {
        serde_json::from_value(json!({
            "gameId": 483_920_751_u64,
            "gameCreation": 1_705_329_000_000_u64,
            "gameDuration": 1530_u64,
            "queueId": queue_id,
            "participants": [{
                "championId": 432,
                "spell1Id": 4,
                "spell2Id": 12,
                "stats": {
                    "win": true,
                    "kills": 10, "deaths": 2, "assists": 8,
                    "champLevel": 18,
                    "item0": 3157, "item1": 3020, "item2": 0, "item3": 0,
                    "item4": 0, "item5": 0, "item6": 0,
                    "perk0": 8112,
                    "totalMinionsKilled": 20,
                    "neutralMinionsKilled": 12,
                    "goldEarned": 13500,
                    "totalDamageDealtToChampions": 28000,
                    "totalHeal": 1200
                }
            }]
        }))
        .unwrap()
    }

    #[test]
    fn to_display_cleans_core_stats() {
        let mut assets = GameDataAssets::default();
        assets
            .spells
            .insert(4, "/lol-game-data/assets/spell/Summoner_Flash.png".into());
        assets
            .items
            .insert(3157, "/lol-game-data/assets/items/item_3157.png".into());
        assets.augments.insert(
            7018,
            CherryAugmentDetail {
                id: 7018,
                name: "".into(),
                icon_path: "/fe/lol-loot/aug_7018.png".into(),
            },
        );

        let mut game = sample_game(450);
        game.participants[0].stats.player_augment1 = 7018;

        let d = game.to_display(&assets).unwrap();
        assert_eq!(d.name, "极地大乱斗");
        assert_eq!(d.map, "嚎哭深渊");
        assert_eq!(d.time, "2024-01-15 14:30");
        assert_eq!(d.duration, "25:30");
        assert!(d.win);
        assert!(!d.remake);
        assert_eq!(d.kda, "9.00");
        assert_eq!(d.cs, 32);
        assert_eq!(d.gold, 13500);
        assert_eq!(
            d.champion_icon_url,
            "/lol-game-data/assets/v1/champion-icons/432.png"
        );
        assert_eq!(d.item_icon_urls.len(), 1);
        assert_eq!(
            d.spell1_icon_url,
            "/lol-game-data/assets/spell/Summoner_Flash.png"
        );
        assert_eq!(d.augment_names, vec!["海克斯强化"]);
        assert_eq!(d.augment_icon_urls, vec!["/fe/lol-loot/aug_7018.png"]);
    }

    #[test]
    fn to_display_perfect_kda_when_zero_deaths() {
        let mut game = sample_game(420);
        game.participants[0].stats.deaths = 0;
        let d = game.to_display(&GameDataAssets::default()).unwrap();
        assert_eq!(d.kda, "Perfect");
        assert_eq!(d.name, "排位单双排");
        assert_eq!(d.map, "召唤师峡谷");
    }

    #[test]
    fn to_display_returns_none_without_participants() {
        let mut game = sample_game(420);
        game.participants.clear();
        assert!(game.to_display(&GameDataAssets::default()).is_none());
    }

    #[test]
    fn secs_to_str_formats_minutes_and_seconds() {
        assert_eq!(secs_to_str(1530), "25:30");
        assert_eq!(secs_to_str(3725), "62:05");
        assert_eq!(secs_to_str(59), "00:59");
    }

    #[test]
    fn timestamp_helpers_format_utc() {
        assert_eq!(timestamp_to_str(1_705_329_000_000), "2024-01-15 14:30");
        assert_eq!(timestamp_to_short_str(1_705_329_000_000), "01-15 14:30");
        assert_eq!(timestamp_to_str(u64::MAX), "1970-01-01 00:00");
    }

    #[test]
    fn resolve_position_maps_standard_lanes_and_roles() {
        assert_eq!(
            resolve_position(Some("TOP"), Some("SOLO")),
            Some("TOP".to_string())
        );
        assert_eq!(
            resolve_position(Some("JUNGLE"), Some("NONE")),
            Some("JUNGLE".to_string())
        );
        assert_eq!(
            resolve_position(Some("MIDDLE"), Some("SOLO")),
            Some("MIDDLE".to_string())
        );
        assert_eq!(
            resolve_position(Some("MID"), Some("SOLO")),
            Some("MIDDLE".to_string())
        );
        assert_eq!(
            resolve_position(Some("BOTTOM"), Some("DUO_CARRY")),
            Some("BOTTOM".to_string())
        );
        assert_eq!(
            resolve_position(Some("BOTTOM"), Some("DUO_SUPPORT")),
            Some("UTILITY".to_string())
        );
        assert_eq!(
            resolve_position(Some("BOT"), Some("SUPPORT")),
            Some("UTILITY".to_string())
        );
        assert_eq!(
            resolve_position(Some("NONE"), Some("SUPPORT")),
            Some("UTILITY".to_string())
        );
        assert_eq!(
            resolve_position(Some(""), Some("SUPPORT")),
            Some("UTILITY".to_string())
        );
        assert_eq!(
            resolve_position(Some("NONE"), Some("DUO_SUPPORT")),
            Some("UTILITY".to_string())
        );
        assert_eq!(
            resolve_position(Some("NONE"), Some("DUO_CARRY")),
            Some("BOTTOM".to_string())
        );
        assert_eq!(resolve_position(Some("NONE"), Some("NONE")), None);
        assert_eq!(resolve_position(None, None), None);
        assert_eq!(resolve_position(Some(""), Some("")), None);
    }

    #[test]
    fn normalize_sgp_position_maps_positions() {
        assert_eq!(normalize_sgp_position(Some("TOP")), Some("TOP".to_string()));
        assert_eq!(
            normalize_sgp_position(Some("JUNGLE")),
            Some("JUNGLE".to_string())
        );
        assert_eq!(
            normalize_sgp_position(Some("MIDDLE")),
            Some("MIDDLE".to_string())
        );
        assert_eq!(
            normalize_sgp_position(Some("BOTTOM")),
            Some("BOTTOM".to_string())
        );
        assert_eq!(
            normalize_sgp_position(Some("UTILITY")),
            Some("UTILITY".to_string())
        );
        assert_eq!(
            normalize_sgp_position(Some("SUPPORT")),
            Some("UTILITY".to_string())
        );
        assert_eq!(normalize_sgp_position(Some("")), None);
        assert_eq!(normalize_sgp_position(Some("NONE")), None);
        assert_eq!(normalize_sgp_position(None), None);
    }

    #[test]
    fn to_display_extracts_position_from_timeline() {
        let mut game = sample_game(420);
        game.participants[0].timeline = Some(LcuParticipantTimeline {
            lane: Some("BOTTOM".into()),
            role: Some("DUO_SUPPORT".into()),
        });
        let d = game.to_display(&GameDataAssets::default()).unwrap();
        assert_eq!(d.position, Some("UTILITY".to_string()));
    }

    #[test]
    fn to_display_extracts_support_when_lane_is_none() {
        let mut game = sample_game(420);
        game.participants[0].timeline = Some(LcuParticipantTimeline {
            lane: Some("NONE".into()),
            role: Some("SUPPORT".into()),
        });
        let d = game.to_display(&GameDataAssets::default()).unwrap();
        assert_eq!(d.position, Some("UTILITY".to_string()));
    }

    #[test]
    fn to_display_extracts_jungle_from_smite_spell() {
        let mut game = sample_game(420);
        game.participants[0].spell1_id = 11;
        game.participants[0].timeline = Some(LcuParticipantTimeline {
            lane: Some("NONE".into()),
            role: Some("NONE".into()),
        });
        let d = game.to_display(&GameDataAssets::default()).unwrap();
        assert_eq!(d.position, Some("JUNGLE".to_string()));
    }

    #[test]
    fn to_display_returns_none_position_for_aram_and_entertainment_modes() {
        let mut game_aram = sample_game(450);
        game_aram.participants[0].timeline = Some(LcuParticipantTimeline {
            lane: Some("TOP".into()),
            role: Some("SOLO".into()),
        });
        let d_aram = game_aram.to_display(&GameDataAssets::default()).unwrap();
        assert_eq!(d_aram.position, None);

        let mut game_hex = sample_game(2400);
        game_hex.participants[0].timeline = Some(LcuParticipantTimeline {
            lane: Some("BOTTOM".into()),
            role: Some("SUPPORT".into()),
        });
        let d_hex = game_hex.to_display(&GameDataAssets::default()).unwrap();
        assert_eq!(d_hex.position, None);
    }

    #[test]
    fn resolve_role_bound_item_position_maps_all_classes() {
        assert_eq!(resolve_role_bound_item_position(1090), Some("TOP"));
        assert_eq!(resolve_role_bound_item_position(1200), Some("TOP"));
        assert_eq!(resolve_role_bound_item_position(1220), Some("TOP"));
        assert_eq!(resolve_role_bound_item_position(1091), Some("MIDDLE"));
        assert_eq!(resolve_role_bound_item_position(1206), Some("MIDDLE"));
        assert_eq!(resolve_role_bound_item_position(1092), Some("BOTTOM"));
        assert_eq!(resolve_role_bound_item_position(1207), Some("BOTTOM"));
        assert_eq!(resolve_role_bound_item_position(1093), Some("UTILITY"));
        assert_eq!(resolve_role_bound_item_position(1208), Some("UTILITY"));
        assert_eq!(resolve_role_bound_item_position(1094), Some("JUNGLE"));
        assert_eq!(resolve_role_bound_item_position(1209), Some("JUNGLE"));
        assert_eq!(resolve_role_bound_item_position(0), None);
        assert_eq!(resolve_role_bound_item_position(9999), None);
    }

    #[test]
    fn to_display_prioritizes_role_bound_item_over_erroneous_timeline() {
        let mut game = sample_game(430);
        game.participants[0].champion_id = 68;
        game.participants[0].stats.role_bound_item = Some(1220);
        game.participants[0].timeline = Some(LcuParticipantTimeline {
            lane: Some("NONE".into()),
            role: Some("SUPPORT".into()),
        });
        let d = game.to_display(&GameDataAssets::default()).unwrap();
        assert_eq!(d.position, Some("TOP".to_string()));
        assert_eq!(d.role_bound_item, Some(1220));
    }

    #[test]
    fn to_display_detects_role_bound_item_in_item_ids() {
        let mut game = sample_game(430);
        game.participants[0].champion_id = 517;
        game.participants[0].stats.item0 = 1206;
        game.participants[0].timeline = Some(LcuParticipantTimeline {
            lane: Some("NONE".into()),
            role: Some("NONE".into()),
        });
        let d = game.to_display(&GameDataAssets::default()).unwrap();
        assert_eq!(d.position, Some("MIDDLE".to_string()));
    }

    // ─── 响应快照（fixture）测试 ───

    struct Fixture {
        name: &'static str,
        raw: &'static str,
    }

    const REAL: &[Fixture] = &[
        Fixture {
            name: "real/top-solo",
            raw: include_str!("../../testdata/match/real/top-solo.json"),
        },
        Fixture {
            name: "real/mid-solo",
            raw: include_str!("../../testdata/match/real/mid-solo.json"),
        },
        Fixture {
            name: "real/jungle-none",
            raw: include_str!("../../testdata/match/real/jungle-none.json"),
        },
        Fixture {
            name: "real/bottom-carry",
            raw: include_str!("../../testdata/match/real/bottom-carry.json"),
        },
        Fixture {
            name: "real/bottom-support",
            raw: include_str!("../../testdata/match/real/bottom-support.json"),
        },
        Fixture {
            name: "real/none-duo",
            raw: include_str!("../../testdata/match/real/none-duo.json"),
        },
        Fixture {
            name: "real/remake",
            raw: include_str!("../../testdata/match/real/remake.json"),
        },
        Fixture {
            name: "real/custom-3100",
            raw: include_str!("../../testdata/match/real/custom-3100.json"),
        },
    ];

    const SYNTHETIC_ARAM: &str = include_str!("../../testdata/match/synthetic/aram-450.json");
    const SYNTHETIC_ARENA: &str =
        include_str!("../../testdata/match/synthetic/arena-1700.json");
    const SYNTHETIC_RANKED: &str =
        include_str!("../../testdata/match/synthetic/ranked-420.json");
    const SYNTHETIC_MINIMAL: &str =
        include_str!("../../testdata/match/synthetic/minimal-420.json");

    fn load_fixture(raw: &str) -> LcuMatchGame {
        let response: LcuMatchHistoryResponse =
            serde_json::from_str(raw).expect("fixture 应能按 LCU 外层响应反序列化");
        assert_eq!(response.games.games.len(), 1, "fixture 约定只含单局");
        response.games.games.into_iter().next().unwrap()
    }

    #[test]
    fn fixture_ranked_420_parses_full_lcu_payload() {
        let game = load_fixture(SYNTHETIC_RANKED);
        let d = game.to_display(&GameDataAssets::default()).unwrap();

        assert_eq!(d.name, "排位单双排");
        assert_eq!(d.map, "召唤师峡谷");
        assert_eq!(d.time, "2024-01-15 14:30");
        assert_eq!(d.duration, "25:30");
        assert!(d.win);
        assert!(!d.remake);
        assert_eq!(d.kda, "5.33");
        assert_eq!(d.cs, 206);
        assert_eq!(d.role_bound_item, Some(1204));
        assert_eq!(d.position, Some("JUNGLE".to_string()));
        assert_eq!(d.item_ids, vec![6692, 3006, 6675, 6676, 1038, 3364, 3340]);
        assert!(d.item_icon_urls.is_empty());
    }

    #[test]
    fn fixture_aram_has_no_position_and_collects_augments() {
        let game = load_fixture(SYNTHETIC_ARAM);
        let d = game.to_display(&GameDataAssets::default()).unwrap();

        assert_eq!(d.name, "极地大乱斗");
        assert_eq!(d.map, "嚎哭深渊");
        assert_eq!(d.duration, "18:08");
        assert!(!d.win);
        assert_eq!(d.kda, "Perfect");
        assert_eq!(d.cs, 63);
        assert_eq!(d.position, None);
        assert_eq!(d.augment_ids, vec![7010, 7018, 7027, 7022]);
    }

    #[test]
    fn fixture_arena_exposes_subteam_placement() {
        let game = load_fixture(SYNTHETIC_ARENA);
        let d = game.to_display(&GameDataAssets::default()).unwrap();

        assert_eq!(d.name, "斗魂竞技场");
        assert_eq!(d.placement, Some(2));
        assert_eq!(d.position, None);
    }

    #[test]
    fn fixture_minimal_payload_degrades_gracefully() {
        let game = load_fixture(SYNTHETIC_MINIMAL);
        let d = game.to_display(&GameDataAssets::default()).unwrap();

        assert_eq!(d.cs, 0);
        assert_eq!(d.gold, 0);
        assert_eq!(d.total_damage, 0);
        assert_eq!(d.total_heal, 0);
        assert_eq!(d.kda, "2.00");
        assert!(!d.remake);
        assert_eq!(d.placement, None);
        assert_eq!(d.position, None);
        assert_eq!(d.spell1_icon_url, "");
        assert_eq!(d.rune_icon_url, "");
        assert!(d.augment_ids.is_empty());
    }

    struct RealExpect {
        name: &'static str,
        queue_id: i32,
        win: bool,
        remake: bool,
        time: &'static str,
        duration: &'static str,
        kda: &'static str,
        cs: i32,
        gold: i32,
        total_damage: i32,
        total_heal: i32,
        champ_level: i32,
        champion_id: i32,
        role_bound_item: Option<i32>,
        position: Option<&'static str>,
        item_ids: [i32; 7],
    }

    const REAL_EXPECT: &[RealExpect] = &[
        RealExpect {
            name: "real/top-solo",
            queue_id: 430,
            win: true,
            remake: false,
            time: "2026-10-21 10:34",
            duration: "28:42",
            kda: "1.40",
            cs: 218,
            gold: 11827,
            total_damage: 14104,
            total_heal: 2636,
            champ_level: 17,
            champion_id: 17,
            role_bound_item: Some(1220),
            position: Some("TOP"),
            item_ids: [1056, 3115, 3006, 3087, 6653, 1028, 3364],
        },
        RealExpect {
            name: "real/mid-solo",
            queue_id: 430,
            win: true,
            remake: false,
            time: "2026-10-21 14:03",
            duration: "30:05",
            kda: "7.00",
            cs: 244,
            gold: 15590,
            total_damage: 29975,
            total_heal: 4646,
            champ_level: 17,
            champion_id: 17,
            role_bound_item: Some(1206),
            position: Some("MIDDLE"),
            item_ids: [1056, 3157, 3115, 3172, 6653, 3087, 3364],
        },
        RealExpect {
            name: "real/jungle-none",
            queue_id: 430,
            win: true,
            remake: false,
            time: "2026-10-20 14:03",
            duration: "33:00",
            kda: "2.29",
            cs: 263,
            gold: 16626,
            total_damage: 36782,
            total_heal: 3498,
            champ_level: 18,
            champion_id: 17,
            role_bound_item: Some(1220),
            position: Some("TOP"),
            item_ids: [3115, 2420, 3087, 3006, 6653, 2510, 3364],
        },
        RealExpect {
            name: "real/bottom-carry",
            queue_id: 430,
            win: false,
            remake: false,
            time: "2026-10-20 13:17",
            duration: "41:28",
            kda: "1.31",
            cs: 239,
            gold: 19369,
            total_damage: 30158,
            total_heal: 9401,
            champ_level: 17,
            champion_id: 18,
            role_bound_item: Some(3006),
            position: Some("BOTTOM"),
            item_ids: [6676, 3026, 3031, 3072, 6675, 3035, 3363],
        },
        RealExpect {
            name: "real/bottom-support",
            queue_id: 430,
            win: true,
            remake: false,
            time: "2026-10-16 13:21",
            duration: "27:49",
            kda: "1.67",
            cs: 47,
            gold: 8150,
            total_damage: 17563,
            total_heal: 3599,
            champ_level: 13,
            champion_id: 63,
            role_bound_item: Some(2055),
            position: Some("UTILITY"),
            item_ids: [3871, 3020, 2031, 2503, 6653, 0, 3364],
        },
        RealExpect {
            name: "real/none-duo",
            queue_id: 430,
            win: false,
            remake: false,
            time: "2026-10-19 13:39",
            duration: "19:21",
            kda: "2.00",
            cs: 159,
            gold: 7786,
            total_damage: 16792,
            total_heal: 1941,
            champ_level: 14,
            champion_id: 17,
            role_bound_item: Some(1220),
            position: Some("TOP"),
            item_ids: [3006, 3087, 1026, 1056, 1043, 3108, 3364],
        },
        RealExpect {
            name: "real/remake",
            queue_id: 430,
            win: true,
            remake: true,
            time: "2026-10-21 13:33",
            duration: "01:13",
            kda: "Perfect",
            cs: 4,
            gold: 596,
            total_damage: 41,
            total_heal: 0,
            champ_level: 1,
            champion_id: 63,
            role_bound_item: Some(1201),
            position: Some("MIDDLE"),
            item_ids: [1056, 2003, 0, 0, 0, 0, 3340],
        },
        RealExpect {
            name: "real/custom-3100",
            queue_id: 3100,
            win: false,
            remake: false,
            time: "2026-10-19 14:51",
            duration: "08:07",
            kda: "2.50",
            cs: 24,
            gold: 3304,
            total_damage: 6821,
            total_heal: 1541,
            champ_level: 6,
            champion_id: 805,
            role_bound_item: Some(0),
            position: None,
            item_ids: [1001, 2031, 1052, 1056, 3057, 0, 3340],
        },
    ];

    #[test]
    fn real_fixtures_produce_expected_display() {
        assert_eq!(REAL.len(), REAL_EXPECT.len(), "样本数与期望表不一致");

        for (fx, exp) in REAL.iter().zip(REAL_EXPECT) {
            assert_eq!(fx.name, exp.name, "样本与期望表顺序不一致");
            let game = load_fixture(fx.raw);
            let d = game.to_display(&GameDataAssets::default()).unwrap();

            assert_eq!(d.queue_id, exp.queue_id, "{}: queueId", fx.name);
            assert_eq!(d.win, exp.win, "{}: win", fx.name);
            assert_eq!(d.remake, exp.remake, "{}: remake", fx.name);
            assert_eq!(d.time, exp.time, "{}: time", fx.name);
            assert_eq!(d.duration, exp.duration, "{}: duration", fx.name);
            assert_eq!(d.kda, exp.kda, "{}: kda", fx.name);
            assert_eq!(d.cs, exp.cs, "{}: cs", fx.name);
            assert_eq!(d.gold, exp.gold, "{}: gold", fx.name);
            assert_eq!(d.total_damage, exp.total_damage, "{}: 伤害", fx.name);
            assert_eq!(d.total_heal, exp.total_heal, "{}: 治疗", fx.name);
            assert_eq!(d.champ_level, exp.champ_level, "{}: 英雄等级", fx.name);
            assert_eq!(d.champion_id, exp.champion_id, "{}: 英雄", fx.name);
            assert_eq!(
                d.role_bound_item, exp.role_bound_item,
                "{}: roleBoundItem",
                fx.name
            );
            assert_eq!(d.position.as_deref(), exp.position, "{}: 分路", fx.name);
            assert_eq!(d.item_ids, exp.item_ids, "{}: 装备", fx.name);
            assert!(d.item_icon_urls.is_empty(), "{}: 图标", fx.name);
        }
    }

    #[test]
    fn real_fixtures_deserialize_and_ignore_unmodelled_fields() {
        for fx in REAL {
            let game = load_fixture(fx.raw);
            assert_eq!(
                game.participants.len(),
                1,
                "{}: current-summoner 的 matches 接口只返回本人",
                fx.name
            );
            assert!(game.game_id > 0, "{}: gameId 应存在", fx.name);
            assert!(game.to_display(&GameDataAssets::default()).is_some());
        }
    }

    #[test]
    fn match_display_implements_match_like_and_computes_stats() {
        use crate::gameflow::match_stats::compute_match_stats;
        let mut display = sample_game(420)
            .to_display(&GameDataAssets::default())
            .unwrap();
        display.win = true;
        display.kills = 10;
        display.deaths = 2;
        display.assists = 5;
        let stats = compute_match_stats(&[display]);
        assert_eq!(stats.total_matches, 1);
        assert_eq!(stats.win_count, 1);
        assert_eq!(stats.win_rate, 100);
        assert!((stats.avg_kda - 7.5).abs() < 1e-4);
    }
}
