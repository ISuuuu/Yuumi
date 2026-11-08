//! 10 人对局信息视图 (GameInfoView)
//! 纯白水晶极光风格，支持左蓝方与右红方 10 人真实看板
//! 数据由 `yuumi_core::parsers::game_info` 实时构建（段位 / 近期 KDA / 宿命对局）

use crate::components::lcu_image::LcuImage;
use crate::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use yuumi_core::gameflow::team_builder::MapSide;
use yuumi_core::parsers::game_info::{phase_label, GameInfoPlayer, LiveGameInfo};

pub struct GameInfoView;

impl GameInfoView {
    /// 渲染 10 人对局信息总看板
    ///
    /// - `data`：最近一次实时快照；退回大厅后仍保留上一场数据
    /// - `phase`：当前 gameflow 阶段
    /// - `is_loading`：正在并发拉取 10 人数据
    /// - `connected`：LCU 是否已连接
    pub fn render(
        data: Option<&LiveGameInfo>,
        phase: &str,
        is_loading: bool,
        connected: bool,
    ) -> gpui::AnyElement {
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_4()
            .child(Self::render_status_bar(data, phase, is_loading, connected))
            .when_some(data, |this, info| {
                this.child(
                    div()
                        .w_full()
                        .flex()
                        .gap_4()
                        .child(Self::render_team_column(
                            &info.blue_team,
                            MapSide::Blue,
                            info.user_side == Some(MapSide::Blue),
                        ))
                        .child(Self::render_team_column(
                            &info.red_team,
                            MapSide::Red,
                            info.user_side == Some(MapSide::Red),
                        )),
                )
            })
            .when(data.is_none(), |this| {
                this.child(Self::render_empty_state(phase, is_loading, connected))
            })
            .into_any_element()
    }

    /// 顶部状态栏：对局模式、阶段、阵营指示与快照提示
    fn render_status_bar(
        data: Option<&LiveGameInfo>,
        phase: &str,
        is_loading: bool,
        connected: bool,
    ) -> gpui::AnyElement {
        let queue_name = data
            .map(|info| info.queue_name.clone())
            .unwrap_or_else(|| "尚未获取对局".to_string());
        let phase_name = data
            .map(|info| info.phase_label.clone())
            .unwrap_or_else(|| phase_label(phase).to_string());
        let is_active = matches!(
            phase,
            "ChampSelect" | "GameStart" | "InProgress" | "Reconnect"
        );
        let is_blue_my_side = data
            .map(|info| info.user_side == Some(MapSide::Blue))
            .unwrap_or(false);
        let is_red_my_side = data
            .map(|info| info.user_side == Some(MapSide::Red))
            .unwrap_or(false);

        let mut left = div()
            .flex()
            .items_center()
            .gap_3()
            .child(div().size(px(10.0)).rounded_full().bg(if is_loading {
                Theme::GOLD
            } else if connected {
                Theme::AURORA_CYAN
            } else {
                Theme::TEXT_MUTED
            }))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(Theme::TEXT_PRIMARY)
                    .child(queue_name),
            )
            .child(
                div()
                    .px_2()
                    .py(px(2.0))
                    .rounded_md()
                    .bg(Theme::BADGE_BG)
                    .text_xs()
                    .text_color(Theme::TEXT_SECONDARY)
                    .child(phase_name),
            );

