//! 工具箱专用 UI 控件库
//!
//! 包含直显卡片行、英雄选择器、技能选择器、带后缀按钮输入框、危险按钮、装配弹窗等。

use crate::components::icon::icon;
use crate::components::text_field::{text_field, TextFieldOptions};
use crate::theme::Theme;
use super::data::{
    get_champion_name, get_spell_name, ChampionEntry, CHAMPIONS, SUMMONER_SPELLS,
};
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::cell::OnceCell;
use std::rc::Rc;

/// 直显行在卡片组中的贴合位置
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CardGroupPosition {
    /// 独立单张卡片（四角圆角 8px，独立外边距）
    Single,
    /// 组内首行（顶部圆角 8px，无下边距，贴合下方）
    Top,
    /// 组内中间行（无圆角，上下贴合）
    Middle,
    /// 组内末行（底部圆角 8px，下外边距 8px，贴合上方）
    Bottom,
}

/// 渲染 1:1 对齐 Vue `.card-item` 的直显行卡片
pub fn direct_card_row(
    id: ElementId,
    pos: CardGroupPosition,
    icon_name: &'static str,
    title: impl Into<SharedString>,
    desc: impl Into<SharedString>,
    control: impl IntoElement,
) -> impl IntoElement {
    let title = title.into();
    let desc = desc.into();

    let (rounded_t, rounded_b, border_b_zero, mb) = match pos {
        CardGroupPosition::Single => (true, true, false, px(8.0)),
        CardGroupPosition::Top => (true, false, false, px(0.0)),
        CardGroupPosition::Middle => (false, false, false, px(0.0)),
        CardGroupPosition::Bottom => (false, true, true, px(8.0)),
    };

    div()
        .id(id)
        .w_full()
        .bg(Theme::CARD_BG)
        .border_1()
        .border_color(Theme::CARD_BORDER)
        .when(border_b_zero, |s| s.border_t_0())
        .when(rounded_t, |s| s.rounded_t(px(8.0)))
        .when(rounded_b, |s| s.rounded_b(px(8.0)))
        .mb(mb)
        .px(px(16.0))
        .py(px(12.0))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(14.0))
        .hover(|s| s.bg(Theme::HOVER_BG))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(14.0))
                .flex_1()
                .child(
                    div()
                        .size(px(32.0))
                        .rounded(px(6.0))
                        .bg(Theme::HOVER_BG)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon(icon_name, 16.0, Theme::TEXT_MUTED)),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .child(
                            div()
                                .text_size(px(13.5))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(Theme::TEXT_PRIMARY)
                                .child(title),
                        )
                        .child(
                            div()
                                .text_size(px(11.5))
                                .text_color(Theme::TEXT_MUTED)
                                .child(desc),
                        ),
                ),
        )
        .child(control)
}

