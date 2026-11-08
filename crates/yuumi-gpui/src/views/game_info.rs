//! 10 人对局信息视图 (GameInfoView)
//! 纯白水晶极光风格，完整复刻 Seraphine / Yuumi 原版：
//! 1. 顶部控制栏：友方/敌方队伍胶囊、英雄阵容头像组、VS徽标、演示预览指示、按队伍(5列)/全部战绩(10列)视图切换
//! 2. 按队伍视图 (ByTeam)：左侧 260px 玩家详情大卡片 (头像、位置、段位、胜率、场均KDA、常用英雄) + 右侧 5 列竖向战绩流
//! 3. 全部战绩视图 (AllMatches)：全屏 10 列横向排列战绩流 (左 5 友方，右 5 敌方)
//! 4. 离线/非对局状态提供「预览对局信息」主入口，优先还原真实 10 人对局快照，无快照时加载高保真演示数据

use crate::components::lcu_image::{LcuImage, LcuImageShape};
use crate::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use yuumi_core::gameflow::team_builder::MapSide;
use yuumi_core::parsers::game_info::{phase_label, GameInfoPlayer, LiveGameInfo};
use yuumi_core::parsers::match_parser::MatchDisplay;

/// 视图模式：按队伍(5列) 或 全部战绩(10列)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameInfoViewMode {
    /// 按队伍模式：左侧队伍 5 人大卡片 + 右侧 5 列战绩卡片流
    ByTeam,
    /// 全部战绩模式：全屏 10 列并排战绩卡片流 (左 5 友方，右 5 敌方)
    AllMatches,
}

impl Default for GameInfoViewMode {
    fn default() -> Self {
        Self::ByTeam
    }
}

/// 选中的队伍 Tab (按队伍模式下生效)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameInfoTeamTab {
    /// 友方阵营 (默认优先我方队伍，若未知则为蓝方)
    Ally,
    /// 敌方阵营
    Enemy,
}

impl Default for GameInfoTeamTab {
    fn default() -> Self {
        Self::Ally
    }
}

pub struct GameInfoView;

impl GameInfoView {
    /// 渲染 10 人对局信息总看板
    pub fn render<F1, F2, F3, F4>(
        data: Option<&LiveGameInfo>,
        phase: &str,
        is_loading: bool,
        connected: bool,
        view_mode: GameInfoViewMode,
        active_tab: GameInfoTeamTab,
        on_load_demo: F1,
        on_exit_demo: F2,
        on_change_mode: F3,
        on_change_tab: F4,
    ) -> gpui::AnyElement
    where
        F1: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static + Clone,
        F2: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static + Clone,
        F3: Fn(GameInfoViewMode, &mut gpui::Window, &mut gpui::App) + 'static + Clone,
        F4: Fn(GameInfoTeamTab, &mut gpui::Window, &mut gpui::App) + 'static + Clone,
    {
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_2()
            .child(Self::render_toolbar(
                data,
                phase,
                is_loading,
                connected,
                view_mode,
                active_tab,
                on_exit_demo,
                on_change_mode.clone(),
                on_change_tab.clone(),
            ))
            .when_some(data, |this, info| {
                match view_mode {
                    GameInfoViewMode::ByTeam => {
                        this.child(Self::render_by_team_view(info, active_tab))
                    }
                    GameInfoViewMode::AllMatches => {
                        this.child(Self::render_all_matches_view(info))
                    }
                }
            })
            .when(data.is_none(), |this| {
                this.child(Self::render_empty_state(
                    phase,
                    is_loading,
                    connected,
                    on_load_demo,
                ))
            })
            .into_any_element()
    }

