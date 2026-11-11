//! 10 人对局信息视图 (GameInfoView)
//! 纯白水晶极光风格，1:1 复刻 Seraphine / Yuumi Vue 原版：
//! - G01: 完整状态机：离线 (LcuOfflineState) / TFT (♟️) / 读取队伍 (⏳) / 等待中 (⏳) / 正常内容
//! - G02: 初始模式为全部战绩 (AllMatches)，按队伍初始页签为友方 (Ally)；两枚模式按钮「按队伍」「全部战绩」
//! - G03: 按队伍左右分栏 250px + 16px + 剩余宽度；左侧双页签 + 独立滚动概览列表；右侧工具条 + 五等宽战绩列同面板；<=1100px 自动上下布局
//! - G04: 敌方未公开判定：选人阶段无有效身份时隐藏右侧 VS / 敌方信息 / 芯片；十列模式只放友方 5 列
//! - G05: 工具条：五列按友方、组队芯片、VS、敌方、模式切换；阵营徽章点击联动切队；十列两半区、中心 VS、两侧标识与芯片
//! - G06: 概览卡 (PlayerCard)：头像环、等级徽标、连胜败、段位图标/名称/LP、胜率与KDA、熟练度、常用位置/英雄；昵称点击进生涯
//! - G07: 列头：五列显示胜负场数、段位、熟练度；十列紧凑模式走三行：胜率竖条、招牌/练角标、头像下胜负、熟练度+段位、第三行位置+标识；昵称点击查战绩
//! - G08: 历史战绩列每人前 10 场：模式、日期、位置、头像与等级、K/D/A；五列显示 KDA 倍率，十列隐藏；胜/负/重开状态色；空态
//! - G09: 组队色按 PremadeGroupChips，芯片仅在 >=2 人成组时渲染，列头与概览卡使用对应组队色
//! - G10: 战绩卡高与独立列滚动：五列内边距 6px、间距 6px、卡高最小 44px、头像 32x32；十列内边距 4px 2px、间距 4px、卡高最小 38px、头像 29x29，双位数等级角标完整显示；每列独立纵向滚动
//! - G11: 正式界面移除预览按钮、退出预览等调试按钮；内部保留 load_snapshot_or_demo 方法供测试调用

use crate::components::icon::icon;
use crate::components::lcu_image::{LcuImage, LcuImageShape};
use crate::components::offline_state::render_lcu_offline_state;
use crate::theme::Theme;
use crate::views::saved_players::tag_badge_colors;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::collections::HashMap;
use yuumi_core::gameflow::team_builder::MapSide;
use yuumi_core::parsers::game_info::{GameInfoPlayer, LiveGameInfo};
use yuumi_core::parsers::match_parser::MatchDisplay;

/// 视图模式：按队伍(5列) 或 全部战绩(10列)
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum GameInfoViewMode {
    /// 按队伍模式：左侧队伍 5 人大卡片 + 右侧 5 列战绩卡片流
    ByTeam,
    /// 全部战绩模式：全屏 10 列并排战绩卡片流 (左 5 友方，右 5 敌方)
    #[default]
    AllMatches,
}

/// 选中的队伍 Tab (按队伍模式下生效)
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum GameInfoTeamTab {
    /// 友方阵营 (默认优先我方队伍，若未知则为蓝方)
    #[default]
    Ally,
    /// 敌方阵营
    Enemy,
}

/// 组队成员集合（用于 PremadeGroupChips）
struct PremadeGroup<'a> {
    group_id: usize,
    members: Vec<&'a GameInfoPlayer>,
}

fn get_premade_groups<'a>(team: &'a [GameInfoPlayer]) -> Vec<PremadeGroup<'a>> {
    let mut map: HashMap<usize, Vec<&'a GameInfoPlayer>> = HashMap::new();
    for p in team {
        if let Some(gid) = p.premade_group_id {
            map.entry(gid).or_default().push(p);
        }
    }
    let mut groups: Vec<PremadeGroup<'a>> = map
        .into_iter()
        .filter(|(_, members)| members.len() >= 2)
        .map(|(group_id, members)| PremadeGroup { group_id, members })
        .collect();
    groups.sort_by_key(|g| g.group_id);
    groups
}

pub struct GameInfoView;

impl GameInfoView {
    /// 加载历史快照或演示数据（供内部测试与开发调试）
    pub fn load_snapshot_or_demo() -> LiveGameInfo {
        LiveGameInfo::load_snapshot_or_demo()
    }

    /// 渲染 10 人对局信息总看板
    #[allow(clippy::too_many_arguments)]
    pub fn render<F1, F2, F3, F4, F5, F6>(
        data: Option<&LiveGameInfo>,
        _phase: &str,
        is_loading: bool,
        connected: bool,
        view_mode: GameInfoViewMode,
        active_tab: GameInfoTeamTab,
        game_info_filter: bool,
        on_change_mode: F1,
        on_change_tab: F2,
        on_player_career: F3,
        on_player_search: F4,
        on_load_demo: F5,
        on_exit_demo: F6,
        window_width: Pixels,
    ) -> gpui::AnyElement
    where
        F1: Fn(GameInfoViewMode, &mut gpui::Window, &mut gpui::App) + 'static + Clone,
        F2: Fn(GameInfoTeamTab, &mut gpui::Window, &mut gpui::App) + 'static + Clone,
        F3: Fn(Option<(String, String)>, &mut gpui::Window, &mut gpui::App) + 'static + Clone,
        F4: Fn(String, Option<String>, &mut gpui::Window, &mut gpui::App) + 'static + Clone,
        F5: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static + Clone,
        F6: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static + Clone,
    {
        // 若当前无对局数据：未连接 / 读取中 / 等待中（均支持一键进入离线/演示预览）
        if data.is_none() {
            if !connected {
                return Self::render_offline_state(on_load_demo);
            }
            if is_loading {
                return Self::render_tip_container(
                    "⏳",
                    "正在读取对局玩家信息…",
                    Some(on_load_demo),
                );
            }
            return Self::render_tip_container(
                "⏳",
                "对局暂未开始，等待对局中…",
                Some(on_load_demo),
            );
        }

        if let Some(info) = data {
            let is_tft = (info.queue_id >= 1090 && info.queue_id <= 1200)
                || info.queue_name.contains("云顶")
                || info.queue_name.contains("TFT");
            if is_tft {
                return Self::render_tip_container(
                    "♟️",
                    "云顶之弈对局中，对局信息页面不显示数据",
                    None as Option<F5>,
                );
            }
        }

        let Some(info) = data else {
            return Self::render_tip_container(
                "⏳",
                "对局暂未开始，等待对局中…",
                Some(on_load_demo),
            );
        };

        // 对局队伍划分
        let user_is_red = info.user_side == Some(MapSide::Red);
        let (ally_team, enemy_team) = if user_is_red {
            (&info.red_team, &info.blue_team)
        } else {
            (&info.blue_team, &info.red_team)
        };

        let is_revealed = is_their_team_revealed(enemy_team, &info.phase);

        div()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .p(px(12.0))
            .child(match view_mode {
                GameInfoViewMode::ByTeam => Self::render_by_team_view(
                    info,
                    ally_team,
                    enemy_team,
                    user_is_red,
                    is_revealed,
                    active_tab,
                    game_info_filter,
                    on_change_mode,
                    on_change_tab,
                    on_player_career,
                    on_player_search,
                    on_exit_demo,
                    window_width,
                ),
                GameInfoViewMode::AllMatches => Self::render_all_matches_view(
                    info,
                    ally_team,
                    enemy_team,
                    user_is_red,
                    is_revealed,
                    game_info_filter,
                    on_change_mode,
                    on_change_tab,
                    on_player_search,
                    on_exit_demo,
                ),
            })
            .into_any_element()
    }

