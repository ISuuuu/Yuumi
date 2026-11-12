//! 单行文本输入框
//!
//! 具备完整的原生文本与输入法（IME 中文/英文/符号）支持：
//! - 注册 `gpui::InputHandler` 接入 Windows IMM32 / macOS / Linux 输入法体系；
//! - 键盘控制快捷键（退格、回车、剪切板 Ctrl+C/V/X/A）在组件内精准响应；
//! - 中文合成暂存（Composing）状态实时内联展示并定位系统输入法候选窗。

use crate::components::icon::icon;
use crate::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::cell::RefCell;
use std::ops::Range;
use std::sync::Arc;

#[derive(Default, Clone)]
struct ImeState {
    base_text: String,
    composing_text: String,
    composing_cursor_utf16: usize,
}

impl ImeState {
    fn sync_base(&mut self, text: &str) {
        self.base_text = text.to_string();
    }

    fn apply_committed_text(&mut self, text: &str, max_chars: Option<usize>) -> String {
        self.composing_text.clear();
        self.composing_cursor_utf16 = 0;
        let clean = text.replace(['\r', '\n'], "");
        if !clean.is_empty() {
            self.base_text.push_str(&clean);
            if let Some(max) = max_chars {
                self.base_text = self.base_text.chars().take(max).collect();
            }
        }
        self.base_text.clone()
    }

    fn set_composing(&mut self, new_text: &str, new_selected_range: Option<Range<usize>>) {
        self.composing_text = new_text.to_string();
        self.composing_cursor_utf16 = new_selected_range
            .map(|r| r.end)
            .unwrap_or_else(|| new_text.encode_utf16().count());
    }

    fn clear_composing(&mut self) {
        self.composing_text.clear();
        self.composing_cursor_utf16 = 0;
    }

    fn marked_utf16_range(&self) -> Option<Range<usize>> {
        if self.composing_text.is_empty() {
            None
        } else {
            let base_len = self.base_text.encode_utf16().count();
            let comp_len = self.composing_text.encode_utf16().count();
            Some(base_len..base_len + comp_len)
        }
    }

    fn selected_utf16_range(&self) -> Range<usize> {
        let caret = self.base_text.encode_utf16().count() + self.composing_cursor_utf16;
        caret..caret
    }

    fn text_for_utf16_range(
        &self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
    ) -> Option<String> {
        let full = format!("{}{}", self.base_text, self.composing_text);
        let u16: Vec<u16> = full.encode_utf16().collect();
        let start = range_utf16.start.min(u16.len());
        let end = range_utf16.end.min(u16.len()).max(start);
        *adjusted_range = Some(start..end);
        String::from_utf16(&u16[start..end]).ok()
    }
}

thread_local! {
    static ACTIVE_IME: RefCell<Option<(FocusHandle, ImeState)>> = const { RefCell::new(None) };
}

fn sync_active_ime(focus: &FocusHandle, is_focused: bool, text: &str) {
    ACTIVE_IME.with(|cell| {
        let mut borrow = cell.borrow_mut();
        if is_focused {
            if let Some((ref cur_focus, ref mut state)) = *borrow {
                if cur_focus == focus {
                    state.sync_base(text);
                    return;
                }
            }
            *borrow = Some((
                focus.clone(),
                ImeState {
                    base_text: text.to_string(),
                    composing_text: String::new(),
                    composing_cursor_utf16: 0,
                },
            ));
        } else if let Some((ref cur_focus, _)) = *borrow {
            if cur_focus == focus {
                *borrow = None;
            }
        }
    });
}

type ChangeCallback = Arc<dyn Fn(String, &mut Window, &mut App) + 'static>;

/// 负责与 GPUI / 操作系统输入法栈交互的平台文本输入处理器
struct TextFieldInputHandler {
    focus: FocusHandle,
    bounds: Bounds<Pixels>,
    max_chars: Option<usize>,
    on_change: ChangeCallback,
}

impl InputHandler for TextFieldInputHandler {
    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<UTF16Selection> {
        ACTIVE_IME.with(|cell| {
            let borrow = cell.borrow();
            let (_, state) = borrow.as_ref().filter(|(f, _)| f == &self.focus)?;
            Some(UTF16Selection {
                range: state.selected_utf16_range(),
                reversed: false,
            })
        })
    }