    /// 顶部工具栏：阵营切换胶囊（含英雄阵容小头像）、VS徽章、预览状态、5/10列模式切换
    fn render_toolbar<F2, F3, F4>(
        data: Option<&LiveGameInfo>,
        phase: &str,
        is_loading: bool,
        connected: bool,
        view_mode: GameInfoViewMode,
        active_tab: GameInfoTeamTab,
        on_exit_demo: F2,
        on_change_mode: F3,
        on_change_tab: F4,
    ) -> gpui::AnyElement
    where
        F2: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
        F3: Fn(GameInfoViewMode, &mut gpui::Window, &mut gpui::App) + 'static + Clone,
        F4: Fn(GameInfoTeamTab, &mut gpui::Window, &mut gpui::App) + 'static + Clone,
    {
        let is_demo = data.map(|info| info.is_demo).unwrap_or(false);
        let queue_name = data
            .map(|info| info.queue_name.clone())
            .unwrap_or_else(|| "尚未获取对局".to_string());
        let phase_name = data
            .map(|info| info.phase_label.clone())
            .unwrap_or_else(|| phase_label(phase).to_string());

        // 解析友方与敌方队伍成员
        let (ally_team, enemy_team) = if let Some(info) = data {
            let user_is_red = info.user_side == Some(MapSide::Red);
            if user_is_red {
                (&info.red_team, &info.blue_team)
            } else {
                (&info.blue_team, &info.red_team)
            }
        } else {
            (&Vec::new() as &Vec<GameInfoPlayer>, &Vec::new() as &Vec<GameInfoPlayer>)
        };

        let mut left_section = div()
            .flex()
            .items_center()
            .gap_3()
            .child(
                div().size(px(10.0)).rounded_full().bg(if is_loading {
                    Theme::GOLD
                } else if is_demo {
                    Theme::ACCENT_PRIMARY
                } else if connected {
                    Theme::AURORA_CYAN
                } else {
                    Theme::TEXT_MUTED
                }),
            )
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

        if is_demo {
            left_section = left_section
                .child(
                    div()
                        .px_2()
                        .py(px(2.0))
                        .rounded_md()
                        .bg(Theme::ACCENT_LIGHT)
                        .border_1()
                        .border_color(Theme::ACCENT_PRIMARY.opacity(0.3))
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(Theme::ACCENT_PRIMARY)
                        .child("演示预览模式"),
                )
                .child(
                    div()
                        .px_2()
                        .py(px(2.0))
                        .rounded_md()
                        .bg(Theme::CARD_BG)
                        .border_1()
                        .border_color(Theme::CARD_BORDER)
                        .hover(|this| {
                            this.bg(Theme::HOVER_BG)
                                .border_color(Theme::TEXT_MUTED)
                        })
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, on_exit_demo)
                        .text_xs()
                        .text_color(Theme::TEXT_SECONDARY)
                        .child("退出预览"),
                );
        }