/// 危险操作按钮（text-danger 红色边框/字体）
pub fn danger_button(
    label: impl Into<SharedString>,
    on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Div {
    div()
        .px(px(14.0))
        .py(px(5.0))
        .rounded_md()
        .border_1()
        .border_color(Theme::LOSS_BORDER)
        .bg(Theme::CARD_BG)
        .text_size(px(12.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(Theme::LOSS_TEXT)
        .cursor_pointer()
        .hover(|s| s.bg(Theme::LOSS_BG).border_color(Theme::LOSS_TEXT))
        .on_mouse_down(MouseButton::Left, on_click)
        .child(label.into())
}

/// 统一下拉选择框组件（支持固定宽度和禁用态）
pub fn select_dropdown<F>(
    id: &'static str,
    width_px: f32,
    current_label: String,
    open: bool,
    disabled: bool,
    options: Vec<(&'static str, &'static str)>,
    on_toggle: F,
    on_select: impl Fn(&'static str, &mut Window, &mut App) + 'static + Clone,
) -> impl IntoElement
where
    F: Fn(&mut Window, &mut App) + 'static + Clone,
{
    let toggle_cb = on_toggle.clone();

    div()
        .id(id)
        .relative()
        .w(px(width_px))
        .when(disabled, |s| s.opacity(0.5).cursor(CursorStyle::OperationNotAllowed))
        .child(
            div()
                .w_full()
                .h(px(28.0))
                .px(px(10.0))
                .rounded_md()
                .border_1()
                .border_color(if open && !disabled {
                    Theme::SIDEBAR_ACTIVE_BORDER
                } else {
                    Theme::CARD_BORDER
                })
                .bg(if open && !disabled {
                    Theme::ACCENT_LIGHT
                } else {
                    Theme::CARD_BG
                })
                .flex()
                .items_center()
                .justify_between()
                .text_size(px(12.0))
                .text_color(Theme::TEXT_SECONDARY)
                .cursor(if disabled {
                    CursorStyle::OperationNotAllowed
                } else {
                    CursorStyle::PointingHand
                })
                .when(!disabled, |this| {
                    this.hover(|s| s.border_color(Theme::ACCENT_PRIMARY))
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| toggle_cb(win, app))
                })
                .child(
                    div()
                        .flex_1()
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(current_label),
                )
                .child(icon(
                    if open && !disabled {
                        "icons/chevron-up.svg"
                    } else {
                        "icons/chevron-down.svg"
                    },
                    12.0,
                    Theme::TEXT_MUTED,
                )),
        )
        .when(open && !disabled, |this| {
            this.child(
                deferred(
                    div()
                        .absolute()
                        .top_full()
                        .left_0()
                        .mt(px(4.0))
                        .w_full()
                        .id(SharedString::from(format!("{}-scroll", id)))
                        .max_h(px(200.0))
                        .overflow_y_scroll()
                        .occlude()
                        .bg(rgb(0xffffff))
                        .border_1()
                        .border_color(Theme::CARD_BORDER)
                        .rounded_lg()
                        .shadow_md()
                        .p(px(4.0))
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .children(options.into_iter().map(|(val, label)| {
                            let select_cb = on_select.clone();
                            div()
                                .w_full()
                                .px(px(10.0))
                                .py(px(5.0))
                                .rounded_md()
                                .text_size(px(12.0))
                                .text_color(Theme::TEXT_PRIMARY)
                                .cursor_pointer()
                                .hover(|s| s.bg(Theme::HOVER_BG))
                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                    select_cb(val, win, app)
                                })
                                .child(label)
                        })),
                )
                .with_priority(5),
            )
        })
}

/// 内嵌后缀按钮输入框容器（对齐 Vue `n-input #suffix`）
pub fn suffix_input<FBtn>(
    id: ElementId,
    text: &str,
    placeholder: &'static str,
    button_label: &'static str,
    button_disabled: bool,
    width_px: f32,
    on_btn_click: FBtn,
) -> impl IntoElement
where
    FBtn: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
{
    div()
        .id(id)
        .w(px(width_px))
        .h(px(32.0))
        .rounded_md()
        .border_1()
        .border_color(Theme::CARD_BORDER)
        .bg(Theme::CARD_BG)
        .px(px(8.0))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(8.0))
        .child(
            div()
                .flex_1()
                .text_size(px(12.5))
                .text_color(if text.is_empty() {
                    Theme::TEXT_MUTED
                } else {
                    Theme::TEXT_PRIMARY
                })
                .child(if text.is_empty() {
                    placeholder.to_string()
                } else {
                    text.to_string()
                }),
        )
        .child(
            div()
                .px(px(10.0))
                .py(px(3.0))
                .rounded(px(4.0))
                .bg(if button_disabled {
                    Theme::HOVER_BG
                } else {
                    Theme::ACCENT_PRIMARY
                })
                .text_size(px(11.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(if button_disabled {
                    Theme::TEXT_MUTED
                } else {
                    gpui::white()
                })
                .cursor(if button_disabled {
                    CursorStyle::OperationNotAllowed
                } else {
                    CursorStyle::PointingHand
                })
                .when(!button_disabled, |this| {
                    this.hover(|s| s.opacity(0.85))
                        .on_mouse_down(MouseButton::Left, on_btn_click)
                })
                .child(button_label),
        )
}

/// [D06-b] 英雄候选上限（Vue ChampionPicker 语义：点选加入、达上限忽略、点已选移除）
pub const PICKER_MAX_COUNT: usize = 5;

/// 英雄选择器（兼容签名：bg_champ 等既有调用点；✕ / 收起 / 完成统一走 on_close）
pub fn champion_picker<FClear, FOpen, FClose, FSelect, FRemove>(
    id: ElementId,
    selected_ids: &[i32],
    is_open: bool,
    search_query: &str,
    on_clear: FClear,
    on_open: FOpen,
    on_close: FClose,
    on_select: FSelect,
    on_remove: FRemove,
) -> impl IntoElement
where
    FClear: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static + Clone,
    FOpen: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static + Clone,
    FClose: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static + Clone,
    FSelect: Fn(i32, &mut Window, &mut App) + 'static + Clone,
    FRemove: Fn(i32, &mut Window, &mut App) + 'static + Clone,
{
    let confirm = on_close.clone();
    let cancel = on_close;
    champion_picker_inner(
        id,
        selected_ids,
        is_open,
        search_query,
        None,
        |_q: String, _win: &mut Window, _app: &mut App| {},
        on_clear,
        on_open,
        confirm,
        cancel,
        on_select,
        on_remove,
    )
}

/// [D06-a/c/d] 英雄选择器（带 IME 搜索输入、确认与取消分离），供选用/禁用卡片使用。
#[allow(clippy::too_many_arguments)]
pub fn champion_picker_searchable<FClear, FOpen, FConfirm, FCancel, FSelect, FRemove, FSearch>(
    id: ElementId,
    selected_ids: &[i32],
    is_open: bool,
    search_query: &str,
    search_focus: Rc<OnceCell<FocusHandle>>,
    on_search: FSearch,
    on_clear: FClear,
    on_open: FOpen,
    on_confirm: FConfirm,
    on_cancel: FCancel,
    on_select: FSelect,
    on_remove: FRemove,
) -> impl IntoElement
where
    FClear: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static + Clone,
    FOpen: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static + Clone,
    FConfirm: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static + Clone,
    FCancel: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static + Clone,
    FSelect: Fn(i32, &mut Window, &mut App) + 'static + Clone,
    FRemove: Fn(i32, &mut Window, &mut App) + 'static + Clone,
    FSearch: Fn(String, &mut Window, &mut App) + 'static + Clone,
{
    champion_picker_inner(
        id,
        selected_ids,
        is_open,
        search_query,
        Some(search_focus),
        on_search,
        on_clear,
        on_open,
        on_confirm,
        on_cancel,
        on_select,
        on_remove,
    )
}

/// 英雄选择器内部渲染：search_focus 为 Some 时展开真实文本输入框（复用 text_field，支持 IME）。
#[allow(clippy::too_many_arguments)]
fn champion_picker_inner<FClear, FOpen, FConfirm, FCancel, FSelect, FRemove, FSearch>(
    id: ElementId,
    selected_ids: &[i32],
    is_open: bool,
    search_query: &str,
    search_focus: Option<Rc<OnceCell<FocusHandle>>>,
    on_search: FSearch,
    on_clear: FClear,
    on_open: FOpen,
    on_confirm: FConfirm,
    on_cancel: FCancel,
    on_select: FSelect,
    on_remove: FRemove,
) -> impl IntoElement
where
    FClear: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static + Clone,
    FOpen: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static + Clone,
    FConfirm: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static + Clone,
    FCancel: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static + Clone,
    FSelect: Fn(i32, &mut Window, &mut App) + 'static + Clone,
    FRemove: Fn(i32, &mut Window, &mut App) + 'static + Clone,
    FSearch: Fn(String, &mut Window, &mut App) + 'static + Clone,
{
    let has_selected = !selected_ids.is_empty();
    let q = search_query.trim().to_lowercase();

    let filtered_champions: Vec<&ChampionEntry> = CHAMPIONS
        .iter()
        .filter(|c| {
            if q.is_empty() {
                true
            } else {
                c.name.contains(&q)
                    || c.pinyin.contains(&q)
                    || c.title.contains(&q)
                    || c.id.to_string() == q
            }
        })
        .collect();

    let open_click = on_open.clone();
    let cancel_click = on_cancel.clone();

    div()
        .id(id)
        .w_full()
        .flex()
        .flex_col()
        .gap(px(8.0))
        // 1. 触发栏（已选 Chips + 右侧操作）
        .child(
            div()
                .w_full()
                .p(px(8.0))
                .rounded_md()
                .border_1()
                .border_color(if is_open {
                    Theme::SIDEBAR_ACTIVE_BORDER
                } else {
                    Theme::CARD_BORDER
                })
                .bg(Theme::CARD_BG)
                .flex()
                .items_center()
                .justify_between()
                .gap(px(10.0))
                // 左侧 Chips
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .flex_wrap()
                        .flex_1()
                        .when(!has_selected, |this| {
                            this.child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(Theme::TEXT_MUTED)
                                    .child("未配置英雄 (点击右侧选择)"),
                            )
                        })
                        .children(selected_ids.iter().map(|&cid| {
                            let remove_cb = on_remove.clone();
                            div()
                                .h(px(26.0))
                                .px(px(8.0))
                                .rounded_full()
                                .bg(Theme::HOVER_BG)
                                .border_1()
                                .border_color(Theme::CARD_BORDER)
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .size(px(16.0))
                                        .rounded_full()
                                        .bg(Theme::ACCENT_LIGHT)
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_size(px(9.0))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(Theme::ACCENT_PRIMARY)
                                        .child("★"),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(Theme::TEXT_PRIMARY)
                                        .child(get_champion_name(cid)),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(Theme::TEXT_MUTED)
                                        .cursor_pointer()
                                        .hover(|s| s.text_color(Theme::LOSS_TEXT))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            move |_ev, win, app| remove_cb(cid, win, app),
                                        )
                                        .child("✕"),
                                )
                        })),
                )
                // 右侧操作
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .when(has_selected, |this| {
                            let clear_cb = on_clear.clone();
                            this.child(
                                div()
                                    .px(px(10.0))
                                    .py(px(4.0))
                                    .rounded_md()
                                    .border_1()
                                    .border_color(Theme::CARD_BORDER)
                                    .bg(Theme::CARD_BG)
                                    .text_size(px(11.5))
                                    .text_color(Theme::TEXT_SECONDARY)
                                    .cursor_pointer()
                                    .hover(|s| s.bg(Theme::HOVER_BG))
                                    .on_mouse_down(MouseButton::Left, clear_cb)
                                    .child("清空"),
                            )
                        })
                        .child(
                            div()
                                .px(px(12.0))
                                .py(px(4.0))
                                .rounded_md()
                                .bg(if is_open {
                                    Theme::SIDEBAR_ACTIVE
                                } else {
                                    Theme::ACCENT_PRIMARY
                                })
                                .text_size(px(11.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(if is_open {
                                    Theme::ACCENT_PRIMARY
                                } else {
                                    gpui::white()
                                })
                                .border_1()
                                .border_color(if is_open {
                                    Theme::ACCENT_PRIMARY
                                } else {
                                    Theme::ACCENT_PRIMARY
                                })
                                .cursor_pointer()
                                .hover(|s| s.opacity(0.85))
                                .on_mouse_down(MouseButton::Left, move |ev, win, app| {
                                    if is_open {
                                        cancel_click(ev, win, app);
                                    } else {
                                        open_click(ev, win, app);
                                    }
                                })
                                .child(if is_open { "收起选择" } else { "选择英雄" }),
                        ),
                ),
        )
        // 2. 展开态选择面板
        .when(is_open, |this| {
            let confirm_cb = on_confirm.clone();
            let cancel_cb = on_cancel.clone();
            this.child(
                div()
                    .w_full()
                    .p(px(12.0))
                    .rounded_md()
                    .border_1()
                    .border_color(Theme::SIDEBAR_ACTIVE_BORDER)
                    .bg(Theme::CARD_BG)
                    .shadow_sm()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    // 顶部标题与关闭（✕ 为取消，回滚打开前配置）
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child(format!("点选常用英雄 (最多 {} 位)", PICKER_MAX_COUNT)),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(Theme::TEXT_MUTED)
                                    .cursor_pointer()
                                    .hover(|s| s.text_color(Theme::TEXT_PRIMARY))
                                    .on_mouse_down(MouseButton::Left, cancel_cb.clone())
                                    .child("✕"),
                            ),
                    )
                    // [D06-a] 真实文本输入（IME / 粘贴 / 退格），写入视图层的搜索词
                    .when_some(search_focus, |this, focus| {
                        let on_change = on_search.clone();
                        this.child(picker_search_field(
                            "champ-picker-search",
                            search_query,
                            focus,
                            on_change,
                        ))
                    })
                    // 英雄网格
                    .child(
                        div()
                            .w_full()
                            .id("champ-picker-grid-scroll")
                            .max_h(px(180.0))
                            .overflow_y_scroll()
                            .flex()
                            .flex_wrap()
                            .gap(px(6.0))
                            .children(filtered_champions.into_iter().map(|champ| {
                                let cid = champ.id;
                                let is_selected = selected_ids.contains(&cid);
                                let select_cb = on_select.clone();

                                div()
                                    .px(px(8.0))
                                    .py(px(5.0))
                                    .rounded_md()
                                    .border_1()
                                    .border_color(if is_selected {
                                        Theme::ACCENT_PRIMARY
                                    } else {
                                        Theme::CARD_BORDER
                                    })
                                    .bg(if is_selected {
                                        Theme::ACCENT_LIGHT
                                    } else {
                                        Theme::HOVER_BG
                                    })
                                    .flex()
                                    .items_center()
                                    .gap(px(6.0))
                                    .cursor_pointer()
                                    .hover(|s| s.border_color(Theme::ACCENT_PRIMARY))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        move |_ev, win, app| select_cb(cid, win, app),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_weight(if is_selected {
                                                FontWeight::BOLD
                                            } else {
                                                FontWeight::NORMAL
                                            })
                                            .text_color(if is_selected {
                                                Theme::ACCENT_PRIMARY
                                            } else {
                                                Theme::TEXT_PRIMARY
                                            })
                                            .child(champ.name),
                                    )
                                    .when(is_selected, |s| {
                                        s.child(
                                            div()
                                                .text_size(px(10.0))
                                                .text_color(Theme::ACCENT_PRIMARY)
                                                .child("✓"),
                                        )
                                    })
                            })),
                    )
                    // 底部确认（[D06-c] 完成才落盘）
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .pt(px(4.0))
                            .border_t_1()
                            .border_color(Theme::CARD_BORDER)
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(Theme::TEXT_MUTED)
                                    .child(format!(
                                        "已配置 {} / 上限 {} 位英雄",
                                        selected_ids.len(),
                                        PICKER_MAX_COUNT
                                    )),
                            )
                            .child(
                                div()
                                    .px(px(14.0))
                                    .py(px(4.0))
                                    .rounded_md()
                                    .bg(Theme::ACCENT_PRIMARY)
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(gpui::white())
                                    .cursor_pointer()
                                    .hover(|s| s.opacity(0.85))
                                    .on_mouse_down(MouseButton::Left, confirm_cb)
                                    .child("完成"),
                            ),
                    ),
            )
        })
}

