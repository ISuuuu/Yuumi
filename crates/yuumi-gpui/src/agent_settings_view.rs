//! 自动化 Agent 功能配置看板（纯白水晶极光风格）

use gpui::{
    div, hsla, px, FontWeight, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Styled,
};
use crate::theme::Theme;
use yuumi_core::agents::{AutoBpConfig, AutoMatchConfig};

#[derive(Clone)]
pub struct AgentSettingsView {
    pub match_config: AutoMatchConfig,
    pub bp_config: AutoBpConfig,
}

impl Default for AgentSettingsView {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentSettingsView {
    pub fn new() -> Self {
        Self {
            match_config: AutoMatchConfig::default(),
            bp_config: AutoBpConfig {
                auto_pick: true,
                auto_pick_intent: true,
                pick_champion_ids: vec![157, 238, 84], // 亚索, 劫, 阿卡丽
                auto_ban: true,
                ban_champion_ids: vec![11, 24],        // 易大师, 贾克斯
                auto_spell: true,
                spell1_id: 4,                          // 闪现
                spell2_id: 14,                         // 点燃
            },
        }
    }

    pub fn render_panel<F1, F2, F3, F4, F5, F6>(
        &self,
        on_toggle_accept: F1,
        on_toggle_reconnect: F2,
        on_toggle_play_again: F3,
        on_toggle_pick: F4,
        on_toggle_ban: F5,
        on_toggle_spell: F6,
    ) -> impl IntoElement
    where
        F1: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
        F2: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
        F3: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
        F4: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
        F5: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
        F6: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    {
        let is_accept = self.match_config.auto_accept;
        let is_reconnect = self.match_config.auto_reconnect;
        let is_play_again = self.match_config.auto_play_again;

        let is_pick = self.bp_config.auto_pick;
        let is_ban = self.bp_config.auto_ban;
        let is_spell = self.bp_config.auto_spell;

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_4()
            // 模块 1：对局流转自动化
            .child(
                div()
                    .w_full()
                    .bg(Theme::CARD_BG)
                    .border_1()
                    .border_color(Theme::CARD_BORDER)
                    .rounded_xl()
                    .p_4()
                    .shadow_sm()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .mb_3()
                            .child(
                                div()
                                    .size(px(8.0))
                                    .rounded_full()
                                    .bg(Theme::AURORA_CYAN),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child("对局流程自动化 (Auto Matchflow)"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            // 开关 1: 自动接受匹配
                            .child(render_toggle_row(
                                "⚡ 自动接受对局 (Auto Accept)",
                                "匹配成功后自动在指定延迟确认就绪，防止错过对局",
                                is_accept,
                                on_toggle_accept,
                            ))
                            // 开关 2: 自动重新连接
                            .child(render_toggle_row(
                                "🔄 掉线自动重连 (Auto Reconnect)",
                                "检测到游戏处于断线重连阶段时，自动向客户端发出重连指令",
                                is_reconnect,
                                on_toggle_reconnect,
                            ))
                            // 开关 3: 延时再来一局
                            .child(render_toggle_row(
                                "🔁 延时再来一局 (Auto Play Again)",
                                "对局结算页面自动点击「再来一局」并返回预设大厅",
                                is_play_again,
                                on_toggle_play_again,
                            )),
                    ),
            )
            // 模块 2：选人/禁人与技能自动化
            .child(
                div()
                    .w_full()
                    .bg(Theme::CARD_BG)
                    .border_1()
                    .border_color(Theme::CARD_BORDER)
                    .rounded_xl()
                    .p_4()
                    .shadow_sm()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .mb_3()
                            .child(
                                div()
                                    .size(px(8.0))
                                    .rounded_full()
                                    .bg(Theme::AURORA_BLUE),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child("选人与禁选自动化 (Auto BP & Spells)"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            // 开关 4: 自动秒选/锁定英雄
                            .child(render_toggle_row(
                                "🎯 自动选定英雄 (Auto Pick)",
                                "轮到自身选人时，优先按预选意向或首选配置自动锁定 (亚索/劫/阿卡丽)",
                                is_pick,
                                on_toggle_pick,
                            ))
                            // 开关 5: 自动禁用英雄
                            .child(render_toggle_row(
                                "🚫 自动禁用英雄 (Auto Ban)",
                                "禁选阶段自动锁定候选禁用英雄 (易大师/贾克斯)",
                                is_ban,
                                on_toggle_ban,
                            ))
                            // 开关 6: 自动配置召唤师技能
                            .child(render_toggle_row(
                                "✨ 自动配置召唤师技能 (Auto Spells)",
                                "选人阶段自动将主副技能配置为 闪现(D) + 点燃(F)",
                                is_spell,
                                on_toggle_spell,
                            )),
                    ),
            )
    }
}

fn render_toggle_row(
    title: &'static str,
    desc: &'static str,
    checked: bool,
    handler: impl Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .p_3()
        .rounded_lg()
        .bg(Theme::BADGE_BG)
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(Theme::TEXT_PRIMARY)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(Theme::TEXT_MUTED)
                        .child(desc),
                ),
        )
        .child(
            div()
                .cursor_pointer()
                .px_3()
                .py(px(4.0))
                .rounded_md()
                .bg(if checked {
                    Theme::ACCENT_LIGHT
                } else {
                    hsla(0.0, 0.0, 0.0, 0.05)
                })
                .border_1()
                .border_color(if checked {
                    Theme::ACCENT_PRIMARY
                } else {
                    Theme::CARD_BORDER
                })
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(if checked {
                    Theme::ACCENT_PRIMARY
                } else {
                    Theme::TEXT_MUTED
                })
                .on_mouse_down(MouseButton::Left, handler)
                .child(if checked { "已启用" } else { "已禁用" }),
        )
}