    /// 模式 1：按队伍模式 (左侧 250px 双页签+玩家详情大卡片纵排 + 右侧工具栏与 5 列竖向战绩流)
    #[allow(clippy::too_many_arguments)]
    fn render_by_team_view<F1, F2, F3, F4, F6>(
        info: &LiveGameInfo,
        ally_team: &[GameInfoPlayer],
        enemy_team: &[GameInfoPlayer],
        user_is_red: bool,
        is_revealed: bool,
        active_tab: GameInfoTeamTab,
        game_info_filter: bool,
        on_change_mode: F1,
        on_change_tab: F2,
        on_player_career: F3,
        on_player_search: F4,
        on_exit_demo: F6,
        window_width: Pixels,
    ) -> gpui::AnyElement
    where
        F1: Fn(GameInfoViewMode, &mut gpui::Window, &mut gpui::App) + 'static + Clone,
        F2: Fn(GameInfoTeamTab, &mut gpui::Window, &mut gpui::App) + 'static + Clone,
        F3: Fn(Option<(String, String)>, &mut gpui::Window, &mut gpui::App) + 'static + Clone,
        F4: Fn(String, Option<String>, &mut gpui::Window, &mut gpui::App) + 'static + Clone,
        F6: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static + Clone,
    {
        let is_ally_tab = active_tab == GameInfoTeamTab::Ally;
        let current_team = if is_ally_tab { ally_team } else { enemy_team };

        let side_accent = if is_ally_tab {
            if user_is_red {
                Theme::RED_SIDE_ACCENT()
            } else {
                Theme::BLUE_SIDE_ACCENT()
            }
        } else {
            if user_is_red {
                Theme::BLUE_SIDE_ACCENT()
            } else {
                Theme::RED_SIDE_ACCENT()
            }
        };

        let on_tab_ally = on_change_tab.clone();
        let on_tab_enemy = on_change_tab.clone();

        // 左侧面板：双页签 + 独立滚动概览卡片列表 (G03)
        let left_panel = div()
            .w(px(250.0))
            .flex_shrink_0()
            .h_full()
            .rounded_xl()
            .border_1()
            .border_color(Theme::CARD_BORDER())
            .bg(Theme::CARD_BG())
            .overflow_hidden()
            .flex()
            .flex_col()
            // 顶部「友方 (n)」「敌方 (n)」双页签 (G03)
            .child(
                div()
                    .h(px(44.0))
                    .flex()
                    .rounded_t(px(11.0))
                    .border_b_1()
                    .border_color(Theme::CARD_BORDER())
                    .bg(Theme::CARD_BG().opacity(0.4))
                    // 友方 (n)
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .when(is_ally_tab, |s| s.rounded_tl(px(11.0)))
                            .text_size(px(13.0))
                            .font_weight(if is_ally_tab {
                                FontWeight::BOLD
                            } else {
                                FontWeight::MEDIUM
                            })
                            .text_color(if is_ally_tab {
                                Theme::ACCENT_PRIMARY()
                            } else {
                                Theme::TEXT_MUTED()
                            })
                            .bg(if is_ally_tab {
                                Theme::CARD_BG()
                            } else {
                                gpui::transparent_black()
                            })
                            .border_b_2()
                            .border_color(if is_ally_tab {
                                Theme::ACCENT_PRIMARY()
                            } else {
                                gpui::transparent_black()
                            })
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                on_tab_ally(GameInfoTeamTab::Ally, win, app);
                            })
                            .child(format!("友方 ({})", ally_team.len())),
                    )
                    // 敌方 (n) —— 未公开时计数为 0 (G04)
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .when(!is_ally_tab, |s| s.rounded_tr(px(11.0)))
                            .text_size(px(13.0))
                            .font_weight(if !is_ally_tab {
                                FontWeight::BOLD
                            } else {
                                FontWeight::MEDIUM
                            })
                            .text_color(if !is_ally_tab {
                                Theme::ACCENT_PRIMARY()
                            } else {
                                Theme::TEXT_MUTED()
                            })
                            .bg(if !is_ally_tab {
                                Theme::CARD_BG()
                            } else {
                                gpui::transparent_black()
                            })
                            .border_b_2()
                            .border_color(if !is_ally_tab {
                                Theme::ACCENT_PRIMARY()
                            } else {
                                gpui::transparent_black()
                            })
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                on_tab_enemy(GameInfoTeamTab::Enemy, win, app);
                            })
                            .child(format!(
                                "敌方 ({})",
                                if is_revealed { enemy_team.len() } else { 0 }
                            )),
                    ),
            )
            // 下方独立滚动的概览列表：内边距 10px 8px，卡距 8px (G03, G10)
            .child(
                div()
                    .id("gameinfo-player-list-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .px(px(8.0))
                    .py(px(10.0))
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .children(current_team.iter().enumerate().map(|(idx, p)| {
                        Self::render_player_full_card(p, side_accent, on_player_career.clone(), idx)
                    }))
                    .when(current_team.is_empty(), |el| {
                        el.child(
                            div()
                                .flex_1()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(12.0))
                                .text_color(Theme::TEXT_MUTED())
                                .child("暂无队伍数据"),
                        )
                    }),
            );

        // 右侧面板：包含顶部工具条和五等宽战绩列在同一面板内 (G03)
        let right_panel = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .rounded_xl()
            .border_1()
            .border_color(Theme::CARD_BORDER())
            .bg(Theme::CARD_BG())
            .overflow_hidden()
            .flex()
            .flex_col()
            // 顶部工具条 (G05)
            .child(Self::render_toolbar(
                info,
                ally_team,
                enemy_team,
                user_is_red,
                is_revealed,
                GameInfoViewMode::ByTeam,
                active_tab,
                on_change_mode,
                on_change_tab,
                on_exit_demo,
            ))
            // 五等宽战绩列：各自独立滚动 (G03, G10)
            .child(div().flex_1().min_h_0().flex().flex_row().children(
                current_team.iter().enumerate().map(|(idx, p)| {
                    Self::render_player_match_column(
                        p,
                        side_accent,
                        is_ally_tab,
                        false,
                        false,
                        game_info_filter,
                        info.queue_id,
                        on_player_search.clone(),
                        idx,
                    )
                }),
            ));

        // <= 1100px 自动改为上下布局 (G03)
        if window_width <= px(1100.0) {
            div()
                .size_full()
                .flex()
                .flex_col()
                .gap(px(12.0))
                .child(div().h(px(200.0)).w_full().child(left_panel))
                .child(right_panel)
                .into_any_element()
        } else {
            div()
                .size_full()
                .flex()
                .flex_row()
                .gap(px(16.0))
                .child(left_panel)
                .child(right_panel)
                .into_any_element()
        }
    }

    /// 模式 2：全部战绩模式 (全屏单面板，顶部工具条 + 10 等宽列横排，左 5 友方，右 5 敌方)
    #[allow(clippy::too_many_arguments)]
    fn render_all_matches_view<F1, F2, F4, F6>(
        info: &LiveGameInfo,
        ally_team: &[GameInfoPlayer],
        enemy_team: &[GameInfoPlayer],
        user_is_red: bool,
        is_revealed: bool,
        game_info_filter: bool,
        on_change_mode: F1,
        on_change_tab: F2,
        on_player_search: F4,
        on_exit_demo: F6,
    ) -> gpui::AnyElement
    where
        F1: Fn(GameInfoViewMode, &mut gpui::Window, &mut gpui::App) + 'static + Clone,
        F2: Fn(GameInfoTeamTab, &mut gpui::Window, &mut gpui::App) + 'static + Clone,
        F4: Fn(String, Option<String>, &mut gpui::Window, &mut gpui::App) + 'static + Clone,
        F6: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static + Clone,
    {
        let ally_accent = if user_is_red {
            Theme::RED_SIDE_ACCENT()
        } else {
            Theme::BLUE_SIDE_ACCENT()
        };
        let enemy_accent = if user_is_red {
            Theme::BLUE_SIDE_ACCENT()
        } else {
            Theme::RED_SIDE_ACCENT()
        };

        div()
            .size_full()
            .rounded_xl()
            .border_1()
            .border_color(Theme::CARD_BORDER())
            .bg(Theme::CARD_BG())
            .overflow_hidden()
            .flex()
            .flex_col()
            // 顶部工具条 (G05)
            .child(Self::render_toolbar(
                info,
                ally_team,
                enemy_team,
                user_is_red,
                is_revealed,
                GameInfoViewMode::AllMatches,
                GameInfoTeamTab::Ally,
                on_change_mode,
                on_change_tab,
                on_exit_demo,
            ))
            // 战绩列横排：十列紧凑模式走 compact 分支 (G04, G07, G10)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    // 友方 5 列
                    .children(ally_team.iter().enumerate().map(|(idx, p)| {
                        Self::render_player_match_column(
                            p,
                            ally_accent,
                            true,
                            true,
                            false,
                            game_info_filter,
                            info.queue_id,
                            on_player_search.clone(),
                            idx,
                        )
                    }))
                    // 敌方 5 列（若未公开则完全隐藏右侧列，G04）
                    .when(is_revealed, |el| {
                        el.children(enemy_team.iter().enumerate().map(|(idx, p)| {
                            Self::render_player_match_column(
                                p,
                                enemy_accent,
                                false,
                                true,
                                idx == 0,
                                game_info_filter,
                                info.queue_id,
                                on_player_search.clone(),
                                ally_team.len() + idx,
                            )
                        }))
                    }),
            )
            .into_any_element()
    }

    /// 顶部工具栏 (G05, G11)
    #[allow(clippy::too_many_arguments)]
    fn render_toolbar<F1, F2, F3>(
        info: &LiveGameInfo,
        ally_team: &[GameInfoPlayer],
        enemy_team: &[GameInfoPlayer],
        user_is_red: bool,
        is_revealed: bool,
        view_mode: GameInfoViewMode,
        active_tab: GameInfoTeamTab,
        on_change_mode: F1,
        on_change_tab: F2,
        on_exit_demo: F3,
    ) -> gpui::AnyElement
    where
        F1: Fn(GameInfoViewMode, &mut gpui::Window, &mut gpui::App) + 'static + Clone,
        F2: Fn(GameInfoTeamTab, &mut gpui::Window, &mut gpui::App) + 'static + Clone,
        F3: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static + Clone,
    {
        let ally_is_red = user_is_red;
        let enemy_is_red = !user_is_red;

        let ally_groups = get_premade_groups(ally_team);
        let enemy_groups = get_premade_groups(enemy_team);

        let make_demo_badge = {
            let on_exit_demo = on_exit_demo.clone();
            move || {
                if info.is_demo {
                    Some(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .px(px(6.0))
                                    .py(px(2.0))
                                    .rounded_md()
                                    .bg(Theme::ACCENT_LIGHT())
                                    .border_1()
                                    .border_color(Theme::ACCENT_PRIMARY().opacity(0.3))
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Theme::ACCENT_PRIMARY())
                                    .child("演示预览模式"),
                            )
                            .child(
                                div()
                                    .px(px(8.0))
                                    .py(px(2.0))
                                    .rounded_md()
                                    .bg(Theme::CARD_BG())
                                    .border_1()
                                    .border_color(Theme::CARD_BORDER())
                                    .hover(|this| {
                                        this.bg(Theme::HOVER_BG()).border_color(Theme::TEXT_MUTED())
                                    })
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, on_exit_demo.clone())
                                    .text_size(px(11.0))
                                    .text_color(Theme::TEXT_SECONDARY())
                                    .child("退出预览"),
                            ),
                    )
                } else {
                    None
                }
            }
        };

        // 10 列视图且敌方已公开：左右对称 50% 阵营与组队信息 (G05)
        if view_mode == GameInfoViewMode::AllMatches && is_revealed {
            return div()
                .w_full()
                .h(px(44.0))
                .flex_shrink_0()
                .rounded_t(px(11.0))
                .border_b_1()
                .border_color(Theme::CARD_BORDER())
                .bg(Theme::CARD_BG().opacity(0.5))
                .px(px(12.0))
                .flex()
                .items_center()
                .justify_between()
                // 左侧 50%：我方阵营胶囊 + 我方组队芯片
                .child(
                    div()
                        .w_1_2()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .overflow_hidden()
                        .child(Self::render_side_pill(
                            format!("友方 ({})", ally_team.len()),
                            ally_is_red,
                            false,
                            false,
                            None,
                        ))
                        .child(render_premade_chips(&ally_groups)),
                )
                // 轴线 VS 分隔
                .child(
                    div()
                        .px(px(7.0))
                        .py(px(2.0))
                        .rounded_full()
                        .border_1()
                        .border_color(Theme::CARD_BORDER())
                        .bg(Theme::CARD_BG())
                        .text_size(px(10.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(Theme::TEXT_MUTED())
                        .child("VS"),
                )
                // 右侧 50%：敌方组队芯片 + 敌方阵营指示 + 模式切换
                .child(
                    div()
                        .w_1_2()
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap(px(8.0))
                        .overflow_hidden()
                        .child(render_premade_chips(&enemy_groups))
                        .child(Self::render_side_pill(
                            format!("敌方 ({})", enemy_team.len()),
                            enemy_is_red,
                            false,
                            false,
                            None,
                        ))
                        .children(make_demo_badge())
                        .child(render_view_segmented_control(view_mode, on_change_mode)),
                )
                .into_any_element();
        }

        // 5 列视图 或 敌方未公开时的单方视图 (G05)
        let on_tab_my = on_change_tab.clone();
        let on_tab_their = on_change_tab;

        div()
            .w_full()
            .h(px(44.0))
            .flex_shrink_0()
            .rounded_t(px(11.0))
            .border_b_1()
            .border_color(Theme::CARD_BORDER())
            .bg(Theme::CARD_BG().opacity(0.5))
            .px(px(12.0))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    // 我方组队区 (阵营徽章可点击联动切队, G05)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(Self::render_side_pill(
                                format!("友方 ({})", ally_team.len()),
                                ally_is_red,
                                view_mode == GameInfoViewMode::ByTeam,
                                active_tab == GameInfoTeamTab::Ally,
                                Some(Box::new(move |win, app| {
                                    on_tab_my(GameInfoTeamTab::Ally, win, app);
                                })),
                            ))
                            .child(render_premade_chips(&ally_groups)),
                    )
                    // VS 与敌方区（仅当公开时显示, G04, G05）
                    .when(is_revealed, |el| {
                        el.child(
                            div()
                                .px(px(7.0))
                                .py(px(2.0))
                                .rounded_full()
                                .border_1()
                                .border_color(Theme::CARD_BORDER())
                                .bg(Theme::CARD_BG())
                                .text_size(px(10.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(Theme::TEXT_MUTED())
                                .child("VS"),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .child(render_premade_chips(&enemy_groups))
                                .child(Self::render_side_pill(
                                    format!("敌方 ({})", enemy_team.len()),
                                    enemy_is_red,
                                    view_mode == GameInfoViewMode::ByTeam,
                                    active_tab == GameInfoTeamTab::Enemy,
                                    Some(Box::new(move |win, app| {
                                        on_tab_their(GameInfoTeamTab::Enemy, win, app);
                                    })),
                                )),
                        )
                    }),
            )
            // 模式切换器与预览控制
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .children(make_demo_badge())
                    .child(render_view_segmented_control(view_mode, on_change_mode)),
            )
            .into_any_element()
    }

    /// 阵营胶囊 (side-pill)
    #[allow(clippy::type_complexity)]
    fn render_side_pill(
        text: String,
        is_red: bool,
        is_clickable: bool,
        is_active: bool,
        on_click: Option<Box<dyn Fn(&mut Window, &mut App)>>,
    ) -> impl IntoElement {
        let (bg, border, text_color, dot_color) = if is_red {
            (
                hsla(350.0 / 360.0, 0.89, 0.60, 0.12),
                hsla(350.0 / 360.0, 0.89, 0.60, 0.35),
                hsla(350.0 / 360.0, 0.89, 0.60, 1.0),
                hsla(350.0 / 360.0, 0.89, 0.60, 1.0),
            )
        } else {
            (
                hsla(217.0 / 360.0, 0.91, 0.60, 0.12),
                hsla(217.0 / 360.0, 0.91, 0.60, 0.35),
                hsla(217.0 / 360.0, 0.91, 0.60, 1.0),
                hsla(217.0 / 360.0, 0.91, 0.60, 1.0),
            )
        };

        let mut pill = div()
            .flex()
            .items_center()
            .gap(px(5.0))
            .px(px(9.0))
            .py(px(3.0))
            .rounded_full()
            .border_1()
            .bg(bg)
            .border_color(border)
            .text_size(px(11.0))
            .font_weight(if is_active {
                FontWeight::EXTRA_BOLD
            } else {
                FontWeight::BOLD
            })
            .text_color(text_color)
            .child(div().size(px(5.0)).rounded_full().bg(dot_color))
            .child(text);

        if is_clickable {
            pill = pill
                .cursor_pointer()
                .opacity(if is_active { 1.0 } else { 0.7 })
                .hover(|s| s.opacity(0.95));
            if let Some(cb) = on_click {
                pill = pill.on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                    cb(win, app);
                });
            }
        }

        pill
    }

    /// 渲染左侧玩家详情大卡片 (PlayerCard) (G06)
    fn render_player_full_card<F>(
        player: &GameInfoPlayer,
        side_accent: Hsla,
        on_player_career: F,
        index: usize,
    ) -> gpui::AnyElement
    where
        F: Fn(Option<(String, String)>, &mut Window, &mut App) + 'static + Clone,
    {
        let is_local = player.is_local_player;
        let is_top = player
            .top_champions
            .first()
            .map(|t| t.champion_id == player.champion_id && player.champion_id > 0)
            .unwrap_or(false);

        let premade_style = player.premade_group_id.map(|idx| {
            let color = Theme::premade_color(idx);
            (color, color.opacity(0.08))
        });

        let (border_color, bg_color) = if let Some((border, bg)) = premade_style {
            (border.opacity(0.55), bg)
        } else if is_local {
            (side_accent.opacity(0.4), Theme::CARD_BG_HOVER())
        } else {
            (Theme::CARD_BORDER(), Theme::CARD_BG())
        };

        let player_puuid = player.puuid.clone();
        let on_career = on_player_career;

        let display_name = if player.summoner_name.is_empty() {
            format!("玩家 {}", index + 1)
        } else {
            player.summoner_name.clone()
        };

        let is_valid_target = !player.puuid.is_empty()
            && player.puuid != "00000000-0000-0000-0000-000000000000"
            && !player.is_bot
            && !player.summoner_name.is_empty()
            && !player.summoner_name.starts_with("玩家 ");

        div()
            .w_full()
            .rounded_xl()
            .border_1()
            .border_color(border_color)
            .bg(bg_color)
            .p(px(8.0))
            .flex()
            .flex_col()
            .gap(px(6.0))
            // 第一层：主体（左侧头像区 + 右侧昵称/段位/KDA） (G06)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    // 头像环区 (profile-icon-wrapper-mini)
                    .child(
                        div()
                            .relative()
                            .size(px(42.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            // 进度/招牌环
                            .child(
                                div()
                                    .size(px(42.0))
                                    .rounded_full()
                                    .border_2()
                                    .border_color(if is_top {
                                        Theme::GOLD()
                                    } else {
                                        Theme::CARD_BORDER()
                                    })
                                    .overflow_hidden()
                                    .child(if player.champion_id > 0 {
                                        LcuImage::new(format!(
                                            "/lol-game-data/assets/v1/champion-icons/{}.png",
                                            player.champion_id
                                        ))
                                        .size(px(42.0))
                                        .shape(LcuImageShape::Circle)
                                        .placeholder("?".to_string())
                                        .into_any_element()
                                    } else {
                                        div()
                                            .size(px(42.0))
                                            .rounded_full()
                                            .bg(Theme::CARD_BG())
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_size(px(14.0))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(Theme::TEXT_MUTED())
                                            .child("?")
                                            .into_any_element()
                                    }),
                            )
                            // 等级徽标
                            .when(player.summoner_level > 0, |el| {
                                el.child(
                                    div()
                                        .absolute()
                                        .bottom(px(-2.0))
                                        .bg(rgba(0x0f172abb))
                                        .px(px(4.0))
                                        .py(px(0.5))
                                        .rounded_full()
                                        .text_size(px(8.5))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(gpui::white())
                                        .child(player.summoner_level.to_string()),
                                )
                            }),
                    )
                    // 右侧信息区
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(1.5))
                            // 第 1 行：玩家昵称 (点击进生涯, G06) + 锁 + 宿命/标记/连胜徽标
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(4.0))
                                    .min_w_0()
                                    .child({
                                        let mut name_el = div()
                                            .text_size(px(12.5))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(Theme::TEXT_PRIMARY())
                                            .truncate();
                                        if is_valid_target {
                                            let is_local_player = is_local;
                                            let target_puuid = player_puuid.clone();
                                            // P0-4：显示名随 puuid 一并传给生涯页，供按名回退
                                            let target_name = display_name.clone();
                                            let on_career = on_career.clone();
                                            name_el = name_el
                                                .cursor_pointer()
                                                .hover(|s| s.text_color(Theme::ACCENT_PRIMARY()))
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    move |_ev, win, app| {
                                                        if is_local_player {
                                                            on_career(None, win, app);
                                                        } else {
                                                            on_career(
                                                                Some((
                                                                    target_puuid.clone(),
                                                                    target_name.clone(),
                                                                )),
                                                                win,
                                                                app,
                                                            );
                                                        }
                                                    },
                                                );
                                        }
                                        name_el.child(display_name)
                                    })
                                    // 战绩隐藏锁
                                    .when(!player.data_available, |el| {
                                        el.child(icon(
                                            "icons/lock.svg",
                                            11.0,
                                            hsla(38.0 / 360.0, 0.92, 0.50, 1.0),
                                        ))
                                    })
                                    // 宿命徽标
                                    .when_some(player.fate.as_ref(), |el, fate| {
                                        el.child(
                                            div()
                                                .px(px(4.0))
                                                .py(px(1.0))
                                                .rounded(px(3.0))
                                                .bg(if fate.ally {
                                                    rgba(0x0577482e)
                                                } else {
                                                    rgba(0xbf242a26)
                                                })
                                                .text_size(px(9.0))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(if fate.ally {
                                                    Theme::WIN_TEXT()
                                                } else {
                                                    Theme::LOSS_TEXT()
                                                })
                                                .child(fate.badge_text()),
                                        )
                                    })
                                    // 路人集标记徽标（大腿 / 避坑 / 绝活等）
                                    .when_some(player.saved_tag.clone(), |el, tag| {
                                        let (tag_bg, tag_text, tag_border) = tag_badge_colors(&tag);
                                        el.child(
                                            div()
                                                .px(px(4.0))
                                                .py(px(1.0))
                                                .rounded(px(3.0))
                                                .bg(tag_bg)
                                                .border_1()
                                                .border_color(tag_border)
                                                .text_size(px(9.0))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(tag_text)
                                                .child(tag),
                                        )
                                    })
                                    // 相遇次数徽标（无自定义 tag 且同局相遇 >= 2 次）
                                    .when(
                                        player.saved_tag.is_none()
                                            && player.saved_encounter_count >= 2,
                                        |el| {
                                            el.child(
                                                div()
                                                    .px(px(4.0))
                                                    .py(px(1.0))
                                                    .rounded(px(3.0))
                                                    .bg(Theme::CARD_BORDER())
                                                    .text_size(px(9.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(Theme::TEXT_MUTED())
                                                    .child(format!(
                                                        "相遇{}次",
                                                        player.saved_encounter_count
                                                    )),
                                            )
                                        },
                                    )
                                    // 连胜 / 连败微徽标 (>=2 场显示, G06)
                                    .when_some(player.streak_text.clone(), |el, streak| {
                                        el.child(
                                            div()
                                                .px(px(4.0))
                                                .py(px(1.0))
                                                .rounded(px(3.0))
                                                .bg(if player.streak_is_win {
                                                    Theme::WIN_BG()
                                                } else {
                                                    Theme::LOSS_BG()
                                                })
                                                .text_size(px(9.0))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(if player.streak_is_win {
                                                    Theme::WIN_TEXT()
                                                } else {
                                                    Theme::LOSS_TEXT()
                                                })
                                                .child(streak),
                                        )
                                    }),
                            )
                            // 第 2 行：排位段位展示
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(4.0))
                                    .text_size(px(11.0))
                                    .child(
                                        div()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(tier_color(&player.rank_tier))
                                            .child(player.rank_display.clone()),
                                    )
                                    .when(player.ranked_wins > 0, |el| {
                                        el.child(
                                            div()
                                                .text_color(Theme::TEXT_MUTED())
                                                .child(format!("· {}胜", player.ranked_wins)),
                                        )
                                    }),
                            )
                            // 第 3 行：近 10 场胜率与 KDA
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(4.0))
                                    .text_size(px(11.0))
                                    .child(
                                        div()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(if player.win_rate >= 50 {
                                                Theme::WIN_TEXT()
                                            } else {
                                                Theme::LOSS_TEXT()
                                            })
                                            .child(format!("{}%", player.win_rate)),
                                    )
                                    .child(div().text_color(Theme::TEXT_MUTED()).child("·"))
                                    .child(
                                        div()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(kda_color(player.avg_kda))
                                            .child(format!("KDA {:.2}", player.avg_kda)),
                                    ),
                            ),
                    ),
            )
            // 第二层：底部画像（常玩位置 + 常用英雄） (G06)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pt(px(4.0))
                    .border_t_1()
                    .border_color(Theme::CARD_BORDER().opacity(0.5))
                    // 左侧：位置/角色
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(Theme::TEXT_MUTED())
                            .child(position_display(&player.position)),
                    )
                    // 右侧：近期常用英雄小头像
                    .child(div().flex().items_center().gap(px(4.0)).children(
                        player.top_champions.iter().take(3).map(|c| {
                            div()
                                .size(px(18.0))
                                .rounded_full()
                                .border_1()
                                .border_color(Theme::CARD_BORDER())
                                .overflow_hidden()
                                .child(
                                    LcuImage::new(format!(
                                        "/lol-game-data/assets/v1/champion-icons/{}.png",
                                        c.champion_id
                                    ))
                                    .size(px(18.0))
                                    .shape(LcuImageShape::Circle)
                                    .placeholder("?".to_string()),
                                )
                        }),
                    )),
            )
            .into_any_element()
    }

    /// 渲染一列玩家战绩流（列头 Header + 垂直 MatchCard 列表） (G07, G08, G10)
    #[allow(clippy::too_many_arguments)]
    fn render_player_match_column<F>(
        player: &GameInfoPlayer,
        side_accent: Hsla,
        _is_ally: bool,
        compact: bool,
        is_first_enemy: bool,
        game_info_filter: bool,
        queue_id: i32,
        on_player_search: F,
        index: usize,
    ) -> gpui::AnyElement
    where
        F: Fn(String, Option<String>, &mut Window, &mut App) + 'static + Clone,
    {
        let premade_bg = player.premade_group_id.map(Theme::premade_color);
        let is_top = player
            .top_champions
            .first()
            .map(|t| t.champion_id == player.champion_id && player.champion_id > 0)
            .unwrap_or(false);

        let player_name = player.summoner_name.clone();
        let player_tag = player.tag_line.clone();

        let display_name = if player.summoner_name.is_empty() {
            format!("玩家 {}", index + 1)
        } else {
            player.summoner_name.clone()
        };

        let is_valid_target = !player.puuid.is_empty()
            && player.puuid != "00000000-0000-0000-0000-000000000000"
            && !player.is_bot
            && !player.summoner_name.is_empty()
            && !player.summoner_name.starts_with("玩家 ");

        // 列头 (G07)
        let col_header = if compact {
            // 10 列紧凑模式 (compact 分支)：三行紧凑布局
            div()
                .h(px(58.0))
                .flex_shrink_0()
                .p(px(3.0))
                .border_b_1()
                .border_color(Theme::CARD_BORDER())
                .bg(if let Some(col) = premade_bg {
                    col.opacity(0.12)
                } else {
                    Theme::CARD_BG().opacity(0.85)
                })
                .flex()
                .flex_col()
                .justify_between()
                // 前两行：左侧头像与胜率/胜负 + 右侧名称与段位
                .child(
                    div()
                        .flex()
                        .items_start()
                        .gap(px(3.5))
                        // 头像列（含胜率竖条、头像、下方胜负）
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_start()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(2.0))
                                        // 胜率竖条 (wr-bar, G07)
                                        .child({
                                            let diff = (player.win_rate as f32 - 50.0).abs();
                                            let fill_h = (26.0 * (diff / 50.0).min(1.0)).max(2.0);
                                            let bar_color = if player.win_rate >= 50 {
                                                Theme::WIN_BAR()
                                            } else {
                                                Theme::LOSS_BAR()
                                            };
                                            div()
                                                .w(px(3.0))
                                                .h(px(26.0))
                                                .rounded(px(1.5))
                                                .bg(Theme::CARD_BORDER())
                                                .overflow_hidden()
                                                .relative()
                                                .child(
                                                    div()
                                                        .absolute()
                                                        .bottom_0()
                                                        .w_full()
                                                        .h(px(fill_h))
                                                        .bg(bar_color),
                                                )
                                        })
                                        // 英雄头像 (26x26, G07, G10)
                                        .child(
                                            div()
                                                .relative()
                                                .size(px(26.0))
                                                .rounded(px(5.0))
                                                .border_1()
                                                .border_color(if is_top {
                                                    Theme::GOLD()
                                                } else {
                                                    Theme::CARD_BORDER()
                                                })
                                                .overflow_hidden()
                                                .child(if player.champion_id > 0 {
                                                    LcuImage::new(format!(
                                                        "/lol-game-data/assets/v1/champion-icons/{}.png",
                                                        player.champion_id
                                                    ))
                                                    .size(px(26.0))
                                                    .shape(LcuImageShape::Rounded)
                                                    .placeholder("?".to_string())
                                                    .into_any_element()
                                                } else {
                                                    div()
                                                        .size(px(26.0))
                                                        .bg(Theme::CARD_BG())
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .text_size(px(10.0))
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_color(Theme::TEXT_MUTED())
                                                        .child("?")
                                                        .into_any_element()
                                                })
                                                // 招牌星标角标 (G07)
                                                .when(is_top, |el| {
                                                    el.child(
                                                        div()
                                                            .absolute()
                                                            .top(px(-1.0))
                                                            .right(px(-1.0))
                                                            .size(px(9.0))
                                                            .rounded_full()
                                                            .bg(Theme::GOLD())
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .text_size(px(6.0))
                                                            .text_color(gpui::white())
                                                            .child("★"),
                                                    )
                                                }),
                                        ),
                                )
                                // 头像下方胜负统计 (G07)
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(1.5))
                                        .text_size(px(8.5))
                                        .font_weight(FontWeight::BOLD)
                                        .child(
                                            div()
                                                .text_color(Theme::WIN_TEXT())
                                                .child(format!("{}胜", player.ranked_wins)),
                                        )
                                        .child(
                                            div()
                                                .text_color(Theme::LOSS_TEXT())
                                                .child(format!("{}负", player.ranked_losses)),
                                        ),
                                ),
                        )
                        // 右侧信息：第一行昵称(点击查战绩)，第二行短段位
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(1.0))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(2.0))
                                        .child({
                                            let mut name_el = div()
                                                .text_size(px(10.5))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(Theme::TEXT_PRIMARY())
                                                .truncate();
                                            if is_valid_target {
                                                let on_search = on_player_search.clone();
                                                let p_name = player_name.clone();
                                                let p_tag = player_tag.clone();
                                                name_el = name_el
                                                    .cursor_pointer()
                                                    .hover(|s| s.text_color(Theme::ACCENT_PRIMARY()))
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        move |_ev, win, app| {
                                                            on_search(
                                                                p_name.clone(),
                                                                if p_tag.is_empty() { None } else { Some(p_tag.clone()) },
                                                                win,
                                                                app,
                                                            );
                                                        },
                                                    );
                                            }
                                            name_el.child(display_name.clone())
                                        })
                                        .when(!player.data_available, |el| {
                                            el.child(icon(
                                                "icons/lock.svg",
                                                9.0,
                                                hsla(38.0 / 360.0, 0.92, 0.50, 1.0),
                                            ))
                                        }),
                                )
                                .child(
                                    div()
                                        .text_size(px(9.5))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(tier_color(&player.rank_tier))
                                        .truncate()
                                        .child(tier_short_name(
                                            &player.rank_tier,
                                            &player.rank_division,
                                        )),
                                ),
                        ),
                )
                // 第三行：左侧常用位置，右侧宿命/标记徽标 (G07)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .text_size(px(8.5))
                        .text_color(Theme::TEXT_MUTED())
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .child(format!("{}{} %", short_position(&player.position), player.win_rate)),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(2.0))
                                .when_some(player.fate.as_ref(), |el, fate| {
                                    el.child(
                                        div()
                                            .size(px(12.0))
                                            .rounded(px(2.5))
                                            .bg(if fate.ally {
                                                rgba(0x0577482e)
                                            } else {
                                                rgba(0xbf242a26)
                                            })
                                            .text_size(px(8.0))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(if fate.ally {
                                                Theme::WIN_TEXT()
                                            } else {
                                                Theme::LOSS_TEXT()
                                            })
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(if fate.ally { "友" } else { "敌" }),
                                    )
                                })
                                .when_some(player.saved_tag.clone(), |el, tag| {
                                    let (tag_bg, tag_text, _) = tag_badge_colors(&tag);
                                    el.child(
                                        div()
                                            .px(px(2.5))
                                            .py(px(0.5))
                                            .rounded(px(2.0))
                                            .bg(tag_bg)
                                            .text_size(px(8.0))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(tag_text)
                                            .child(tag),
                                    )
                                }),
                        ),
                )
        } else {
            // 5 列正常模式 (!compact 分支)：高度 48px，左右布局 (G07)
            div()
                .h(px(48.0))
                .flex_shrink_0()
                .p(px(6.0))
                .border_b_1()
                .border_color(Theme::CARD_BORDER())
                .bg(if let Some(col) = premade_bg {
                    col.opacity(0.12)
                } else {
                    Theme::CARD_BG().opacity(0.85)
                })
                .flex()
                .items_center()
                .gap(px(6.0))
                // 英雄头像 (28x28, G07, G10)
                .child(
                    div()
                        .size(px(28.0))
                        .rounded(px(6.0))
                        .border_1()
                        .border_color(if is_top {
                            Theme::GOLD()
                        } else {
                            Theme::CARD_BORDER()
                        })
                        .overflow_hidden()
                        .child(if player.champion_id > 0 {
                            LcuImage::new(format!(
                                "/lol-game-data/assets/v1/champion-icons/{}.png",
                                player.champion_id
                            ))
                            .size(px(28.0))
                            .shape(LcuImageShape::Rounded)
                            .placeholder("?".to_string())
                            .into_any_element()
                        } else {
                            div()
                                .size(px(28.0))
                                .bg(Theme::CARD_BG())
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(11.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(Theme::TEXT_MUTED())
                                .child("?")
                                .into_any_element()
                        }),
                )
                // 信息区：昵称 + 胜负/段位 + 标记徽标
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(1.5))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(3.0))
                                .child({
                                    let mut name_el = div()
                                        .text_size(px(11.5))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(Theme::TEXT_PRIMARY())
                                        .truncate();
                                    if is_valid_target {
                                        let on_search = on_player_search.clone();
                                        let p_name = player_name.clone();
                                        let p_tag = player_tag.clone();
                                        name_el = name_el
                                            .cursor_pointer()
                                            .hover(|s| s.text_color(Theme::ACCENT_PRIMARY()))
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                move |_ev, win, app| {
                                                    on_search(
                                                        p_name.clone(),
                                                        if p_tag.is_empty() {
                                                            None
                                                        } else {
                                                            Some(p_tag.clone())
                                                        },
                                                        win,
                                                        app,
                                                    );
                                                },
                                            );
                                    }
                                    name_el.child(display_name)
                                })
                                .when(!player.data_available, |el| {
                                    el.child(icon(
                                        "icons/lock.svg",
                                        10.0,
                                        hsla(38.0 / 360.0, 0.92, 0.50, 1.0),
                                    ))
                                })
                                .when_some(player.fate.as_ref(), |el, fate| {
                                    el.child(
                                        div()
                                            .px(px(3.0))
                                            .py(px(0.5))
                                            .rounded(px(2.0))
                                            .bg(if fate.ally {
                                                rgba(0x0577482e)
                                            } else {
                                                rgba(0xbf242a26)
                                            })
                                            .text_size(px(8.5))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(if fate.ally {
                                                Theme::WIN_TEXT()
                                            } else {
                                                Theme::LOSS_TEXT()
                                            })
                                            .child(fate.badge_text()),
                                    )
                                })
                                .when_some(player.saved_tag.clone(), |el, tag| {
                                    let (tag_bg, tag_text, tag_border) = tag_badge_colors(&tag);
                                    el.child(
                                        div()
                                            .px(px(3.0))
                                            .py(px(0.5))
                                            .rounded(px(2.0))
                                            .bg(tag_bg)
                                            .border_1()
                                            .border_color(tag_border)
                                            .text_size(px(8.5))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(tag_text)
                                            .child(tag),
                                    )
                                })
                                .when(
                                    player.saved_tag.is_none() && player.saved_encounter_count >= 2,
                                    |el| {
                                        el.child(
                                            div()
                                                .px(px(3.0))
                                                .py(px(0.5))
                                                .rounded(px(2.0))
                                                .bg(Theme::CARD_BORDER())
                                                .text_size(px(8.5))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(Theme::TEXT_MUTED())
                                                .child(format!(
                                                    "相遇{}次",
                                                    player.saved_encounter_count
                                                )),
                                        )
                                    },
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .text_size(px(10.0))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(2.0))
                                        .child(
                                            div()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(Theme::WIN_TEXT())
                                                .child(format!("{}胜", player.ranked_wins)),
                                        )
                                        .child(
                                            div()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(Theme::LOSS_TEXT())
                                                .child(format!("{}负", player.ranked_losses)),
                                        ),
                                )
                                .child(
                                    div()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(tier_color(&player.rank_tier))
                                        .truncate()
                                        .child(tier_short_name(
                                            &player.rank_tier,
                                            &player.rank_division,
                                        )),
                                ),
                        ),
                )
        };

        // 垂直战绩列表：独立纵向滚动，每人严格前 10 局 (G08, G10)
        let filtered_matches: Vec<&MatchDisplay> = if game_info_filter && queue_id > 0 {
            let q_matches: Vec<&MatchDisplay> = player
                .matches
                .iter()
                .filter(|m| m.queue_id == queue_id)
                .collect();
            if q_matches.is_empty() {
                player.matches.iter().collect()
            } else {
                q_matches
            }
        } else {
            player.matches.iter().collect()
        };

        let matches_pane = if filtered_matches.is_empty() {
            if !player.data_available {
                div().flex_1().into_any_element()
            } else {
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(4.0))
                    .text_color(Theme::TEXT_MUTED())
                    .child(div().text_size(px(22.0)).child("📭"))
                    .child(div().text_size(px(11.0)).child("暂无近况战绩"))
                    .into_any_element()
            }
        } else {
            // 固定 10 个等分槽位：不足 10 局时用透明占位撑满，保证卡片高度始终与满 10 局的列一致，
            // 同时 10 张卡片均分列表可用高度，刚好占满整列（不再在底部留白）。
            let mut slots: Vec<gpui::AnyElement> = filtered_matches
                .iter()
                .take(10)
                .map(|m| Self::render_match_card(m, compact))
                .collect();
            let empty_slots = 10usize.saturating_sub(slots.len());
            slots.extend((0..empty_slots).map(|_| div().flex_1().into_any_element()));

            let matches_list = div()
                .id(ElementId::NamedInteger(
                    "col-matches-scroll".into(),
                    index as u64,
                ))
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p(if compact { px(3.0) } else { px(6.0) })
                .flex()
                .flex_col()
                .gap(if compact { px(4.0) } else { px(6.0) })
                .children(slots);
            matches_list.into_any_element()
        };

        let mut column = div()
            .flex_1()
            .min_w(if compact { px(68.0) } else { px(96.0) })
            .h_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .border_r_1()
            .border_color(Theme::CARD_BORDER())
            .child(div().h(px(3.0)).w_full().bg(side_accent))
            .child(col_header)
            .child(matches_pane);

        // 十列视图下两队之间的队伍分割线 (G10)
        if is_first_enemy {
            column = column.border_l_2();
        }

        column.into_any_element()
    }

    /// 渲染单场战绩卡片 (MatchCard) (G08, G10)
    fn render_match_card(m: &MatchDisplay, compact: bool) -> gpui::AnyElement {
        let is_win = m.win;
        let is_remake = m.remake;

        // 对齐 Vue .col-match 的 135° 浅色渐变：首端约为状态色的 70%、末端 20%
        // （默认胜负色即 14% → 4%）。色相仍取自运行时胜负/重开色，保留自定义配色；
        // 两端预合成到页面底色，卡片阴影不再透进卡面。
        let (bg, border, text_color) = if is_remake {
            (
                Theme::gradient_over_page(
                    Theme::REMAKE_BG().opacity(0.7),
                    Theme::REMAKE_BG().opacity(0.2),
                ),
                Theme::REMAKE_BORDER(),
                Theme::REMAKE_TEXT(),
            )
        } else if is_win {
            (
                Theme::gradient_over_page(
                    Theme::WIN_BG().opacity(0.7),
                    Theme::WIN_BG().opacity(0.2),
                ),
                Theme::WIN_BORDER(),
                Theme::WIN_TEXT(),
            )
        } else {
            (
                Theme::gradient_over_page(
                    Theme::LOSS_BG().opacity(0.7),
                    Theme::LOSS_BG().opacity(0.2),
                ),
                Theme::LOSS_BORDER(),
                Theme::LOSS_TEXT(),
            )
        };

        let time_text = format_month_day(m.time_stamp);
        let card_avatar_size = if compact { px(29.0) } else { px(32.0) };

        div()
            .w_full()
            // 与占位槽位一起按 10 等分撑满列表高度，卡片高度随列高自适应；
            // 最小高度取内容实际所需，窗口过矮时改为列表滚动而不是卡片互相重叠
            .flex_1()
            .min_h(if compact { px(50.0) } else { px(58.0) })
            .p(if compact { px(3.0) } else { px(4.0) })
            .px(if compact { px(4.0) } else { px(7.0) })
            .rounded(if compact { px(6.0) } else { px(8.0) })
            .bg(bg)
            .border_1()
            .border_color(border)
            .flex()
            .flex_col()
            .justify_between()
            .gap(if compact { px(1.0) } else { px(2.0) })
            // 顶部行：模式名 + 日期 (G08)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_size(if compact { px(8.5) } else { px(9.5) })
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_color(text_color)
                            .truncate()
                            .child(if is_remake {
                                "重开".to_string()
                            } else {
                                m.name.clone()
                            }),
                    )
                    .child(
                        div()
                            .text_color(Theme::TEXT_MUTED())
                            .flex_shrink_0()
                            .child(time_text),
                    ),
            )
            // 主体行：英雄头像（带等级角标，支持双位数完整展示）+ KDA (G08, G10)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap(if compact { px(4.0) } else { px(6.0) })
                    .child(
                        div()
                            .relative()
                            .size(card_avatar_size)
                            .rounded(px(6.0))
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
                            // 等级角标 (G10: 双位数等级角标完整显示)
                            .child(
                                div()
                                    .absolute()
                                    .bottom(px(-1.0))
                                    .right(px(-1.0))
                                    .min_w(if compact { px(10.0) } else { px(11.0) })
                                    .h(if compact { px(10.0) } else { px(11.0) })
                                    .bg(rgba(0x0f172abb))
                                    .px(px(1.5))
                                    .rounded(px(2.5))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(if compact { px(7.0) } else { px(8.0) })
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(gpui::white())
                                    .child(m.champ_level.to_string()),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .justify_center()
                            // KDA 击杀/死亡/助攻 (D 为红字加粗, G08)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .text_size(if compact { px(11.0) } else { px(12.5) })
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY())
                                    .child(format!("{}", m.kills))
                                    .child(span_sep())
                                    .child(
                                        div()
                                            .font_weight(FontWeight::EXTRA_BOLD)
                                            .text_color(Theme::LOSS_TEXT())
                                            .child(format!("{}", m.deaths)),
                                    )
                                    .child(span_sep())
                                    .child(format!("{}", m.assists)),
                            )
                            // 五列模式额外展示 KDA 倍率，十列紧凑模式隐藏该行 (G08)
                            .when(!compact, |el| {
                                let (ratio_text, ratio_fg, ratio_bg) =
                                    format_kda_ratio(m.kills, m.deaths, m.assists);
                                el.child(
                                    div()
                                        .w_auto()
                                        .px(px(3.5))
                                        .py(px(0.5))
                                        .rounded(px(3.0))
                                        .bg(ratio_bg)
                                        .text_size(px(9.0))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(ratio_fg)
                                        .child(format!("{} KDA", ratio_text)),
                                )
                            }),
                    ),
            )
            .into_any_element()
    }

    /// 离线空态 (LcuOfflineState, G01)
    fn render_offline_state<F>(on_load_demo: F) -> gpui::AnyElement
    where
        F: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    {
        let demo_btn = div()
            .mt(px(20.0))
            .px(px(20.0))
            .py(px(9.0))
            .rounded_lg()
            .bg(Theme::ACCENT_PRIMARY())
            .hover(|this| this.opacity(0.85))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, on_load_demo)
            .flex()
            .items_center()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(13.5))
                    .font_weight(FontWeight::BOLD)
                    .text_color(gpui::white())
                    .child("预览对局信息"),
            )
            .into_any_element();

        render_lcu_offline_state(Some(demo_btn))
    }

    /// 统一提示框 (G01)
    fn render_tip_container<F>(
        icon_emoji: &'static str,
        tip_text: &'static str,
        on_load_demo: Option<F>,
    ) -> gpui::AnyElement
    where
        F: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    {
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .py_24()
            .px_8()
            .gap(px(12.0))
            .child(div().text_size(px(44.0)).child(icon_emoji))
            .child(
                div()
                    .text_size(px(13.5))
                    .text_color(Theme::TEXT_MUTED())
                    .child(tip_text),
            )
            .when_some(on_load_demo, |el, load_demo| {
                el.child(
                    div()
                        .mt(px(8.0))
                        .px(px(18.0))
                        .py(px(7.0))
                        .rounded_lg()
                        .bg(Theme::ACCENT_PRIMARY())
                        .hover(|this| this.opacity(0.85))
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, load_demo)
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(gpui::white())
                                .child("预览对局信息"),
                        ),
                )
            })
            .into_any_element()
    }
}

