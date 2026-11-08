//! 对局阵营判定与红蓝方队伍映射（下沉自前端 src/utils/teamBuilder.ts）

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MapSide {
    Blue,
    Red,
}

impl MapSide {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Blue => "blue",
            Self::Red => "red",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SnapshotParticipant<'a> {
    pub puuid: Option<&'a str>,
    pub summoner_id: Option<i64>,
    pub summoner_name: Option<&'a str>,
    pub game_name: Option<&'a str>,
    pub display_name: Option<&'a str>,
    pub cell_id: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct TeamBuildContext<'a> {
    pub current_summoner_id: Option<i64>,
    pub current_summoner_puuid: Option<&'a str>,
    pub map_side: Option<MapSide>,
    pub champ_select_team_snapshot: &'a [SnapshotParticipant<'a>],
}

pub fn matches_participant<'a>(p: &SnapshotParticipant<'a>, ctx: &TeamBuildContext<'a>) -> bool {
    let match_local = (ctx.current_summoner_id.is_some()
        && p.summoner_id == ctx.current_summoner_id)
        || (ctx.current_summoner_puuid.is_some() && p.puuid == ctx.current_summoner_puuid);
    if match_local {
        return true;
    }

    ctx.champ_select_team_snapshot.iter().any(|cs| {
        (p.puuid.is_some() && cs.puuid.is_some() && p.puuid == cs.puuid)
            || (p.summoner_id.is_some()
                && cs.summoner_id.is_some()
                && p.summoner_id == cs.summoner_id)
            || (p.summoner_name.is_some()
                && cs.display_name.is_some()
                && p.summoner_name == cs.display_name)
            || (p.game_name.is_some() && cs.game_name.is_some() && p.game_name == cs.game_name)
    })
}

pub fn resolve_team_side<'a>(
    team_one: &'a [SnapshotParticipant<'a>],
    team_two: &'a [SnapshotParticipant<'a>],
    ctx: &TeamBuildContext<'a>,
) -> (Option<MapSide>, bool) {
    let is_team_one = team_one.iter().any(|p| matches_participant(p, ctx));
    let is_team_two = team_two.iter().any(|p| matches_participant(p, ctx));

    if is_team_one {
        return (Some(MapSide::Blue), true);
    }
    if is_team_two {
        return (Some(MapSide::Red), true);
    }

    // 脱敏推断：根据选人快照中的 cellId 推断（0..4 蓝方，5..9 红方）
    let mut determined_side = ctx.map_side;
    if determined_side.is_none() && !ctx.champ_select_team_snapshot.is_empty() {
        let has_red_cell = ctx
            .champ_select_team_snapshot
            .iter()
            .any(|cs| cs.cell_id.unwrap_or(-1) >= 5);
        let has_blue_cell = ctx.champ_select_team_snapshot.iter().any(|cs| {
            let cid = cs.cell_id.unwrap_or(-1);
            (0..5).contains(&cid)
        });

        if has_red_cell && !has_blue_cell {
            determined_side = Some(MapSide::Red);
        } else if has_blue_cell && !has_red_cell {
            determined_side = Some(MapSide::Blue);
        }
    }

    (determined_side, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_team_side_direct_match() {
        let current_puuid = "puuid-my";
        let team_one = vec![
            SnapshotParticipant {
                puuid: Some("puuid-my"),
                ..Default::default()
            },
            SnapshotParticipant {
                puuid: Some("puuid-2"),
                ..Default::default()
            },
        ];
        let team_two = vec![SnapshotParticipant {
            puuid: Some("puuid-enemy"),
            ..Default::default()
        }];

        let ctx = TeamBuildContext {
            current_summoner_puuid: Some(current_puuid),
            ..Default::default()
        };

        let (side, matched) = resolve_team_side(&team_one, &team_two, &ctx);
        assert_eq!(side, Some(MapSide::Blue));
        assert!(matched);
    }

    #[test]
    fn test_resolve_team_side_cell_id_infer() {
        let team_one = vec![SnapshotParticipant {
            puuid: Some("anon-1"),
            ..Default::default()
        }];
        let team_two = vec![SnapshotParticipant {
            puuid: Some("anon-2"),
            ..Default::default()
        }];

        let snapshot = vec![
            SnapshotParticipant {
                cell_id: Some(6),
                ..Default::default()
            },
            SnapshotParticipant {
                cell_id: Some(7),
                ..Default::default()
            },
        ];

        let ctx = TeamBuildContext {
            champ_select_team_snapshot: &snapshot,
            ..Default::default()
        };

        let (side, matched) = resolve_team_side(&team_one, &team_two, &ctx);
        assert_eq!(side, Some(MapSide::Red));
        assert!(!matched);
    }
}