    fn marked_text_range(&mut self, _window: &mut Window, _cx: &mut App) -> Option<Range<usize>> {
        ACTIVE_IME.with(|cell| {
            let borrow = cell.borrow();
            let (_, state) = borrow.as_ref().filter(|(f, _)| f == &self.focus)?;
            state.marked_utf16_range()
        })
    }

    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<String> {
        ACTIVE_IME.with(|cell| {
            let borrow = cell.borrow();
            let (_, state) = borrow.as_ref().filter(|(f, _)| f == &self.focus)?;
            state.text_for_utf16_range(range_utf16, adjusted_range)
        })
    }

    fn replace_text_in_range(
        &mut self,
        _replacement_range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut App,
    ) {
        let next_text = ACTIVE_IME.with(|cell| {
            let mut borrow = cell.borrow_mut();
            let (_, ref mut state) = borrow.as_mut().filter(|(f, _)| f == &self.focus)?;
            Some(state.apply_committed_text(text, self.max_chars))
        });
        if let Some(next) = next_text {
            (self.on_change)(next, window, cx);
        }
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        window: &mut Window,
        _cx: &mut App,
    ) {
        ACTIVE_IME.with(|cell| {
            let mut borrow = cell.borrow_mut();
            if let Some((_, ref mut state)) = borrow.as_mut().filter(|(f, _)| f == &self.focus) {
                state.set_composing(new_text, new_selected_range);
            }
        });
        window.refresh();
    }

