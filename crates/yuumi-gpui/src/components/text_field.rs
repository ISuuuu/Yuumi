//! 单行文本输入框
//!
//! gpui 0.2 尚未提供内置文本输入元素，这里给出最小可用实现：
//! 由视图持有文本，按键事件在元素内处理并回调新文本。

use crate::components::icon::icon;
use crate::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::*;

/// 判断是否为控制类特殊键名
fn is_special_key(key: &str) -> bool {
    matches!(
        key,
        "backspace"
            | "enter"
            | "delete"
            | "escape"
            | "tab"
            | "control"
            | "shift"
            | "alt"
            | "meta"
            | "super"
            | "capslock"
            | "up"
            | "down"
            | "left"
            | "right"
            | "home"
            | "end"
            | "pageup"
            | "pagedown"
            | "insert"
            | "f1"
            | "f2"
            | "f3"
            | "f4"
            | "f5"
            | "f6"
            | "f7"
            | "f8"
            | "f9"
            | "f10"
            | "f11"
            | "f12"
    )
}

/// 提取输入法合成字符、常规按键字符或中文上屏文本
fn extract_typed_text(keystroke: &Keystroke) -> Option<String> {
    if let Some(ref kc) = keystroke.key_char {
        if !kc.is_empty() {
            return Some(kc.clone());
        }
    }
    let key = keystroke.key.as_str();
    if key == "space" {
        Some(" ".to_string())
    } else if !is_special_key(key) && !key.is_empty() {
        Some(key.to_string())
    } else {
        None
    }
}
#[derive(Clone, Copy, Default)]
pub struct TextFieldOptions {
    /// 禁用输入（例如后台搜索中）
    pub disabled: bool,
    /// 显示加载指示
    pub loading: bool,
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

        // 快捷键支持：Ctrl+V (粘贴), Ctrl+C (复制), Ctrl+X (剪切), Ctrl+A (全选清空/替换)
        if keystroke.modifiers.control || keystroke.modifiers.platform {
            match keystroke.key.as_str() {
                "v" => {
                    if let Some(item) = app.read_from_clipboard() {
                        if let Some(clip) = item.text() {
                            let mut next = keyed_text.clone();
                            let clean = clip.replace(['\r', '\n'], "");
                            next.push_str(&clean);
                            if let Some(max) = options.max_chars {
                                let chars: String = next.chars().take(max).collect();
                                on_change(chars, window, app);
                            } else {
                                on_change(next, window, app);
                            }
                        }
                    }
                    return;
                }
                "c" => {
                    if !keyed_text.is_empty() {
                        app.write_to_clipboard(ClipboardItem::new_string(keyed_text.clone()));
                    }
                    return;
                }
                "x" => {
                    if !keyed_text.is_empty() {
                        app.write_to_clipboard(ClipboardItem::new_string(keyed_text.clone()));
                        on_change(String::new(), window, app);
                    }
                    return;
                }
                "a" => {
                    // 全选后用户继续退格或输入将替换
                    return;
                }
                "backspace" => {
                    // Ctrl+退格：清空文本
                    on_change(String::new(), window, app);
                    return;
                }
                _ => return,
            }
        }