/// [D06-a] 选择器搜索输入框：复用 `text_field`（支持 IME 组合、退格、粘贴），
/// 首次点击时惰性创建 FocusHandle（FocusHandle 只能由 App 创建）。
fn picker_search_field(
    id: &'static str,
    text: &str,
    focus_cell: Rc<OnceCell<FocusHandle>>,
    on_change: impl Fn(String, &mut Window, &mut App) + 'static + Clone,
) -> AnyElement {
    if let Some(handle) = focus_cell.get().cloned() {
        let clear_cb = on_change.clone();
        return div()
            .w(px(240.0))
            .child(text_field(
                id,
                text,
                "搜索英雄名称 / 拼音 / ID…",
                px(240.0),
                &handle,
                false,
                TextFieldOptions {
                    max_chars: Some(40),
                    ..Default::default()
                },
                on_change,
                move |win, app| clear_cb(String::new(), win, app),
                |_text, _win, _app| {},
            ))
            .into_any_element();
    }

    let cell = focus_cell.clone();
    div()
        .id(id)
        .w(px(240.0))
        .h(px(30.0))
        .rounded_lg()
        .bg(Theme::CARD_BG)
        .border_1()
        .border_color(Theme::CARD_BORDER)
        .px(px(10.0))
        .flex()
        .items_center()
        .cursor_text()
        .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
            let handle = cell.get_or_init(|| app.focus_handle()).clone();
            window.focus(&handle);
        })
        .child(
            div()
                .text_size(px(12.5))
                .text_color(if text.is_empty() {
                    Theme::TEXT_MUTED
                } else {
                    Theme::TEXT_PRIMARY
                })
                .child(if text.is_empty() {
                    "搜索英雄名称 / 拼音 / ID…".to_string()
                } else {
                    text.to_string()
                }),
        )
        .into_any_element()
}

