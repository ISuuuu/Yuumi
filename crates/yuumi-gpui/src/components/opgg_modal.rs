//! OP.GG 辅助数据模态浮层组件 (对照 Vue `src/components/OpggModal.vue`)
//!
//! 纯白水晶极光风格：提供英雄梯队榜单与符文出装推荐查看，支持分路筛选与详情切换。

use crate::components::icon::icon;
use crate::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OpggTab {
    TierList,
    Build,
}

#[allow(dead_code)]
pub struct OpggModalView {
    pub is_open: bool,
    pub active_tab: OpggTab,
    pub current_position: &'static str,
    pub selected_champ_name: Option<String>,
}

impl Default for OpggModalView {
    fn default() -> Self {
        Self {
            is_open: false,
            active_tab: OpggTab::TierList,
            current_position: "MID",
            selected_champ_name: None,
        }
    }
}

pub fn opgg_modal(
    is_open: bool,
    active_tab: OpggTab,
    current_position: &'static str,
    selected_champ_name: Option<&str>,
    on_change_tab: impl Fn(OpggTab, &mut Window, &mut App) + 'static + Clone,
    on_change_position: impl Fn(&'static str, &mut Window, &mut App) + 'static + Clone,
    on_close: impl Fn(&mut Window, &mut App) + 'static + Clone,
) -> impl IntoElement {
    if !is_open {
        return div().into_any_element();
    }

    let tab_cb1 = on_change_tab.clone();
    let tab_cb2 = on_change_tab;
    let close_cb1 = on_close.clone();
    let close_cb2 = on_close;

    let positions: [(&'static str, &'static str); 5] = [
        ("TOP", "上单"),
        ("JUG", "打野"),
        ("MID", "中单"),
        ("ADC", "下路"),
        ("SUP", "辅助"),
    ];

    div()
        .id("opgg-modal-overlay")
        .occlude()
        .absolute()
        .inset_0()
        .bg(hsla(0.0, 0.0, 0.0, 0.35))
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
            close_cb1(win, app);
        })
        .child(
            div()
                .id("opgg-modal-card")
                .w(px(680.0))
                .h(px(560.0))
                .bg(Theme::CARD_BG)
                .border_1()
                .border_color(Theme::CARD_BORDER)
                .rounded(Theme::RADIUS_LG)
                .shadow_lg()
                .overflow_hidden()
                .flex()
                .flex_col()
                .on_mouse_down(MouseButton::Left, |_ev, _win, app| {
                    app.stop_propagation();
                })
                // 头部栏
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .h(px(46.0))
                        .px(px(16.0))
                        .border_b_1()
                        .border_color(Theme::CARD_BORDER)
                        .bg(Theme::HOVER_BG)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.0))
                                .child(icon("icons/opgg.svg", 22.0, Theme::ACCENT_PRIMARY))
                                .child(
                                    div()
                                        .text_size(px(15.0))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(Theme::TEXT_PRIMARY)
                                        .child("OP.GG 英雄数据辅助"),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(4.0))
                                        .ml(px(8.0))
                                        .child(
                                            div()
                                                .px(px(8.0))
                                                .py(px(2.0))
                                                .rounded_md()
                                                .text_size(px(12.0))
                                                .cursor_pointer()
                                                .when(active_tab == OpggTab::TierList, |s| {
                                                    s.bg(Theme::ACCENT_PRIMARY)
                                                        .text_color(hsla(0.0, 0.0, 1.0, 1.0))
                                                })
                                                .when(active_tab != OpggTab::TierList, |s| {
                                                    s.text_color(Theme::TEXT_MUTED)
                                                        .hover(|h| h.bg(Theme::HOVER_BG))
                                                })
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    move |_ev, win, app| {
                                                        tab_cb1(OpggTab::TierList, win, app)
                                                    },
                                                )
                                                .child("英雄梯队"),
                                        )
                                        .child(
                                            div()
                                                .px(px(8.0))
                                                .py(px(2.0))
                                                .rounded_md()
                                                .text_size(px(12.0))
                                                .cursor_pointer()
                                                .when(active_tab == OpggTab::Build, |s| {
                                                    s.bg(Theme::ACCENT_PRIMARY)
                                                        .text_color(hsla(0.0, 0.0, 1.0, 1.0))
                                                })
                                                .when(active_tab != OpggTab::Build, |s| {
                                                    s.text_color(Theme::TEXT_MUTED)
                                                        .hover(|h| h.bg(Theme::HOVER_BG))
                                                })
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    move |_ev, win, app| {
                                                        tab_cb2(OpggTab::Build, win, app)
                                                    },
                                                )
                                                .child("符文出装"),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .size(px(26.0))
                                .rounded(px(4.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .hover(|s| s.bg(Theme::HOVER_BG_STRONG))
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                    close_cb2(win, app);
                                })
                                .child(icon("icons/close.svg", 15.0, Theme::TEXT_MUTED)),
                        ),
                )
                // 位置分路筛选栏
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .h(px(40.0))
                        .px(px(16.0))
                        .border_b_1()
                        .border_color(Theme::CARD_BORDER)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .children(positions.into_iter().map(|(pos, label)| {
                                    let is_active = current_position == pos;
                                    let act = on_change_position.clone();
                                    div()
                                        .px(px(8.0))
                                        .py(px(3.0))
                                        .rounded_md()
                                        .text_size(px(12.0))
                                        .cursor_pointer()
                                        .when(is_active, |s| {
                                            s.bg(Theme::ACCENT_LIGHT)
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(Theme::ACCENT_PRIMARY)
                                        })
                                        .when(!is_active, |s| {
                                            s.text_color(Theme::TEXT_MUTED)
                                                .hover(|h| h.bg(Theme::HOVER_BG))
                                        })
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            move |_ev, win, app| act(pos, win, app),
                                        )
                                        .child(label)
                                })),
                        )
                        .when_some(selected_champ_name, |el, name| {
                            el.child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(Theme::TEXT_SECONDARY)
                                    .child(format!("当前英雄: {}", name)),
                            )
                        }),
                )
                // 主体内容区
                .child(
                    div()
                        .flex_1()
                        .p(px(16.0))
                        .overflow_hidden()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .text_color(Theme::TEXT_SECONDARY)
                                .child("已连接 OP.GG 远程 API 隔离层"),
                        )
                        .child(
                            div()
                                .text_size(px(11.5))
                                .text_color(Theme::TEXT_MUTED)
                                .child("进入英雄选择时将自动预选当前分路最强 T1 符文与核心出装推荐"),
                        ),
                ),
        )
        .into_any_element()
}
