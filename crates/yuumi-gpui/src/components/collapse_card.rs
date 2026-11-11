//! 可折叠水晶卡片与表单设置行组件 (CollapseCard & SettingRow)
//!
//! 1:1 对照 Vue Naive UI 的 `n-collapse`、`n-collapse-item`、`.card-item`、`.setting-row` 样式
//! 遵循纯白水晶极光设计规范。

use crate::components::icon::icon;
use crate::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::*;

/// 可折叠水晶卡片
///
/// 包含可点击切换折叠的头部（左侧图标/标题/说明，右侧状态文本与箭头）以及展开后的内容区。
#[allow(clippy::too_many_arguments)]
pub fn collapse_card(
    id: ElementId,
    open: bool,
    icon_name: Option<&'static str>,
    title: impl Into<SharedString>,
    desc: impl Into<SharedString>,
    status_text: Option<String>,
    on_toggle: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    content: impl IntoElement,
) -> Stateful<Div> {
    let title = title.into();
    let desc = desc.into();

    div()
        .id(id)
        .w_full()
        // Vue .collapse-card 用 --settings-card-bg；卡片带 shadow_sm()，底色需先合成到页面底色
        .bg(Theme::over_page(Theme::SETTINGS_CARD_BG()))
        .border_1()
        .border_color(if open {
            Theme::SIDEBAR_ACTIVE_BORDER()
        } else {
            Theme::CARD_BORDER()
        })
        .rounded(px(12.0))
        .shadow_sm()
        .overflow_hidden()
        // 头部可点击区域
        .child(
            div()
                .w_full()
                .when(open, |s| s.rounded_t(px(11.0)))
                .when(!open, |s| s.rounded(px(11.0)))
                .px(px(24.0))
                .py(px(16.0))
                .flex()
                .items_center()
                .justify_between()
                .cursor_pointer()
                // Vue .collapse-card 头部悬停用 --settings-card-bg-hover
                .hover(|s| s.bg(Theme::SETTINGS_CARD_BG_HOVER()))
                .on_mouse_down(MouseButton::Left, on_toggle)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        .flex_1()
                        .when_some(icon_name, |this, icon_path| {
                            this.child(icon(
                                icon_path,
                                16.0,
                                if open {
                                    Theme::ACCENT_PRIMARY()
                                } else {
                                    Theme::TEXT_MUTED()
                                },
                            ))
                        })
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .text_size(px(13.5))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(Theme::TEXT_PRIMARY())
                                        .child(title),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(Theme::TEXT_MUTED())
                                        .child(desc),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .when_some(status_text, |this, st| {
                            this.child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(if open {
                                        Theme::ACCENT_PRIMARY()
                                    } else {
                                        Theme::TEXT_MUTED()
                                    })
                                    .child(st),
                            )
                        })
                        .child(icon(
                            if open {
                                "icons/chevron-down.svg"
                            } else {
                                "icons/chevron-right.svg"
                            },
                            14.0,
                            Theme::TEXT_MUTED(),
                        )),
                ),
        )
        // 展开后的内容区
        .when(open, |this| {
            this.child(
                div()
                    .w_full()
                    .rounded_b(px(11.0))
                    // 对齐 Vue .n-collapse-item__content 的 --hover-bg 浅灰层（父级卡片已是实色）
                    .bg(Theme::HOVER_BG())
                    .border_t_1()
                    .border_color(Theme::CARD_BORDER().opacity(0.6))
                    .px(px(24.0))
                    .py(px(16.0))
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .child(content),
            )
        })
}

/// 标准设置行（左侧标题/说明，右侧控件）
pub fn setting_row(
    title: impl Into<SharedString>,
    desc: Option<&'static str>,
    control: impl IntoElement,
) -> Div {
    let title = title.into();

    div()
        .w_full()
        .py(px(6.0))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(16.0))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .flex_1()
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(Theme::TEXT_PRIMARY())
                        .child(title),
                )
                .when_some(desc, |this, d| {
                    this.child(div().text_size(px(11.5)).text_color(Theme::TEXT_MUTED()).child(d))
                }),
        )
        .child(control)
}

/// 设置页标准卡片条目（独立单行卡片，有下底边或完整圆角）
pub fn settings_card_item(
    title: impl Into<SharedString>,
    desc: Option<impl Into<SharedString>>,
    border_bottom: bool,
    control: impl IntoElement,
) -> Div {
    let title = title.into();
    let desc_opt = desc.map(|d| d.into());

    div()
        .w_full()
        .px(px(24.0))
        .py(px(16.0))
        // Vue .setting-row 的 --settings-row-bg 叠在折叠卡展开区之上
        .bg(Theme::SETTINGS_ROW_BG())
        .when(border_bottom, |s| {
            s.border_b_1().border_color(Theme::CARD_BORDER().opacity(0.5))
        })
        .flex()
        .items_center()
        .justify_between()
        .gap(px(16.0))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .flex_1()
                .child(
                    div()
                        .text_size(px(13.5))
                        .font_weight(FontWeight::BOLD)
                        .text_color(Theme::TEXT_PRIMARY())
                        .child(title),
                )
                .when_some(desc_opt, |this, d| {
                    this.child(
                        div()
                            .text_size(px(12.0))
                            .text_color(Theme::TEXT_MUTED())
                            .line_height(relative(1.4))
                            .child(d),
                    )
                }),
        )
        .child(control)
}