/// 召唤师技能选择器组件（D / F 两槽位卡片 + 展开网格面板）
pub fn spell_picker<FSlot, FSelect, FRemove, FClose>(
    d_spell_id: Option<i32>,
    f_spell_id: Option<i32>,
    active_slot: Option<&'static str>,
    on_slot_click: FSlot,
    on_select: FSelect,
    on_remove: FRemove,
    on_close: FClose,
) -> Div
where
    FSlot: Fn(&'static str, &mut Window, &mut App) + 'static + Clone,
    FSelect: Fn(i32, &mut Window, &mut App) + 'static + Clone,
    FRemove: Fn(&'static str, &mut Window, &mut App) + 'static + Clone,
    FClose: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static + Clone,
{
    let is_picking = active_slot.is_some();

    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(10.0))
        // 槽位卡片容器
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(16.0))
                // D 键槽位
                .child(render_spell_slot(
                    "D 键",
                    "D",
                    d_spell_id,
                    active_slot == Some("D"),
                    {
                        let cb = on_slot_click.clone();
                        move |win, app| cb("D", win, app)
                    },
                    {
                        let cb = on_remove.clone();
                        move |win, app| cb("D", win, app)
                    },
                ))
                // F 键槽位
                .child(render_spell_slot(
                    "F 键",
                    "F",
                    f_spell_id,
                    active_slot == Some("F"),
                    {
                        let cb = on_slot_click.clone();
                        move |win, app| cb("F", win, app)
                    },
                    {
                        let cb = on_remove.clone();
                        move |win, app| cb("F", win, app)
                    },
                )),
        )
        // 展开的技能选择网格面板
        .when(is_picking, |this| {
            let slot_label = match active_slot {
                Some("D") => "D 键",
                Some("F") => "F 键",
                _ => "",
            };
            this.child(
                div()
                    .w_full()
                    .p(px(12.0))
                    .rounded_md()
                    .border_1()
                    .border_color(Theme::SIDEBAR_ACTIVE_BORDER)
                    .bg(Theme::CARD_BG)
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child(format!("选择 {} 召唤师技能", slot_label)),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(Theme::TEXT_MUTED)
                                    .cursor_pointer()
                                    .hover(|s| s.text_color(Theme::TEXT_PRIMARY))
                                    .on_mouse_down(MouseButton::Left, on_close)
                                    .child("✕"),
                            ),
                    )
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .flex_wrap()
                            .gap(px(8.0))
                            .children(SUMMONER_SPELLS.iter().map(|(sid, sname)| {
                                let id_val = *sid;
                                let is_cur = match active_slot {
                                    Some("D") => d_spell_id == Some(id_val),
                                    Some("F") => f_spell_id == Some(id_val),
                                    _ => false,
                                };
                                let select_cb = on_select.clone();

                                div()
                                    .px(px(12.0))
                                    .py(px(6.0))
                                    .rounded_md()
                                    .border_1()
                                    .border_color(if is_cur {
                                        Theme::ACCENT_PRIMARY
                                    } else {
                                        Theme::CARD_BORDER
                                    })
                                    .bg(if is_cur {
                                        Theme::ACCENT_LIGHT
                                    } else {
                                        Theme::HOVER_BG
                                    })
                                    .cursor_pointer()
                                    .hover(|s| s.border_color(Theme::ACCENT_PRIMARY))
                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                        select_cb(id_val, win, app)
                                    })
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_weight(if is_cur {
                                                FontWeight::BOLD
                                            } else {
                                                FontWeight::NORMAL
                                            })
                                            .text_color(if is_cur {
                                                Theme::ACCENT_PRIMARY
                                            } else {
                                                Theme::TEXT_PRIMARY
                                            })
                                            .child(*sname),
                                    )
                            })),
                    ),
            )
        })
}