    fn unmark_text(&mut self, window: &mut Window, _cx: &mut App) {
        ACTIVE_IME.with(|cell| {
            let mut borrow = cell.borrow_mut();
            if let Some((_, ref mut state)) = borrow.as_mut().filter(|(f, _)| f == &self.focus) {
                state.clear_composing();
            }
        });
        window.refresh();
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<Bounds<Pixels>> {
        Some(self.bounds)
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<usize> {
        ACTIVE_IME.with(|cell| {
            let borrow = cell.borrow();
            let (_, state) = borrow.as_ref().filter(|(f, _)| f == &self.focus)?;
            Some(state.base_text.encode_utf16().count())
        })
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
/// - `on_change`：每次键入/退格/输入法提交后回调新文本
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

    sync_active_ime(focus, is_focused, &current);

    let (composing_display, has_composing) = ACTIVE_IME.with(|cell| {
        let borrow = cell.borrow();
        let c_text = borrow
            .as_ref()
            .filter(|(f, _)| f == focus)
            .map(|(_, s)| s.composing_text.clone())
            .unwrap_or_default();
        let has = !c_text.is_empty();
        (c_text, has)
    });

    let keyed_text = current.clone();
    let on_change_key = on_change.clone();
    let on_key = move |ev: &KeyDownEvent, window: &mut Window, app: &mut App| {
        if disabled {
            return;
        }
        let keystroke = &ev.keystroke;

        // 快捷键支持：Ctrl+V (粘贴), Ctrl+C (复制), Ctrl+X (剪切), Ctrl+A (全选清空/替换)
        if keystroke.modifiers.control || keystroke.modifiers.platform {
            match keystroke.key.as_str() {
                "v" => {
                    app.stop_propagation();
                    if let Some(item) = app.read_from_clipboard() {
                        if let Some(clip) = item.text() {
                            let mut next = keyed_text.clone();
                            let clean = clip.replace(['\r', '\n'], "");
                            next.push_str(&clean);
                            if let Some(max) = options.max_chars {
                                let chars: String = next.chars().take(max).collect();
                                on_change_key(chars, window, app);
                            } else {
                                on_change_key(next, window, app);
                            }
                        }
                    }
                    return;
                }
                "c" => {
                    app.stop_propagation();
                    if !keyed_text.is_empty() {
                        app.write_to_clipboard(ClipboardItem::new_string(keyed_text.clone()));
                    }
                    return;
                }
                "x" => {
                    app.stop_propagation();
                    if !keyed_text.is_empty() {
                        app.write_to_clipboard(ClipboardItem::new_string(keyed_text.clone()));
                        on_change_key(String::new(), window, app);
                    }
                    return;
                }
                "a" => {
                    app.stop_propagation();
                    return;
                }
                "backspace" => {
                    app.stop_propagation();
                    if !keyed_text.is_empty() {
                        on_change_key(String::new(), window, app);
                    }
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
            "backspace" | "delete" => {
                app.stop_propagation();
                if !keyed_text.is_empty() {
                    next.pop();
                    on_change_key(next, window, app);
                }
            }
            "enter" => {
                app.stop_propagation();
                on_submit(&keyed_text, window, app);
            }
            "escape" | "tab" => {}
            _ => {
                // 常规字符输入与输入法（中文/符号）均交由操作系统 TranslateMessage -> InputHandler 处理
            }
        }
    };

    let clear_cb = on_clear;
    let focus_for_input = focus.clone();
    let focus_for_prepaint = focus.clone();
    let current_for_prepaint = current.clone();
    let max_chars = options.max_chars;
    let on_change_input = on_change;

    div()
        .id(id)
        .relative()
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
            canvas(
                move |_bounds, window, _cx| {
                    sync_active_ime(
                        &focus_for_prepaint,
                        focus_for_prepaint.is_focused(window),
                        &current_for_prepaint,
                    );
                },
                move |bounds, (), window, cx| {
                    if focus_for_input.is_focused(window) && !disabled {
                        window.handle_input(
                            &focus_for_input,
                            TextFieldInputHandler {
                                focus: focus_for_input.clone(),
                                bounds: Bounds {
                                    origin: bounds.origin,
                                    size: size(width, px(28.0)),
                                },
                                max_chars,
                                on_change: Arc::new(on_change_input),
                            },
                            cx,
                        );
                    }
                },
            )
            .size_0(),
        )
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
                .text_color(if has_text || has_composing {
                    Theme::TEXT_PRIMARY()
                } else {
                    Theme::TEXT_MUTED()
                })
                .child(if has_composing {
                    div()
                        .flex()
                        .items_center()
                        .child(current)
                        .child(
                            div()
                                .text_color(Theme::ACCENT_PRIMARY())
                                .child(composing_display),
                        )
                        .into_any_element()
                } else if has_text {
                    current.into_any_element()
                } else {
                    placeholder.to_string().into_any_element()
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
                        ACTIVE_IME.with(|cell| {
                            *cell.borrow_mut() = None;
                        });
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

    sync_active_ime(focus, is_focused, &current);

    let (composing_display, has_composing) = ACTIVE_IME.with(|cell| {
        let borrow = cell.borrow();
        let c_text = borrow
            .as_ref()
            .filter(|(f, _)| f == focus)
            .map(|(_, s)| s.composing_text.clone())
            .unwrap_or_default();
        let has = !c_text.is_empty();
        (c_text, has)
    });

    let keyed_text = current.clone();
    let submit_for_key = on_submit.clone();
    let on_change_key = on_change.clone();
    let on_key = move |ev: &KeyDownEvent, window: &mut Window, app: &mut App| {
        if disabled || loading {
            return;
        }
        let keystroke = &ev.keystroke;

        // 快捷键支持：Ctrl+V (粘贴), Ctrl+C (复制), Ctrl+X (剪切), Ctrl+A (全选), Ctrl+Backspace (清空)
        if keystroke.modifiers.control || keystroke.modifiers.platform {
            match keystroke.key.as_str() {
                "v" => {
                    app.stop_propagation();
                    if let Some(item) = app.read_from_clipboard() {
                        if let Some(clip) = item.text() {
                            let mut next = keyed_text.clone();
                            let clean = clip.replace(['\r', '\n'], "");
                            next.push_str(&clean);
                            let chars: String = next.chars().take(64).collect();
                            on_change_key(chars, window, app);
                        }
                    }
                    return;
                }
                "c" => {
                    app.stop_propagation();
                    if !keyed_text.is_empty() {
                        app.write_to_clipboard(ClipboardItem::new_string(keyed_text.clone()));
                    }
                    return;
                }
                "x" => {
                    app.stop_propagation();
                    if !keyed_text.is_empty() {
                        app.write_to_clipboard(ClipboardItem::new_string(keyed_text.clone()));
                        on_change_key(String::new(), window, app);
                    }
                    return;
                }
                "a" => {
                    app.stop_propagation();
                    return;
                }
                "backspace" => {
                    app.stop_propagation();
                    if !keyed_text.is_empty() {
                        on_change_key(String::new(), window, app);
                    }
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
            "backspace" | "delete" => {
                app.stop_propagation();
                if !keyed_text.is_empty() {
                    next.pop();
                    on_change_key(next, window, app);
                }
            }
            "enter" => {
                app.stop_propagation();
                if !keyed_text.trim().is_empty() {
                    submit_for_key(&keyed_text, window, app);
                }
            }
            "escape" | "tab" => {}
            _ => {
                // 常规字符输入与输入法（中文/符号）均交由操作系统 TranslateMessage -> InputHandler 处理
            }
        }
    };

    let clear_cb = on_clear;
    let submit_cb = on_submit;
    let submit_text = current.clone();
    // Naive UI 2.44.1 的浅色 quaternary 按钮使用中性文字色，禁用时半透明。
    let search_icon_color: Hsla = rgb(0x333639).into();
    let focus_for_input = focus.clone();
    let focus_for_prepaint = focus.clone();
    let current_for_prepaint = current.clone();
    let on_change_input = on_change;

    div()
        .id(id)
        .relative()
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
            canvas(
                move |_bounds, window, _cx| {
                    sync_active_ime(
                        &focus_for_prepaint,
                        focus_for_prepaint.is_focused(window),
                        &current_for_prepaint,
                    );
                },
                move |bounds, (), window, cx| {
                    if focus_for_input.is_focused(window) && !disabled && !loading {
                        window.handle_input(
                            &focus_for_input,
                            TextFieldInputHandler {
                                focus: focus_for_input.clone(),
                                bounds: Bounds {
                                    origin: bounds.origin,
                                    size: size(max_width, px(28.0)),
                                },
                                max_chars: Some(64),
                                on_change: Arc::new(on_change_input),
                            },
                            cx,
                        );
                    }
                },
            )
            .size_0(),
        )
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
                .text_color(if has_text || has_composing {
                    Theme::TEXT_PRIMARY()
                } else {
                    Theme::TEXT_MUTED()
                })
                .child(if has_composing {
                    div()
                        .flex()
                        .items_center()
                        .child(current)
                        .child(
                            div()
                                .text_color(Theme::ACCENT_PRIMARY())
                                .child(composing_display),
                        )
                        .into_any_element()
                } else if has_text {
                    current.into_any_element()
                } else {
                    placeholder.to_string().into_any_element()
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
                                ACTIVE_IME.with(|cell| {
                                    *cell.borrow_mut() = None;
                                });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn ime_state_handles_composing_and_chinese_commit() {
        let mut state = ImeState::default();
        state.sync_base("我爱");

        // 拼音合成阶段
        state.set_composing("timo", Some(4..4));
        assert_eq!(state.marked_utf16_range(), Some(2..6));
        assert_eq!(state.selected_utf16_range(), 6..6);

        let mut adjusted = None;
        let slice = state.text_for_utf16_range(0..6, &mut adjusted);
        assert_eq!(slice.as_deref(), Some("我爱timo"));
        assert_eq!(adjusted, Some(0..6));

        // 中文上屏提交
        let committed = state.apply_committed_text("提莫", None);
        assert_eq!(committed, "我爱提莫");
        assert_eq!(state.marked_utf16_range(), None);
        assert_eq!(state.selected_utf16_range(), 4..4);
    }

    #[::core::prelude::v1::test]
    fn ime_state_respects_max_chars_with_cjk_and_strips_newlines() {
        let mut state = ImeState::default();
        state.sync_base("英雄");
        let committed = state.apply_committed_text("联盟\r\n提莫", Some(4));
        assert_eq!(committed, "英雄联盟");
    }

    #[::core::prelude::v1::test]
    fn ime_state_cancel_composing_preserves_base_text() {
        let mut state = ImeState::default();
        state.sync_base("Teemo");
        state.set_composing("nihao", None);
        assert!(state.marked_utf16_range().is_some());

        // lparam=0 取消合成或 unmark_text
        let committed = state.apply_committed_text("", None);
        assert_eq!(committed, "Teemo");
        assert_eq!(state.marked_utf16_range(), None);

        state.set_composing("abc", None);
        state.clear_composing();
        assert_eq!(state.marked_utf16_range(), None);
    }
}
