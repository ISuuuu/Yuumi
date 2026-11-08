use std::collections::HashMap;
use gpui::*;
use crate::components::badge::{Badge, BadgeVariant};
use crate::components::card::Card;
use crate::match_model::{GameDetail, GameDetailTeam, MatchItem};
use crate::theme;

/// 战绩查询主视图
pub struct SearchView {
    /// 搜索输入框文本
    pub search_query: String,
    /// 选中的队列过滤 (None: 全部, Some(420): 单双, Some(450): 大乱斗, Some(440): 灵活)
    pub active_queue_filter: Option<i32>,
    /// 历史对局列表
    pub matches: Vec<MatchItem>,
    /// 当前选中的对局 ID
    pub selected_game_id: Option<u64>,
    /// 对局详情缓存 (game_id -> GameDetail)
    pub details_cache: HashMap<u64, GameDetail>,
    /// 详情加载状态
    pub is_loading_detail: bool,
}

impl SearchView {
    pub fn new() -> Self {
        Self {
            search_query: String::new(),
            active_queue_filter: None,
            matches: Vec::new(),
            selected_game_id: None,
            details_cache: HashMap::new(),
            is_loading_detail: false,
        }
    }

    pub fn with_matches(mut self, matches: Vec<MatchItem>) -> Self {
        if let Some(first) = matches.first() {
            self.selected_game_id = Some(first.display.game_id);
        }
        self.matches = matches;
        self
    }

    /// 过滤后的战绩列表
    pub fn filtered_matches(&self) -> Vec<&MatchItem> {
        match self.active_queue_filter {
            None => self.matches.iter().collect(),
            Some(qid) => self
                .matches
                .iter()
                .filter(|m| m.display.queue_id == qid)
                .collect(),
        }
    }

    pub fn render(
        &self,
        on_select_match: impl Fn(u64, &mut Window, &mut App) + 'static + Clone,
        on_filter_queue: impl Fn(Option<i32>, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let filtered = self.filtered_matches();
        let selected_game_id = self
            .selected_game_id
            .or_else(|| filtered.first().map(|m| m.display.game_id));

        let current_detail = selected_game_id.and_then(|id| self.details_cache.get(&id));

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme::BG_MAIN)
            .p_4()
            .gap_3()
            .overflow_hidden()
            // 顶部搜索与模式筛选卡片
            .child(self.render_header_card(on_filter_queue))
            // 下方主内容：左侧列表 + 右侧对抗详情
            .child(
                div()
                    .flex()
                    .flex_1()
                    .gap_3()
                    .overflow_hidden()
                    // 左侧战绩流水卡片
                    .child(self.render_matches_list(filtered, selected_game_id, on_select_match))
                    // 右侧 10 人对抗详情卡片
                    .child(self.render_detail_panel(current_detail, selected_game_id)),
            )
    }

