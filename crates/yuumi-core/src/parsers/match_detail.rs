//! 对局详情数据解析与清洗模型 (GameDetail / 10 人对抗详情)

use serde::{Deserialize, Serialize};

use crate::lcu::client::{build_auth_header, LcuClient};
use crate::lcu::game_data::GameDataAssets;
use crate::parsers::match_parser::{dedupe_augment_ids, LcuMatchStats, LcuParticipantTimeline};

// ─── LCU 原始响应结构 ───

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawGameDetail {
    pub game_id: u64,
    pub queue_id: i32,
    pub map_id: Option<u32>,
    pub game_creation: u64,
    pub game_duration: u64,
    #[serde(default)]
    pub participant_identities: Vec<RawParticipantIdentity>,
    #[serde(default)]
    pub participants: Vec<RawParticipant>,
    #[serde(default)]
    pub teams: Vec<RawTeam>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawParticipantIdentity {
    pub participant_id: i32,
    #[serde(default)]
    pub player: Option<RawPlayerInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawPlayerInfo {
    #[serde(default)]
    pub game_name: String,
    #[serde(default)]
    pub tag_line: String,
    #[serde(default)]
    pub summoner_name: String,
    #[serde(default)]
    pub puuid: String,
    #[serde(default)]
    pub summoner_id: u64,
    #[serde(default)]
    pub profile_icon: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawParticipant {
    pub participant_id: i32,
    pub team_id: i32,
    pub champion_id: i32,
    pub spell1_id: i32,
    pub spell2_id: i32,
    pub stats: LcuMatchStats,
    #[serde(default)]
    pub timeline: Option<LcuParticipantTimeline>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawTeam {
    pub team_id: i32,
    #[serde(default)]
    pub win: Option<StringOrBool>,
    #[serde(default)]
    pub baron_kills: i32,
    #[serde(default)]
    pub dragon_kills: i32,
    #[serde(default)]
    pub tower_kills: i32,
    #[serde(default)]
    pub inhibitor_kills: i32,
    #[serde(default)]
    pub rift_herald_kills: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StringOrBool {
    String(String),
    Bool(bool),
}

impl StringOrBool {
    pub fn is_win(&self) -> bool {
        match self {
            StringOrBool::String(s) => s.eq_ignore_ascii_case("Win"),
            StringOrBool::Bool(b) => *b,
        }
    }
}

// ─── 清洗后的展示结构体 ───

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDetail {
    pub game_id: u64,
    pub queue_id: i32,
    pub map_id: u32,
    pub duration: String,
    pub date: String,
    pub queue_name: String,
    pub map_name: String,
    pub win: bool,
    pub blue: GameDetailTeam,
    pub red: GameDetailTeam,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDetailTeam {
    pub team_id: i32,
    pub win: bool,
    pub kills: i32,
    pub tower_kills: i32,
    pub inhibitor_kills: i32,
    pub baron_kills: i32,
    pub dragon_kills: i32,
    pub rift_herald_kills: i32,
    pub players: Vec<GameDetailPlayer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDetailPlayer {
    pub participant_id: i32,
    pub team_id: i32,
    pub champion_id: i32,
    pub name: String,
    pub puuid: String,
    pub summoner_id: u64,
    pub level: i32,
    pub kills: i32,
    pub deaths: i32,
    pub assists: i32,
    pub cs: i32,
    pub gold: i32,
    pub damage: i32,
    pub win: bool,
    pub spell1_id: i32,
    pub spell2_id: i32,
    pub rune_id: i32,
    pub item_ids: Vec<i32>,
    pub ward_item_id: Option<i32>,
    pub champion_icon_url: String,
    pub spell1_icon_url: String,
    pub spell2_icon_url: String,
    pub rune_icon_url: String,
    pub item_icon_urls: Vec<String>,
    pub ward_icon_url: String,
    pub augment_ids: Vec<i32>,
    pub augment_names: Vec<String>,
}

impl RawGameDetail {
    /// 清洗为前端 10 人对局详情
    pub fn to_game_detail(&self, assets: &GameDataAssets) -> GameDetail {
        let mins = self.game_duration / 60;
        let secs = self.game_duration % 60;
        let duration = format!("{}:{:02}", mins, secs);

        let date = if self.game_creation > 0 {
            use chrono::{DateTime, Local, Utc};
            let dt = DateTime::<Utc>::from_timestamp_millis(self.game_creation as i64)
                .map(|t| t.with_timezone(&Local))
                .unwrap_or_else(Local::now);
            dt.format("%Y-%m-%d %H:%M").to_string()
        } else {
            String::new()
        };

        let queue_name = match self.queue_id {
            420 => "排位单双排",
            440 => "排位灵活组排",
            430 => "匹配模式",
            450 => "极地大乱斗",
            2400 => "海克斯大乱斗",
            2450 => "经典海斗",
            400 => "征召模式",
            480 | 490 => "快速模式",
            1700 => "斗魂竞技场",
            _ => "其他模式",
        }
        .to_string();

        let map_id = self.map_id.unwrap_or(11);
        let map_name = match map_id {
            11 => "召唤师峡谷",
            12 => "嚎哭深渊",
            30 => "斗魂竞技场",
            _ => "符文大陆",
        }
        .to_string();

        // 建立 participantId -> 玩家身份映射
        let mut player_map = std::collections::HashMap::new();
        for id in &self.participant_identities {
            if let Some(player) = &id.player {
                let base_name = if !player.game_name.is_empty() {
                    player.game_name.clone()
                } else if !player.summoner_name.is_empty() {
                    player.summoner_name.clone()
                } else {
                    format!("玩家{}", id.participant_id)
                };
                let display_name = if !player.tag_line.is_empty() {
                    format!("{}#{}", base_name, player.tag_line)
                } else {
                    base_name
                };
                player_map.insert(
                    id.participant_id,
                    (display_name, player.puuid.clone(), player.summoner_id),
                );
            }
        }

        let mut blue_players = Vec::new();
        let mut red_players = Vec::new();

        for p in &self.participants {
            let (name, puuid, summoner_id) = player_map
                .get(&p.participant_id)
                .cloned()
                .unwrap_or_else(|| (format!("玩家{}", p.participant_id), String::new(), 0));

            let stats = &p.stats;
            let cs =
                stats.total_minions_killed.unwrap_or(0) + stats.neutral_minions_killed.unwrap_or(0);
            let gold = stats.gold_earned.unwrap_or(0);
            let damage = stats.total_damage_dealt_to_champions.unwrap_or(0);

            let item_ids = vec![
                stats.item0,
                stats.item1,
                stats.item2,
                stats.item3,
                stats.item4,
                stats.item5,
            ];
            let ward_item_id = if stats.item6 > 0 {
                Some(stats.item6)
            } else {
                None
            };

            let mut item_icon_urls = Vec::new();
            for &id in &item_ids {
                if id > 0 {
                    let path = assets.items.get(&id).cloned().unwrap_or_else(|| {
                        format!("/lol-game-data/assets/v1/item-icons/{}.png", id)
                    });
                    item_icon_urls.push(path);
                } else {
                    item_icon_urls.push(String::new());
                }
            }

            let ward_icon_url =
                if let Some(wid) = ward_item_id {
                    assets.items.get(&wid).cloned().unwrap_or_else(|| {
                        format!("/lol-game-data/assets/v1/item-icons/{}.png", wid)
                    })
                } else {
                    String::new()
                };

            let champion_icon_url = format!(
                "/lol-game-data/assets/v1/champion-icons/{}.png",
                p.champion_id
            );

            let spell1_icon_url = assets.spells.get(&p.spell1_id).cloned().unwrap_or_default();
            let spell2_icon_url = assets.spells.get(&p.spell2_id).cloned().unwrap_or_default();
            let rune_icon_url = assets.runes.get(&stats.perk0).cloned().unwrap_or_default();

            let aug_ids = dedupe_augment_ids(stats.augments.iter().copied().chain([
                stats.player_augment1,
                stats.player_augment2,
                stats.player_augment3,
                stats.player_augment4,
                stats.player_augment5,
            ]));

            let mut augment_names = Vec::new();
            for &aid in &aug_ids {
                let name = assets
                    .augments
                    .get(&aid)
                    .map(|a| a.name.clone())
                    .unwrap_or_else(|| "海克斯强化".to_string());
                augment_names.push(name);
            }

            let player = GameDetailPlayer {
                participant_id: p.participant_id,
                team_id: p.team_id,
                champion_id: p.champion_id,
                name,
                puuid,
                summoner_id,
                level: stats.champ_level,
                kills: stats.kills,
                deaths: stats.deaths,
                assists: stats.assists,
                cs,
                gold,
                damage,
                win: stats.win,
                spell1_id: p.spell1_id,
                spell2_id: p.spell2_id,
                rune_id: stats.perk0,
                item_ids,
                ward_item_id,
                champion_icon_url,
                spell1_icon_url,
                spell2_icon_url,
                rune_icon_url,
                item_icon_urls,
                ward_icon_url,
                augment_ids: aug_ids,
                augment_names,
            };

            if p.team_id == 100 {
                blue_players.push(player);
            } else {
                red_players.push(player);
            }
        }

        let blue_team_raw = self.teams.iter().find(|t| t.team_id == 100);
        let red_team_raw = self.teams.iter().find(|t| t.team_id == 200);

        let blue_win = blue_team_raw
            .and_then(|t| t.win.as_ref().map(|w| w.is_win()))
            .or_else(|| blue_players.first().map(|p| p.win))
            .unwrap_or(false);

        let red_win = red_team_raw
            .and_then(|t| t.win.as_ref().map(|w| w.is_win()))
            .or_else(|| red_players.first().map(|p| p.win))
            .unwrap_or(false);

        let blue_kills = blue_players.iter().map(|p| p.kills).sum();
        let red_kills = red_players.iter().map(|p| p.kills).sum();

        let blue = GameDetailTeam {
            team_id: 100,
            win: blue_win,
            kills: blue_kills,
            tower_kills: blue_team_raw.map(|t| t.tower_kills).unwrap_or(0),
            inhibitor_kills: blue_team_raw.map(|t| t.inhibitor_kills).unwrap_or(0),
            baron_kills: blue_team_raw.map(|t| t.baron_kills).unwrap_or(0),
            dragon_kills: blue_team_raw.map(|t| t.dragon_kills).unwrap_or(0),
            rift_herald_kills: blue_team_raw.map(|t| t.rift_herald_kills).unwrap_or(0),
            players: blue_players,
        };

        let red = GameDetailTeam {
            team_id: 200,
            win: red_win,
            kills: red_kills,
            tower_kills: red_team_raw.map(|t| t.tower_kills).unwrap_or(0),
            inhibitor_kills: red_team_raw.map(|t| t.inhibitor_kills).unwrap_or(0),
            baron_kills: red_team_raw.map(|t| t.baron_kills).unwrap_or(0),
            dragon_kills: red_team_raw.map(|t| t.dragon_kills).unwrap_or(0),
            rift_herald_kills: red_team_raw.map(|t| t.rift_herald_kills).unwrap_or(0),
            players: red_players,
        };

        GameDetail {
            game_id: self.game_id,
            queue_id: self.queue_id,
            map_id,
            duration,
            date,
            queue_name,
            map_name,
            win: blue_win,
            blue,
            red,
        }
    }
}

/// 从 LCU 获取单局 10 人完整对局详情
pub async fn fetch_game_detail(
    client: &LcuClient,
    game_id: u64,
    assets: &GameDataAssets,
) -> Result<GameDetail, String> {
    let url = format!(
        "https://127.0.0.1:{}/lol-match-history/v1/games/{}",
        client.port, game_id
    );
    let auth = build_auth_header(&client.token);

    let resp = client
        .http_client
        .get(&url)
        .header("Authorization", auth)
        .send()
        .await
        .map_err(|e| format!("LCU 请求失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("获取对局详情失败: HTTP {}", resp.status()));
    }

    let raw: RawGameDetail = resp
        .json()
        .await
        .map_err(|e| format!("解析对局详情 JSON 失败: {}", e))?;

    Ok(raw.to_game_detail(assets))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_real_fixture_top_solo() {
        let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("src-tauri")
            .join("testdata")
            .join("match")
            .join("real")
            .join("top-solo.json");

        if let Ok(content) = std::fs::read_to_string(&fixture_path) {
            let val: serde_json::Value = serde_json::from_str(&content).unwrap();
            let game_val = &val["games"]["games"][0];
            let raw: RawGameDetail = serde_json::from_value(game_val.clone()).unwrap();

            let assets = GameDataAssets::default();
            let detail = raw.to_game_detail(&assets);

            assert_eq!(detail.game_id, 11395344574);
            assert_eq!(detail.queue_id, 430);
            assert_eq!(detail.map_name, "召唤师峡谷");
            assert_eq!(detail.blue.players.len(), 1);
            assert_eq!(detail.blue.players[0].champion_id, 17);
            assert_eq!(detail.blue.players[0].kills, 1);
            assert_eq!(detail.blue.players[0].deaths, 5);
            assert_eq!(detail.blue.players[0].assists, 6);
        }
    }
}
