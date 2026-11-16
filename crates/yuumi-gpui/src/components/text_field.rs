//! 单行文本输入框
//!
//! 具备完整的原生文本编辑与输入法（IME 中文/英文/符号）支持：
//! - 注册 `gpui::InputHandler` 接入 Windows IMM32 / macOS / Linux 输入法体系；
//! - 维护插入点、选区、水平滚动与撤销栈（方向键、Home/End、Shift 扩展选区、Ctrl+A/C/V/X/Z、Backspace/Delete）；
//! - 支持鼠标点击定位光标与左键拖拽选区；
//! - 中文合成暂存（Composing）状态实时内联展示并定位系统输入法候选窗。

use crate::components::icon::icon;
use crate::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::cell::RefCell;
use std::ops::Range;
use std::sync::Arc;

const MAX_UNDO_STEPS: usize = 64;

fn clamp_to_char_boundary(s: &str, idx: usize) -> usize {
    let mut i = idx.min(s.len());
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn prev_char_boundary(s: &str, idx: usize) -> usize {
    let i = clamp_to_char_boundary(s, idx);
    if i == 0 {
        return 0;
    }
    s[..i]
        .char_indices()
        .next_back()
        .map(|(pos, _)| pos)
        .unwrap_or(0)
}

fn next_char_boundary(s: &str, idx: usize) -> usize {
    let i = clamp_to_char_boundary(s, idx);
    if i >= s.len() {
        return s.len();
    }
    i + s[i..].chars().next().map(char::len_utf8).unwrap_or(0)
}

fn utf8_to_utf16(s: &str, byte_idx: usize) -> usize {
    let clamped = clamp_to_char_boundary(s, byte_idx);
    s[..clamped].encode_utf16().count()
}

fn utf16_to_utf8(s: &str, u16_idx: usize) -> usize {
    let mut acc = 0usize;
    for (byte_idx, ch) in s.char_indices() {
        let len = ch.len_utf16();
        if acc + len > u16_idx {
            return byte_idx;
        }
        acc += len;
    }
    s.len()
}

#[derive(Default, Clone)]
struct ImeState {
    base_text: String,
    cursor: usize,
    selection_anchor: Option<usize>,
    composing_text: String,
    composing_cursor_utf16: usize,
    undo_stack: Vec<(String, usize)>,
    scroll_x: f32,
    text_origin_x: f32,
    viewport_width: f32,
    /// `(byte_offset_in_base_text, x_offset_from_text_start)`
    char_offsets: Vec<(usize, f32)>,
    is_dragging: bool,
}

impl ImeState {
    fn new(text: &str) -> Self {
        let len = text.len();
        Self {
            base_text: text.to_string(),
            cursor: len,
            ..Default::default()
        }
    }

    fn sync_base(&mut self, text: &str) {
        if self.base_text != text {
            self.base_text = text.to_string();
            self.cursor = clamp_to_char_boundary(&self.base_text, self.cursor);
            if let Some(anchor) = self.selection_anchor {
                let clamped = clamp_to_char_boundary(&self.base_text, anchor);
                self.selection_anchor = (clamped != self.cursor).then_some(clamped);
            }
        }
    }

    fn selection_range(&self) -> Option<Range<usize>> {
        let anchor = self.selection_anchor?;
        let a = clamp_to_char_boundary(&self.base_text, anchor);
        let b = clamp_to_char_boundary(&self.base_text, self.cursor);
        if a == b {
            None
        } else if a < b {
            Some(a..b)
        } else {
            Some(b..a)
        }
    }

    fn selected_text(&self) -> Option<&str> {
        let r = self.selection_range()?;
        self.base_text.get(r)
    }

    fn push_undo(&mut self) {
        let snapshot = (self.base_text.clone(), self.cursor);
        if self.undo_stack.last() != Some(&snapshot) {
            self.undo_stack.push(snapshot);
            if self.undo_stack.len() > MAX_UNDO_STEPS {
                self.undo_stack.remove(0);
            }
        }
    }

    fn undo(&mut self) -> Option<String> {
        let (prev_text, prev_cursor) = self.undo_stack.pop()?;
        self.clear_composing();
        self.base_text = prev_text;
        self.cursor = clamp_to_char_boundary(&self.base_text, prev_cursor);
        self.selection_anchor = None;
        Some(self.base_text.clone())
    }

    fn select_all(&mut self) {
        if self.base_text.is_empty() {
            self.cursor = 0;
            self.selection_anchor = None;
        } else {
            self.selection_anchor = Some(0);
            self.cursor = self.base_text.len();
        }
    }

    fn set_cursor(&mut self, next_cursor: usize, extend_selection: bool) {
        let next = clamp_to_char_boundary(&self.base_text, next_cursor);
        if extend_selection {
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some(self.cursor);
            }
            self.cursor = next;
            if self.selection_anchor == Some(self.cursor) {
                self.selection_anchor = None;
            }
        } else {
            self.selection_anchor = None;
            self.cursor = next;
        }
    }

    fn move_left(&mut self, extend: bool) {
        if !extend {
            if let Some(r) = self.selection_range() {
                self.set_cursor(r.start, false);
                return;
            }
        }
        let next = prev_char_boundary(&self.base_text, self.cursor);
        self.set_cursor(next, extend);
    }

    fn move_right(&mut self, extend: bool) {
        if !extend {
            if let Some(r) = self.selection_range() {
                self.set_cursor(r.end, false);
                return;
            }
        }
        let next = next_char_boundary(&self.base_text, self.cursor);
        self.set_cursor(next, extend);
    }

    fn move_home(&mut self, extend: bool) {
        self.set_cursor(0, extend);
    }

    fn move_end(&mut self, extend: bool) {
        self.set_cursor(self.base_text.len(), extend);
    }

    /// 用 `insert` 替换 `range`（UTF-8 字节范围），尊重 `max_chars` 上限
    fn replace_range_utf8(
        &mut self,
        range: Range<usize>,
        insert: &str,
        max_chars: Option<usize>,
    ) -> String {
        self.clear_composing();
        let start = clamp_to_char_boundary(&self.base_text, range.start);
        let end = clamp_to_char_boundary(&self.base_text, range.end.max(start));
        let clean = insert.replace(['\r', '\n'], "");

        let allowed_insert: String = if let Some(max) = max_chars {
            let prefix_chars = self.base_text[..start].chars().count();
            let suffix_chars = self.base_text[end..].chars().count();
            let remaining = max.saturating_sub(prefix_chars + suffix_chars);
            clean.chars().take(remaining).collect()
        } else {
            clean
        };

        let mut next =
            String::with_capacity(self.base_text.len() - (end - start) + allowed_insert.len());
        next.push_str(&self.base_text[..start]);
        next.push_str(&allowed_insert);
        let new_cursor = next.len();
        next.push_str(&self.base_text[end..]);

        self.base_text = next;
        self.cursor = new_cursor;
        self.selection_anchor = None;
        self.base_text.clone()
    }

    fn apply_committed_text_in_range(
        &mut self,
        replacement_range_utf16: Option<Range<usize>>,
        text: &str,
        max_chars: Option<usize>,
    ) -> String {
        self.clear_composing();
        let clean = text.replace(['\r', '\n'], "");
        let target_range = if let Some(r_u16) = replacement_range_utf16 {
            let s = utf16_to_utf8(&self.base_text, r_u16.start);
            let e = utf16_to_utf8(&self.base_text, r_u16.end.max(r_u16.start));
            s..e
        } else if let Some(sel) = self.selection_range() {
            sel
        } else {
            let c = clamp_to_char_boundary(&self.base_text, self.cursor);
            c..c
        };

        if clean.is_empty() && target_range.is_empty() {
            return self.base_text.clone();
        }

        self.push_undo();
        self.replace_range_utf8(target_range, &clean, max_chars)
    }

    #[cfg(test)]
    fn apply_committed_text(&mut self, text: &str, max_chars: Option<usize>) -> String {
        self.apply_committed_text_in_range(None, text, max_chars)
    }

    fn backspace(&mut self, clear_all_before: bool) -> Option<String> {
        if let Some(sel) = self.selection_range() {
            self.push_undo();
            return Some(self.replace_range_utf8(sel, "", None));
        }
        let c = clamp_to_char_boundary(&self.base_text, self.cursor);
        if c == 0 {
            return None;
        }
        self.push_undo();
        let start = if clear_all_before {
            0
        } else {
            prev_char_boundary(&self.base_text, c)
        };
        Some(self.replace_range_utf8(start..c, "", None))
    }

    fn delete(&mut self) -> Option<String> {
        if let Some(sel) = self.selection_range() {
            self.push_undo();
            return Some(self.replace_range_utf8(sel, "", None));
        }
        let c = clamp_to_char_boundary(&self.base_text, self.cursor);
        if c >= self.base_text.len() {
            return None;
        }
        self.push_undo();
        let end = next_char_boundary(&self.base_text, c);
        Some(self.replace_range_utf8(c..end, "", None))
    }

    fn cut_selection(&mut self) -> Option<(String, String)> {
        let sel = self.selection_range()?;
        let copied = self.base_text[sel.clone()].to_string();
        self.push_undo();
        let next = self.replace_range_utf8(sel, "", None);
        Some((copied, next))
    }

    fn set_composing(&mut self, new_text: &str, new_selected_range: Option<Range<usize>>) {
        self.composing_text = new_text.to_string();
        let comp_u16_len = new_text.encode_utf16().count();
        self.composing_cursor_utf16 = new_selected_range
            .map(|r| r.end.min(comp_u16_len))
            .unwrap_or(comp_u16_len);
    }

    fn clear_composing(&mut self) {
        self.composing_text.clear();
        self.composing_cursor_utf16 = 0;
    }

    fn marked_utf16_range(&self) -> Option<Range<usize>> {
        if self.composing_text.is_empty() {
            None
        } else {
            let cursor_u16 = utf8_to_utf16(&self.base_text, self.cursor);
            let comp_len = self.composing_text.encode_utf16().count();
            Some(cursor_u16..cursor_u16 + comp_len)
        }
    }

    fn selected_utf16_range(&self) -> Range<usize> {
        if !self.composing_text.is_empty() {
            let caret = utf8_to_utf16(&self.base_text, self.cursor) + self.composing_cursor_utf16;
            return caret..caret;
        }
        if let Some(sel) = self.selection_range() {
            utf8_to_utf16(&self.base_text, sel.start)..utf8_to_utf16(&self.base_text, sel.end)
        } else {
            let caret = utf8_to_utf16(&self.base_text, self.cursor);
            caret..caret
        }
    }

    fn full_text_with_composing(&self) -> String {
        if self.composing_text.is_empty() {
            self.base_text.clone()
        } else {
            let c = clamp_to_char_boundary(&self.base_text, self.cursor);
            format!(
                "{}{}{}",
                &self.base_text[..c],
                self.composing_text,
                &self.base_text[c..]
            )
        }
    }

    fn text_for_utf16_range(
        &self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
    ) -> Option<String> {
        let full = self.full_text_with_composing();
        let u16: Vec<u16> = full.encode_utf16().collect();
        let start = range_utf16.start.min(u16.len());
        let end = range_utf16.end.min(u16.len()).max(start);
        *adjusted_range = Some(start..end);
        String::from_utf16(&u16[start..end]).ok()
    }

    fn byte_index_for_x(&self, local_x: f32) -> usize {
        if self.char_offsets.is_empty() {
            return self.base_text.len();
        }
        let target_x = (local_x + self.scroll_x).max(0.0);
        let mut best_idx = 0usize;
        let mut best_dist = f32::MAX;
        for &(byte_idx, x) in &self.char_offsets {
            let dist = (target_x - x).abs();
            if dist < best_dist {
                best_dist = dist;
                best_idx = byte_idx;
            }
        }
        best_idx
    }

    fn x_for_byte_index(&self, byte_idx: usize) -> f32 {
        let target = clamp_to_char_boundary(&self.base_text, byte_idx);
        for &(idx, x) in &self.char_offsets {
            if idx == target {
                return x;
            }
        }
        self.char_offsets.last().map(|(_, x)| *x).unwrap_or(0.0)
    }

    fn ensure_cursor_visible(&mut self, caret_x: f32, viewport_width: f32) {
        if viewport_width <= 0.0 {
            self.scroll_x = 0.0;
            return;
        }
        let max_x = self.char_offsets.last().map(|(_, x)| *x).unwrap_or(0.0);
        if max_x <= viewport_width {
            self.scroll_x = 0.0;
            return;
        }
        let pad = 8.0_f32.min(viewport_width * 0.2);
        if caret_x < self.scroll_x + pad {
            self.scroll_x = (caret_x - pad).max(0.0);
        } else if caret_x > self.scroll_x + viewport_width - pad {
            self.scroll_x = (caret_x - (viewport_width - pad)).max(0.0);
        }
        let max_scroll = (max_x - viewport_width + 4.0).max(0.0);
        self.scroll_x = self.scroll_x.clamp(0.0, max_scroll);
    }
}

