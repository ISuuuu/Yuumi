//! 10 人对局信息视图 (GameInfoView)
//! 纯白水晶极光风格，支持左/上蓝方与右/下红方 10 人看板
//! 深度复用 yuumi-core 的阵营推断、身份门禁与战绩统计算法

use crate::theme::Theme;
use gpui::*;
use yuumi_core::gameflow::match_stats::{compute_match_stats, MatchLike};
use yuumi_core::gameflow::player_identity::{
    get_valid_tpid, is_identity_compatible, EntryIdentity, IdentityPayload,
};
use yuumi_core::gameflow::team_builder::{
    resolve_team_side, MapSide, SnapshotParticipant, TeamBuildContext,
};

/// 简化对局样本（实现 MatchLike 接口以便计算统计数据）
#[derive(Debug, Clone)]
pub struct SimplePlayerMatch {
    pub is_remake: bool,
    pub is_win: bool,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub position: Option<String>,
    pub champion_id: i64,
}

impl MatchLike for SimplePlayerMatch {
    fn is_remake(&self) -> bool {
        self.is_remake
    }

    fn is_win(&self) -> bool {
        self.is_win
    }

    fn kills(&self) -> u32 {
        self.kills
    }

    fn deaths(&self) -> u32 {
        self.deaths
    }

    fn assists(&self) -> u32 {
        self.assists
    }

    fn position(&self) -> Option<&str> {
        self.position.as_deref()
    }

    fn champion_id(&self) -> i64 {
        self.champion_id
    }
}

/// 玩家排位与个人近期数据
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct GameInfoPlayer {
    pub puuid: String,
    pub summoner_id: i64,
    pub cell_id: i64,
    pub summoner_name: String,
    pub tag_line: String,
    pub champion_id: i64,
    pub champion_name: String,
    pub champion_title: String,
    pub rank_tier: String,
    pub rank_division: String,
    pub rank_lp: u32,
    pub team_participant_id: Option<String>,
    pub party_id: Option<String>,
    pub premade_group_id: Option<usize>,
    pub win_rate: u32,
    pub avg_kda: f64,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub streak_text: Option<String>,
    pub streak_is_win: bool,
    pub is_local_player: bool,
    pub role: &'static str,
}

/// 10 人对局信息视图模型
#[derive(Debug, Clone)]
pub struct GameInfoData {
    pub blue_team: Vec<GameInfoPlayer>,
    pub red_team: Vec<GameInfoPlayer>,
    pub user_side: Option<MapSide>,
    pub side_matched: bool,
    pub queue_name: String,
}

#[allow(clippy::too_many_arguments)]
fn make_mock_player(
    puuid: &str,
    summoner_id: i64,
    cell_id: i64,
    name: &str,
    tag: &str,
    champ_id: i64,
    champ_name: &str,
    champ_title: &str,
    tier: &str,
    division: &str,
    lp: u32,
    role: &'static str,
    premade: Option<usize>,
    tpid: Option<String>,
    wins: usize,
    losses: usize,
    k: u32,
    d: u32,
    a: u32,
    is_local: bool,
) -> GameInfoPlayer {
    let mut matches = Vec::new();
    for _ in 0..wins {
        matches.push(SimplePlayerMatch {
            is_remake: false,
            is_win: true,
            kills: k,
            deaths: d,
            assists: a,
            position: Some("MIDDLE".into()),
            champion_id: champ_id,
        });
    }
    for _ in 0..losses {
        matches.push(SimplePlayerMatch {
            is_remake: false,
            is_win: false,
            kills: k.saturating_sub(2),
            deaths: d + 2,
            assists: a,
            position: Some("MIDDLE".into()),
            champion_id: champ_id,
        });
    }
    let computed = compute_match_stats(&matches);
    let streak_text = computed.streak.as_ref().map(|s| {
        format!(
            "{}{}",
            s.count,
            match s.streak_type {
                yuumi_core::gameflow::match_stats::StreakType::Win => "连胜",
                yuumi_core::gameflow::match_stats::StreakType::Loss => "连败",
            }
        )
    });
    let streak_is_win = computed
        .streak
        .as_ref()
        .is_some_and(|s| s.streak_type == yuumi_core::gameflow::match_stats::StreakType::Win);

    GameInfoPlayer {
        puuid: puuid.into(),
        summoner_id,
        cell_id,
        summoner_name: name.into(),
        tag_line: tag.into(),
        champion_id: champ_id,
        champion_name: champ_name.into(),
        champion_title: champ_title.into(),
        rank_tier: tier.into(),
        rank_division: division.into(),
        rank_lp: lp,
        team_participant_id: tpid,
        party_id: None,
        premade_group_id: premade,
        win_rate: computed.win_rate,
        avg_kda: computed.avg_kda,
        kills: k,
        deaths: d,
        assists: a,
        streak_text,
        streak_is_win,
        is_local_player: is_local,
        role,
    }
}

