//! 通用下拉控件
//!
//! 对齐 Vue 的 `.dropdown-trigger` / `.queue-dropdown-menu`：
//! 触发按钮为「文字 + 箭头」，菜单贴在按钮正下方、左边缘与按钮对齐，
//! 菜单项左侧内边距与按钮一致，使两者的文字处在同一起始线上。
//! `deferred(...).with_priority` 只负责让菜单盖住后续兄弟节点。

use crate::components::icon::icon;
use crate::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::*;

/// 按钮水平内边距，同时用作菜单项左侧内边距（文字对齐基准）
const PADDING_X: f32 = 10.0;

/// 下拉触发按钮 + 浮层菜单
pub fn dropdown_trigger(
    id: &'static str,
    label: String,
    open: bool,
    menu: impl IntoElement,
    on_toggle: impl Fn(&mut Window, &mut App) + 'static + Clone,
) -> impl IntoElement {
    let on_click = on_toggle.clone();

    div()
        .id(id)
        .relative()
        .child(
            div()
                .px(px(PADDING_X))
                .py(px(5.0))
                .rounded_md()
                .border_1()
                .border_color(if open {
                    Theme::SIDEBAR_ACTIVE_BORDER
                } else {
                    Theme::CARD_BORDER
                })
                .bg(if open {
                    Theme::ACCENT_LIGHT
                } else {
                    Theme::CARD_BG
                })
                .flex()
                .items_center()
                .gap(px(4.0))
                .text_size(px(12.0))
                .text_color(Theme::TEXT_SECONDARY)
                .cursor_pointer()
                .hover(|s| s.border_color(Theme::ACCENT_PRIMARY))
                .on_mouse_down(MouseButton::Left, move |_ev, win, app| on_click(win, app))
                .child(label)
                .child(icon(
                    if open {
                        "icons/chevron-up.svg"
                    } else {
                        "icons/chevron-down.svg"
                    },
                    12.0,
                    Theme::TEXT_MUTED,
                )),
        )
        .when(open, |this| {
            this.child(
                deferred(
                    div()
                        .absolute()
                        .top_full()
                        .left_0()
                        .mt(px(4.0))
                        .id(SharedString::from(format!("{}-menu", id)))
                        .occlude()
                        // 不透明背景，避免下方战绩卡片文字透上来
                        .bg(rgb(0xffffff))
                        .border_1()
                        .border_color(Theme::CARD_BORDER)
                        .rounded_lg()
                        .shadow_md()
                        .child(menu),
                )
                .with_priority(2),
            )
        })
}

/// 下拉菜单项
pub fn dropdown_item(
    label: &'static str,
    active: bool,
    on_select: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .px(px(PADDING_X))
        .py(px(6.0))
        .text_size(px(12.5))
        .text_color(if active {
            Theme::ACCENT_PRIMARY
        } else {
            Theme::TEXT_SECONDARY
        })
        .bg(if active {
            Theme::ACCENT_LIGHT
        } else {
            transparent_black()
        })
        .cursor_pointer()
        .hover(|s| s.bg(Theme::HOVER_BG))
        .on_mouse_down(MouseButton::Left, move |_ev, win, app| on_select(win, app))
        .child(label)
}

/// 菜单容器（由调用方决定最小宽度）
pub fn dropdown_menu(
    min_width: f32,
    items: impl IntoIterator<Item = impl IntoElement>,
) -> impl IntoElement {
    div()
        .w(px(min_width))
        .py(px(4.0))
        .flex()
        .flex_col()
        .children(items)
}