thread_local! {
    static ACTIVE_IME: RefCell<Option<(FocusHandle, ImeState)>> = const { RefCell::new(None) };
}

fn with_focus_state<R>(focus: &FocusHandle, f: impl FnOnce(&mut ImeState) -> R) -> Option<R> {
    ACTIVE_IME.with(|cell| {
        let mut borrow = cell.borrow_mut();
        let (_, ref mut state) = borrow.as_mut().filter(|(fh, _)| fh == focus)?;
        Some(f(state))
    })
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
            *borrow = Some((focus.clone(), ImeState::new(text)));
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
        replacement_range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (next_text, changed) = ACTIVE_IME
            .with(|cell| {
                let mut borrow = cell.borrow_mut();
                let (_, ref mut state) = borrow.as_mut().filter(|(f, _)| f == &self.focus)?;
                let prev_base = state.base_text.clone();
                let next =
                    state.apply_committed_text_in_range(replacement_range, text, self.max_chars);
                let changed = next != prev_base;
                Some((next, changed))
            })
            .unwrap_or_default();

        if changed {
            (self.on_change)(next_text, window, cx);
        } else {
            window.refresh();
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
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<usize> {
        ACTIVE_IME.with(|cell| {
            let borrow = cell.borrow();
            let (_, state) = borrow.as_ref().filter(|(f, _)| f == &self.focus)?;
            let local_x = f32::from(point.x) - state.text_origin_x;
            let byte_idx = state.byte_index_for_x(local_x);
            Some(utf8_to_utf16(&state.base_text, byte_idx))
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

/// 统一处理单行输入框按键（Ctrl+A/C/V/X/Z、方向键、Home/End、Shift 选区、Backspace/Delete、Enter）
#[allow(clippy::too_many_arguments)]
fn handle_key_down(
    focus: &FocusHandle,
    fallback_text: &str,
    max_chars: Option<usize>,
    require_non_empty_submit: bool,
    ev: &KeyDownEvent,
    window: &mut Window,
    app: &mut App,
    on_change: &impl Fn(String, &mut Window, &mut App),
    on_submit: &impl Fn(&str, &mut Window, &mut App),
) {
    let is_composing_now = ACTIVE_IME.with(|cell| {
        cell.borrow()
            .as_ref()
            .filter(|(f, _)| f == focus)
            .is_some_and(|(_, s)| !s.composing_text.is_empty())
    });
    if is_composing_now {
        return;
    }

    sync_active_ime(focus, true, fallback_text);
    let keystroke = &ev.keystroke;

    if keystroke.modifiers.control || keystroke.modifiers.platform {
        match keystroke.key.as_str() {
            "v" => {
                app.stop_propagation();
                if let Some(clip) = app.read_from_clipboard().and_then(|item| item.text()) {
                    if let Some(next) = with_focus_state(focus, |s| {
                        s.apply_committed_text_in_range(None, &clip, max_chars)
                    }) {
                        on_change(next, window, app);
                    }
                }
                return;
            }
            "c" => {
                app.stop_propagation();
                if let Some(copied) = with_focus_state(focus, |s| {
                    s.selected_text()
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| s.base_text.clone())
                }) {
                    if !copied.is_empty() {
                        app.write_to_clipboard(ClipboardItem::new_string(copied));
                    }
                }
                return;
            }
            "x" => {
                app.stop_propagation();
                let cut_result = with_focus_state(focus, |s| {
                    if let Some(res) = s.cut_selection() {
                        Some(res)
                    } else if !s.base_text.is_empty() {
                        let all = s.base_text.clone();
                        s.push_undo();
                        let next = s.replace_range_utf8(0..all.len(), "", None);
                        Some((all, next))
                    } else {
                        None
                    }
                })
                .flatten();
                if let Some((copied, next)) = cut_result {
                    app.write_to_clipboard(ClipboardItem::new_string(copied));
                    on_change(next, window, app);
                }
                return;
            }
            "a" => {
                app.stop_propagation();
                let _ = with_focus_state(focus, |s| {
                    s.select_all();
                });
                window.refresh();
                return;
            }
            "z" => {
                app.stop_propagation();
                if let Some(Some(next)) = with_focus_state(focus, |s| s.undo()) {
                    on_change(next, window, app);
                }
                return;
            }
            "backspace" => {
                app.stop_propagation();
                if let Some(Some(next)) = with_focus_state(focus, |s| s.backspace(true)) {
                    on_change(next, window, app);
                }
                return;
            }
            _ => return,
        }
    }

    if keystroke.modifiers.alt {
        return;
    }

    let shift = keystroke.modifiers.shift;
    match keystroke.key.as_str() {
        "left" => {
            app.stop_propagation();
            let _ = with_focus_state(focus, |s| s.move_left(shift));
            window.refresh();
        }
        "right" => {
            app.stop_propagation();
            let _ = with_focus_state(focus, |s| s.move_right(shift));
            window.refresh();
        }
        "home" => {
            app.stop_propagation();
            let _ = with_focus_state(focus, |s| s.move_home(shift));
            window.refresh();
        }
        "end" => {
            app.stop_propagation();
            let _ = with_focus_state(focus, |s| s.move_end(shift));
            window.refresh();
        }
        "backspace" => {
            app.stop_propagation();
            if let Some(Some(next)) = with_focus_state(focus, |s| s.backspace(false)) {
                on_change(next, window, app);
            }
        }
        "delete" => {
            app.stop_propagation();
            if let Some(Some(next)) = with_focus_state(focus, |s| s.delete()) {
                on_change(next, window, app);
            }
        }
        "enter" => {
            app.stop_propagation();
            let cur = with_focus_state(focus, |s| s.base_text.clone())
                .unwrap_or_else(|| fallback_text.to_string());
            if !require_non_empty_submit || !cur.trim().is_empty() {
                on_submit(&cur, window, app);
            }
        }
        "escape" | "tab" => {}
        _ => {}
    }
}

struct RenderSnapshot {
    base_text: String,
    cursor: usize,
    selection: Option<Range<usize>>,
    composing_text: String,
    scroll_x: f32,
    caret_x: f32,
    sel_start_x: f32,
    sel_end_x: f32,
}

fn read_render_snapshot(focus: &FocusHandle, fallback_text: &str) -> RenderSnapshot {
    ACTIVE_IME.with(|cell| {
        let borrow = cell.borrow();
        if let Some((_, state)) = borrow.as_ref().filter(|(f, _)| f == focus) {
            let selection = state.selection_range();
            let (sel_start_x, sel_end_x) = if let Some(ref sel) = selection {
                (
                    state.x_for_byte_index(sel.start),
                    state.x_for_byte_index(sel.end),
                )
            } else {
                (0.0, 0.0)
            };
            let caret_x = state.x_for_byte_index(state.cursor);
            return RenderSnapshot {
                base_text: state.base_text.clone(),
                cursor: clamp_to_char_boundary(&state.base_text, state.cursor),
                selection,
                composing_text: state.composing_text.clone(),
                scroll_x: state.scroll_x,
                caret_x,
                sel_start_x,
                sel_end_x,
            };
        }
        RenderSnapshot {
            base_text: fallback_text.to_string(),
            cursor: fallback_text.len(),
            selection: None,
            composing_text: String::new(),
            scroll_x: 0.0,
            caret_x: 0.0,
            sel_start_x: 0.0,
            sel_end_x: 0.0,
        }
    })
}

/// 文本布局测算 + IME InputHandler 注册 + 选区/文本/光标内联渲染
fn render_editable_text_area(
    focus: &FocusHandle,
    text: &str,
    placeholder: &str,
    is_focused: bool,
    disabled: bool,
    max_chars: Option<usize>,
    on_change: ChangeCallback,
) -> Div {
    let snapshot = read_render_snapshot(focus, text);
    let has_text = !snapshot.base_text.is_empty();
    let has_composing = !snapshot.composing_text.is_empty();

    let focus_for_prepaint = focus.clone();
    let focus_for_input = focus.clone();
    let current_for_prepaint = text.to_string();

    let focus_for_down = focus.clone();
    let focus_for_move = focus.clone();
    let focus_for_up = focus.clone();
    let fallback_for_down = text.to_string();

    div()
        .flex_1()
        .min_w(px(0.0))
        .h_full()
        .relative()
        .overflow_hidden()
        .flex()
        .items_center()
        .on_mouse_down(MouseButton::Left, move |ev, window, _cx| {
            window.focus(&focus_for_down);
            sync_active_ime(&focus_for_down, true, &fallback_for_down);
            let _ = with_focus_state(&focus_for_down, |s| {
                let local_x = f32::from(ev.position.x) - s.text_origin_x;
                let idx = s.byte_index_for_x(local_x);
                s.set_cursor(idx, ev.modifiers.shift);
                s.is_dragging = true;
            });
            window.refresh();
        })
        .on_mouse_move(move |ev, window, _cx| {
            if !ev.dragging() {
                return;
            }
            let updated = with_focus_state(&focus_for_move, |s| {
                if !s.is_dragging {
                    return false;
                }
                let local_x = f32::from(ev.position.x) - s.text_origin_x;
                let idx = s.byte_index_for_x(local_x);
                s.set_cursor(idx, true);
                true
            })
            .unwrap_or(false);
            if updated {
                window.refresh();
            }
        })
        .on_mouse_up(MouseButton::Left, move |_ev, _window, _cx| {
            let _ = with_focus_state(&focus_for_up, |s| {
                s.is_dragging = false;
            });
        })
        .child(
            canvas(
                move |bounds, window, _cx| {
                    let focused_now = focus_for_prepaint.is_focused(window);
                    sync_active_ime(&focus_for_prepaint, focused_now, &current_for_prepaint);
                    if focused_now {
                        let style = window.text_style();
                        let font_size = px(12.5);
                        let run_font = style.font();
                        let _ = with_focus_state(&focus_for_prepaint, |s| {
                            s.text_origin_x = f32::from(bounds.origin.x);
                            s.viewport_width = f32::from(bounds.size.width);
                            let mut offsets = Vec::new();
                            if s.base_text.is_empty() {
                                offsets.push((0, 0.0));
                            } else {
                                let run = TextRun {
                                    len: s.base_text.len(),
                                    font: run_font,
                                    color: Theme::TEXT_PRIMARY(),
                                    background_color: None,
                                    underline: None,
                                    strikethrough: None,
                                };
                                let shaped = window.text_system().shape_line(
                                    SharedString::from(s.base_text.clone()),
                                    font_size,
                                    &[run],
                                    None,
                                );
                                for (idx, _) in s.base_text.char_indices() {
                                    offsets.push((idx, f32::from(shaped.x_for_index(idx))));
                                }
                                offsets.push((
                                    s.base_text.len(),
                                    f32::from(shaped.x_for_index(s.base_text.len())),
                                ));
                            }
                            s.char_offsets = offsets;
                            let caret_x = s.x_for_byte_index(s.cursor);
                            s.ensure_cursor_visible(caret_x, s.viewport_width);
                        });
                    }
                    bounds
                },
                move |bounds, _prepaint_bounds, window, cx| {
                    if focus_for_input.is_focused(window) && !disabled {
                        window.handle_input(
                            &focus_for_input,
                            TextFieldInputHandler {
                                focus: focus_for_input.clone(),
                                bounds,
                                max_chars,
                                on_change: on_change.clone(),
                            },
                            cx,
                        );
                    }
                },
            )
            .size_full()
            .absolute()
            .inset_0(),
        )
        .child(
            div()
                .relative()
                .whitespace_nowrap()
                .flex()
                .items_center()
                .text_size(px(12.5))
                .text_color(if has_text || has_composing {
                    Theme::TEXT_PRIMARY()
                } else {
                    Theme::TEXT_MUTED()
                })
                .when(is_focused && snapshot.scroll_x > 0.0, |d| {
                    d.ml(px(-snapshot.scroll_x))
                })
                .when(
                    is_focused
                        && !has_composing
                        && snapshot.selection.is_some()
                        && snapshot.sel_end_x > snapshot.sel_start_x,
                    |d| {
                        d.child(
                            div()
                                .absolute()
                                .top(px(1.0))
                                .bottom(px(1.0))
                                .left(px(snapshot.sel_start_x))
                                .w(px(snapshot.sel_end_x - snapshot.sel_start_x))
                                .rounded(px(2.0))
                                .bg(Theme::ACCENT_LIGHT()),
                        )
                    },
                )
                .child(if has_composing {
                    let c = snapshot.cursor;
                    let before = snapshot.base_text[..c].to_string();
                    let after = snapshot.base_text[c..].to_string();
                    div()
                        .flex()
                        .items_center()
                        .when(!before.is_empty(), |d| d.child(before))
                        .child(
                            div()
                                .px(px(1.0))
                                .border_b_1()
                                .border_color(Theme::ACCENT_PRIMARY())
                                .text_color(Theme::ACCENT_PRIMARY())
                                .child(snapshot.composing_text),
                        )
                        .when(!after.is_empty(), |d| d.child(after))
                } else if has_text {
                    div().child(snapshot.base_text)
                } else {
                    div().child(placeholder.to_string())
                })
                .when(
                    is_focused && !disabled && !has_composing && snapshot.selection.is_none(),
                    |d| {
                        d.child(
                            div()
                                .absolute()
                                .top(px(2.0))
                                .bottom(px(2.0))
                                .left(px(snapshot.caret_x))
                                .w(px(1.5))
                                .bg(Theme::ACCENT_PRIMARY()),
                        )
                    },
                ),
        )
}

/// 渲染一个可聚焦、可输入的单行文本框
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

    let keyed_text = current.clone();
    let on_change_key = on_change.clone();
    let on_submit_key = on_submit;
    let focus_for_key = focus.clone();
    let on_key = move |ev: &KeyDownEvent, window: &mut Window, app: &mut App| {
        if disabled {
            return;
        }
        handle_key_down(
            &focus_for_key,
            &keyed_text,
            options.max_chars,
            false,
            ev,
            window,
            app,
            &on_change_key,
            &on_submit_key,
        );
    };

    let clear_cb = on_clear;
    let focus_for_clear = focus.clone();
    let on_change_arc: ChangeCallback = Arc::new(on_change);

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
        .child(render_editable_text_area(
            focus,
            &current,
            placeholder,
            is_focused,
            disabled,
            options.max_chars,
            on_change_arc,
        ))
        .when(has_text && !disabled && !options.loading, |d| {
            d.child(
                div()
                    .id((id, 1u64))
                    .size(px(18.0))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|s| s.bg(rgba(0x2e333817)))
                    .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                        app.stop_propagation();
                        let _ = with_focus_state(&focus_for_clear, |s| {
                            s.push_undo();
                            s.replace_range_utf8(0..s.base_text.len(), "", None);
                        });
                        clear_cb(window, app);
                    })
                    .child(icon("icons/close.svg", 10.0, Theme::TEXT_MUTED())),
            )
        })
        .when(options.loading, |d| {
            d.child(
                div()
                    .text_size(px(11.0))
                    .text_color(Theme::TEXT_MUTED())
                    .child("..."),
            )
        })
}

