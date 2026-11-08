//! 工具箱与自动化流程视图 (ToolsView)
//! 遵循纯白水晶极光设计语言，集成对局自动化流转、BP 选择、大乱斗板凳席与客户端快捷小工具

use gpui::*;
use gpui::prelude::FluentBuilder;
use crate::components::badge::Badge;
use crate::components::card::Card;
use crate::components::switch::Switch;
use crate::theme::Theme;
pub use yuumi_core::agents::{AutoBpConfig, AutoMatchConfig};

#[derive(Clone)]
pub struct ToolsView {
    pub match_config: AutoMatchConfig,
    pub bp_config: AutoBpConfig,
    pub bench_overlay: bool,
    pub auto_trade: bool,
    pub auto_honor: bool,
    pub lock_settings: bool,
}

impl Default for ToolsView {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolsView {
    pub fn new() -> Self {
        Self {
            match_config: AutoMatchConfig {
                auto_accept: true,
                auto_accept_delay: 2,
                auto_reconnect: true,
                auto_play_again: false,
                auto_play_again_delay: 5,
                auto_honor: true,
            },
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
            bench_overlay: true,
            auto_trade: true,
            auto_honor: true,
            lock_settings: false,
        }
    }

    pub fn render_panel<F1, F2, F3, F4, F5, F6, F7, F8, F9>(
        &self,
        on_toggle_accept: F1,
        on_toggle_reconnect: F2,
        on_toggle_play_again: F3,
        on_toggle_pick: F4,
        on_toggle_ban: F5,
        on_toggle_spell: F6,
        on_toggle_bench_overlay: F7,
        on_toggle_trade: F8,
        on_toggle_honor: F9,
    ) -> impl IntoElement
    where
        F1: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
        F2: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
        F3: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
        F4: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
        F5: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
        F6: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
        F7: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
        F8: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
        F9: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    {
        div()
            .id("tools-scroll-view")
            .size_full()
            .flex()
            .flex_col()
            .gap_5()
            .p_4()
            .overflow_y_scroll()
            // 顶部标题
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child("自动化与实用工具 (Tools & Automation)"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::TEXT_MUTED)
                                    .child("配置英雄选择自动化、对局全生命周期流转助手及客户端高频实用小工具"),
                            ),
                    )
                    .child(
                        Badge::new("Direct3D 11 原生驱动")
                            .variant(crate::components::badge::BadgeVariant::Primary),
                    ),
            )
            // 分组 1：英雄选择阶段 (Champ Select)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(render_section_header("英雄选择阶段 (Champ Select Automation)", Theme::AURORA_CYAN))
                    .child(
                        div()
                            .grid_cols(2)
                            .gap_3()
                            // 卡片 1: 自动接受匹配
                            .child(
                                render_interactive_card(
                                    "⚡ 自动接受对局",
                                    "匹配到对局后自动在设定延迟确认就绪，防止错过排位",
                                    self.match_config.auto_accept,
                                    Some(format!("延时: {} 秒", self.match_config.auto_accept_delay)),
                                    on_toggle_accept,
                                )
                            )
                            // 卡片 2: 自动秒选/锁定英雄
                            .child(
                                render_interactive_card(
                                    "🎯 自动秒选/锁定英雄",
                                    "选人阶段优先亮起意向英雄并按首选列表自动锁定",
                                    self.bp_config.auto_pick,
                                    Some("首选: 亚索 · 劫 · 阿卡丽".to_string()),
                                    on_toggle_pick,
                                )
                            )
                            // 卡片 3: 自动禁用英雄
                            .child(
                                render_interactive_card(
                                    "🚫 自动禁用英雄",
                                    "禁用阶段自动锁定首选 ban 位，防止忘记禁英雄",
                                    self.bp_config.auto_ban,
                                    Some("首禁: 易大师 · 贾克斯".to_string()),
                                    on_toggle_ban,
                                )
                            )
                            // 卡片 4: 自动配置召唤师技能
                            .child(
                                render_interactive_card(
                                    "✨ 自动配置召唤师技能",
                                    "进入选人房间时自动校验并配置 D 闪 / F 点燃",
                                    self.bp_config.auto_spell,
                                    Some("D: 闪现 (4) · F: 点燃 (14)".to_string()),
                                    on_toggle_spell,
                                )
                            )
                            // 卡片 5: 自动同意换英雄/换楼层
                            .child(
                                render_interactive_card(
                                    "🤝 自动同意队友换人/换楼",
                                    "队友发起英雄交换或楼层调整请求时，自动点击确认",
                                    self.auto_trade,
                                    None,
                                    on_toggle_trade,
                                )
                            )
                            // 卡片 6: 大乱斗板凳席独立悬浮窗
                            .child(
                                render_interactive_card(
                                    "🪟 大乱斗板凳席悬浮窗",
                                    "极地大乱斗选人时弹出置顶悬浮窗，直观呈现全队掷点板凳席",
                                    self.bench_overlay,
                                    Some("支持透明置顶 · 快捷捡英雄".to_string()),
                                    on_toggle_bench_overlay,
                                )
                            ),
                    ),
            )
            // 分组 2：对局流程与结算 (Gameflow & Honor)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(render_section_header("对局流程与结算自动化 (Gameflow & Reconnect)", Theme::AURORA_BLUE))
                    .child(
                        div()
                            .grid_cols(2)
                            .gap_3()
                            // 卡片 7: 自动点赞队友
                            .child(
                                render_interactive_card(
                                    "👍 结算自动点赞队友",
                                    "游戏结束后自动为表现最佳或随机队友点赞，获取荣誉点数",
                                    self.auto_honor,
                                    None,
                                    on_toggle_honor,
                                )
                            )
                            // 卡片 8: 掉线自动重连
                            .child(
                                render_interactive_card(
                                    "🔄 掉线自动重新连接",
                                    "检测到意外断线或客户端处于 Reconnect 状态时自动重连",
                                    self.match_config.auto_reconnect,
                                    None,
                                    on_toggle_reconnect,
                                )
                            )
                            // 卡片 9: 延时再来一局
                            .child(
                                render_interactive_card(
                                    "🔁 结算自动「再来一局」",
                                    "结算页面停留指定秒数后自动点击再来一局返回房间",
                                    self.match_config.auto_play_again,
                                    Some(format!("延时: {} 秒", self.match_config.auto_play_again_delay)),
                                    on_toggle_play_again,
                                )
                            )
                            // 卡片 10: 大乱斗快速换回
                            .child(
                                render_action_card(
                                    "🎲 ARAM 快速摇号换回",
                                    "掷骰投出备选英雄并极速换回当前英雄，丰富板凳席",
                                    "立即触发",
                                )
                            ),
                    ),
            )
            // 分组 3：客户端实用小工具 (Client Utilities)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(render_section_header("客户端实用小工具 (Client Utilities)", Theme::GOLD))
                    .child(
                        div()
                            .grid_cols(3)
                            .gap_3()
                            .child(render_action_card(
                                "📐 修复窗口大小与缩放",
                                "重置因分辨率错位导致的黑边，还原 1600x900 100% 原始缩放",
                                "一键修复",
                            ))
                            .child(render_action_card(
                                "🧹 客户端缓存与临时文件清理",
                                "安全清除 CEF 浏览器缓存、临时资源包，释放磁盘占用",
                                "清理缓存",
                            ))
                            .child(render_action_card(
                                "👁️ 实时直接观战召唤师",
                                "输入召唤师名称直接调用游戏客户端观战引擎启动观战",
                                "启动观战",
                            ))
                            .child(render_action_card(
                                "👥 批量清理长期离线好友",
                                "智能检索超过 90 天未上线的游戏好友，支持一键批量移除",
                                "好友管理",
                            ))
                            .child(render_action_card(
                                "🔒 锁定游戏键位配置文件",
                                "将 game.cfg 属性设置为只读，防止客户端更新覆盖键位设置",
                                "锁定设置",
                            ))
                            .child(render_action_card(
                                "📦 战利品全自动开箱与分解",
                                "一键遍历打开传送门、法球、杰作宝箱并自动分解英雄碎片",
                                "前往战利品",
                            )),
                    ),
            )
    }
}

