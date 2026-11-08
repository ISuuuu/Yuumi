use gpui::prelude::FluentBuilder;
use gpui::*;
use crate::match_model::{position_to_zh, MatchItem};
use crate::theme::Theme;

pub struct CareerHeaderStats {
    pub total_games: usize,
    pub wins: usize,
    pub losses: usize,
    pub remakes: usize,
    pub win_rate: f32,
    pub avg_kda_ratio: f32,
    pub avg_kills: f32,
    pub avg_deaths: f32,
    pub avg_assists: f32,
}

impl CareerHeaderStats {
    pub fn compute(matches: &[MatchItem]) -> Self {
        let total_games = matches.len();
        if total_games == 0 {
            return Self {
                total_games: 0,
                wins: 0,
                losses: 0,
                remakes: 0,
                win_rate: 0.0,
                avg_kda_ratio: 0.0,
                avg_kills: 0.0,
                avg_deaths: 0.0,
                avg_assists: 0.0,
            };
        }

        let mut wins = 0;
        let mut losses = 0;
        let mut remakes = 0;
        let mut total_k = 0;
        let mut total_d = 0;
        let mut total_a = 0;

        for m in matches {
            if m.remake {
                remakes += 1;
            } else if m.win {
                wins += 1;
            } else {
                losses += 1;
            }
            total_k += m.kills;
            total_d += m.deaths;
            total_a += m.assists;
        }

        let valid_games = wins + losses;
        let win_rate = if valid_games > 0 {
            (wins as f32 / valid_games as f32) * 100.0
        } else {
            0.0
        };

        let avg_kills = total_k as f32 / total_games as f32;
        let avg_deaths = total_d as f32 / total_games as f32;
        let avg_assists = total_a as f32 / total_games as f32;
        let avg_kda_ratio = if total_d > 0 {
            (total_k + total_a) as f32 / total_d as f32
        } else {
            (total_k + total_a) as f32
        };

        Self {
            total_games,
            wins,
            losses,
            remakes,
            win_rate,
            avg_kda_ratio,
            avg_kills,
            avg_deaths,
            avg_assists,
        }
    }
}

pub struct CareerView;