/// 数值步进器（数值居左，减加按钮居右，宽度 140px，1:1 对标 Naive UI n-input-number）
pub fn number_stepper(
    id: ElementId,
    value_display: String,
    can_decrement: bool,
    can_increment: bool,
    on_decrement: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    on_increment: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(140.0))
        .h(px(28.0))
        .rounded_md()
        .border_1()
        .border_color(Theme::CARD_BORDER())
        .bg(Theme::CARD_BG())
        .flex()
        .items_center()
        .justify_between()
        .overflow_hidden()
        // 左侧数值显示
        .child(
            div()
                .flex_1()
                .h_full()
                .px(px(10.0))
                .flex()
                .items_center()
                .text_size(px(12.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(Theme::TEXT_PRIMARY())
                .child(value_display),
        )
        // 右侧减加按钮组
        .child(
            div()
                .flex()
                .items_center()
                .h_full()
                .border_l_1()
                .border_color(Theme::CARD_BORDER().opacity(0.6))
                // 减按钮
                .child(
                    div()
                        .w(px(26.0))
                        .h_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(if can_decrement {
                            Theme::TEXT_PRIMARY()
                        } else {
                            Theme::TEXT_MUTED().opacity(0.3)
                        })
                        .cursor(if can_decrement {
                            CursorStyle::PointingHand
                        } else {
                            CursorStyle::OperationNotAllowed
                        })
                        .when(can_decrement, |this| {
                            this.hover(|s| s.bg(Theme::HOVER_BG()))
                                .on_mouse_down(MouseButton::Left, on_decrement)
                        })
                        .child("−"),
                )
                // 分隔细线
                .child(
                    div()
                        .w(px(1.0))
                        .h(px(14.0))
                        .bg(Theme::CARD_BORDER().opacity(0.6)),
                )
                // 加按钮
                .child(
                    div()
                        .w(px(26.0))
                        .h_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(if can_increment {
                            Theme::TEXT_PRIMARY()
                        } else {
                            Theme::TEXT_MUTED().opacity(0.3)
                        })
                        .cursor(if can_increment {
                            CursorStyle::PointingHand
                        } else {
                            CursorStyle::OperationNotAllowed
                        })
                        .when(can_increment, |this| {
                            this.hover(|s| s.bg(Theme::HOVER_BG()))
                                .on_mouse_down(MouseButton::Left, on_increment)
                        })
                        .child("+"),
                ),
        )
}

/// 标准复选框（16x16 方块 + 勾选标记 + 文本标签，对标 Naive UI n-checkbox）
pub fn checkbox(
    id: ElementId,
    label: impl Into<SharedString>,
    checked: bool,
    on_toggle: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let label = label.into();
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(6.0))
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, on_toggle)
        .child(
            div()
                .size(px(16.0))
                .rounded(px(3.0))
                .border_1()
                .border_color(if checked {
                    Theme::ACCENT_PRIMARY()
                } else {
                    Theme::CARD_BORDER()
                })
                .bg(if checked {
                    Theme::ACCENT_PRIMARY()
                } else {
                    Theme::CARD_BG()
                })
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(11.0))
                .text_color(gpui::white())
                .font_weight(FontWeight::BOLD)
                .child(if checked { "✓" } else { "" }),
        )
        .child(
            div()
                .text_size(px(12.5))
                .text_color(Theme::TEXT_PRIMARY())
                .child(label),
        )
}

/// 分路切换选项卡（默认 / 上单 / 打野 / 中单 / 下路 / 辅助）
pub fn lane_tabs<F>(selected_lane: &'static str, on_select: F) -> Div
where
    F: Fn(&'static str, &mut Window, &mut App) + 'static + Clone,
{
    const LANES: [(&str, &str); 6] = [
        ("default", "通用默认"),
        ("top", "上单"),
        ("jug", "打野"),
        ("mid", "中单"),
        ("bot", "下路"),
        ("sup", "辅助"),
    ];

    div()
        .w_full()
        .flex()
        .items_center()
        .gap(px(6.0))
        .py(px(4.0))
        .children(LANES.into_iter().map(|(val, label)| {
            let active = selected_lane == val;
            let cb = on_select.clone();
            div()
                .px(px(12.0))
                .py(px(4.0))
                .rounded_md()
                .border_1()
                .border_color(if active {
                    Theme::SIDEBAR_ACTIVE_BORDER()
                } else {
                    Theme::CARD_BORDER()
                })
                .bg(if active {
                    Theme::ACCENT_LIGHT()
                } else {
                    Theme::CARD_BG()
                })
                .text_size(px(12.0))
                .text_color(if active {
                    Theme::ACCENT_PRIMARY()
                } else {
                    Theme::TEXT_SECONDARY()
                })
                .cursor_pointer()
                .hover(|s| s.border_color(Theme::ACCENT_PRIMARY()))
                .on_mouse_down(MouseButton::Left, move |_ev, win, app| cb(val, win, app))
                .child(label)
        }))
}

/// 小操作按钮（标准次要按钮）
pub fn action_button(
    label: impl Into<SharedString>,
    primary: bool,
    on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Div {
    div()
        .px(px(12.0))
        .py(px(5.0))
        .rounded_md()
        .border_1()
        .border_color(if primary {
            Theme::ACCENT_PRIMARY()
        } else {
            Theme::CARD_BORDER()
        })
        .bg(if primary {
            Theme::ACCENT_PRIMARY()
        } else {
            Theme::CARD_BG()
        })
        .text_size(px(12.0))
        .text_color(if primary {
            gpui::white()
        } else {
            Theme::TEXT_PRIMARY()
        })
        .cursor_pointer()
        .hover(|s| {
            if primary {
                s.opacity(0.85)
            } else {
                s.bg(Theme::HOVER_BG())
            }
        })
        .on_mouse_down(MouseButton::Left, on_click)
        .child(label.into())
}
