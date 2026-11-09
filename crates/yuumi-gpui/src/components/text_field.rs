//! 单行文本输入框
//!
//! gpui 0.2 尚未提供内置文本输入元素，这里给出最小可用实现：
//! 由视图持有文本，按键事件在元素内处理并回调新文本。

use crate::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::*;

/// 输入框的可选行为
#[derive(Clone, Copy, Default)]
pub struct TextFieldOptions {
    /// 禁用输入（例如后台搜索中）
    pub disabled: bool,
    /// 显示加载指示
    pub loading: bool,
    /// 前置图标（文本形式）
    pub leading_icon: Option<&'static str>,
    /// 最大字符数；None 表示不限制
    pub max_chars: Option<usize>,
}

/// 渲染一个可聚焦、可输入的单行文本框
///
/// - `text`：当前文本（视图持有）
/// - `focus`：由视图/窗口持有的焦点句柄，点击框体即聚焦
/// - `on_change`：每次键入/退格后回调新文本
/// - `on_clear`：点击清除按钮时回调
/// - `on_submit`：回车时回调当前文本
#[allow(clippy::too_many_arguments)]
pub fn text_field(
    id: &'static str,
    text: &str,
    placeholder: &str,
    width: Pixels,
    focus: &FocusHandle,
    is_focused: bool,
    options: TextFieldOptions,
    on_change: impl Fn(String, &mut Window, &mut App) + 'static + Clone,
    on_clear: impl Fn(&mut Window, &mut App) + 'static + Clone,
    on_submit: impl Fn(&str, &mut Window, &mut App) + 'static + Clone,
) -> impl IntoElement {
    let current = text.to_string();
    let has_text = !current.is_empty();
    let focus_for_click = focus.clone();
    let disabled = options.disabled;

    let keyed_text = current.clone();
    let on_key = move |ev: &KeyDownEvent, window: &mut Window, app: &mut App| {
        if disabled {
            return;
        }
        let keystroke = &ev.keystroke;
        if keystroke.modifiers.control || keystroke.modifiers.platform || keystroke.modifiers.alt {
            return;
        }

        let mut next = keyed_text.clone();
        match keystroke.key.as_str() {
            "backspace" => {
                next.pop();
            }
            "enter" => {
                on_submit(&keyed_text, window, app);
                return;
            }
            "delete" | "escape" | "tab" => return,
            _ => {
                // 部分键盘事件不带 key_char（如合成消息、某些 IME 布局），退回单字符 key
                let typed = keystroke
                    .key_char
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .or_else(|| {
                        let key = keystroke.key.as_str();
                        if key == "space" {
                            Some(" ".to_string())
                        } else if key.chars().count() == 1 {
                            Some(key.to_string())
                        } else {
                            None
                        }
                    });
                let within_limit = options
                    .max_chars
                    .is_none_or(|max| next.chars().count() < max);
                match typed {
                    Some(ch) if within_limit => next.push_str(&ch),
                    _ => return,
                }
            }
        }

        on_change(next, window, app);
    };

    let clear_cb = on_clear;

    div()
        .id(id)
        .track_focus(focus)
        .h(px(30.0))
        .w(width)
        .rounded_lg()
        .bg(Theme::CARD_BG)
        .border_1()
        .border_color(if disabled || is_focused {
            Theme::ACCENT_PRIMARY
        } else {
            Theme::CARD_BORDER
        })
        .px(px(10.0))
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, move |_ev, window, _cx| {
            window.focus(&focus_for_click);
        })
        .on_key_down(on_key)
        .when_some(options.leading_icon, |this, glyph| {
            this.child(
                div()
                    .flex_none()
                    .text_size(px(12.0))
                    .text_color(Theme::TEXT_MUTED)
                    .child(glyph),
            )
        })
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_size(px(12.5))
                .text_color(if has_text {
                    Theme::TEXT_PRIMARY
                } else {
                    Theme::TEXT_MUTED
                })
                .child(if has_text {
                    current
                } else {
                    placeholder.to_string()
                }),
        )
        .when(options.loading, |this| {
            this.child(
                div()
                    .flex_none()
                    .text_size(px(12.0))
                    .text_color(Theme::ACCENT_PRIMARY)
                    .child("⏳"),
            )
        })
        .when(has_text && !disabled, |this| {
            this.child(
                div()
                    .flex_none()
                    .text_size(px(12.0))
                    .text_color(Theme::TEXT_MUTED)
                    .cursor_pointer()
                    .hover(|s| s.text_color(Theme::TEXT_PRIMARY))
                    .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                        clear_cb(window, app);
                    })
                    .child("✕"),
            )
        })
}