/// KDA 分隔符 `/`
fn span_sep() -> impl IntoElement {
    div()
        .text_color(Theme::TEXT_MUTED().opacity(0.5))
        .mx(px(2.0))
        .child("/")
}

/// 视图分段控制器：「按队伍」「全部战绩」两枚带图标按钮 (G02)
fn render_view_segmented_control<F>(
    view_mode: GameInfoViewMode,
    on_change_mode: F,
) -> impl IntoElement
where
    F: Fn(GameInfoViewMode, &mut Window, &mut App) + 'static + Clone,
{
    let on_team = on_change_mode.clone();
    let on_all = on_change_mode;

    div()
        .flex()
        .items_center()
        .p(px(2.0))
        .rounded(px(8.0))
        .bg(Theme::BADGE_BG())
        .border_1()
        .border_color(Theme::CARD_BORDER())
        .gap(px(2.0))
        // 按队伍
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.0))
                .px(px(8.0))
                .py(px(4.0))
                .rounded(px(6.0))
                .cursor_pointer()
                .bg(if view_mode == GameInfoViewMode::ByTeam {
                    Theme::CARD_BG()
                } else {
                    gpui::transparent_black()
                })
                .text_color(if view_mode == GameInfoViewMode::ByTeam {
                    Theme::ACCENT_PRIMARY()
                } else {
                    Theme::TEXT_MUTED()
                })
                .font_weight(FontWeight::BOLD)
                .text_size(px(11.5))
                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                    on_team(GameInfoViewMode::ByTeam, win, app);
                })
                .child(icon(
                    "icons/user.svg",
                    12.0,
                    if view_mode == GameInfoViewMode::ByTeam {
                        Theme::ACCENT_PRIMARY()
                    } else {
                        Theme::TEXT_MUTED()
                    },
                ))
                .child("按队伍"),
        )
        // 全部战绩
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.0))
                .px(px(8.0))
                .py(px(4.0))
                .rounded(px(6.0))
                .cursor_pointer()
                .bg(if view_mode == GameInfoViewMode::AllMatches {
                    Theme::CARD_BG()
                } else {
                    gpui::transparent_black()
                })
                .text_color(if view_mode == GameInfoViewMode::AllMatches {
                    Theme::ACCENT_PRIMARY()
                } else {
                    Theme::TEXT_MUTED()
                })
                .font_weight(FontWeight::BOLD)
                .text_size(px(11.5))
                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                    on_all(GameInfoViewMode::AllMatches, win, app);
                })
                // 四方网格图标
                .child(
                    div()
                        .size(px(11.0))
                        .flex()
                        .flex_col()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .child(div().size(px(4.5)).rounded(px(1.0)).bg(
                                    if view_mode == GameInfoViewMode::AllMatches {
                                        Theme::ACCENT_PRIMARY()
                                    } else {
                                        Theme::TEXT_MUTED()
                                    },
                                ))
                                .child(div().size(px(4.5)).rounded(px(1.0)).bg(
                                    if view_mode == GameInfoViewMode::AllMatches {
                                        Theme::ACCENT_PRIMARY()
                                    } else {
                                        Theme::TEXT_MUTED()
                                    },
                                )),
                        )
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .child(div().size(px(4.5)).rounded(px(1.0)).bg(
                                    if view_mode == GameInfoViewMode::AllMatches {
                                        Theme::ACCENT_PRIMARY()
                                    } else {
                                        Theme::TEXT_MUTED()
                                    },
                                ))
                                .child(div().size(px(4.5)).rounded(px(1.0)).bg(
                                    if view_mode == GameInfoViewMode::AllMatches {
                                        Theme::ACCENT_PRIMARY()
                                    } else {
                                        Theme::TEXT_MUTED()
                                    },
                                )),
                        ),
                )
                .child("全部战绩"),
        )
}

