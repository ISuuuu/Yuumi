//! 战绩统计与位置/连胜聚合算法（下沉自前端 src/utils/matchStats.ts & matchAnalysis.ts）

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PositionKey {
    Top,
    Jungle,
    Middle,
    Bottom,
    Utility,
}

impl PositionKey {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Top => "TOP",
            Self::Jungle => "JUNGLE",
            Self::Middle => "MIDDLE",
            Self::Bottom => "BOTTOM",
            Self::Utility => "UTILITY",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Top => "上路",
            Self::Jungle => "打野",
            Self::Middle => "中路",
            Self::Bottom => "下路",
            Self::Utility => "辅助",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            Self::Top => "上",
            Self::Jungle => "野",
            Self::Middle => "中",
            Self::Bottom => "下",
            Self::Utility => "辅",
        }
    }
}

pub fn normalize_position(pos: Option<&str>) -> Option<PositionKey> {
    let p = pos?.trim();
    if p.is_empty() {
        return None;
    }
    match p.to_uppercase().as_str() {
        "TOP" => Some(PositionKey::Top),
        "JUNGLE" | "JUG" => Some(PositionKey::Jungle),
        "MIDDLE" | "MID" => Some(PositionKey::Middle),
        "BOTTOM" | "BOT" | "ADC" => Some(PositionKey::Bottom),
        "UTILITY" | "SUPPORT" | "SUP" => Some(PositionKey::Utility),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StreakType {
    Win,
    Loss,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreakInfo {
    pub streak_type: StreakType,
    pub count: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PositionStats {
    pub position: Option<PositionKey>,
    pub count: u32,
    pub percentage: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AggregatedPositions {
    pub total_valid: u32,
    pub primary_position: Option<PositionKey>,
    pub primary_percentage: u32,
    pub secondary_position: Option<PositionKey>,
    pub secondary_percentage: u32,
    pub counts: HashMap<PositionKey, u32>,
    pub percentages: HashMap<PositionKey, u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChampionAggregation {
    pub champion_id: i64,
    pub games: u32,
    pub wins: u32,
    pub losses: u32,
    pub win_rate: u32,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub kda: f64,
    pub primary_position: Option<PositionKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputedMatchStats {
    pub total_matches: usize,
    pub valid_matches: usize,
    pub remake_count: usize,
    pub win_count: usize,
    pub loss_count: usize,
    pub win_rate: u32,
    pub avg_kda: f64,
    pub streak: Option<StreakInfo>,
    pub positions: AggregatedPositions,
    pub top_champions: Vec<ChampionAggregation>,
}

pub trait MatchLike {
    fn is_remake(&self) -> bool;
    fn is_win(&self) -> bool;
    fn kills(&self) -> u32;
    fn deaths(&self) -> u32;
    fn assists(&self) -> u32;
    fn position(&self) -> Option<&str>;
    fn champion_id(&self) -> i64;
}

pub fn compute_match_stats<M: MatchLike>(matches: &[M]) -> ComputedMatchStats {
    let mut total_kills = 0u32;
    let mut total_deaths = 0u32;
    let mut total_assists = 0u32;
    let mut remake_count = 0usize;
    let mut current_win_count = 0usize;
    let mut current_loss_count = 0usize;

    for m in matches {
        if m.is_remake() {
            remake_count += 1;
        } else {
            total_kills += m.kills();
            total_deaths += m.deaths();
            total_assists += m.assists();
            if m.is_win() {
                current_win_count += 1;
            } else {
                current_loss_count += 1;
            }
        }
    }

    let valid_matches = matches.len().saturating_sub(remake_count);
    let win_rate = if valid_matches > 0 {
        ((current_win_count as f64 / valid_matches as f64) * 100.0).round() as u32
    } else {
        0
    };

    let deaths_for_calc = if total_deaths == 0 {
        1.0
    } else {
        total_deaths as f64
    };
    let avg_kda = (total_kills + total_assists) as f64 / deaths_for_calc;

    // 连胜/连败计算（排除重开）
    let mut streak: Option<StreakInfo> = None;
    let valid_streak_matches: Vec<&M> = matches.iter().filter(|m| !m.is_remake()).collect();
    if !valid_streak_matches.is_empty() {
        let first_win = valid_streak_matches[0].is_win();
        let mut count = 0u32;
        for m in &valid_streak_matches {
            if m.is_win() == first_win {
                count += 1;
            } else {
                break;
            }
        }
        if count >= 2 {
            streak = Some(StreakInfo {
                streak_type: if first_win {
                    StreakType::Win
                } else {
                    StreakType::Loss
                },
                count,
            });
        }
    }

    // 分路位置聚合
    let mut pos_counts: HashMap<PositionKey, u32> = HashMap::new();
    let mut total_valid_positions = 0u32;

    for m in matches {
        if let Some(pos) = normalize_position(m.position()) {
            *pos_counts.entry(pos).or_insert(0) += 1;
            total_valid_positions += 1;
        }
    }

    let mut pos_percentages: HashMap<PositionKey, u32> = HashMap::new();
    let mut sorted_positions: Vec<(PositionKey, u32)> = pos_counts
        .iter()
        .map(|(&pos, &count)| (pos, count))
        .collect();
    sorted_positions.sort_by_key(|b| std::cmp::Reverse(b.1));

    if total_valid_positions > 0 {
        for (pos, count) in &pos_counts {
            let pct = ((*count as f64 / total_valid_positions as f64) * 100.0).round() as u32;
            pos_percentages.insert(*pos, pct);
        }
    }

    let (primary_position, primary_percentage) = sorted_positions
        .first()
        .map(|(pos, count)| {
            (
                *pos,
                if total_valid_positions > 0 {
                    ((*count as f64 / total_valid_positions as f64) * 100.0).round() as u32
                } else {
                    0
                },
            )
        })
        .map(|(p, pct)| (Some(p), pct))
        .unwrap_or((None, 0));

    let (secondary_position, secondary_percentage) = sorted_positions
        .get(1)
        .map(|(pos, count)| {
            (
                *pos,
                if total_valid_positions > 0 {
                    ((*count as f64 / total_valid_positions as f64) * 100.0).round() as u32
                } else {
                    0
                },
            )
        })
        .map(|(p, pct)| (Some(p), pct))
        .unwrap_or((None, 0));

    let positions = AggregatedPositions {
        total_valid: total_valid_positions,
        primary_position,
        primary_percentage,
        secondary_position,
        secondary_percentage,
        counts: pos_counts,
        percentages: pos_percentages,
    };

    // 常用英雄聚合（前 5 个）
    type ChampAgg = (u32, u32, u32, u32, u32, HashMap<PositionKey, u32>);
    let mut champ_map: HashMap<i64, ChampAgg> = HashMap::new();
    for m in matches {
        if m.is_remake() {
            continue;
        }
        let cid = m.champion_id();
        if cid <= 0 {
            continue;
        }
        let entry = champ_map
            .entry(cid)
            .or_insert((0, 0, 0, 0, 0, HashMap::new()));
        entry.0 += 1; // games
        if m.is_win() {
            entry.1 += 1; // wins
        } else {
            entry.2 += 1; // losses
        }
        entry.3 += m.kills();
        entry.4 += m.deaths();
        entry
            .5
            .entry(normalize_position(m.position()).unwrap_or(PositionKey::Middle))
            .and_modify(|c| *c += 1)
            .or_insert(1);
    }

    let mut champ_aggregations: Vec<ChampionAggregation> = champ_map
        .into_iter()
        .map(|(cid, (games, wins, losses, kills, deaths, pos_map))| {
            let win_rate = if games > 0 {
                ((wins as f64 / games as f64) * 100.0).round() as u32
            } else {
                0
            };
            let d_calc = if deaths == 0 { 1.0 } else { deaths as f64 };
            let kda = (kills as f64) / d_calc;
            let mut top_pos: Vec<(PositionKey, u32)> = pos_map.into_iter().collect();
            top_pos.sort_by_key(|b| std::cmp::Reverse(b.1));
            let primary_pos = top_pos.first().map(|(p, _)| *p);

            ChampionAggregation {
                champion_id: cid,
                games,
                wins,
                losses,
                win_rate,
                kills,
                deaths,
                assists: 0,
                kda,
                primary_position: primary_pos,
            }
        })
        .collect();

    champ_aggregations.sort_by(|a, b| {
        b.games
            .cmp(&a.games)
            .then_with(|| b.win_rate.cmp(&a.win_rate))
    });
    champ_aggregations.truncate(5);

    ComputedMatchStats {
        total_matches: matches.len(),
        valid_matches,
        remake_count,
        win_count: current_win_count,
        loss_count: current_loss_count,
        win_rate,
        avg_kda,
        streak,
        positions,
        top_champions: champ_aggregations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SimpleMatch {
        remake: bool,
        win: bool,
        k: u32,
        d: u32,
        a: u32,
        pos: Option<&'static str>,
        cid: i64,
    }

    impl MatchLike for SimpleMatch {
        fn is_remake(&self) -> bool {
            self.remake
        }
        fn is_win(&self) -> bool {
            self.win
        }
        fn kills(&self) -> u32 {
            self.k
        }
        fn deaths(&self) -> u32 {
            self.d
        }
        fn assists(&self) -> u32 {
            self.a
        }
        fn position(&self) -> Option<&str> {
            self.pos
        }
        fn champion_id(&self) -> i64 {
            self.cid
        }
    }

    #[test]
    fn test_compute_match_stats_basic() {
        let matches = vec![
            SimpleMatch {
                remake: false,
                win: true,
                k: 10,
                d: 2,
                a: 5,
                pos: Some("MID"),
                cid: 103,
            },
            SimpleMatch {
                remake: false,
                win: true,
                k: 5,
                d: 1,
                a: 8,
                pos: Some("MIDDLE"),
                cid: 103,
            },
            SimpleMatch {
                remake: false,
                win: false,
                k: 2,
                d: 5,
                a: 3,
                pos: Some("TOP"),
                cid: 266,
            },
            SimpleMatch {
                remake: true,
                win: false,
                k: 0,
                d: 0,
                a: 0,
                pos: None,
                cid: 103,
            },
        ];

        let stats = compute_match_stats(&matches);
        assert_eq!(stats.total_matches, 4);
        assert_eq!(stats.valid_matches, 3);
        assert_eq!(stats.remake_count, 1);
        assert_eq!(stats.win_count, 2);
        assert_eq!(stats.loss_count, 1);
        assert_eq!(stats.win_rate, 67); // 2/3 = 66.67% -> 67%
        assert_eq!(stats.streak.unwrap().streak_type, StreakType::Win);
        assert_eq!(stats.positions.primary_position, Some(PositionKey::Middle));
    }
}