    /// 顶部搜索与快捷筛选
    fn render_header_card(
        &self,
        on_filter_queue: impl Fn(Option<i32>, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        Card::new()
            .p(px(12.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme::TEXT_PRIMARY)
                                    .child("召唤师查询"),
                            )
                            // 搜索输入胶囊
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .bg(theme::BG_INPUT)
                                    .border_1()
                                    .border_color(theme::BORDER_SECONDARY)
                                    .rounded(px(6.0))
                                    .px_3()
                                    .py_1()
                                    .w(px(280.0))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(if self.search_query.is_empty() {
                                                theme::TEXT_MUTED
                                            } else {
                                                theme::TEXT_PRIMARY
                                            })
                                            .child(if self.search_query.is_empty() {
                                                "输入召唤师名称#标签 (如: 玩家#CN1)".to_string()
                                            } else {
                                                self.search_query.clone()
                                            }),
                                    ),
                            )
                            // 查询高亮按钮
                            .child(
                                div()
                                    .px_3()
                                    .py_1()
                                    .bg(theme::PRIMARY)
                                    .text_color(theme::TEXT_ON_PRIMARY)
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .rounded(px(6.0))
                                    .cursor_pointer()
                                    .child("查询"),
                            ),
                    )
                    // 快捷队列过滤胶囊标签组
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child({
                                let cb = on_filter_queue.clone();
                                div()
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                        cb(None, win, app);
                                    })
                                    .child(
                                        Badge::new("全部模式")
                                            .variant(if self.active_queue_filter.is_none() {
                                                BadgeVariant::Primary
                                            } else {
                                                BadgeVariant::Ghost
                                            }),
                                    )
                            })
                            .child({
                                let cb = on_filter_queue.clone();
                                div()
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                        cb(Some(420), win, app);
                                    })
                                    .child(
                                        Badge::new("单双排位")
                                            .variant(if self.active_queue_filter == Some(420) {
                                                BadgeVariant::Primary
                                            } else {
                                                BadgeVariant::Ghost
                                            }),
                                    )
                            })
                            .child({
                                let cb = on_filter_queue.clone();
                                div()
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                        cb(Some(450), win, app);
                                    })
                                    .child(
                                        Badge::new("极地大乱斗")
                                            .variant(if self.active_queue_filter == Some(450) {
                                                BadgeVariant::Primary
                                            } else {
                                                BadgeVariant::Ghost
                                            }),
                                    )
                            })
                            .child({
                                let cb = on_filter_queue.clone();
                                div()
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                        cb(Some(440), win, app);
                                    })
                                    .child(
                                        Badge::new("灵活组排")
                                            .variant(if self.active_queue_filter == Some(440) {
                                                BadgeVariant::Primary
                                            } else {
                                                BadgeVariant::Ghost
                                            }),
                                    )
                            }),
                    ),
            )
    }

    /// 左侧战绩列表 (320px)
    fn render_matches_list(
        &self,
        filtered: Vec<&MatchItem>,
        selected_game_id: Option<u64>,
        on_select_match: impl Fn(u64, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        div()
            .id("search-matches-scroll")
            .flex()
            .flex_col()
            .w(px(320.0))
            .gap_2()
            .overflow_y_scroll()
            .children(filtered.into_iter().map(|item| {
                let is_selected = Some(item.display.game_id) == selected_game_id;
                let win = item.display.win;
                let game_id = item.display.game_id;
                let cb = on_select_match.clone();

                let border_color = if is_selected {
                    theme::PRIMARY
                } else if win {
                    theme::WIN_PRIMARY
                } else {
                    theme::LOSS_PRIMARY
                };

                div()
                    .flex()
                    .flex_col()
                    .bg(if is_selected {
                        theme::BG_ELEVATED
                    } else {
                        theme::BG_CARD
                    })
                    .border_l_4()
                    .border_color(border_color)
                    .border_1()
                    .border_color(if is_selected {
                        theme::PRIMARY
                    } else {
                        theme::BORDER_SECONDARY
                    })
                    .rounded_r(px(6.0))
                    .p_3()
                    .gap_1p5()
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                        cb(game_id, win, app);
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(if win {
                                                theme::WIN_PRIMARY
                                            } else {
                                                theme::LOSS_PRIMARY
                                            })
                                            .child(if win { "胜利" } else { "败北" }),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme::TEXT_MUTED)
                                            .child(item.display.name.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme::TEXT_MUTED)
                                    .child(item.display.duration.clone()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme::TEXT_PRIMARY)
                                            .child(format!("英雄 ID: {}", item.display.champion_id)),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme::TEXT_MUTED)
                                            .child(format!("Lv.{}", item.display.champ_level)),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme::TEXT_PRIMARY)
                                    .child(format!(
                                        "{}/{}/{}",
                                        item.display.kills, item.display.deaths, item.display.assists
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_xs()
                            .text_color(theme::TEXT_MUTED)
                            .child(format!("补刀: {} ({}金币)", item.display.cs, item.display.gold))
                            .child(item.display.short_time.clone()),
                    )
            }))
    }

    /// 右侧对抗详情面板
    fn render_detail_panel(
        &self,
        detail: Option<&GameDetail>,
        selected_game_id: Option<u64>,
    ) -> impl IntoElement {
        Card::new()
            .flex_1()
            .p_4()
            .overflow_hidden()
            .child(match detail {
                Some(d) => div()
                    .id("match-detail-scroll")
                    .flex()
                    .flex_col()
                    .size_full()
                    .gap_3()
                    .overflow_y_scroll()
                    // 头部大 Banner
                    .child(self.render_detail_banner(d))
                    // 蓝方队伍卡片
                    .child(self.render_team_card(&d.blue, true))
                    // 红方队伍卡片
                    .child(self.render_team_card(&d.red, false))
                    .into_any_element(),
                None => div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .size_full()
                    .gap_3()
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::TEXT_PRIMARY)
                            .child(if self.is_loading_detail {
                                "正在拉取对局 10 人详细对抗数据...".to_string()
                            } else if let Some(gid) = selected_game_id {
                                format!("已选择对局 ID: {}，等待加载详情", gid)
                            } else {
                                "请在左侧选择一场对局以查看 10 人详细对抗".to_string()
                            }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::TEXT_MUTED)
                            .child("支持查看红蓝双方完整 5v5 KDA、推塔/大龙等团队目标、出装及对英雄伤害"),
                    )
                    .into_any_element(),
            })
    }

    /// 详情顶部胜负 Banner
    fn render_detail_banner(&self, d: &GameDetail) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .p_3()
            .rounded(px(8.0))
            .bg(if d.win {
                theme::WIN_MUTED
            } else {
                theme::LOSS_MUTED
            })
            .border_1()
            .border_color(if d.win {
                theme::WIN_PRIMARY
            } else {
                theme::LOSS_PRIMARY
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .text_color(if d.win {
                                theme::WIN_PRIMARY
                            } else {
                                theme::LOSS_PRIMARY
                            })
                            .child(if d.win { "胜利" } else { "败北" }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme::TEXT_PRIMARY)
                                    .child(format!("{} · {}", d.map_name, d.queue_name)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme::TEXT_MUTED)
                                    .child(format!("时长: {} · 日期: {}", d.duration, d.date)),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::TEXT_MUTED)
                            .child(format!("Game ID: {}", d.game_id)),
                    )
                    .child(Badge::new(d.queue_name.clone()).ghost()),
            )
    }

    /// 队伍卡片 (蓝方 100 / 红方 200)
    fn render_team_card(&self, team: &GameDetailTeam, is_blue: bool) -> impl IntoElement {
        let max_damage = team
            .players
            .iter()
            .map(|p| p.damage)
            .max()
            .unwrap_or(1)
            .max(1);

        div()
            .flex()
            .flex_col()
            .bg(theme::BG_CARD)
            .border_1()
            .border_color(if team.win {
                theme::WIN_BORDER
            } else {
                theme::BORDER_SECONDARY
            })
            .rounded(px(8.0))
            .overflow_hidden()
            // 队伍标题条与目标资源
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .py_2()
                    .bg(if is_blue {
                        theme::PRIMARY_MUTED
                    } else {
                        theme::BG_ELEVATED
                    })
                    .border_b_1()
                    .border_color(theme::BORDER_SECONDARY)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(if is_blue {
                                        theme::PRIMARY
                                    } else {
                                        theme::TEXT_PRIMARY
                                    })
                                    .child(if is_blue { "蓝方队伍" } else { "红方队伍" }),
                            )
                            .child(
                                Badge::new(if team.win { "胜方" } else { "败方" })
                                    .variant(if team.win {
                                        BadgeVariant::Win
                                    } else {
                                        BadgeVariant::Loss
                                    }),
                            ),
                    )
                    // 团队目标（推塔、水晶、大龙、巨龙）
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .text_xs()
                            .text_color(theme::TEXT_SECONDARY)
                            .child(format!("总击杀: {}", team.kills))
                            .child(format!("防御塔: {}", team.tower_kills))
                            .child(format!("水晶: {}", team.inhibitor_kills))
                            .child(format!("大龙: {}", team.baron_kills))
                            .child(format!("巨龙: {}", team.dragon_kills)),
                    ),
            )
            // 队伍 5 人详细列表
            .child(
                div()
                    .flex()
                    .flex_col()
                    .children(team.players.iter().map(|p| {
                        let damage_pct = ((p.damage as f32 / max_damage as f32) * 100.0).clamp(0.0, 100.0);

                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_3()
                            .py_2()
                            .border_b_1()
                            .border_color(theme::BORDER_SECONDARY)
                            .hover(|s| s.bg(theme::BG_CARD_HOVER))
                            // 左侧：英雄头像/等级 + 玩家名称
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2p5()
                                    .w(px(180.0))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .size(px(32.0))
                                            .rounded_full()
                                            .bg(theme::BG_INPUT)
                                            .border_1()
                                            .border_color(theme::BORDER_SECONDARY)
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(theme::TEXT_PRIMARY)
                                            .child(format!("{}", p.champion_id)),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(theme::TEXT_PRIMARY)
                                                    .overflow_hidden()
                                                    .child(p.name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(theme::TEXT_MUTED)
                                                    .child(format!("等级 Lv.{}", p.level)),
                                            ),
                                    ),
                            )
                            // 装备栏 (6格 + 1饰品格)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .w(px(160.0))
                                    .children(p.item_ids.iter().map(|&item_id| {
                                        div()
                                            .size(px(20.0))
                                            .rounded(px(3.0))
                                            .bg(theme::BG_INPUT)
                                            .border_1()
                                            .border_color(theme::BORDER_SECONDARY)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_xs()
                                            .text_color(theme::TEXT_MUTED)
                                            .child(if item_id > 0 {
                                                format!("{}", item_id % 100)
                                            } else {
                                                "".to_string()
                                            })
                                    }))
                                    .child(
                                        div()
                                            .size(px(20.0))
                                            .rounded_full()
                                            .bg(theme::BG_ELEVATED)
                                            .border_1()
                                            .border_color(theme::BORDER_SECONDARY)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_xs()
                                            .text_color(theme::TEXT_MUTED)
                                            .child("眼"),
                                    ),
                            )
                            // KDA
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .w(px(80.0))
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(theme::TEXT_PRIMARY)
                                            .child(format!("{}/{}/{}", p.kills, p.deaths, p.assists)),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme::TEXT_MUTED)
                                            .child(if p.deaths == 0 {
                                                "完美".to_string()
                                            } else {
                                                format!("{:.1} KDA", (p.kills + p.assists) as f32 / p.deaths as f32)
                                            }),
                                    ),
                            )
                            // 补刀与金币
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .items_end()
                                    .w(px(70.0))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme::TEXT_PRIMARY)
                                            .child(format!("{} 刀", p.cs)),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme::GOLD)
                                            .child(format!("{:.1}k", p.gold as f32 / 1000.0)),
                                    ),
                            )
                            // 伤害进度条与数值
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .w(px(100.0))
                                    .child(
                                        div()
                                            .flex()
                                            .justify_between()
                                            .text_xs()
                                            .child(
                                                div()
                                                    .text_color(theme::TEXT_SECONDARY)
                                                    .child(format!("{}", p.damage)),
                                            )
                                            .child(
                                                div()
                                                    .text_color(theme::TEXT_MUTED)
                                                    .child(format!("{:.0}%", damage_pct)),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .h(px(4.0))
                                            .w_full()
                                            .rounded(px(2.0))
                                            .bg(theme::BG_INPUT)
                                            .overflow_hidden()
                                            .child(
                                                div()
                                                    .h_full()
                                                    .w(relative(damage_pct / 100.0))
                                                    .rounded(px(2.0))
                                                    .bg(if is_blue {
                                                        theme::PRIMARY
                                                    } else {
                                                        theme::LOSS_PRIMARY
                                                    }),
                                            ),
                                    ),
                            )
                    })),
            )
    }
}