/// 弹性宽度的搜索输入框（带清空按钮与右侧圆形搜索图标按钮，1:1 对照 Vue Search.vue:1192）
#[allow(clippy::too_many_arguments)]
pub fn search_input_field(
    id: &'static str,
    text: &str,
    placeholder: &str,
    max_width: Pixels,
    focus: &FocusHandle,
    is_focused: bool,
    disabled: bool,
    loading: bool,
    on_change: impl Fn(String, &mut Window, &mut App) + 'static + Clone,
    on_clear: impl Fn(&mut Window, &mut App) + 'static + Clone,
    on_submit: impl Fn(&str, &mut Window, &mut App) + 'static + Clone,
) -> impl IntoElement {
    let current = text.to_string();
    let has_text = !current.is_empty();
    let can_search = !disabled && !loading && !current.trim().is_empty();
    let focus_for_click = focus.clone();

    let keyed_text = current.clone();
    let submit_for_key = on_submit.clone();
    let on_key = move |ev: &KeyDownEvent, window: &mut Window, app: &mut App| {
        if disabled || loading {
            return;
        }
        let keystroke = &ev.keystroke;
        if keystroke.modifiers.control || keystroke.modifiers.platform || keystroke.modifiers.alt {
            return;
        }

        let mut next = keyed_text.clone();
        match keystroke.key.as_str() {
            "backspace" => {
                next.pop();
            }
            "enter" => {
                if !keyed_text.trim().is_empty() {
                    submit_for_key(&keyed_text, window, app);
                }
                return;
            }
            "delete" | "escape" | "tab" => return,
            _ => {
                let typed = keystroke
                    .key_char
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .or_else(|| {
                        let key = keystroke.key.as_str();
                        if key == "space" {
                            Some(" ".to_string())
                        } else if key.chars().count() == 1 {
                            Some(key.to_string())
                        } else {
                            None
                        }
                    });
                match typed {
                    Some(ch) if next.chars().count() < 64 => next.push_str(&ch),
                    _ => return,
                }
            }
        }

        on_change(next, window, app);
    };

    let clear_cb = on_clear;
    let submit_cb = on_submit;
    let submit_text = current.clone();

    div()
        .id(id)
        .track_focus(focus)
        .h(px(32.0))
        .w_full()
        .max_w(max_width)
        .rounded(px(6.0))
        .bg(Theme::CARD_BG)
        .border_1()
        .border_color(if is_focused {
            Theme::ACCENT_PRIMARY
        } else {
            Theme::CARD_BORDER
        })
        .px(px(10.0))
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .cursor_text()
        .on_mouse_down(MouseButton::Left, move |_ev, window, _cx| {
            window.focus(&focus_for_click);
        })
        .on_key_down(on_key)
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_size(px(12.5))
                .text_color(if has_text {
                    Theme::TEXT_PRIMARY
                } else {
                    Theme::TEXT_MUTED
                })
                .child(if has_text {
                    current
                } else {
                    placeholder.to_string()
                }),
        )
        // 清除按钮
        .when(has_text && !disabled && !loading, |this| {
            this.child(
                div()
                    .flex_none()
                    .text_size(px(12.0))
                    .text_color(Theme::TEXT_MUTED)
                    .cursor_pointer()
                    .hover(|s| s.text_color(Theme::TEXT_PRIMARY))
                    .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                        clear_cb(window, app);
                    })
                    .child("✕"),
            )
        })
        // 加载指示
        .when(loading, |this| {
            this.child(
                div()
                    .flex_none()
                    .text_size(px(12.0))
                    .text_color(Theme::ACCENT_PRIMARY)
                    .child("⏳"),
            )
        })
        // 右侧圆形搜索按钮（对照 Vue n-button quaternary circle size="tiny"）
        .child(
            div()
                .id("search-suffix-btn")
                .flex_none()
                .size(px(22.0))
                .rounded(px(11.0))
                .flex()
                .items_center()
                .justify_center()
                .bg(if can_search {
                    Theme::ACCENT_LIGHT
                } else {
                    Theme::HOVER_BG
                })
                .text_color(if can_search {
                    Theme::ACCENT_PRIMARY
                } else {
                    Theme::TEXT_MUTED
                })
                .text_size(px(11.0))
                .cursor_pointer()
                .when(can_search, |this| {
                    this.hover(|s| s.bg(Theme::ACCENT_PRIMARY).text_color(Theme::BG_APP))
                        .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                            submit_cb(&submit_text, window, app);
                        })
                })
                .child("🔍"),
        )
}