/// 渲染分组标题
fn render_section_header(title: &'static str, dot_color: Hsla) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .size(px(8.0))
                .rounded_full()
                .bg(dot_color),
        )
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(Theme::TEXT_PRIMARY)
                .child(title),
        )
}

/// 渲染带 Switch 的极光卡片
fn render_interactive_card<F>(
    title: &'static str,
    desc: &'static str,
    checked: bool,
    extra_badge: Option<String>,
    on_toggle: F,
) -> impl IntoElement
where
    F: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
{
    Card::new()
        .p_3()
        .border_color(if checked { Theme::ACCENT_PRIMARY.opacity(0.35) } else { Theme::CARD_BORDER })
        .child(
            div()
                .flex()
                .items_start()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .flex_1()
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
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(Theme::TEXT_PRIMARY)
                                        .child(title),
                                )
                                .when_some(extra_badge, |this, badge_text| {
                                    this.child(
                                        Badge::new(badge_text)
                                            .variant(if checked {
                                                crate::components::badge::BadgeVariant::Primary
                                            } else {
                                                crate::components::badge::BadgeVariant::Default
                                            })
                                    )
                                }),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(Theme::TEXT_MUTED)
                                .child(desc),
                        ),
                )
                .child(
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, on_toggle)
                        .child(Switch::new(checked)),
                ),
        )
}

/// 渲染带触发按钮的操作卡片
fn render_action_card(
    title: &'static str,
    desc: &'static str,
    button_text: &'static str,
) -> impl IntoElement {
    Card::new()
        .p_3()
        .child(
            div()
                .flex()
                .flex_col()
                .justify_between()
                .gap_3()
                .h_full()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(Theme::TEXT_PRIMARY)
                                .child(title),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(Theme::TEXT_MUTED)
                                .child(desc),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .child(
                            div()
                                .px_3()
                                .py_1()
                                .rounded(px(6.0))
                                .bg(Theme::BADGE_BG)
                                .border_1()
                                .border_color(Theme::CARD_BORDER)
                                .hover(|this| this.bg(Theme::HOVER_BG).border_color(Theme::ACCENT_PRIMARY.opacity(0.4)))
                                .cursor_pointer()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(Theme::TEXT_PRIMARY)
                                .child(button_text),
                        ),
                ),
        )
}