/// 渲染一个可聚焦、可输入、带清除与搜索动作的搜索输入框
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

    let keyed_text = current.clone();
    let submit_for_key = on_submit.clone();
    let on_change_key = on_change.clone();
    let focus_for_key = focus.clone();
    let on_key = move |ev: &KeyDownEvent, window: &mut Window, app: &mut App| {
        if disabled || loading {
            return;
        }
        handle_key_down(
            &focus_for_key,
            &keyed_text,
            Some(64),
            true,
            ev,
            window,
            app,
            &on_change_key,
            &submit_for_key,
        );
    };

    let clear_cb = on_clear;
    let submit_cb = on_submit;
    let submit_text = current.clone();
    let search_icon_color: Hsla = rgb(0x333639).into();
    let focus_for_clear = focus.clone();
    let on_change_arc: ChangeCallback = Arc::new(on_change);

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
        .child(render_editable_text_area(
            focus,
            &current,
            placeholder,
            is_focused,
            disabled || loading,
            Some(64),
            on_change_arc,
        ))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.0))
                .when(has_text && !loading, |d| {
                    d.child(
                        div()
                            .id((id, 1u64))
                            .size(px(18.0))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|s| s.bg(rgba(0x2e333817)))
                            .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                                app.stop_propagation();
                                let _ = with_focus_state(&focus_for_clear, |s| {
                                    s.push_undo();
                                    s.replace_range_utf8(0..s.base_text.len(), "", None);
                                });
                                clear_cb(window, app);
                            })
                            .child(icon("icons/close.svg", 10.0, Theme::TEXT_MUTED())),
                    )
                })
                .child(
                    div()
                        .id((id, 2u64))
                        .size(px(22.0))
                        .rounded(px(7.0))
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
        let mut state = ImeState::new("我爱");

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
        let mut state = ImeState::new("英雄");
        let committed = state.apply_committed_text("联盟\r\n提莫", Some(4));
        assert_eq!(committed, "英雄联盟");
    }

    #[::core::prelude::v1::test]
    fn ime_state_cancel_composing_preserves_base_text() {
        let mut state = ImeState::new("Teemo");
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

    #[::core::prelude::v1::test]
    fn ime_state_set_composing_clamps_out_of_bounds_cursor_position() {
        let mut state = ImeState::new("亚索");

        // 模拟 Windows IME 错误码 (-1 转为 usize::MAX) 传入时的边界保护
        state.set_composing("liulang", Some(usize::MAX..usize::MAX));
        assert_eq!(state.composing_cursor_utf16, 7);
        assert_eq!(state.selected_utf16_range(), 9..9);

        // 退格到空串
        state.set_composing("", Some(0..0));
        assert_eq!(state.composing_cursor_utf16, 0);
        assert_eq!(state.marked_utf16_range(), None);
        assert_eq!(state.selected_utf16_range(), 2..2);
    }

    #[::core::prelude::v1::test]
    fn edit_state_supports_middle_insert_delete_selection_and_undo() {
        let mut state = ImeState::new("名字#1234");
        // 光标左移 4 次到 '#' 后
        for _ in 0..4 {
            state.move_left(false);
        }
        assert_eq!(state.cursor, "名字#".len());

        // 按 Shift+Right 选中 "1234"，再直接输入替换
        for _ in 0..4 {
            state.move_right(true);
        }
        assert_eq!(state.selected_text(), Some("1234"));
        let next = state.apply_committed_text("5678", None);
        assert_eq!(next, "名字#5678");

        // 移动到中间测试 Delete 删除右侧字符而非末尾
        state.move_home(false);
        state.move_right(false);
        state.move_right(false);
        let after_del = state.delete().unwrap();
        assert_eq!(after_del, "名字5678");

        // 撤销恢复
        let undone = state.undo().unwrap();
        assert_eq!(undone, "名字#5678");

        // Ctrl+A 全选后直接输入
        state.select_all();
        let replaced = state.apply_committed_text("提莫#0001", None);
        assert_eq!(replaced, "提莫#0001");
    }

    #[::core::prelude::v1::test]
    fn edit_state_handles_emoji_utf16_and_replacement_range() {
        let mut state = ImeState::new("Hi😀12");
        // "Hi" = 2 u16, "😀" = 2 u16 (surrogate pair), "12" = 2 u16
        let replaced = state.apply_committed_text_in_range(Some(2..4), "🎉", None);
        assert_eq!(replaced, "Hi🎉12");
        assert_eq!(state.selected_utf16_range(), 4..4);
    }
}
