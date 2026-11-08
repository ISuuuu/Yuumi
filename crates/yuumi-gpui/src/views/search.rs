use crate::components::badge::{Badge, BadgeVariant};
use crate::components::card::Card;
use crate::match_model::{GameDetail, GameDetailTeam, MatchItem};
use crate::theme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::collections::HashMap;

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
    /// 是否正在进行网络搜索召唤师/战绩
    pub is_searching: bool,
    /// 搜索失败或空态错误提示
    pub search_error: Option<String>,
    /// 当前查询到的召唤师显示名
    pub current_summoner_name: Option<String>,
    /// 键盘焦点句柄
    pub focus_handle: FocusHandle,
}

impl SearchView {
    pub fn new(cx: &mut App) -> Self {
        Self {
            search_query: String::new(),
            active_queue_filter: None,
            matches: Vec::new(),
            selected_game_id: None,
            details_cache: HashMap::new(),
            is_loading_detail: false,
            is_searching: false,
            search_error: None,
            current_summoner_name: None,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn with_matches(mut self, matches: Vec<MatchItem>) -> Self {
        if let Some(first) = matches.first() {
            self.selected_game_id = Some(first.display.game_id);
        }
        self.matches = matches;
        self
    }

    /// 设置搜索查询文本
    pub fn set_search_query(&mut self, query: impl Into<String>) {
        self.search_query = query.into();
    }

    /// 设置搜索状态
    pub fn set_searching(&mut self, is_searching: bool) {
        self.is_searching = is_searching;
        if is_searching {
            self.search_error = None;
        }
    }

    /// 设置搜索失败提示
    pub fn set_search_error(&mut self, err: impl Into<String>) {
        self.search_error = Some(err.into());
        self.is_searching = false;
    }

    /// 填充搜索结果（召唤师信息及对局列表）
    pub fn set_search_results(&mut self, summoner_name: String, matches: Vec<MatchItem>) {
        self.current_summoner_name = Some(summoner_name);
        self.is_searching = false;
        self.search_error = None;
        if let Some(first) = matches.first() {
            self.selected_game_id = Some(first.display.game_id);
        } else {
            self.selected_game_id = None;
        }
        self.matches = matches;
    }

    /// 添加单局详情缓存
    pub fn insert_game_detail(&mut self, game_id: u64, detail: GameDetail) {
        self.details_cache.insert(game_id, detail);
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
        on_search: impl Fn(&str, &mut Window, &mut App) + 'static + Clone,
        on_input_change: impl Fn(String, &mut Window, &mut App) + 'static + Clone,
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
            .child(self.render_header_card(on_filter_queue, on_search, on_input_change))
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
        on_search: impl Fn(&str, &mut Window, &mut App) + 'static + Clone,
        on_input_change: impl Fn(String, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let query_text = self.search_query.clone();
        let is_searching = self.is_searching;

        Card::new().p(px(12.0)).child(
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
                        .child({
                            let q_for_key = query_text.clone();
                            let on_search_key = on_search.clone();
                            let on_input_key = on_input_change.clone();
                            let focus_handle = self.focus_handle.clone();
                            div()
                                .id("search-query-input")
                                .track_focus(&focus_handle)
                                .flex()
                                .items_center()
                                .justify_between()
                                .bg(theme::BG_INPUT)
                                .border_1()
                                .border_color(if is_searching {
                                    theme::PRIMARY
                                } else {
                                    theme::BORDER_SECONDARY
                                })
                                .rounded(px(6.0))
                                .px_3()
                                .py_1()
                                .w(px(280.0))
                                .on_mouse_down(MouseButton::Left, move |_ev, win, _app| {
                                    win.focus(&focus_handle);
                                })
                                .on_key_down(move |ev: &KeyDownEvent, win, app| {
                                    if is_searching {
                                        return;
                                    }
                                    let key_str = &ev.keystroke.key;
                                    if key_str == "enter" {
                                        on_search_key(&q_for_key, win, app);
                                    } else if key_str == "backspace" {
                                        let mut s = q_for_key.clone();
                                        s.pop();
                                        on_input_key(s, win, app);
                                    } else if let Some(ref c) = ev.keystroke.key_char {
                                        let mut s = q_for_key.clone();
                                        s.push_str(c);
                                        on_input_key(s, win, app);
                                    }
                                })
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme::TEXT_MUTED)
                                                .child("🔍"),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(if self.search_query.is_empty() {
                                                    theme::TEXT_MUTED
                                                } else {
                                                    theme::TEXT_PRIMARY
                                                })
                                                .child(if self.search_query.is_empty() {
                                                    "输入召唤师名称#标签 (按回车或点击查询)"
                                                        .to_string()
                                                } else {
                                                    self.search_query.clone()
                                                }),
                                        ),
                                )
                                .when(is_searching, |el| {
                                    el.child(div().text_xs().text_color(theme::PRIMARY).child("⏳"))
                                })
                        })
                        // 查询高亮按钮
                        .child({
                            let q = query_text.clone();
                            let cb = on_search.clone();
                            div()
                                .id("btn-search-trigger")
                                .flex()
                                .items_center()
                                .gap_1p5()
                                .px_3()
                                .py_1()
                                .bg(if is_searching {
                                    theme::PRIMARY_MUTED
                                } else {
                                    theme::PRIMARY
                                })
                                .text_color(theme::TEXT_ON_PRIMARY)
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .rounded(px(6.0))
                                .cursor_pointer()
                                .hover(|s| s.opacity(0.85))
                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                    if !is_searching {
                                        cb(&q, win, app);
                                    }
                                })
                                .child(if is_searching {
                                    "查询中..."
                                } else {
                                    "查询"
                                })
                        })
                        // 提示信息（例如当前玩家名或错误）
                        .when(
                            self.current_summoner_name.is_some() || self.search_error.is_some(),
                            |el| {
                                if let Some(err) = &self.search_error {
                                    el.child(
                                        div()
                                            .text_xs()
                                            .text_color(theme::LOSS_PRIMARY)
                                            .child(err.clone()),
                                    )
                                } else if let Some(name) = &self.current_summoner_name {
                                    el.child(
                                        div()
                                            .text_xs()
                                            .text_color(theme::TEXT_MUTED)
                                            .child(format!("当前: {}", name)),
                                    )
                                } else {
                                    el
                                }
                            },
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
                                .child(Badge::new("全部模式").variant(
                                    if self.active_queue_filter.is_none() {
                                        BadgeVariant::Primary
                                    } else {
                                        BadgeVariant::Ghost
                                    },
                                ))
                        })
                        .child({
                            let cb = on_filter_queue.clone();
                            div()
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                    cb(Some(420), win, app);
                                })
                                .child(Badge::new("单双排位").variant(
                                    if self.active_queue_filter == Some(420) {
                                        BadgeVariant::Primary
                                    } else {
                                        BadgeVariant::Ghost
                                    },
                                ))
                        })
                        .child({
                            let cb = on_filter_queue.clone();
                            div()
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                    cb(Some(450), win, app);
                                })
                                .child(Badge::new("极地大乱斗").variant(
                                    if self.active_queue_filter == Some(450) {
                                        BadgeVariant::Primary
                                    } else {
                                        BadgeVariant::Ghost
                                    },
                                ))
                        })
                        .child({
                            let cb = on_filter_queue.clone();
                            div()
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                    cb(Some(440), win, app);
                                })
                                .child(Badge::new("灵活组排").variant(
                                    if self.active_queue_filter == Some(440) {
                                        BadgeVariant::Primary
                                    } else {
                                        BadgeVariant::Ghost
                                    },
                                ))
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
        if self.is_searching {
            return div()
                .id("search-matches-scroll")
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .w(px(320.0))
                .p_4()
                .gap_2()
                .bg(theme::BG_CARD)
                .rounded(px(6.0))
                .border_1()
                .border_color(theme::BORDER_SECONDARY)
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::PRIMARY)
                        .child("⏳ 正在查询战绩历史..."),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::TEXT_MUTED)
                        .child("正在通过 LCU 拉取召唤师档案与近期对局"),
                )
                .into_any_element();
        }

        if filtered.is_empty() {
            return div()
                .id("search-matches-scroll")
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .w(px(320.0))
                .p_4()
                .gap_2()
                .bg(theme::BG_CARD)
                .rounded(px(6.0))
                .border_1()
                .border_color(theme::BORDER_SECONDARY)
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::TEXT_PRIMARY)
                        .child(if self.search_error.is_some() {
                            "未找到召唤师或离线中"
                        } else {
                            "暂无战绩记录"
                        }),
                )
                .child(div().text_xs().text_color(theme::TEXT_MUTED).child(
                    if let Some(err) = &self.search_error {
                        err.clone()
                    } else {
                        "请检查客户端是否开启或输入正确的召唤师名".to_string()
                    },
                ))
                .into_any_element();
        }

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
                                            .child(format!(
                                                "英雄 ID: {}",
                                                item.display.champion_id
                                            )),
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
                                        item.display.kills,
                                        item.display.deaths,
                                        item.display.assists
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
                            .child(format!(
                                "补刀: {} ({}金币)",
                                item.display.cs, item.display.gold
                            ))
                            .child(item.display.short_time.clone()),
                    )
            }))
            .into_any_element()
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
                            .child(if self.is_searching {
                                "正在查询召唤师与对局数据...".to_string()
                            } else if self.is_loading_detail {
                                "正在拉取对局 10 人详细对抗数据...".to_string()
                            } else if let Some(gid) = selected_game_id {
                                format!("已选择对局 ID: {}，等待加载详情", gid)
                            } else if self.search_error.is_some() {
                                "无对局对抗数据".to_string()
                            } else {
                                "请在左侧选择一场对局以查看 10 人详细对抗".to_string()
                            }),
                    )
                    .child(div().text_xs().text_color(theme::TEXT_MUTED).child(
                        if let Some(err) = &self.search_error {
                            err.clone()
                        } else {
                            "支持查看红蓝双方完整 5v5 KDA、推塔/大龙等团队目标、出装及对英雄伤害"
                                .to_string()
                        },
                    ))
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
                                    .child(if is_blue {
                                        "蓝方队伍"
                                    } else {
                                        "红方队伍"
                                    }),
                            )
                            .child(Badge::new(if team.win { "胜方" } else { "败方" }).variant(
                                if team.win {
                                    BadgeVariant::Win
                                } else {
                                    BadgeVariant::Loss
                                },
                            )),
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
                        let damage_pct =
                            ((p.damage as f32 / max_damage as f32) * 100.0).clamp(0.0, 100.0);

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
                                            .child(format!(
                                                "{}/{}/{}",
                                                p.kills, p.deaths, p.assists
                                            )),
                                    )
                                    .child(div().text_xs().text_color(theme::TEXT_MUTED).child(
                                        if p.deaths == 0 {
                                            "完美".to_string()
                                        } else {
                                            format!(
                                                "{:.1} KDA",
                                                (p.kills + p.assists) as f32 / p.deaths as f32
                                            )
                                        },
                                    )),
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