/// 组队芯片渲染组件 (PremadeGroupChips, G09)
fn render_premade_chips(groups: &[PremadeGroup]) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .children(groups.iter().map(|group| {
            let color = Theme::premade_color(group.group_id);
            div()
                .flex()
                .items_center()
                .gap(px(5.0))
                .px(px(6.0))
                .py(px(2.0))
                .rounded_full()
                .border_1()
                .border_color(color.opacity(0.5))
                .bg(color.opacity(0.12))
                // 队伍色小圆点
                .child(div().size(px(5.0)).rounded_full().bg(color))
                // 重叠成员小头像 (-4px 负边距, G09)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .children(group.members.iter().enumerate().map(|(idx, m)| {
                            let mut av = div()
                                .size(px(18.0))
                                .rounded_full()
                                .border_1()
                                .border_color(gpui::white().opacity(0.85))
                                .overflow_hidden();
                            if idx > 0 {
                                av = av.ml(px(-4.0));
                            }
                            if m.champion_id > 0 {
                                av.child(
                                    LcuImage::new(format!(
                                        "/lol-game-data/assets/v1/champion-icons/{}.png",
                                        m.champion_id
                                    ))
                                    .size(px(18.0))
                                    .shape(LcuImageShape::Circle)
                                    .placeholder("?".to_string()),
                                )
                            } else {
                                av.bg(gpui::white().opacity(0.2))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(8.5))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_MUTED())
                                    .child(if !m.summoner_name.is_empty() {
                                        m.summoner_name.chars().next().unwrap_or('?').to_string()
                                    } else {
                                        "?".to_string()
                                    })
                            }
                        })),
                )
        }))
}