        if keystroke.modifiers.alt {
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
            "delete" => {
                next.pop();
            }
            "escape" | "tab" => return,
            _ => {
                let typed = extract_typed_text(keystroke);
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
        .h(px(28.0))
        .w(width)
        .rounded(px(10.0))
        .bg(Theme::CARD_BG())
        .border_1()
        .border_color(if is_focused {
            Theme::ACCENT_PRIMARY()
        } else {
            Theme::CARD_BORDER()
        })
        .map(|mut el| {
            if is_focused {
                el.style().box_shadow = Some(vec![BoxShadow {
                    color: Theme::ACCENT_LIGHT(),
                    offset: point(px(0.0), px(0.0)),
                    blur_radius: px(0.0),
                    spread_radius: px(3.0),
                }]);
            }
            el
        })
        .hover(|s| {
            if !disabled && !is_focused {
                s.border_color(Theme::ACCENT_PRIMARY())
            } else {
                s
            }
        })
        .px(px(10.0))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(6.0))
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
                .flex()
                .items_center()
                .gap(px(1.0))
                .text_size(px(12.5))
                .text_color(if has_text {
                    Theme::TEXT_PRIMARY()
                } else {
                    Theme::TEXT_MUTED()
                })
                .child(if has_text {
                    current
                } else {
                    placeholder.to_string()
                })
                // [P1-4] 聚焦时的文本输入光标指示（静态细竖线）
                .when(is_focused && !disabled, |this| {
                    this.child(
                        div()
                            .flex_none()
                            .w(px(1.5))
                            .h(px(14.0))
                            .rounded(px(1.0))
                            .bg(Theme::ACCENT_PRIMARY()),
                    )
                }),
        )
        .when(options.loading, |this| {
            this.child(
                div()
                    .flex_none()
                    .text_size(px(12.0))
                    .text_color(Theme::ACCENT_PRIMARY())
                    .child("⏳"),
            )
        })
        .when(has_text && !disabled && !options.loading, |this| {
            let clear_cb = clear_cb.clone();
            this.child(
                div()
                    .flex_none()
                    .size(px(18.0))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .text_color(Theme::TEXT_MUTED())
                    .hover(|s| s.text_color(Theme::TEXT_PRIMARY()).bg(Theme::HOVER_BG()))
                    .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                        app.stop_propagation();
                        clear_cb(window, app);
                    })
                    .child(svg().path("icons/close.svg").flex_none().size(px(12.0))),
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

        // 快捷键支持：Ctrl+V (粘贴), Ctrl+C (复制), Ctrl+X (剪切), Ctrl+A (全选), Ctrl+Backspace (清空)
        if keystroke.modifiers.control || keystroke.modifiers.platform {
            match keystroke.key.as_str() {
                "v" => {
                    if let Some(item) = app.read_from_clipboard() {
                        if let Some(clip) = item.text() {
                            let mut next = keyed_text.clone();
                            let clean = clip.replace(['\r', '\n'], "");
                            next.push_str(&clean);
                            let chars: String = next.chars().take(64).collect();
                            on_change(chars, window, app);
                        }
                    }
                    return;
                }
                "c" => {
                    if !keyed_text.is_empty() {
                        app.write_to_clipboard(ClipboardItem::new_string(keyed_text.clone()));
                    }
                    return;
                }
                "x" => {
                    if !keyed_text.is_empty() {
                        app.write_to_clipboard(ClipboardItem::new_string(keyed_text.clone()));
                        on_change(String::new(), window, app);
                    }
                    return;
                }
                "a" => {
                    return;
                }
                "backspace" => {
                    on_change(String::new(), window, app);
                    return;
                }
                _ => return,
            }
        }

        if keystroke.modifiers.alt {
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
            "delete" => {
                next.pop();
            }
            "escape" | "tab" => return,
            _ => {
                let typed = extract_typed_text(keystroke);
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
    // Naive UI 2.44.1 的浅色 quaternary 按钮使用中性文字色，禁用时半透明。
    let search_icon_color: Hsla = rgb(0x333639).into();

    div()
        .id(id)
        .track_focus(focus)
        .h(px(28.0))
        .w_full()
        .max_w(max_width)
        .rounded(px(10.0))
        .bg(Theme::CARD_BG())
        .border_1()
        .border_color(if is_focused {
            Theme::ACCENT_PRIMARY()
        } else {
            Theme::CARD_BORDER()
        })
        .map(|mut el| {
            if is_focused {
                el.style().box_shadow = Some(vec![BoxShadow {
                    color: Theme::ACCENT_LIGHT(),
                    offset: point(px(0.0), px(0.0)),
                    blur_radius: px(0.0),
                    spread_radius: px(3.0),
                }]);
            }
            el
        })
        .hover(|s| {
            if !disabled && !is_focused {
                s.border_color(Theme::ACCENT_PRIMARY())
            } else {
                s
            }
        })
        .px(px(10.0))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(6.0))
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
                .flex()
                .items_center()
                .gap(px(1.0))
                .text_size(px(14.0))
                .text_color(if has_text {
                    Theme::TEXT_PRIMARY()
                } else {
                    Theme::TEXT_MUTED()
                })
                .child(if has_text {
                    current
                } else {
                    placeholder.to_string()
                })
                // [P1-4] 聚焦时的文本输入光标指示（静态细竖线）
                .when(is_focused && !disabled, |this| {
                    this.child(
                        div()
                            .flex_none()
                            .w(px(1.5))
                            .h(px(14.0))
                            .rounded(px(1.0))
                            .bg(Theme::ACCENT_PRIMARY()),
                    )
                }),
        )
        // 右侧操作区：清除按钮 + 放大镜搜索按钮 (Vue quaternary circle tiny 1:1)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.0))
                // 清除图标：位于放大镜左侧，仅在有文字且未禁用未加载时显示
                .when(has_text && !disabled && !loading, |suffix| {
                    let clear_cb = clear_cb.clone();
                    suffix.child(
                        div()
                            .id("search-clear-btn")
                            .flex_none()
                            .size(px(18.0))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_color(Theme::TEXT_MUTED())
                            .hover(|s| s.text_color(Theme::TEXT_PRIMARY()).bg(Theme::HOVER_BG()))
                            .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                                app.stop_propagation();
                                clear_cb(window, app);
                            })
                            .child(svg().path("icons/close.svg").flex_none().size(px(12.0))),
                    )
                })
                // Vue 的 22px 按钮内，Naive UI 图标槽将 SVG 的实际绘制宽度限制为 14px。
                .child(
                    div()
                        .id("search-suffix-btn")
                        .flex_none()
                        .size(px(22.0))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(can_search, |btn| {
                            let submit_cb = submit_cb.clone();
                            let submit_text = submit_text.clone();
                            btn.cursor_pointer()
                                .hover(|s| s.bg(rgba(0x2e333817)))
                                .active(|s| s.bg(rgba(0x2e333821)))
                                .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                                    app.stop_propagation();
                                    submit_cb(&submit_text, window, app);
                                })
                        })
                        .child(icon(
                            "icons/search.svg",
                            14.0,
                            search_icon_color.opacity(if can_search { 1.0 } else { 0.5 }),
                        )),
                ),
        )
}
