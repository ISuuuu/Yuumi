//! 工具箱专用 UI 控件库
//!
//! 包含直显卡片行、英雄选择器、技能选择器、带后缀按钮输入框、危险按钮、装配弹窗等。

use crate::components::icon::icon;
use crate::theme::Theme;
use super::data::{
    get_champion_name, get_spell_name, ChampionEntry,
    CHAMPIONS, MOCK_BADGES, SUMMONER_SPELLS,
};
use gpui::prelude::FluentBuilder;
use gpui::*;

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

/// 英雄选择器组件（收起态 Chips + 展开态搜索网格）
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
    let close_click = on_close.clone();

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
                                        close_click(ev, win, app);
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
            let close_cb = on_close.clone();
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
                    // 顶部标题与关闭
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
                                    .child("点选常用英雄 (单选替换)"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(Theme::TEXT_MUTED)
                                    .cursor_pointer()
                                    .hover(|s| s.text_color(Theme::TEXT_PRIMARY))
                                    .on_mouse_down(MouseButton::Left, close_cb.clone())
                                    .child("✕"),
                            ),
                    )
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
                    // 底部确认
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
                                    .child(format!("已配置 {} 位英雄", selected_ids.len())),
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
                                    .on_mouse_down(MouseButton::Left, close_cb)
                                    .child("完成"),
                            ),
                    ),
            )
        })
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
pub fn badges_modal<FClose, FToggleBadge, FClear, FSave>(
    selected_badges: &[String],
    _selected_title: &str,
    on_close: FClose,
    on_toggle_badge: FToggleBadge,
    on_clear: FClear,
    on_save: FSave,
) -> impl IntoElement
where
    FClose: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static + Clone,
    FToggleBadge: Fn(&'static str, &mut Window, &mut App) + 'static + Clone,
    FClear: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static + Clone,
    FSave: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static + Clone,
{
    let close_cb = on_close.clone();

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
                    .w(px(540.0))
                    .max_h(px(580.0))
                    .bg(rgb(0xffffff))
                    .rounded_xl()
                    .shadow_lg()
                    .border_1()
                    .border_color(Theme::CARD_BORDER)
                    .p(px(20.0))
                    .flex()
                    .flex_col()
                    .gap(px(14.0))
                    // 弹窗标题
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
                    // 称号区
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child("当前称号: 无可匹敌"),
                            ),
                    )
                    // 3 个勋章槽位卡
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child(format!("挑战勋章槽位 ({}/3)", selected_badges.len())),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(10.0))
                                    .children((0..3).map(|idx| {
                                        let badge_id_opt = selected_badges.get(idx);
                                        let badge_detail = badge_id_opt.and_then(|id| {
                                            MOCK_BADGES.iter().find(|b| b.id == id.as_str())
                                        });

                                        div()
                                            .flex_1()
                                            .h(px(54.0))
                                            .p(px(8.0))
                                            .rounded_lg()
                                            .border_1()
                                            .border_color(if badge_detail.is_some() {
                                                Theme::SIDEBAR_ACTIVE_BORDER
                                            } else {
                                                Theme::CARD_BORDER
                                            })
                                            .bg(if badge_detail.is_some() {
                                                Theme::ACCENT_LIGHT
                                            } else {
                                                Theme::HOVER_BG
                                            })
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(match badge_detail {
                                                Some(b) => div()
                                                    .flex()
                                                    .flex_col()
                                                    .items_center()
                                                    .child(
                                                        div()
                                                            .text_size(px(12.0))
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(Theme::ACCENT_PRIMARY)
                                                            .child(b.name),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(10.0))
                                                            .text_color(Theme::TEXT_MUTED)
                                                            .child("已佩戴"),
                                                    ),
                                                None => div()
                                                    .text_size(px(11.5))
                                                    .text_color(Theme::TEXT_MUTED)
                                                    .child(format!("槽位 {}", idx + 1)),
                                            })
                                    })),
                            ),
                    )
                    // 挑战勋章候选列表
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(Theme::TEXT_MUTED)
                                    .child("点击可选勋章加入或卸下槽位:"),
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
                                    .children(MOCK_BADGES.iter().map(|b| {
                                        let is_equipped = selected_badges.iter().any(|id| id == b.id);
                                        let bid = b.id;
                                        let toggle_cb = on_toggle_badge.clone();

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
                                                toggle_cb(bid, win, app)
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
                                                            .child(b.name),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(11.0))
                                                            .text_color(Theme::TEXT_MUTED)
                                                            .child(b.desc),
                                                    ),
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
                                            )
                                    })),
                            ),
                    )
                    // 底部操作栏
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
    )
    .with_priority(10)
}