/// 敌方未公开判定：选人阶段无有效身份时判定未公开 (G04)
pub fn is_their_team_revealed(their_team: &[GameInfoPlayer], phase: &str) -> bool {
    if their_team.is_empty() {
        return false;
    }
    if phase == "ChampSelect" {
        their_team.iter().any(|p| {
            p.summoner_id != 0
                || (!p.puuid.is_empty() && p.puuid != "00000000-0000-0000-0000-000000000000")
                || p.champion_id > 0
                || p.is_bot
                || (!p.summoner_name.is_empty()
                    && !p.summoner_name.starts_with("玩家")
                    && p.summoner_name != "未知")
        })
    } else {
        true
    }
}

/// 段位短名（如 "钻石1" / "大师" / "未定级"）
fn tier_short_name(tier: &str, division: &str) -> String {
    let t_short = match tier.trim().to_ascii_uppercase().as_str() {
        "IRON" => "黑铁",
        "BRONZE" => "青铜",
        "SILVER" => "白银",
        "GOLD" => "黄金",
        "PLATINUM" => "铂金",
        "EMERALD" => "翡翠",
        "DIAMOND" => "钻石",
        "MASTER" => "大师",
        "GRANDMASTER" => "宗师",
        "CHALLENGER" => "王者",
        _ => "未定级",
    };
    if t_short == "未定级" || matches!(t_short, "大师" | "宗师" | "王者") {
        return t_short.to_string();
    }
    let div = division.trim();
    if div.is_empty() || div.eq_ignore_ascii_case("NA") {
        t_short.to_string()
    } else {
        format!("{}{}", t_short, div)
    }
}

