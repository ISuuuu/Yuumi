//! 最近队友 (RecentTeammate) 聚合与模型
//!
//! 根据近期对局详情 (RawGameDetail)，以当前召唤师 PUUID 为基准，
//! 提取相同阵营的队友，聚合出场次数、胜负场、最后同局时间及标签。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::lcu::client::{build_auth_header, LcuClient};
use crate::parsers::match_detail::RawGameDetail;

/// 最近同队队友简要统计
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentTeammate {
    /// 召唤师名称（包含 tag_line，如 "玩家名#CN1"）
    pub name: String,
    /// 召唤师 PUUID
    pub puuid: String,
    /// 头像资源路径，如 "/lol-game-data/assets/v1/profile-icons/123.jpg"
    pub icon: String,
    /// 共同对局总场次
    pub total: usize,
    /// 共同获胜场次
    pub wins: usize,
    /// 共同落败场次
    pub losses: usize,
    /// 最后同局时间戳 (ms)
    pub last_play_time: u64,
    /// 路人集标记（若存在）
    #[serde(default)]
    pub tag: Option<String>,
}

impl RecentTeammate {
    /// 胜率百分比 (0~100)
    pub fn win_rate(&self) -> u32 {
        (self.wins * 100).checked_div(self.total).unwrap_or(0) as u32
    }
}

/// 纯逻辑：根据给定的对局详情列表与目标玩家 PUUID，聚合最近队友
pub fn aggregate_recent_teammates(
    details: &[RawGameDetail],
    target_puuid: &str,
    tagged_map: &HashMap<String, String>,
) -> Vec<RecentTeammate> {
    if target_puuid.is_empty() || details.is_empty() {
        return Vec::new();
    }

    let mut stats_map: HashMap<String, RecentTeammate> = HashMap::new();

    for detail in details {
        // 1. 查找目标玩家的 participantId
        let target_pid = detail.participant_identities.iter().find_map(|ident| {
            if let Some(player) = &ident.player {
                if player.puuid == target_puuid {
                    return Some(ident.participant_id);
                }
            }
            None
        });

        let target_pid = match target_pid {
            Some(pid) => pid,
            None => continue,
        };

        // 2. 查找目标玩家的阵营标识 (Arena 1700 用 subteam_placement, 其余用 team_id) 及 remake 状态
        let target_participant = match detail
            .participants
            .iter()
            .find(|p| p.participant_id == target_pid)
        {
            Some(p) => p,
            None => continue,
        };

        let is_arena = detail.queue_id == 1700;
        let target_team_key: i64 = if is_arena {
            target_participant.stats.subteam_placement.unwrap_or(0) as i64
        } else {
            target_participant.team_id as i64
        };

        let is_remake = target_participant.stats.game_ended_in_early_surrender;

        // 3. 收集其他同队队友
        for p in &detail.participants {
            if p.participant_id == target_pid {
                continue;
            }

            let p_team_key: i64 = if is_arena {
                p.stats.subteam_placement.unwrap_or(0) as i64
            } else {
                p.team_id as i64
            };

            if p_team_key == target_team_key {
                // 查找该队友的 identity
                let player_info = detail.participant_identities.iter().find_map(|ident| {
                    if ident.participant_id == p.participant_id {
                        ident.player.as_ref()
                    } else {
                        None
                    }
                });

                let player_info = match player_info {
                    Some(info) if !info.puuid.is_empty() => info,
                    _ => continue,
                };

                let mut name = if !player_info.game_name.is_empty() {
                    player_info.game_name.clone()
                } else if !player_info.summoner_name.is_empty() {
                    player_info.summoner_name.clone()
                } else {
                    format!("Player{}", p.participant_id)
                };

                if !player_info.tag_line.is_empty() && !name.is_empty() {
                    name = format!("{}#{}", name, player_info.tag_line);
                }

                let icon = format!(
                    "/lol-game-data/assets/v1/profile-icons/{}.jpg",
                    player_info.profile_icon
                );

                let entry = stats_map
                    .entry(player_info.puuid.clone())
                    .or_insert_with(|| RecentTeammate {
                        name,
                        puuid: player_info.puuid.clone(),
                        icon,
                        total: 0,
                        wins: 0,
                        losses: 0,
                        last_play_time: detail.game_creation,
                        tag: tagged_map.get(&player_info.puuid).cloned(),
                    });

                entry.total += 1;
                if !is_remake {
                    if p.stats.win {
                        entry.wins += 1;
                    } else {
                        entry.losses += 1;
                    }
                }
                if detail.game_creation > entry.last_play_time {
                    entry.last_play_time = detail.game_creation;
                }
            }
        }
    }

    let mut teammates: Vec<RecentTeammate> = stats_map.into_values().collect();
    // 排序：被标记优先，其次总场次降序
    teammates.sort_by(|a, b| {
        let a_tagged = tagged_map.contains_key(&a.puuid);
        let b_tagged = tagged_map.contains_key(&b.puuid);
        b_tagged.cmp(&a_tagged).then_with(|| b.total.cmp(&a.total))
    });
    teammates.truncate(5);

    teammates
}