/// 生成 10 人对局模拟数据
pub fn load_mock_game_info() -> GameInfoData {
    // 构造对局双方参赛者快照
    let blue_snapshots = vec![
        SnapshotParticipant {
            puuid: Some("puuid-blue-top"),
            summoner_id: Some(10001),
            summoner_name: Some("寒霜之刃"),
            game_name: Some("寒霜之刃"),
            cell_id: Some(0),
            ..Default::default()
        },
        SnapshotParticipant {
            puuid: Some("puuid-blue-jug"),
            summoner_id: Some(10002),
            summoner_name: Some("暗影猎犬"),
            game_name: Some("暗影猎犬"),
            cell_id: Some(1),
            ..Default::default()
        },
        SnapshotParticipant {
            puuid: Some("puuid-local-mid"),
            summoner_id: Some(10003),
            summoner_name: Some("悠米本米"),
            game_name: Some("悠米本米"),
            cell_id: Some(2),
            ..Default::default()
        },
        SnapshotParticipant {
            puuid: Some("puuid-blue-adc"),
            summoner_id: Some(10004),
            summoner_name: Some("金克丝超闪"),
            game_name: Some("金克丝超闪"),
            cell_id: Some(3),
            ..Default::default()
        },
        SnapshotParticipant {
            puuid: Some("puuid-blue-sup"),
            summoner_id: Some(10005),
            summoner_name: Some("时光守护者"),
            game_name: Some("时光守护者"),
            cell_id: Some(4),
            ..Default::default()
        },
    ];

    let red_snapshots = vec![
        SnapshotParticipant {
            puuid: Some("puuid-red-top"),
            summoner_id: Some(20001),
            summoner_name: Some("诺克萨斯之手"),
            game_name: Some("诺克萨斯之手"),
            cell_id: Some(5),
            ..Default::default()
        },
        SnapshotParticipant {
            puuid: Some("puuid-red-jug"),
            summoner_id: Some(20002),
            summoner_name: Some("盲僧小王"),
            game_name: Some("盲僧小王"),
            cell_id: Some(6),
            ..Default::default()
        },
        SnapshotParticipant {
            puuid: Some("puuid-red-mid"),
            summoner_id: Some(20003),
            summoner_name: Some("九尾妖狐"),
            game_name: Some("九尾妖狐"),
            cell_id: Some(7),
            ..Default::default()
        },
        SnapshotParticipant {
            puuid: Some("puuid-red-adc"),
            summoner_id: Some(20004),
            summoner_name: Some("狂暴圣枪"),
            game_name: Some("狂暴圣枪"),
            cell_id: Some(8),
            ..Default::default()
        },
        SnapshotParticipant {
            puuid: Some("puuid-red-sup"),
            summoner_id: Some(20005),
            summoner_name: Some("幻翎洛洛"),
            game_name: Some("幻翎洛洛"),
            cell_id: Some(9),
            ..Default::default()
        },
    ];

    let ctx = TeamBuildContext {
        current_summoner_id: Some(10003),
        current_summoner_puuid: Some("puuid-local-mid"),
        map_side: None,
        champ_select_team_snapshot: &blue_snapshots,
    };

    // 1. 使用 yuumi-core 阵营推断算法
    let (user_side, side_matched) = resolve_team_side(&blue_snapshots, &red_snapshots, &ctx);

    // 2. 身份门禁核验
    let entry = EntryIdentity {
        puuid: Some("puuid-local-mid"),
        summoner_id: Some(10003),
    };
    let payload = IdentityPayload {
        puuid: Some("puuid-local-mid"),
        summoner_id: Some(10003),
        cell_id: Some(2),
    };
    let _ = is_identity_compatible(Some(entry), payload);

    // 3. 组队高亮提取：蓝方两人开黑 (tpid = party-blue-duo)，红方三人开黑 (tpid = party-red-trio)
    let tpid_blue_duo = get_valid_tpid(Some("party-blue-duo"), None);
    let tpid_red_trio = get_valid_tpid(Some("party-red-trio"), None);

    let blue_team = vec![
        make_mock_player(
            "puuid-blue-top",
            10001,
            0,
            "寒霜之刃",
            "CN1",
            84,
            "阿卡丽",
            "离群之刺",
            "宗师",
            "I",
            485,
            "上单",
            None,
            None,
            14,
            6,
            7,
            3,
            5,
            false,
        ),
        make_mock_player(
            "puuid-blue-jug",
            10002,
            1,
            "暗影猎犬",
            "001",
            121,
            "卡兹克",
            "虚空掠夺者",
            "大师",
            "I",
            120,
            "打野",
            None,
            None,
            12,
            8,
            5,
            4,
            8,
            false,
        ),
        make_mock_player(
            "puuid-local-mid",
            10003,
            2,
            "悠米本米",
            "GPUI",
            350,
            "悠米",
            "魔法猫咪",
            "王者",
            "I",
            890,
            "中单",
            None,
            None,
            16,
            4,
            9,
            2,
            7,
            true,
        ),
        make_mock_player(
            "puuid-blue-adc",
            10004,
            3,
            "金克丝超闪",
            "ADC",
            222,
            "金克丝",
            "暴走萝莉",
            "大师",
            "I",
            230,
            "下路",
            Some(0),
            tpid_blue_duo.clone(),
            15,
            5,
            11,
            4,
            6,
            false,
        ),
        make_mock_player(
            "puuid-blue-sup",
            10005,
            4,
            "时光守护者",
            "SUP",
            26,
            "基兰",
            "时光守护者",
            "大师",
            "I",
            180,
            "辅助",
            Some(0),
            tpid_blue_duo,
            13,
            7,
            2,
            3,
            16,
            false,
        ),
    ];

    let red_team = vec![
        make_mock_player(
            "puuid-red-top",
            20001,
            5,
            "诺克萨斯之手",
            "KR1",
            122,
            "德莱厄斯",
            "诺克萨斯之手",
            "宗师",
            "I",
            512,
            "上单",
            Some(1),
            tpid_red_trio.clone(),
            11,
            9,
            8,
            5,
            4,
            false,
        ),
        make_mock_player(
            "puuid-red-jug",
            20002,
            6,
            "盲僧小王",
            "JUG",
            64,
            "李青",
            "盲僧",
            "宗师",
            "I",
            620,
            "打野",
            Some(1),
            tpid_red_trio.clone(),
            13,
            7,
            7,
            4,
            9,
            false,
        ),
        make_mock_player(
            "puuid-red-mid",
            20003,
            7,
            "九尾妖狐",
            "MID",
            103,
            "阿狸",
            "九尾妖狐",
            "王者",
            "I",
            780,
            "中单",
            Some(1),
            tpid_red_trio,
            14,
            6,
            8,
            3,
            8,
            false,
        ),
        make_mock_player(
            "puuid-red-adc",
            20004,
            8,
            "狂暴圣枪",
            "BOT",
            236,
            "卢锡安",
            "圣枪游侠",
            "大师",
            "I",
            340,
            "下路",
            None,
            None,
            10,
            10,
            9,
            6,
            5,
            false,
        ),
        make_mock_player(
            "puuid-red-sup",
            20005,
            9,
            "幻翎洛洛",
            "RAK",
            497,
            "洛",
            "幻翎",
            "大师",
            "I",
            210,
            "辅助",
            None,
            None,
            9,
            11,
            1,
            5,
            14,
            false,
        ),
    ];

    GameInfoData {
        blue_team,
        red_team,
        user_side,
        side_matched,
        queue_name: "单双排位赛 5v5 (召唤师峡谷)".into(),
    }
}