/// 辅助渲染单个技能槽位卡片
fn render_spell_slot<FClick, FRemove>(
    key_title: &'static str,
    key_badge: &'static str,
    spell_id: Option<i32>,
    is_active: bool,
    on_click: FClick,
    on_remove: FRemove,
) -> Div
where
    FClick: Fn(&mut Window, &mut App) + 'static + Clone,
    FRemove: Fn(&mut Window, &mut App) + 'static + Clone,
{
    let has_spell = spell_id.is_some();
    let click_cb = on_click.clone();

    div()
        .w(px(170.0))
        .h(px(52.0))
        .p(px(8.0))
        .rounded_md()
        .border_1()
        .border_color(if is_active {
            Theme::SIDEBAR_ACTIVE_BORDER
        } else {
            Theme::CARD_BORDER
        })
        .bg(if is_active {
            Theme::ACCENT_LIGHT
        } else {
            Theme::CARD_BG
        })
        .flex()
        .items_center()
        .justify_between()
        .cursor_pointer()
        .hover(|s| s.border_color(Theme::ACCENT_PRIMARY))
        .on_mouse_down(MouseButton::Left, move |_ev, win, app| click_cb(win, app))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .size(px(24.0))
                        .rounded(px(4.0))
                        .bg(Theme::ACCENT_PRIMARY)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(gpui::white())
                        .child(key_badge),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(1.0))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(Theme::TEXT_MUTED)
                                .child(key_title),
                        )
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(if has_spell {
                                    Theme::TEXT_PRIMARY
                                } else {
                                    Theme::TEXT_MUTED
                                })
                                .child(match spell_id {
                                    Some(id) => get_spell_name(id).to_string(),
                                    None => "+ 选择技能".to_string(),
                                }),
                        ),
                ),
        )
        .when(has_spell, |this| {
            let remove_cb = on_remove.clone();
            this.child(
                div()
                    .text_size(px(11.0))
                    .text_color(Theme::TEXT_MUTED)
                    .cursor_pointer()
                    .hover(|s| s.text_color(Theme::LOSS_TEXT))
                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| remove_cb(win, app))
                    .child("✕"),
            )
        })
}