/// 段位主题色
fn tier_color(tier: &str) -> Hsla {
    match tier.trim().to_ascii_uppercase().as_str() {
        "IRON" => hsla(215.0 / 360.0, 0.08, 0.55, 1.0),
        "BRONZE" => hsla(30.0 / 360.0, 0.90, 0.38, 1.0),
        "SILVER" => hsla(212.0 / 360.0, 0.18, 0.58, 1.0),
        "GOLD" => hsla(38.0 / 360.0, 0.92, 0.44, 1.0),
        "PLATINUM" => hsla(174.0 / 360.0, 0.85, 0.31, 1.0),
        "EMERALD" => hsla(160.0 / 360.0, 0.95, 0.30, 1.0),
        "DIAMOND" => hsla(217.0 / 360.0, 0.91, 0.60, 1.0),
        "MASTER" => hsla(262.0 / 360.0, 0.89, 0.66, 1.0),
        "GRANDMASTER" => hsla(0.0 / 360.0, 0.84, 0.60, 1.0),
        "CHALLENGER" => hsla(38.0 / 360.0, 0.92, 0.50, 1.0),
        _ => Theme::TEXT_MUTED(),
    }
}

/// KDA 数值着色
fn kda_color(kda: f64) -> Hsla {
    if kda >= 5.0 {
        hsla(38.0 / 360.0, 0.92, 0.50, 1.0)
    } else if kda >= 4.0 {
        hsla(217.0 / 360.0, 0.91, 0.60, 1.0)
    } else if kda >= 3.0 {
        hsla(160.0 / 360.0, 0.84, 0.39, 1.0)
    } else {
        Theme::TEXT_MUTED()
    }
}