pub struct GameInfoView;

impl GameInfoView {
    /// 渲染 10 人对局信息总看板
    pub fn render(data: &GameInfoData) -> gpui::AnyElement {
        let is_blue_my_side = data.user_side == Some(MapSide::Blue);
        let is_red_my_side = data.user_side == Some(MapSide::Red);

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_4()
            // 顶部状态栏：对局模式与阵营指示
            .child(
                div()
                    .w_full()
                    .bg(Theme::CARD_BG)
                    .border_1()
                    .border_color(Theme::CARD_BORDER)
                    .rounded_xl()
                    .px_4()
                    .py_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(div().size(px(10.0)).rounded_full().bg(Theme::AURORA_CYAN))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child(data.queue_name.clone()),
                            )
                            .child(
                                div()
                                    .px_2()
                                    .py(px(2.0))
                                    .rounded_md()
                                    .bg(Theme::BADGE_BG)
                                    .text_xs()
                                    .text_color(Theme::TEXT_SECONDARY)
                                    .child(if data.side_matched {
                                        "精准推断自身对局"
                                    } else {
                                        "脱敏槽位推断"
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_4()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .size(px(8.0))
                                            .rounded_full()
                                            .bg(Theme::BLUE_SIDE_ACCENT),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(Theme::TEXT_SECONDARY)
                                            .child(if is_blue_my_side {
                                                "蓝方 (我方队伍)"
                                            } else {
                                                "蓝方队伍"
                                            }),
                                    ),
                            )
                            .child(div().text_xs().text_color(Theme::TEXT_MUTED).child("VS"))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .size(px(8.0))
                                            .rounded_full()
                                            .bg(Theme::RED_SIDE_ACCENT),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(Theme::TEXT_SECONDARY)
                                            .child(if is_red_my_side {
                                                "红方 (我方队伍)"
                                            } else {
                                                "红方队伍"
                                            }),
                                    ),
                            ),
                    ),
            )
            // 10 人卡片双列对照排布 (左蓝右红)
            .child(
                div()
                    .w_full()
                    .flex()
                    .gap_4()
                    // 蓝方 5 人列表
                    .child(Self::render_team_column(
                        &data.blue_team,
                        MapSide::Blue,
                        is_blue_my_side,
                    ))
                    // 红方 5 人列表
                    .child(Self::render_team_column(
                        &data.red_team,
                        MapSide::Red,
                        is_red_my_side,
                    )),
            )
            .into_any_element()
    }

    /// 渲染单侧队伍 5 人卡片纵向排布
    fn render_team_column(
        players: &[GameInfoPlayer],
        side: MapSide,
        is_my_team: bool,
    ) -> gpui::AnyElement {
        let (title_text, side_accent, side_bg, side_border) = match side {
            MapSide::Blue => (
                if is_my_team {
                    "蓝方阵营 (友方)"
                } else {
                    "蓝方阵营"
                },
                Theme::BLUE_SIDE_ACCENT,
                Theme::BLUE_SIDE_BG,
                Theme::BLUE_SIDE_BORDER,
            ),
            MapSide::Red => (
                if is_my_team {
                    "红方阵营 (友方)"
                } else {
                    "红方阵营 (敌方)"
                },
                Theme::RED_SIDE_ACCENT,
                Theme::RED_SIDE_BG,
                Theme::RED_SIDE_BORDER,
            ),
        };

        div()
            .flex_1()
            .flex()
            .flex_col()
            .gap_3()
            // 阵营标题条
            .child(
                div()
                    .w_full()
                    .px_3()
                    .py_2()
                    .rounded_lg()
                    .bg(side_bg)
                    .border_1()
                    .border_color(side_border)
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().size(px(8.0)).rounded_full().bg(side_accent))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child(title_text),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::TEXT_MUTED)
                            .child(format!("共 {} 名选手", players.len())),
                    ),
            )
            // 5 位玩家卡片
            .children(
                players
                    .iter()
                    .map(|p| Self::render_player_card(p, side_accent)),
            )
            .into_any_element()
    }

    /// 渲染单个玩家精细卡片
    fn render_player_card(player: &GameInfoPlayer, side_accent: Hsla) -> gpui::AnyElement {
        let is_local = player.is_local_player;
        let premade_style = player.premade_group_id.map(|idx| {
            let color = Theme::premade_color(idx);
            (color, format!("组队 {}", idx + 1))
        });

        let mut left_meta = div().flex().items_center().gap_2();
        left_meta = left_meta.child(
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(Theme::TEXT_PRIMARY)
                .child(format!("{} #{}", player.summoner_name, player.tag_line)),
        );
        if is_local {
            left_meta = left_meta.child(
                div()
                    .px_2()
                    .py(px(1.0))
                    .rounded_md()
                    .bg(side_accent)
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .child("本机"),
            );
        }
        if let Some((color, tag)) = premade_style {
            left_meta = left_meta.child(
                div()
                    .px_2()
                    .py(px(1.0))
                    .rounded_md()
                    .bg(Theme::BADGE_BG)
                    .border_1()
                    .border_color(color)
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(color)
                    .child(tag),
            );
        }

        let mut card = div()
            .w_full()
            .bg(if is_local {
                Theme::CARD_BG_HOVER
            } else {
                Theme::CARD_BG
            })
            .border_1()
            .border_color(if is_local {
                side_accent
            } else {
                Theme::CARD_BORDER
            })
            .rounded_xl()
            .p_3()
            .flex()
            .items_center()
            .justify_between()
            .hover(|s| s.bg(Theme::CARD_BG_HOVER))
            // 左半部分：英雄头像、名称、分路、召唤师名与排位
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    // 英雄头像 / 占位
                    .child(
                        div()
                            .size(px(46.0))
                            .rounded_lg()
                            .bg(Theme::BADGE_BG)
                            .border_2()
                            .border_color(if is_local {
                                side_accent
                            } else {
                                Theme::CARD_BORDER
                            })
                            .overflow_hidden()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child(
                                        player
                                            .champion_name
                                            .chars()
                                            .next()
                                            .unwrap_or('?')
                                            .to_string(),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::TEXT_MUTED)
                                    .child(player.role),
                            ),
                    )
                    // 玩家文本信息
                    .child(
                        div().flex().flex_col().gap_1().child(left_meta).child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(Theme::TEXT_SECONDARY)
                                        .child(format!(
                                            "{} · {}",
                                            player.champion_name, player.champion_title
                                        )),
                                )
                                .child(
                                    div()
                                        .px_2()
                                        .py(px(1.0))
                                        .rounded_md()
                                        .bg(Theme::BADGE_BG)
                                        .text_xs()
                                        .text_color(Theme::TEXT_MUTED)
                                        .child(format!(
                                            "{} {} ({} LP)",
                                            player.rank_tier, player.rank_division, player.rank_lp
                                        )),
                                ),
                        ),
                    ),
            );

        let mut right_pane = div().flex().items_center().gap_4().child(
            div()
                .flex()
                .flex_col()
                .items_end()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(if player.win_rate >= 50 {
                            Theme::WIN_TEXT
                        } else {
                            Theme::LOSS_TEXT
                        })
                        .child(format!("胜率 {}%", player.win_rate)),
                )
                .child(div().text_xs().text_color(Theme::TEXT_MUTED).child(format!(
                    "{:.2} KDA ({}/{}/{})",
                    player.avg_kda, player.kills, player.deaths, player.assists
                ))),
        );

        if let Some(streak) = player.streak_text.clone() {
            right_pane = right_pane.child(
                div()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(if player.streak_is_win {
                        Theme::WIN_BG
                    } else {
                        Theme::LOSS_BG
                    })
                    .border_1()
                    .border_color(if player.streak_is_win {
                        Theme::WIN_BORDER
                    } else {
                        Theme::LOSS_BORDER
                    })
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(if player.streak_is_win {
                        Theme::WIN_TEXT
                    } else {
                        Theme::LOSS_TEXT
                    })
                    .child(streak),
            );
        }

        card = card.child(right_pane);
        card.into_any_element()
    }
}