/// 批量从 LCU 获取多局详情并聚合最近队友（最多 20 局，受控并发）
pub async fn fetch_recent_teammates(
    client: &LcuClient,
    game_ids: &[u64],
    target_puuid: &str,
    tagged_map: &HashMap<String, String>,
) -> Result<Vec<RecentTeammate>, String> {
    if game_ids.is_empty() || target_puuid.is_empty() {
        return Ok(Vec::new());
    }

    let auth = build_auth_header(&client.token);
    let port = client.port;
    let http = client.http_client.clone();

    // 限制单次最多查询 20 局，并发数受控在 5 以内
    let ids: Vec<u64> = game_ids.iter().take(20).copied().collect();
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(5));
    let mut handles = Vec::new();

    for gid in ids {
        let auth = auth.clone();
        let http = http.clone();
        let sem = semaphore.clone();
        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.ok()?;
            let url = format!(
                "https://127.0.0.1:{}/lol-match-history/v1/games/{}",
                port, gid
            );
            let resp = http
                .get(&url)
                .header("Authorization", auth)
                .send()
                .await
                .ok()?;
            if !resp.status().is_success() {
                return None;
            }
            resp.json::<RawGameDetail>().await.ok()
        }));
    }

    let mut details = Vec::new();
    for h in handles {
        if let Ok(Some(detail)) = h.await {
            details.push(detail);
        }
    }

    Ok(aggregate_recent_teammates(
        &details,
        target_puuid,
        tagged_map,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::match_detail::{RawParticipant, RawParticipantIdentity, RawPlayerInfo};
    use crate::parsers::match_parser::LcuMatchStats;

    #[test]
    fn test_aggregate_recent_teammates_basic() {
        let my_puuid = "my-puuid-123";
        let mate_puuid = "mate-puuid-456";
        let enemy_puuid = "enemy-puuid-789";

        let detail = RawGameDetail {
            game_id: 10001,
            queue_id: 420,
            map_id: Some(11),
            game_creation: 1700000000000,
            game_duration: 1800,
            participant_identities: vec![
                RawParticipantIdentity {
                    participant_id: 1,
                    player: Some(RawPlayerInfo {
                        game_name: "MySelf".to_string(),
                        tag_line: "CN1".to_string(),
                        summoner_name: "MySelf".to_string(),
                        puuid: my_puuid.to_string(),
                        summoner_id: 1,
                        profile_icon: 100,
                    }),
                },
                RawParticipantIdentity {
                    participant_id: 2,
                    player: Some(RawPlayerInfo {
                        game_name: "GoodMate".to_string(),
                        tag_line: "CN1".to_string(),
                        summoner_name: "GoodMate".to_string(),
                        puuid: mate_puuid.to_string(),
                        summoner_id: 2,
                        profile_icon: 200,
                    }),
                },
                RawParticipantIdentity {
                    participant_id: 3,
                    player: Some(RawPlayerInfo {
                        game_name: "EnemyGuy".to_string(),
                        tag_line: "CN1".to_string(),
                        summoner_name: "EnemyGuy".to_string(),
                        puuid: enemy_puuid.to_string(),
                        summoner_id: 3,
                        profile_icon: 300,
                    }),
                },
            ],
            participants: vec![
                RawParticipant {
                    participant_id: 1,
                    team_id: 100,
                    champion_id: 1,
                    spell1_id: 4,
                    spell2_id: 14,
                    stats: LcuMatchStats {
                        win: true,
                        kills: 5,
                        deaths: 2,
                        assists: 10,
                        champ_level: 16,
                        item0: 0,
                        item1: 0,
                        item2: 0,
                        item3: 0,
                        item4: 0,
                        item5: 0,
                        item6: 0,
                        perk0: 0,
                        total_minions_killed: None,
                        neutral_minions_killed: None,
                        gold_earned: None,
                        total_damage_dealt_to_champions: None,
                        total_heal: None,
                        game_ended_in_early_surrender: false,
                        subteam_placement: None,
                        augments: vec![],
                        player_augment1: 0,
                        player_augment2: 0,
                        player_augment3: 0,
                        player_augment4: 0,
                        player_augment5: 0,
                        role_bound_item: None,
                    },
                    timeline: None,
                },
                RawParticipant {
                    participant_id: 2,
                    team_id: 100,
                    champion_id: 2,
                    spell1_id: 4,
                    spell2_id: 12,
                    stats: LcuMatchStats {
                        win: true,
                        kills: 10,
                        deaths: 1,
                        assists: 8,
                        champ_level: 16,
                        item0: 0,
                        item1: 0,
                        item2: 0,
                        item3: 0,
                        item4: 0,
                        item5: 0,
                        item6: 0,
                        perk0: 0,
                        total_minions_killed: None,
                        neutral_minions_killed: None,
                        gold_earned: None,
                        total_damage_dealt_to_champions: None,
                        total_heal: None,
                        game_ended_in_early_surrender: false,
                        subteam_placement: None,
                        augments: vec![],
                        player_augment1: 0,
                        player_augment2: 0,
                        player_augment3: 0,
                        player_augment4: 0,
                        player_augment5: 0,
                        role_bound_item: None,
                    },
                    timeline: None,
                },
                RawParticipant {
                    participant_id: 3,
                    team_id: 200,
                    champion_id: 3,
                    spell1_id: 4,
                    spell2_id: 7,
                    stats: LcuMatchStats {
                        win: false,
                        kills: 2,
                        deaths: 8,
                        assists: 3,
                        champ_level: 14,
                        item0: 0,
                        item1: 0,
                        item2: 0,
                        item3: 0,
                        item4: 0,
                        item5: 0,
                        item6: 0,
                        perk0: 0,
                        total_minions_killed: None,
                        neutral_minions_killed: None,
                        gold_earned: None,
                        total_damage_dealt_to_champions: None,
                        total_heal: None,
                        game_ended_in_early_surrender: false,
                        subteam_placement: None,
                        augments: vec![],
                        player_augment1: 0,
                        player_augment2: 0,
                        player_augment3: 0,
                        player_augment4: 0,
                        player_augment5: 0,
                        role_bound_item: None,
                    },
                    timeline: None,
                },
            ],
            teams: vec![],
        };

        let tagged = HashMap::new();
        let res = aggregate_recent_teammates(&[detail], my_puuid, &tagged);

        assert_eq!(res.len(), 1);
        let mate = &res[0];
        assert_eq!(mate.puuid, mate_puuid);
        assert_eq!(mate.name, "GoodMate#CN1");
        assert_eq!(mate.total, 1);
        assert_eq!(mate.wins, 1);
        assert_eq!(mate.losses, 0);
        assert_eq!(mate.win_rate(), 100);
        assert_eq!(mate.icon, "/lol-game-data/assets/v1/profile-icons/200.jpg");
    }
}