/// KDA 倍率与样式
fn format_kda_ratio(kills: i32, deaths: i32, assists: i32) -> (String, Hsla, Hsla) {
    if deaths == 0 {
        (
            "Perf".to_string(),
            Theme::GOLD(),
            Theme::GOLD().opacity(0.12),
        )
    } else {
        let r = (kills + assists) as f32 / deaths as f32;
        let text = if r >= 10.0 {
            format!("{:.0}", r)
        } else {
            format!("{:.1}", r)
        };
        if r >= 5.0 {
            (text, Theme::GOLD(), Theme::GOLD().opacity(0.12))
        } else if r >= 3.0 {
            (
                text,
                Theme::BLUE_SIDE_ACCENT(),
                Theme::BLUE_SIDE_ACCENT().opacity(0.12),
            )
        } else if r >= 2.0 {
            (text, Theme::WIN_TEXT(), Theme::WIN_TEXT().opacity(0.12))
        } else {
            (text, Theme::TEXT_MUTED(), Theme::BADGE_BG())
        }
    }
}

/// 常用位置简称（如 "中" / "上"）
fn short_position(pos: &str) -> &'static str {
    match pos.trim().to_ascii_uppercase().as_str() {
        "TOP" | "上路" => "上",
        "JUNGLE" | "打野" => "野",
        "MIDDLE" | "MID" | "中路" => "中",
        "BOTTOM" | "BOT" | "ADC" | "下路" => "下",
        "UTILITY" | "SUPPORT" | "SUP" | "辅助" => "辅",
        _ => "位",
    }
}

/// 常用位置全称展示
fn position_display(pos: &str) -> &'static str {
    match pos.trim().to_ascii_uppercase().as_str() {
        "TOP" | "上路" => "上路",
        "JUNGLE" | "打野" => "打野",
        "MIDDLE" | "MID" | "中路" => "中路",
        "BOTTOM" | "BOT" | "ADC" | "下路" => "下路",
        "UTILITY" | "SUPPORT" | "SUP" | "辅助" => "辅助",
        _ => "常玩位置",
    }
}

/// 格式化对局毫秒时间戳为月/日 (MM/DD)
fn format_month_day(timestamp: u64) -> String {
    if timestamp == 0 {
        return "近期".to_string();
    }
    if let Some(dt) = chrono::DateTime::from_timestamp_millis(timestamp as i64) {
        let local: chrono::DateTime<chrono::Local> = chrono::DateTime::from(dt);
        local.format("%m/%d").to_string()
    } else {
        "近期".to_string()
    }
}