impl CareerView {
    /// 渲染顶部统计面板
    pub fn render_header(stats: &CareerHeaderStats) -> impl IntoElement {
        div()
            .w_full()
            .bg(Theme::CARD_BG)
            .border_1()
            .border_color(Theme::CARD_BORDER)
            .rounded_xl()
            .p_4()
            .mb_4()
            .flex()
            .items_center()
            .justify_between()
            // 召唤师信息与战绩概览
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(
                        // 头像占位框
                        div()
                            .size(px(56.0))
                            .rounded_xl()
                            .bg(Theme::ACCENT_LIGHT)
                            .border_2()
                            .border_color(Theme::AURORA_CYAN)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::ACCENT_PRIMARY)
                            .child("悠"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_base()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(Theme::TEXT_PRIMARY)
                                            .child("召唤师 #GPUI"),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py(px(1.0))
                                            .rounded_md()
                                            .bg(Theme::BADGE_BG)
                                            .text_xs()
                                            .text_color(Theme::TEXT_SECONDARY)
                                            .child("Lv. 428"),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::TEXT_MUTED)
                                    .child(format!(
                                        "近 {} 场对局统计 · 数据源来自真实 LCU 抓包样本",
                                        stats.total_games
                                    )),
                            ),
                    ),
            )
            // 中间：胜率与 KDA 指标
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_6()
                    // 胜率环形/条形概览
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(if stats.win_rate >= 50.0 {
                                        Theme::WIN_TEXT
                                    } else {
                                        Theme::LOSS_TEXT
                                    })
                                    .child(format!("{:.1}%", stats.win_rate)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::TEXT_SECONDARY)
                                    .child(format!(
                                        "{} 胜 {} 负 ({} 重开)",
                                        stats.wins, stats.losses, stats.remakes
                                    )),
                            ),
                    )
                    // 平均 KDA
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child(format!("{:.2}:1 KDA", stats.avg_kda_ratio)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::TEXT_MUTED)
                                    .child(format!(
                                        "{:.1} / {:.1} / {:.1}",
                                        stats.avg_kills, stats.avg_deaths, stats.avg_assists
                                    )),
                            ),
                    ),
            )
            // 右侧：位置偏好指示
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(Theme::TEXT_SECONDARY)
                            .child("分路分布: 中单 40% · 下路 30% · 上单 20% · 辅助 10%"),
                    )
                    .child(
                        div()
                            .h(px(6.0))
                            .w(px(180.0))
                            .rounded_full()
                            .bg(Theme::BADGE_BG)
                            .overflow_hidden()
                            .flex()
                            .child(div().w_2_5().h_full().bg(Theme::AURORA_CYAN))
                            .child(div().w_1_4().h_full().bg(Theme::AURORA_BLUE))
                            .child(div().w_1_5().h_full().bg(Theme::ACCENT_PRIMARY)),
                    ),
            )
    }

    /// 渲染单条紧凑战绩卡片（112px 黄金行高）
    pub fn render_match_card(
        m: &MatchItem,
        idx: usize,
        on_toggle: impl Fn(usize, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let toggle_cb = on_toggle.clone();

        let (bg_color, border_color, bar_color, win_text_color, win_label) = if m.remake {
            (
                Theme::REMAKE_BG,
                Theme::REMAKE_BORDER,
                Theme::REMAKE_BAR,
                Theme::REMAKE_TEXT,
                "重开",
            )
        } else if m.win {
            (
                Theme::WIN_BG,
                Theme::WIN_BORDER,
                Theme::WIN_BAR,
                Theme::WIN_TEXT,
                "胜利",
            )
        } else {
            (
                Theme::LOSS_BG,
                Theme::LOSS_BORDER,
                Theme::LOSS_BAR,
                Theme::LOSS_TEXT,
                "失败",
            )
        };

        let champion_name = m.champion_name.clone();
        let queue_name = m.queue_name.clone();
        let duration_str = m.duration_str.clone();
        let time_str = m.time_str.clone();
        let kda_ratio = m.kda_ratio.clone();
        let cs_str = format!("{} 刀 ({})", m.cs, m.cs_per_minute);
        let gold_str = m.gold_str.clone();
        let damage_str = m.damage_str.clone();
        let pos_str = position_to_zh(m.position.as_deref());
        let items = m.items;
        let is_expanded = m.expanded;

        div()
            .w_full()
            .mb_2()
            .flex()
            .flex_col()
            .rounded_xl()
            .border_1()
            .border_color(border_color)
            .bg(bg_color)
            .shadow_sm()
            .overflow_hidden()
            .child(
                div()
                    .h(px(112.0))
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pr_4()
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_ev, window, cx| {
                        toggle_cb(idx, window, cx);
                    })
                    // 1. 左侧胜负竖条与英雄模块
                    .child(
                        div()
                            .h_full()
                            .flex()
                            .items_center()
                            .child(
                                // 4px 胜负状态竖条
                                div().w(px(4.0)).h_full().bg(bar_color),
                            )
                            .child(
                                div()
                                    .px_3()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    // 英雄头像槽与等级
                                    .child(
                                        div()
                                            .relative()
                                            .size(px(52.0))
                                            .rounded_lg()
                                            .bg(Theme::CARD_BG)
                                            .border_1()
                                            .border_color(border_color)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(Theme::TEXT_PRIMARY)
                                                    .child(champion_name),
                                            )
                                            // 等级角标
                                            .child(
                                                div()
                                                    .absolute()
                                                    .bottom(px(1.0))
                                                    .right(px(1.0))
                                                    .px_1()
                                                    .rounded_sm()
                                                    .bg(rgba(0x000000bb))
                                                    .text_xs()
                                                    .text_color(rgb(0xffffff))
                                                    .child(format!("{}", m.champ_level)),
                                            ),
                                    )
                                    // 技能与主符文占位
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.0))
                                            .child(
                                                div()
                                                    .size(px(22.0))
                                                    .rounded_md()
                                                    .bg(Theme::BADGE_BG)
                                                    .border_1()
                                                    .border_color(Theme::CARD_BORDER)
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .text_xs()
                                                    .text_color(Theme::TEXT_MUTED)
                                                    .child("D"),
                                            )
                                            .child(
                                                div()
                                                    .size(px(22.0))
                                                    .rounded_md()
                                                    .bg(Theme::BADGE_BG)
                                                    .border_1()
                                                    .border_color(Theme::CARD_BORDER)
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .text_xs()
                                                    .text_color(Theme::TEXT_MUTED)
                                                    .child("F"),
                                            ),
                                    ),
                            ),
                    )
                    // 2. 模式、胜负与对局时间
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .w(px(120.0))
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child(queue_name),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(win_text_color)
                                            .child(win_label),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(Theme::TEXT_SECONDARY)
                                            .child(duration_str),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::TEXT_MUTED)
                                    .child(format!("{} · {}", time_str, pos_str)),
                            ),
                    )
                    // 3. KDA 与评级（等宽数据排齐）
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .w(px(130.0))
                            .items_start()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child(format!("{} / {} / {}", m.kills, m.deaths, m.assists)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(Theme::TEXT_SECONDARY)
                                    .child(kda_ratio),
                            ),
                    )
                    // 4. 补刀、经济与输出
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .w(px(110.0))
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child(cs_str),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::TEXT_MUTED)
                                    .child(format!("金币 {}", gold_str)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::TEXT_MUTED)
                                    .child(format!("输出 {}", damage_str)),
                            ),
                    )
                    // 5. 装备网格（6 装备 + 1 饰品）
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .children(items.iter().enumerate().map(|(slot_idx, &item_id)| {
                                let is_trinket = slot_idx == 6;
                                let is_empty = item_id == 0;
                                div()
                                    .size(px(26.0))
                                    .rounded_md()
                                    .bg(if is_empty {
                                        Theme::ITEM_SLOT_BG
                                    } else {
                                        Theme::CARD_BG
                                    })
                                    .border_1()
                                    .border_color(if is_trinket {
                                        Theme::AURORA_CYAN
                                    } else {
                                        Theme::ITEM_SLOT_BORDER
                                    })
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_xs()
                                    .text_color(Theme::TEXT_MUTED)
                                    .child(if is_empty {
                                        "".to_string()
                                    } else {
                                        format!("{}", item_id % 100)
                                    })
                            })),
                    )
                    // 6. 右侧展开按钮
                    .child(
                        div()
                            .p_2()
                            .rounded_md()
                            .hover(|s| s.bg(rgba(0x00000010)))
                            .text_sm()
                            .text_color(Theme::TEXT_SECONDARY)
                            .child(if is_expanded { "▲" } else { "▼" }),
                    ),
            )
            // 展开态详情抽屉
            .when(is_expanded, |this| {
                this.child(
                    div()
                        .w_full()
                        .p_3()
                        .border_t_1()
                        .border_color(border_color)
                        .bg(Theme::CARD_BG)
                        .flex()
                        .items_center()
                        .justify_between()
                        .text_xs()
                        .text_color(Theme::TEXT_SECONDARY)
                        .child(div().child(format!(
                            "对局编号: {} · 伤害占比: 26.4% · 承受伤害: 18.2k",
                            m.game_id
                        )))
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(
                                    div()
                                        .px_2()
                                        .py_1()
                                        .rounded_md()
                                        .bg(Theme::BADGE_BG)
                                        .cursor_pointer()
                                        .child("下载回放"),
                                )
                                .child(
                                    div()
                                        .px_2()
                                        .py_1()
                                        .rounded_md()
                                        .bg(Theme::ACCENT_LIGHT)
                                        .text_color(Theme::ACCENT_PRIMARY)
                                        .cursor_pointer()
                                        .child("查看完整图表"),
                                ),
                        ),
                )
            })
    }
}