/// 称号与挑战勋章自定义装配弹窗（对齐 Vue `BadgesPreferencesCard.vue`）
///
/// D11：候选列表换成 `services::challenges` 读取的真实拥有称号/勋章，支持称号搜索、
/// 勋章搜索、段位筛选、槽位移除、加载/错误/重试。选中态由 `render_panel` 传入的
/// `selected_badges` / `selected_title` 决定；弹层会话状态由
/// `personalization::badges_view_state()` 投影提供（调用点按工单边界不可修改）。
/// 所有交互复用既有 4 个回调，`on_toggle_badge(String)` 以「前缀:值」承载弹层指令。
pub fn badges_modal<FClose, FToggleBadge, FClear, FSave>(
    selected_badges: &[String],
    selected_title: &str,
    on_close: FClose,
    on_toggle_badge: FToggleBadge,
    on_clear: FClear,
    on_save: FSave,
) -> impl IntoElement
where
    FClose: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static + Clone,
    FToggleBadge: Fn(String, &mut Window, &mut App) + 'static + Clone,
    FClear: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static + Clone,
    FSave: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static + Clone,
{
    use crate::components::text_field::{text_field, TextFieldOptions};
    use crate::views::tools::personalization::badges_view_state;
    use std::cell::OnceCell;
    use std::rc::Rc;

    // gpui 0.2 的 FocusHandle 只能由 App 创建；渲染线程本地惰性持有两个搜索框焦点
    thread_local! {
        static BADGE_SEARCH_FOCUS: Rc<OnceCell<FocusHandle>> = Rc::new(OnceCell::new());
        static TITLE_SEARCH_FOCUS: Rc<OnceCell<FocusHandle>> = Rc::new(OnceCell::new());
    }

    fn matches_tier(level: &str, filter: &str) -> bool {
        let lvl = level.to_uppercase();
        match filter {
            "apex" => matches!(lvl.as_str(), "CHALLENGER" | "GRANDMASTER"),
            "high" => matches!(lvl.as_str(), "MASTER" | "DIAMOND"),
            "mid" => matches!(lvl.as_str(), "PLATINUM" | "EMERALD" | "GOLD"),
            "low" => matches!(lvl.as_str(), "SILVER" | "BRONZE" | "IRON"),
            _ => true,
        }
    }

    fn level_label(level: &str) -> &'static str {
        match level.to_uppercase().as_str() {
            "CHALLENGER" => "王者",
            "GRANDMASTER" => "宗师",
            "MASTER" => "大师",
            "DIAMOND" => "钻石",
            "EMERALD" => "翡翠",
            "PLATINUM" => "铂金",
            "GOLD" => "黄金",
            "SILVER" => "白银",
            "BRONZE" => "黄铜",
            "IRON" => "黑铁",
            _ => "未定级",
        }
    }

    let state = badges_view_state();
    let loading = state.loading;
    let error = state.error.clone();
    let tier_filter = state.tier_filter.clone();
    let badge_query = state.badge_search.trim().to_lowercase();
    let title_query = state.title_search.trim().to_lowercase();

    let filtered_badges: Vec<_> = state
        .owned_badges
        .iter()
        .filter(|b| {
            let text_ok = badge_query.is_empty()
                || b.name.to_lowercase().contains(&badge_query)
                || b.description.to_lowercase().contains(&badge_query);
            text_ok && matches_tier(&b.level, &tier_filter)
        })
        .collect();
    let filtered_titles: Vec<_> = state
        .owned_titles
        .iter()
        .filter(|t| title_query.is_empty() || t.name.to_lowercase().contains(&title_query))
        .collect();

    let tier_filters: Vec<(&'static str, &'static str)> = vec![
        ("all", "全部段位"),
        ("apex", "王者/宗师"),
        ("high", "大师/钻石"),
        ("mid", "铂金/黄金"),
        ("low", "白银及以下"),
    ];

    // 惰性创建焦点的搜索框闭包（对齐 spectate.rs 的 lazy_text_field 模式）
    let on_cmd = on_toggle_badge.clone();
    let render_search = move |id: &'static str,
                              text: String,
                              placeholder: &'static str,
                              cmd: &'static str,
                              focus_cell: Rc<OnceCell<FocusHandle>>|
     -> AnyElement {
        let send_change = on_cmd.clone();
        let send_clear = on_cmd.clone();
        if let Some(handle) = focus_cell.get().cloned() {
            return div()
                .w(px(190.0))
                .child(text_field(
                    id,
                    &text,
                    placeholder,
                    px(190.0),
                    &handle,
                    false,
                    TextFieldOptions {
                        max_chars: Some(64),
                        ..Default::default()
                    },
                    move |next, win, app| send_change(format!("{}:{}", cmd, next), win, app),
                    move |win, app| send_clear(format!("{}:", cmd), win, app),
                    move |_s, _win, _app| {},
                ))
                .into_any_element();
        }
        let cell = focus_cell.clone();
        div()
            .id(id)
            .w(px(190.0))
            .h(px(30.0))
            .rounded_md()
            .bg(Theme::CARD_BG)
            .border_1()
            .border_color(Theme::CARD_BORDER)
            .px(px(10.0))
            .flex()
            .items_center()
            .text_size(px(12.0))
            .text_color(Theme::TEXT_MUTED)
            .cursor_text()
            .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                let handle = cell.get_or_init(|| app.focus_handle()).clone();
                window.focus(&handle);
            })
            .child(if text.is_empty() {
                placeholder.to_string()
            } else {
                text
            })
            .into_any_element()
    };

    let badge_focus = BADGE_SEARCH_FOCUS.with(|c| c.clone());
    let title_focus = TITLE_SEARCH_FOCUS.with(|c| c.clone());
    let title_search_box = render_search(
        "badges-title-search",
        state.title_search.clone(),
        "搜索拥有的称号…",
        "titlesearch",
        title_focus,
    );
    let badge_search_box = render_search(
        "badges-badge-search",
        state.badge_search.clone(),
        "搜索勋章名称或说明…",
        "search",
        badge_focus,
    );

    let close_cb = on_close.clone();
    let cancel_cb = on_close.clone();
    let retry_cb = on_toggle_badge.clone();
    let filter_cb = on_toggle_badge.clone();
    let title_cb = on_toggle_badge.clone();
    let remove_cb = on_toggle_badge.clone();
    let toggle_cb = on_toggle_badge.clone();

    deferred(
        div()
            .absolute()
            .inset_0()
            .occlude()
            .bg(rgba(0x00000088))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(640.0))
                    .max_h(px(600.0))
                    .bg(rgb(0xffffff))
                    .rounded_xl()
                    .shadow_lg()
                    .border_1()
                    .border_color(Theme::CARD_BORDER)
                    .p(px(20.0))
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    // 弹窗标题 + 关闭（取消，零写入）
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .border_b_1()
                            .border_color(Theme::CARD_BORDER)
                            .pb(px(12.0))
                            .child(
                                div()
                                    .text_size(px(16.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child("装配称号与挑战勋章"),
                            )
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .text_color(Theme::TEXT_MUTED)
                                    .cursor_pointer()
                                    .hover(|s| s.text_color(Theme::TEXT_PRIMARY))
                                    .on_mouse_down(MouseButton::Left, close_cb)
                                    .child("✕"),
                            ),
                    )
                    // 加载态 / 错误态（含重试入口）
                    .when(loading, |this| {
                        this.child(
                            div()
                                .p(px(24.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(12.5))
                                .text_color(Theme::TEXT_MUTED)
                                .child("正在读取您的挑战与称号数据…"),
                        )
                    })
                    .when_some(error, |this, err| {
                        let act = retry_cb.clone();
                        this.child(
                            div()
                                .p(px(10.0))
                                .rounded_md()
                                .bg(Theme::LOSS_BG)
                                .border_1()
                                .border_color(Theme::LOSS_BORDER)
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap(px(10.0))
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(Theme::LOSS_TEXT)
                                        .child(err.clone()),
                                )
                                .child(
                                    div()
                                        .px(px(12.0))
                                        .py(px(4.0))
                                        .rounded_md()
                                        .border_1()
                                        .border_color(Theme::LOSS_BORDER)
                                        .text_size(px(12.0))
                                        .text_color(Theme::LOSS_TEXT)
                                        .cursor_pointer()
                                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                            act("retry".to_string(), win, app)
                                        })
                                        .child("重新加载"),
                                ),
                        )
                    })
                    // 称号区（真实拥有列表 + 搜索）
                    .when(!loading, |this| {
                        this.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .text_size(px(12.5))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(Theme::TEXT_PRIMARY)
                                        .child(format!("佩戴称号（当前: {}）",
                                            if selected_title.is_empty() { "无称号" } else { selected_title })),
                                )
                                .child(title_search_box)
                                .child(
                                    div()
                                        .w_full()
                                        .id("badges-title-scroll")
                                        .max_h(px(72.0))
                                        .overflow_y_scroll()
                                        .flex()
                                        .flex_wrap()
                                        .gap(px(6.0))
                                        // 「（无称号）」
                                        .child({
                                            let act = title_cb.clone();
                                            let active = selected_title.is_empty()
                                                || selected_title == "0"
                                                || selected_title == "-1";
                                            div()
                                                .px(px(10.0))
                                                .py(px(4.0))
                                                .rounded_md()
                                                .border_1()
                                                .border_color(if active {
                                                    Theme::ACCENT_PRIMARY
                                                } else {
                                                    Theme::CARD_BORDER
                                                })
                                                .bg(if active {
                                                    Theme::ACCENT_LIGHT
                                                } else {
                                                    Theme::HOVER_BG
                                                })
                                                .text_size(px(11.5))
                                                .text_color(Theme::TEXT_PRIMARY)
                                                .cursor_pointer()
                                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                                    act("title:".to_string(), win, app)
                                                })
                                                .child("（无称号）")
                                        })
                                        .children(filtered_titles.into_iter().map(|t| {
                                            let act = title_cb.clone();
                                            let id = t.id.clone();
                                            let active = selected_title == t.id;
                                            div()
                                                .px(px(10.0))
                                                .py(px(4.0))
                                                .rounded_md()
                                                .border_1()
                                                .border_color(if active {
                                                    Theme::ACCENT_PRIMARY
                                                } else {
                                                    Theme::CARD_BORDER
                                                })
                                                .bg(if active {
                                                    Theme::ACCENT_LIGHT
                                                } else {
                                                    Theme::HOVER_BG
                                                })
                                                .text_size(px(11.5))
                                                .text_color(Theme::TEXT_PRIMARY)
                                                .cursor_pointer()
                                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                                    act(format!("title:{}", id), win, app)
                                                })
                                                .child(t.name.clone())
                                        })),
                                ),
                        )
                    })
                    // 3 个勋章槽位（真实名称 + 可移除）
                    .when(!loading, |this| {
                        this.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .text_size(px(12.5))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(Theme::TEXT_PRIMARY)
                                        .child(format!("已选挑战勋章 ({}/3)", selected_badges.len())),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(10.0))
                                        .children((0..3).map(|idx| {
                                            let bid_opt = selected_badges.get(idx);
                                            let detail_name = bid_opt.and_then(|id| {
                                                state
                                                    .owned_badges
                                                    .iter()
                                                    .find(|b| b.id == *id)
                                                    .map(|b| b.name.clone())
                                            });
                                            let act = remove_cb.clone();
                                            let has = detail_name.is_some();
                                            div()
                                                .flex_1()
                                                .h(px(50.0))
                                                .p(px(8.0))
                                                .rounded_lg()
                                                .border_1()
                                                .border_color(if has {
                                                    Theme::SIDEBAR_ACTIVE_BORDER
                                                } else {
                                                    Theme::CARD_BORDER
                                                })
                                                .bg(if has {
                                                    Theme::ACCENT_LIGHT
                                                } else {
                                                    Theme::HOVER_BG
                                                })
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .gap(px(6.0))
                                                .child(
                                                    div()
                                                        .text_size(px(11.5))
                                                        .text_color(if has {
                                                            Theme::ACCENT_PRIMARY
                                                        } else {
                                                            Theme::TEXT_MUTED
                                                        })
                                                        .child(match &detail_name {
                                                            Some(n) => n.clone(),
                                                            None => format!("槽位 {} (空)", idx + 1),
                                                        }),
                                                )
                                                .when(has, |s| {
                                                    s.child(
                                                        div()
                                                            .text_size(px(11.0))
                                                            .text_color(Theme::TEXT_MUTED)
                                                            .cursor_pointer()
                                                            .hover(|s| s.text_color(Theme::LOSS_TEXT))
                                                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                                                act(format!("remove:{}", idx), win, app)
                                                            })
                                                            .child("✕"),
                                                    )
                                                })
                                        })),
                                ),
                        )
                    })
                    // 候选勋章库：搜索 + 段位筛选 + 真实列表
                    .when(!loading, |this| {
                        this.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(Theme::TEXT_MUTED)
                                        .child(format!("候选勋章库（已获 {} 枚）", state.owned_badges.len())),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(8.0))
                                        .child(badge_search_box)
                                        .child(
                                            div()
                                                .flex()
                                                .flex_wrap()
                                                .gap(px(4.0))
                                                .children(tier_filters.into_iter().map(|(val, label)| {
                                                    let act = filter_cb.clone();
                                                    let active = tier_filter == val;
                                                    div()
                                                        .px(px(8.0))
                                                        .py(px(3.0))
                                                        .rounded_md()
                                                        .border_1()
                                                        .border_color(if active {
                                                            Theme::ACCENT_PRIMARY
                                                        } else {
                                                            Theme::CARD_BORDER
                                                        })
                                                        .bg(if active {
                                                            Theme::ACCENT_LIGHT
                                                        } else {
                                                            Theme::CARD_BG
                                                        })
                                                        .text_size(px(11.0))
                                                        .text_color(Theme::TEXT_SECONDARY)
                                                        .cursor_pointer()
                                                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                                            act(format!("filter:{}", val), win, app)
                                                        })
                                                        .child(label)
                                                })),
                                        ),
                                )
                                .child(
                                    div()
                                        .w_full()
                                        .id("badges-modal-list-scroll")
                                        .max_h(px(180.0))
                                        .overflow_y_scroll()
                                        .flex()
                                        .flex_col()
                                        .gap(px(6.0))
                                        .children(filtered_badges.into_iter().map(|b| {
                                            let is_equipped = selected_badges.iter().any(|id| id == &b.id);
                                            let bid = b.id.clone();
                                            let act = toggle_cb.clone();
                                            let level = level_label(&b.level);

                                            div()
                                                .w_full()
                                                .px(px(12.0))
                                                .py(px(8.0))
                                                .rounded_md()
                                                .border_1()
                                                .border_color(if is_equipped {
                                                    Theme::ACCENT_PRIMARY
                                                } else {
                                                    Theme::CARD_BORDER
                                                })
                                                .bg(if is_equipped {
                                                    Theme::ACCENT_LIGHT
                                                } else {
                                                    Theme::CARD_BG
                                                })
                                                .flex()
                                                .items_center()
                                                .justify_between()
                                                .cursor_pointer()
                                                .hover(|s| s.border_color(Theme::ACCENT_PRIMARY))
                                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                                    act(bid.clone(), win, app)
                                                })
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .gap(px(1.0))
                                                        .child(
                                                            div()
                                                                .text_size(px(12.5))
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                                .text_color(Theme::TEXT_PRIMARY)
                                                                .child(b.name.clone()),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(11.0))
                                                                .text_color(Theme::TEXT_MUTED)
                                                                .child(if b.description.is_empty() {
                                                                    "暂无描述".to_string()
                                                                } else {
                                                                    b.description.clone()
                                                                }),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap(px(6.0))
                                                        .child(
                                                            div()
                                                                .px(px(6.0))
                                                                .py(px(2.0))
                                                                .rounded_md()
                                                                .text_size(px(10.5))
                                                                .text_color(Theme::TEXT_MUTED)
                                                                .child(level),
                                                        )
                                                        .child(
                                                            div()
                                                                .px(px(8.0))
                                                                .py(px(2.0))
                                                                .rounded_md()
                                                                .text_size(px(11.0))
                                                                .font_weight(FontWeight::MEDIUM)
                                                                .text_color(if is_equipped {
                                                                    Theme::ACCENT_PRIMARY
                                                                } else {
                                                                    Theme::TEXT_MUTED
                                                                })
                                                                .child(if is_equipped { "已装备" } else { "装备" }),
                                                        ),
                                                )
                                        })),
                                ),
                        )
                    })
                    // 底部操作栏：清空 / 取消（零写入）/ 保存
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .pt(px(12.0))
                            .border_t_1()
                            .border_color(Theme::CARD_BORDER)
                            .child(
                                div()
                                    .px(px(12.0))
                                    .py(px(6.0))
                                    .rounded_md()
                                    .border_1()
                                    .border_color(Theme::CARD_BORDER)
                                    .bg(Theme::CARD_BG)
                                    .text_size(px(12.0))
                                    .text_color(Theme::TEXT_SECONDARY)
                                    .cursor_pointer()
                                    .hover(|s| s.bg(Theme::HOVER_BG))
                                    .on_mouse_down(MouseButton::Left, on_clear)
                                    .child("清空所有装配"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .px(px(14.0))
                                            .py(px(6.0))
                                            .rounded_md()
                                            .border_1()
                                            .border_color(Theme::CARD_BORDER)
                                            .bg(Theme::CARD_BG)
                                            .text_size(px(12.0))
                                            .text_color(Theme::TEXT_SECONDARY)
                                            .cursor_pointer()
                                            .hover(|s| s.bg(Theme::HOVER_BG))
                                            .on_mouse_down(MouseButton::Left, cancel_cb)
                                            .child("取消"),
                                    )
                                    .child(
                                        div()
                                            .px(px(18.0))
                                            .py(px(6.0))
                                            .rounded_md()
                                            .bg(Theme::ACCENT_PRIMARY)
                                            .text_size(px(12.5))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(gpui::white())
                                            .cursor_pointer()
                                            .hover(|s| s.opacity(0.85))
                                            .on_mouse_down(MouseButton::Left, on_save)
                                            .child("保存装配"),
                                    ),
                            ),
                    ),
            ),
    )
    .with_priority(10)
}