        // 中间队伍切换区（仅当有数据时提供交互）
        let center_section = if data.is_some() {
            let is_ally_active = active_tab == GameInfoTeamTab::Ally;
            let on_tab_ally = on_change_tab.clone();
            let on_tab_enemy = on_change_tab;

            div()
                .flex()
                .items_center()
                .gap_2()
                // 友方队伍胶囊按钮
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py_1p5()
                        .rounded_lg()
                        .border_1()
                        .bg(if is_ally_active {
                            Theme::BLUE_SIDE_BG
                        } else {
                            Theme::CARD_BG
                        })
                        .border_color(if is_ally_active {
                            Theme::BLUE_SIDE_ACCENT
                        } else {
                            Theme::CARD_BORDER
                        })
                        .hover(|s| s.bg(Theme::BLUE_SIDE_BG))
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            on_tab_ally(GameInfoTeamTab::Ally, win, app);
                        })
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(if is_ally_active {
                                    Theme::BLUE_SIDE_ACCENT
                                } else {
                                    Theme::TEXT_SECONDARY
                                })
                                .child(format!("友方 ({})", ally_team.len())),
                        )
                        // 友方 5 名英雄小头像横排
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .children(ally_team.iter().take(5).map(|p| {
                                    div()
                                        .size(px(18.0))
                                        .rounded_full()
                                        .border_1()
                                        .border_color(Theme::BLUE_SIDE_ACCENT.opacity(0.6))
                                        .overflow_hidden()
                                        .child(
                                            LcuImage::new(format!(
                                                "/lol-game-data/assets/v1/champion-icons/{}.png",
                                                p.champion_id
                                            ))
                                            .size(px(18.0))
                                            .shape(LcuImageShape::Circle)
                                            .placeholder("?".to_string()),
                                        )
                                })),
                        ),
                )
                // VS 徽标
                .child(
                    div()
                        .px_2()
                        .py(px(2.0))
                        .rounded_md()
                        .bg(Theme::BADGE_BG)
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(Theme::TEXT_MUTED)
                        .child("VS"),
                )
                // 敌方队伍胶囊按钮
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py_1p5()
                        .rounded_lg()
                        .border_1()
                        .bg(if !is_ally_active {
                            Theme::RED_SIDE_BG
                        } else {
                            Theme::CARD_BG
                        })
                        .border_color(if !is_ally_active {
                            Theme::RED_SIDE_ACCENT
                        } else {
                            Theme::CARD_BORDER
                        })
                        .hover(|s| s.bg(Theme::RED_SIDE_BG))
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            on_tab_enemy(GameInfoTeamTab::Enemy, win, app);
                        })
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(if !is_ally_active {
                                    Theme::RED_SIDE_ACCENT
                                } else {
                                    Theme::TEXT_SECONDARY
                                })
                                .child(format!("敌方 ({})", enemy_team.len())),
                        )
                        // 敌方 5 名英雄小头像横排
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .children(enemy_team.iter().take(5).map(|p| {
                                    div()
                                        .size(px(18.0))
                                        .rounded_full()
                                        .border_1()
                                        .border_color(Theme::RED_SIDE_ACCENT.opacity(0.6))
                                        .overflow_hidden()
                                        .child(
                                            LcuImage::new(format!(
                                                "/lol-game-data/assets/v1/champion-icons/{}.png",
                                                p.champion_id
                                            ))
                                            .size(px(18.0))
                                            .shape(LcuImageShape::Circle)
                                            .placeholder("?".to_string()),
                                        )
                                })),
                        ),
                )
        } else {
            div()
        };

        // 右侧视图切换分段控件
        let on_mode_team = on_change_mode.clone();
        let on_mode_all = on_change_mode;
        let right_section = div()
            .flex()
            .items_center()
            .p(px(2.0))
            .rounded_lg()
            .bg(Theme::BADGE_BG)
            .border_1()
            .border_color(Theme::CARD_BORDER)
            .child(
                div()
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .bg(if view_mode == GameInfoViewMode::ByTeam {
                        Theme::CARD_BG
                    } else {
                        gpui::transparent_black()
                    })
                    .border_1()
                    .border_color(if view_mode == GameInfoViewMode::ByTeam {
                        Theme::CARD_BORDER
                    } else {
                        gpui::transparent_black()
                    })
                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                        on_mode_team(GameInfoViewMode::ByTeam, win, app);
                    })
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(if view_mode == GameInfoViewMode::ByTeam {
                                Theme::ACCENT_PRIMARY
                            } else {
                                Theme::TEXT_MUTED
                            })
                            .child("按队伍 (5)"),
                    ),
            )
            .child(
                div()
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .bg(if view_mode == GameInfoViewMode::AllMatches {
                        Theme::CARD_BG
                    } else {
                        gpui::transparent_black()
                    })
                    .border_1()
                    .border_color(if view_mode == GameInfoViewMode::AllMatches {
                        Theme::CARD_BORDER
                    } else {
                        gpui::transparent_black()
                    })
                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                        on_mode_all(GameInfoViewMode::AllMatches, win, app);
                    })
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(if view_mode == GameInfoViewMode::AllMatches {
                                Theme::ACCENT_PRIMARY
                            } else {
                                Theme::TEXT_MUTED
                            })
                            .child("全部战绩 (10)"),
                    ),
            );

        div()
            .w_full()
            .bg(Theme::CARD_BG)
            .border_1()
            .border_color(Theme::CARD_BORDER)
            .rounded_xl()
            .px_3()
            .py_2()
            .flex()
            .items_center()
            .justify_between()
            .child(left_section)
            .child(center_section)
            .child(right_section)
            .into_any_element()
    }

    /// 模式 1：按队伍模式 (左侧 220px 5 人卡片 + 右侧 5 列竖向战绩流)
    fn render_by_team_view(info: &LiveGameInfo, active_tab: GameInfoTeamTab) -> gpui::AnyElement {
        let user_is_red = info.user_side == Some(MapSide::Red);
        let (selected_team, side_accent, is_ally) = match active_tab {
            GameInfoTeamTab::Ally => {
                if user_is_red {
                    (&info.red_team, Theme::RED_SIDE_ACCENT, true)
                } else {
                    (&info.blue_team, Theme::BLUE_SIDE_ACCENT, true)
                }
            }
            GameInfoTeamTab::Enemy => {
                if user_is_red {
                    (&info.blue_team, Theme::BLUE_SIDE_ACCENT, false)
                } else {
                    (&info.red_team, Theme::RED_SIDE_ACCENT, false)
                }
            }
        };

        div()
            .w_full()
            .flex()
            .gap_2p5()
            // 左侧：220px 宽度的队伍 5 名玩家详情大卡片纵排
            .child(
                div()
                    .w(px(220.0))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(
                        selected_team
                            .iter()
                            .map(|p| Self::render_player_full_card(p, side_accent)),
                    ),
            )
            // 右侧：5 列横向排列的玩家近期战绩流 (自适应撑满剩余区域)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .gap_2()
                    .children(selected_team.iter().map(|p| {
                        Self::render_player_match_column(p, side_accent, is_ally, false)
                    })),
            )
            .into_any_element()
    }

    /// 模式 2：全部战绩模式 (全屏 10 列并排，左 5 友方，右 5 敌方)
    fn render_all_matches_view(info: &LiveGameInfo) -> gpui::AnyElement {
        let user_is_red = info.user_side == Some(MapSide::Red);
        let (ally_team, enemy_team) = if user_is_red {
            (&info.red_team, &info.blue_team)
        } else {
            (&info.blue_team, &info.red_team)
        };

        div()
            .w_full()
            .flex()
            .gap_1()
            // 左侧 5 列 (友方)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .gap_1()
                    .children(ally_team.iter().map(|p| {
                        Self::render_player_match_column(p, Theme::BLUE_SIDE_ACCENT, true, true)
                    })),
            )
            // 中间阵营分割指示条
            .child(
                div()
                    .w(px(2.0))
                    .rounded_full()
                    .bg(Theme::CARD_BORDER)
                    .mx_1()
                    .my_1(),
            )
            // 右侧 5 列 (敌方)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .gap_1()
                    .children(enemy_team.iter().map(|p| {
                        Self::render_player_match_column(p, Theme::RED_SIDE_ACCENT, false, true)
                    })),
            )
            .into_any_element()
    }

    /// 渲染左侧玩家详情大卡片（含头像、位置角标、段位、胜率、常用英雄）
    fn render_player_full_card(player: &GameInfoPlayer, side_accent: Hsla) -> gpui::AnyElement {
        let is_local = player.is_local_player;
        let display_name = if player.tag_line.is_empty() {
            player.summoner_name.clone()
        } else {
            format!("{} #{}", player.summoner_name, player.tag_line)
        };

        let premade_style = player.premade_group_id.map(|idx| {
            let color = Theme::premade_color(idx);
            (color, format!("组队 {}", idx + 1))
        });

        // 标签集合 (本机 / 组队 / 宿命对局)
        let mut badges = div().flex().items_center().gap_1();
        if is_local {
            badges = badges.child(
                div()
                    .px_1p5()
                    .py(px(1.0))
                    .rounded_md()
                    .bg(side_accent)
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(gpui::white())
                    .child("本机"),
            );
        }
        if let Some((color, tag)) = premade_style {
            badges = badges.child(
                div()
                    .px_1p5()
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
            badges = badges.child(
                div()
                    .px_1p5()
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

        div()
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
            .p_2p5()
            .flex()
            .flex_col()
            .gap_1p5()
            .hover(|s| s.bg(Theme::CARD_BG_HOVER))
            // 头部：头像 + 召唤师名 + 标签
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .relative()
                            .size(px(36.0))
                            .rounded_lg()
                            .border_1()
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
                                .size(px(36.0))
                                .shape(LcuImageShape::Rounded)
                                .placeholder("?".to_string()),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .bottom_0()
                                    .left_0()
                                    .right_0()
                                    .bg(rgba(0x000000bb))
                                    .py(px(1.0))
                                    .flex()
                                    .justify_center()
                                    .text_xs()
                                    .text_color(gpui::white())
                                    .child(player.position.clone()),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child(display_name),
                            )
                            .child(badges),
                    ),
            )
            // 段位与排位胜负
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(Theme::BADGE_BG)
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(Theme::TEXT_SECONDARY)
                            .child(player.rank_display.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(if player.win_rate >= 50 {
                                Theme::WIN_TEXT
                            } else {
                                Theme::LOSS_TEXT
                            })
                            .child(format!(
                                "{}% ({}胜{}负)",
                                player.win_rate, player.ranked_wins, player.ranked_losses
                            )),
                    ),
            )
            // 常用英雄小列表 (若有)
            .when(!player.top_champions.is_empty(), |this| {
                this.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .pt_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(Theme::TEXT_MUTED)
                                .child("常用英雄:"),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1p5()
                                .children(player.top_champions.iter().take(3).map(|top| {
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_1()
                                        .child(
                                            div()
                                                .size(px(16.0))
                                                .rounded_full()
                                                .overflow_hidden()
                                                .child(
                                                    LcuImage::new(format!(
                                                        "/lol-game-data/assets/v1/champion-icons/{}.png",
                                                        top.champion_id
                                                    ))
                                                    .size(px(16.0))
                                                    .shape(LcuImageShape::Circle)
                                                    .placeholder("?".to_string()),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(Theme::TEXT_SECONDARY)
                                                .child(format!("{}%", top.win_rate)),
                                        )
                                })),
                        ),
                )
            })
            // 底部：场均 KDA 与连胜/连败徽章
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pt_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::TEXT_MUTED)
                            .child(format!("{:.2} KDA", player.avg_kda)),
                    )
                    .when_some(player.streak_text.clone(), |el, streak| {
                        el.child(
                            div()
                                .px_1p5()
                                .py(px(1.0))
                                .rounded_md()
                                .bg(if player.streak_is_win {
                                    Theme::WIN_BG
                                } else {
                                    Theme::LOSS_BG
                                })
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(if player.streak_is_win {
                                    Theme::WIN_TEXT
                                } else {
                                    Theme::LOSS_TEXT
                                })
                                .child(streak),
                        )
                    }),
            )
            .into_any_element()
    }

    /// 渲染一列玩家战绩流（列头 Header + 垂直 MatchCard 列表）
    fn render_player_match_column(
        player: &GameInfoPlayer,
        side_accent: Hsla,
        _is_ally: bool,
        compact: bool,
    ) -> gpui::AnyElement {
        let premade_bg = player.premade_group_id.map(|idx| Theme::premade_color(idx));

        let display_name = if player.tag_line.is_empty() {
            player.summoner_name.clone()
        } else {
            player.summoner_name.clone()
        };

        // 列头
        let header_avatar_size = if compact { px(20.0) } else { px(24.0) };
        let col_header = div()
            .w_full()
            .rounded_t_lg()
            .bg(if let Some(col) = premade_bg {
                col.opacity(0.12)
            } else {
                Theme::CARD_BG
            })
            .border_1()
            .border_b_2()
            .border_color(if let Some(col) = premade_bg {
                col
            } else {
                side_accent
            })
            .p(if compact { px(4.0) } else { px(8.0) })
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(if compact { px(4.0) } else { px(8.0) })
                    .child(
                        div()
                            .size(header_avatar_size)
                            .rounded_md()
                            .overflow_hidden()
                            .child(
                                LcuImage::new(format!(
                                    "/lol-game-data/assets/v1/champion-icons/{}.png",
                                    player.champion_id
                                ))
                                .size(header_avatar_size)
                                .shape(LcuImageShape::Rounded)
                                .placeholder("?".to_string()),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .overflow_hidden()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child(display_name),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(if player.win_rate >= 50 {
                                                Theme::WIN_TEXT
                                            } else {
                                                Theme::LOSS_TEXT
                                            })
                                            .child(if compact {
                                                format!("{}W{}L", player.ranked_wins, player.ranked_losses)
                                            } else {
                                                format!(
                                                    "{}胜 {}负",
                                                    player.ranked_wins, player.ranked_losses
                                                )
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(Theme::TEXT_MUTED)
                                            .child(player.rank_display.clone()),
                                    ),
                            ),
                    ),
            );

        // 战绩列表 (最多 10 局)
        let matches_pane = if player.matches.is_empty() {
            div()
                .w_full()
                .py_8()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_1()
                .child(div().text_xs().text_color(Theme::TEXT_MUTED).child("暂无近况战绩"))
                .into_any_element()
        } else {
            div()
                .w_full()
                .flex()
                .flex_col()
                .gap_1p5()
                .children(
                    player
                        .matches
                        .iter()
                        .take(10)
                        .map(|m| Self::render_match_card(m, compact)),
                )
                .into_any_element()
        };

        div()
            .flex_1()
            .min_w(if compact { px(76.0) } else { px(110.0) })
            .flex()
            .flex_col()
            .gap_1p5()
            .child(col_header)
            .child(matches_pane)
            .into_any_element()
    }

    /// 渲染单场战绩卡片 (MatchCard)
    fn render_match_card(m: &MatchDisplay, compact: bool) -> gpui::AnyElement {
        let is_win = m.win;
        let is_remake = m.remake;

        let (bg, border, text_color) = if is_remake {
            (Theme::REMAKE_BG, Theme::REMAKE_BORDER, Theme::REMAKE_TEXT)
        } else if is_win {
            (Theme::WIN_BG, Theme::WIN_BORDER, Theme::WIN_TEXT)
        } else {
            (Theme::LOSS_BG, Theme::LOSS_BORDER, Theme::LOSS_TEXT)
        };

        let ratio = if m.deaths == 0 {
            "Perf".to_string()
        } else {
            let r = (m.kills + m.assists) as f32 / m.deaths as f32;
            format!("{:.1}", r)
        };

        let time_text = format_relative_time(m.time_stamp, compact);
        let card_avatar_size = if compact { px(22.0) } else { px(28.0) };

        div()
            .w_full()
            .p(if compact { px(4.0) } else { px(8.0) })
            .rounded_lg()
            .bg(bg)
            .border_1()
            .border_color(border)
            .flex()
            .flex_col()
            .gap(if compact { px(2.0) } else { px(4.0) })
            // 顶部：模式名 + 相对时间
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(text_color)
                            .child(if is_remake {
                                "重开".to_string()
                            } else {
                                m.name.clone()
                            }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::TEXT_MUTED)
                            .child(time_text),
                    ),
            )
            // 卡片主体：英雄头像（带等级角标）+ KDA + KDA 倍率
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(if compact { px(4.0) } else { px(8.0) })
                    .child(
                        div()
                            .relative()
                            .size(card_avatar_size)
                            .rounded_md()
                            .overflow_hidden()
                            .child(
                                LcuImage::new(format!(
                                    "/lol-game-data/assets/v1/champion-icons/{}.png",
                                    m.champion_id
                                ))
                                .size(card_avatar_size)
                                .shape(LcuImageShape::Rounded)
                                .placeholder("?".to_string()),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .bottom_0()
                                    .right_0()
                                    .bg(rgba(0x000000bb))
                                    .px_1()
                                    .rounded_tl_sm()
                                    .text_xs()
                                    .text_color(gpui::white())
                                    .child(m.champ_level.to_string()),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child(format!("{}/{}/{}", m.kills, m.deaths, m.assists)),
                            )
                            .when(!compact, |el| {
                                el.child(
                                    div()
                                        .text_xs()
                                        .text_color(Theme::TEXT_SECONDARY)
                                        .child(format!("{} KDA", ratio)),
                                )
                            }),
                    ),
            )
            .into_any_element()
    }

    /// 空态引导：未连接 / 拉取中 / 未进入对局
    fn render_empty_state<F>(
        phase: &str,
        is_loading: bool,
        connected: bool,
        on_load_demo: F,
    ) -> gpui::AnyElement
    where
        F: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    {
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
            .child(
                div()
                    .mt_3()
                    .px_5()
                    .py_2p5()
                    .rounded_lg()
                    .bg(Theme::ACCENT_PRIMARY)
                    .hover(|this| this.opacity(0.85))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, on_load_demo)
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(gpui::white())
                            .child("预览对局信息"),
                    ),
            )
            .into_any_element()
    }
}

/// 格式化对局毫秒时间戳为相对时间描述 (支持 compact 短格式)
fn format_relative_time(timestamp: u64, compact: bool) -> String {
    if timestamp == 0 {
        return "近期".to_string();
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(timestamp);
    if now > timestamp {
        let diff_secs = (now - timestamp) / 1000;
        if diff_secs < 60 {
            "刚刚".to_string()
        } else if diff_secs < 3600 {
            if compact {
                format!("{}m前", diff_secs / 60)
            } else {
                format!("{}分钟前", diff_secs / 60)
            }
        } else if diff_secs < 86400 {
            if compact {
                format!("{}h前", diff_secs / 3600)
            } else {
                format!("{}小时前", diff_secs / 3600)
            }
        } else if diff_secs < 86400 * 30 {
            if compact {
                format!("{}d前", diff_secs / 86400)
            } else {
                format!("{}天前", diff_secs / 86400)
            }
        } else {
            "1月前".to_string()
        }
    } else {
        "刚刚".to_string()
    }
}