        if is_loading {
            left = left.child(
                div()
                    .px_2()
                    .py(px(2.0))
                    .rounded_md()
                    .bg(Theme::ACCENT_LIGHT)
                    .text_xs()
                    .text_color(Theme::ACCENT_PRIMARY)
                    .child("正在拉取 10 人数据…"),
            );
        } else if data.is_some() && !is_active {
            left = left.child(
                div()
                    .px_2()
                    .py(px(2.0))
                    .rounded_md()
                    .bg(Theme::BADGE_BG)
                    .text_xs()
                    .text_color(Theme::TEXT_MUTED)
                    .child("上一场对局快照"),
            );
        }

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
            .child(left)
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
            )
            .into_any_element()
    }

    /// 空态引导：未连接 / 拉取中 / 未进入对局
    fn render_empty_state(phase: &str, is_loading: bool, connected: bool) -> gpui::AnyElement {
        let (title, subtitle) = if !connected {
            (
                "尚未连接到英雄联盟客户端",
                "启动客户端并登录后会自动检测 LCU 进程，无需手动操作。",
            )
        } else if is_loading {
            (
                "正在并发拉取双方 10 名玩家数据",
                "正在读取段位、近 20 场战绩与同局交手记录，请稍候…",
            )
        } else if matches!(phase, "ChampSelect" | "GameStart" | "InProgress") {
            (
                "正在读取当前对局名单",
                "对局已开始，正在等待客户端返回双方参赛者信息。",
            )
        } else {
            (
                "当前未进入对局",
                "进入选人阶段或加载对局后，这里会自动呈现双方 10 人的段位与近期战绩。",
            )
        };

        div()
            .w_full()
            .bg(Theme::CARD_BG)
            .border_1()
            .border_color(Theme::CARD_BORDER)
            .rounded_xl()
            .py_16()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .child(
                div()
                    .size(px(56.0))
                    .rounded_full()
                    .bg(Theme::ACCENT_LIGHT)
                    .border_1()
                    .border_color(Theme::AURORA_CYAN)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .text_color(Theme::ACCENT_PRIMARY)
                    .child("10"),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(Theme::TEXT_PRIMARY)
                    .child(title),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Theme::TEXT_MUTED)
                    .child(subtitle),
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

        let display_name = if player.tag_line.is_empty() {
            player.summoner_name.clone()
        } else {
            format!("{} #{}", player.summoner_name, player.tag_line)
        };

        let mut left_meta = div().flex().items_center().gap_2().child(
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(Theme::TEXT_PRIMARY)
                .child(display_name),
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
        if player.is_bot {
            left_meta = left_meta.child(
                div()
                    .px_2()
                    .py(px(1.0))
                    .rounded_md()
                    .bg(Theme::BADGE_BG)
                    .text_xs()
                    .text_color(Theme::TEXT_MUTED)
                    .child("机器人"),
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
        if let Some(fate) = player.fate.as_ref() {
            let (fg, bg, border) = if fate.ally {
                (Theme::WIN_TEXT, Theme::WIN_BG, Theme::WIN_BORDER)
            } else {
                (Theme::LOSS_TEXT, Theme::LOSS_BG, Theme::LOSS_BORDER)
            };
            left_meta = left_meta.child(
                div()
                    .px_2()
                    .py(px(1.0))
                    .rounded_md()
                    .bg(bg)
                    .border_1()
                    .border_color(border)
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(fg)
                    .child(fate.badge_text()),
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
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .relative()
                            .size(px(46.0))
                            .rounded_lg()
                            .border_2()
                            .border_color(if is_local {
                                side_accent
                            } else {
                                Theme::CARD_BORDER
                            })
                            .overflow_hidden()
                            .child(
                                LcuImage::new(format!(
                                    "/lol-game-data/assets/v1/champion-icons/{}.png",
                                    player.champion_id
                                ))
                                .size(px(46.0))
                                .shape(crate::components::lcu_image::LcuImageShape::Rounded)
                                .placeholder(
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
                                    .absolute()
                                    .bottom_0()
                                    .left_0()
                                    .right_0()
                                    .bg(rgba(0x000000aa))
                                    .py(px(1.0))
                                    .flex()
                                    .justify_center()
                                    .text_xs()
                                    .text_color(rgb(0xffffff))
                                    .child(player.position.clone()),
                            ),
                    )
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
                                        .child(player.champion_name.clone()),
                                )
                                .child(
                                    div()
                                        .px_2()
                                        .py(px(1.0))
                                        .rounded_md()
                                        .bg(Theme::BADGE_BG)
                                        .text_xs()
                                        .text_color(Theme::TEXT_MUTED)
                                        .child(player.rank_display.clone()),
                                ),
                        ),
                    ),
            );

        let mut right_pane = div().flex().items_center().gap_4();

        if player.data_available {
            right_pane = right_pane.child(
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
                            .child(format!(
                                "胜率 {}% ({}-{})",
                                player.win_rate, player.ranked_wins, player.ranked_losses
                            )),
                    )
                    .child(div().text_xs().text_color(Theme::TEXT_MUTED).child(format!(
                        "{:.2} KDA ({:.1}/{:.1}/{:.1})",
                        player.avg_kda, player.avg_kills, player.avg_deaths, player.avg_assists
                    ))),
            );
        } else {
            right_pane = right_pane.child(
                div()
                    .flex()
                    .flex_col()
                    .items_end()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(Theme::TEXT_MUTED)
                            .child("战绩获取中"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::TEXT_MUTED)
                            .child("暂无可见战绩"),
                    ),
            );
        }

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
