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
///
/// [P1-1] 点击外部收起不再 `stop_propagation()`（那是 Capture 阶段，会吞掉外部第一次点击）。
/// 触发按钮在展开态的 Bubble `on_mouse_down` 不再重复触发 toggle：
/// 展开时点击自身按钮，由菜单的 Capture 阶段 dismiss 先关闭，Bubble 阶段直接跳过。
pub fn dropdown_trigger(
    id: &'static str,
    label: String,
    open: bool,
    menu: impl IntoElement,
    on_toggle: impl Fn(&mut Window, &mut App) + 'static + Clone,
) -> impl IntoElement {
    let on_click = on_toggle.clone();
    let open_at_render = open;

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
                    Theme::SIDEBAR_ACTIVE_BORDER()
                } else {
                    Theme::CARD_BORDER()
                })
                .bg(if open {
                    Theme::ACCENT_LIGHT()
                } else {
                    Theme::CARD_BG()
                })
                .flex()
                .items_center()
                .gap(px(4.0))
                .text_size(px(12.0))
                .text_color(Theme::TEXT_SECONDARY())
                .cursor_pointer()
                .hover(|s| s.border_color(Theme::ACCENT_PRIMARY()))
                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                    // 展开态：Capture 阶段的 dismiss 已把菜单收起，这里跳过避免「关了又开」
                    if !open_at_render {
                        on_click(win, app);
                    }
                })
                .child(label)
                .child(icon(
                    if open {
                        "icons/chevron-up.svg"
                    } else {
                        "icons/chevron-down.svg"
                    },
                    12.0,
                    Theme::TEXT_MUTED(),
                )),
        )
        .when(open, |this| {
            let on_dismiss = on_toggle.clone();
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
                        .bg(Theme::POPUP_BG())
                        .border_1()
                        .border_color(Theme::CARD_BORDER())
                        .rounded_lg()
                        .shadow_md()
                        .on_mouse_down_out(move |_ev, win, app| {
                            on_dismiss(win, app);
                        })
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
            Theme::ACCENT_PRIMARY()
        } else {
            Theme::TEXT_SECONDARY()
        })
        .bg(if active {
            Theme::ACCENT_LIGHT()
        } else {
            transparent_black()
        })
        .cursor_pointer()
        .hover(|s| s.bg(Theme::HOVER_BG()))
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

/// 表单下拉选择框（固定宽度，对齐 Naive UI n-select）
///
/// 触发区显示当前选中项文本与展开/收起箭头，点击触发展开浮层菜单，菜单与触发框等宽并精确锚定在正下方。
/// [P1-1] dismiss 不再 `stop_propagation()`；展开态点击自身触发按钮时由 Capture dismiss
/// 关闭、Bubble 阶段跳过 toggle，避免「先关后开」。
pub fn select_dropdown(
    id: ElementId,
    width_px: f32,
    current_label: impl Into<SharedString>,
    open: bool,
    menu: impl IntoElement,
    on_toggle: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let current_label = current_label.into();
    let on_toggle = std::rc::Rc::new(on_toggle);
    let on_click = on_toggle.clone();
    let open_at_render = open;

    div()
        .id(id)
        .relative()
        .child(
            div()
                .w(px(width_px))
                .h(px(28.0))
                .px(px(10.0))
                .rounded_md()
                .border_1()
                .border_color(if open {
                    Theme::SIDEBAR_ACTIVE_BORDER()
                } else {
                    Theme::CARD_BORDER()
                })
                .bg(if open {
                    Theme::ACCENT_LIGHT()
                } else {
                    Theme::CARD_BG()
                })
                .flex()
                .items_center()
                .justify_between()
                .cursor_pointer()
                .hover(|s| s.border_color(Theme::ACCENT_PRIMARY()))
                .on_mouse_down(MouseButton::Left, move |ev, win, app| {
                    // 展开态：Capture 阶段的 dismiss 已把菜单收起，这里跳过避免「关了又开」
                    if !open_at_render {
                        on_click(ev, win, app);
                    }
                })
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(Theme::TEXT_PRIMARY())
                        .child(current_label),
                )
                .child(icon(
                    if open {
                        "icons/chevron-up.svg"
                    } else {
                        "icons/chevron-down.svg"
                    },
                    12.0,
                    Theme::TEXT_MUTED(),
                )),
        )
        .when(open, |this| {
            let on_dismiss = on_toggle.clone();
            this.child(
                deferred(
                    div()
                        .absolute()
                        .top_full()
                        .left_0()
                        .mt(px(4.0))
                        .w(px(width_px))
                        .occlude()
                        .bg(Theme::POPUP_BG())
                        .border_1()
                        .border_color(Theme::CARD_BORDER())
                        .rounded_lg()
                        .shadow_md()
                        .overflow_hidden()
                        .py(px(4.0))
                        .on_mouse_down_out(move |ev, win, app| {
                            on_dismiss(ev, win, app);
                        })
                        .child(menu),
                )
                .with_priority(20),
            )
        })
}

/// 表单下拉选择框的选项行
pub fn select_dropdown_item(
    label: impl Into<SharedString>,
    active: bool,
    on_select: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Div {
    let label = label.into();
    div()
        .w_full()
        .px(px(10.0))
        .py(px(6.0))
        .text_size(px(12.0))
        .text_color(if active {
            Theme::ACCENT_PRIMARY()
        } else {
            Theme::TEXT_PRIMARY()
        })
        .font_weight(if active {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
        })
        .bg(if active {
            Theme::ACCENT_LIGHT()
        } else {
            transparent_black()
        })
        .cursor_pointer()
        .hover(|s| s.bg(Theme::HOVER_BG()))
        .on_mouse_down(MouseButton::Left, on_select)
        .child(label)
}
