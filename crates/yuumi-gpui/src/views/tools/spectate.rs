//! 工具箱「游戏」组：实时观战与好友僵尸号清理卡片。
//!
//! 归属工单 D01（观战输入/方式/提交、好友扫描、筛选、多选、批量删除）。
//! 自 `views/tools.rs` 按工单边界搬移；D01 起改为真实 LCU 数据与服务调用。

use super::components::*;
use super::{ToolsAction, ToolsView};
use crate::components::collapse_card::{collapse_card, setting_row};
use crate::components::dropdown::{
    dropdown_menu, select_dropdown as generic_select_dropdown, select_dropdown_item,
};
use crate::components::text_field::{text_field, TextFieldOptions};
use crate::services::friend_cleaner::{filter_counts, filtered_friends, FriendActivity};
use crate::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::cell::OnceCell;
use std::rc::Rc;

impl ToolsView {
    // 行 7: 实时直接观战召唤师 (折叠卡 - T-V07)
    pub(super) fn render_spectate_card<F>(&self, window: &Window, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let open = self.open_cards.contains("spectate");
        let act_toggle = on_action.clone();
        let act_launch = on_action.clone();
        let act_input = on_action.clone();
        let act_clear = on_action.clone();
        let act_submit = on_action.clone();
        let act_toggle_dd = on_action.clone();
        let act_select_method = on_action.clone();

        let name_empty = self.spectate_name_buffer.trim().is_empty();
        let focus_cell = self.spectate_name_focus.clone();

        collapse_card(
            "tools-card-spectate".into(),
            open,
            Some("icons/search.svg"),
            "实时直接观战召唤师",
            "输入游戏玩家召唤师名称或 Riot ID，直接调用游戏客户端引擎启动观战",
            Some("点击展开".to_string()),
            move |_ev, win, app| act_toggle(ToolsAction::ToggleCollapse("spectate"), win, app),
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                // 行 1: 召唤师输入框带内嵌后缀按钮
                .child(setting_row(
                    "召唤师名称",
                    Some("输入目标召唤师昵称#TAG，直接启动对局实时观战"),
                    div()
                        .w(px(300.0))
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(lazy_text_field(
                            "input-spectate-name",
                            &self.spectate_name_buffer,
                            "输入游戏昵称#TAG...",
                            216.0,
                            40,
                            &focus_cell,
                            window,
                            move |next, win, app| {
                                act_input(ToolsAction::SpectateSummonerNameInput(next), win, app)
                            },
                            move |win, app| {
                                act_clear(
                                    ToolsAction::SpectateSummonerNameInput(String::new()),
                                    win,
                                    app,
                                )
                            },
                            move |_text, win, app| {
                                act_submit(ToolsAction::LaunchSpectate, win, app)
                            },
                        ))
                        .child(
                            div()
                                .px(px(10.0))
                                .py(px(4.0))
                                .rounded(px(4.0))
                                .bg(if name_empty {
                                    Theme::HOVER_BG()
                                } else {
                                    Theme::ACCENT_PRIMARY()
                                })
                                .text_size(px(11.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(if name_empty {
                                    Theme::TEXT_MUTED()
                                } else {
                                    gpui::white()
                                })
                                .cursor(if name_empty {
                                    CursorStyle::OperationNotAllowed
                                } else {
                                    CursorStyle::PointingHand
                                })
                                .when(!name_empty, |this| {
                                    let act_launch = act_launch.clone();
                                    this.hover(|s| s.opacity(0.85)).on_mouse_down(
                                        MouseButton::Left,
                                        move |_ev, win, app| {
                                            act_launch(ToolsAction::LaunchSpectate, win, app)
                                        },
                                    )
                                })
                                .child("启动观战"),
                        ),
                ))
                // 行 2: 120px 启动方式下拉框
                .child(setting_row(
                    "启动方式",
                    Some("选择通过 LCU 原生客户端接口或 CMD 独立拉起游戏本体"),
                    select_dropdown(
                        "dropdown-spectate-method",
                        120.0,
                        if self.spectate_method == "CMD" {
                            "CMD"
                        } else {
                            "LCU API"
                        }
                        .to_string(),
                        self.open_spectate_method_dropdown,
                        false,
                        vec![("LCU", "LCU API"), ("CMD", "CMD")],
                        move |win, app| {
                            act_toggle_dd(
                                ToolsAction::ToggleDropdownMenu("spectate_method"),
                                win,
                                app,
                            )
                        },
                        move |val, win, app| {
                            act_select_method(
                                ToolsAction::SetSpectateMethod(val.to_string()),
                                win,
                                app,
                            )
                        },
                    ),
                )),
        )
    }

    // 行 8: 好友管理与僵尸号清理 (折叠卡 - T-V08)
    pub(super) fn render_friend_cleaner_card<F>(
        &self,
        window: &Window,
        on_action: F,
    ) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let open = self.open_cards.contains("friendcleaner");
        let act_toggle = on_action.clone();
        let act_scan = on_action.clone();
        let act_del = on_action.clone();
        let act_toggle_filter = on_action.clone();
        let act_select_filter = on_action.clone();
        let act_select_all = on_action.clone();
        let act_clear_sel = on_action.clone();

        let now = crate::services::friend_cleaner::now_ms();
        let counts = filter_counts(&self.cleaner_friends, now);
        let search_query = self.cleaner_search_live.borrow().clone();
        let filtered = filtered_friends(
            &self.cleaner_friends,
            &self.cleaner_filter,
            &search_query,
            now,
        );
        let filtered_count = filtered.len();

        // 筛选下拉：命中数来自真实好友数据，替换原硬编码的 (48)/(23)/(62)
        let filter_options: Vec<(&'static str, String)> = vec![
            (
                "days180",
                format!("超过 180 天未打游戏 ({})", counts.days180),
            ),
            (
                "days360",
                format!("超过 360 天未打游戏 ({})", counts.days360),
            ),
            ("days90", format!("超过 90 天未打游戏 ({})", counts.days90)),
            ("never", format!("从未打过游戏 ({})", counts.never)),
            ("hidden", format!("🔒 战绩已隐藏 ({})", counts.hidden)),
            ("all", format!("全部好友 ({})", counts.all)),
        ];
        let filter_label = filter_options
            .iter()
            .find(|(value, _)| *value == self.cleaner_filter.as_str())
            .map(|(_, label)| label.clone())
            .unwrap_or_else(|| filter_options[0].1.clone());

        let select_action = act_select_filter.clone();
        let current_filter = self.cleaner_filter.clone();
        let filter_menu = dropdown_menu(
            250.0,
            filter_options
                .iter()
                .map(|(value, label)| {
                    let value = *value;
                    let select = select_action.clone();
                    select_dropdown_item(
                        label.clone(),
                        current_filter == value,
                        move |_ev, win, app| {
                            select(ToolsAction::SetCleanerFilter(value.to_string()), win, app)
                        },
                    )
                })
                .collect::<Vec<_>>(),
        );

        let search_cell = self.cleaner_search_live.clone();
        let search_focus = self.cleaner_search_focus.clone();
        let change_cell = search_cell.clone();
        let clear_cell = search_cell.clone();

        let header_status = if self.cleaner_scanning {
            "扫描中…".to_string()
        } else if self.cleaner_has_scanned {
            format!(
                "已扫描 · 共 {} 人 / 筛选出 {} 人",
                self.cleaner_friends.len(),
                filtered_count
            )
        } else {
            "未扫描".to_string()
        };

        collapse_card(
            "tools-card-friendcleaner".into(),
            open,
            Some("icons/user.svg"),
            "好友管理与僵尸号清理",
            "智能检索超过设定天数未上线的游戏好友，支持一键批量移除",
            Some(header_status),
            move |_ev, win, app| act_toggle(ToolsAction::ToggleCollapse("friendcleaner"), win, app),
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                // 未扫描态：仅显示扫描好友按钮
                .when(!self.cleaner_has_scanned, |this| {
                    let scan_cb = act_scan.clone();
                    let scanning = self.cleaner_scanning;
                    this.child(
                        div()
                            .p(px(16.0))
                            .rounded_md()
                            .bg(Theme::HOVER_BG())
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(Theme::TEXT_PRIMARY())
                                            .child(if scanning {
                                                "正在读取好友列表并检测最后活跃时间…"
                                            } else {
                                                "扫描好友列表并自动检测每位好友的最后活跃时间"
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.5))
                                            .text_color(Theme::TEXT_MUTED())
                                            .child("穿透查询最近对局与熟练度时间，保护隐藏战绩玩家不被误删"),
                                    ),
                            )
                            .child(
                                div()
                                    .px(px(16.0))
                                    .py(px(6.0))
                                    .rounded_md()
                                    .bg(Theme::ACCENT_PRIMARY())
                                    .text_size(px(12.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(gpui::white())
                                    .cursor(if scanning {
                                        CursorStyle::OperationNotAllowed
                                    } else {
                                        CursorStyle::PointingHand
                                    })
                                    .when(!scanning, |this| {
                                        this.hover(|s| s.opacity(0.85)).on_mouse_down(
                                            MouseButton::Left,
                                            move |_ev, win, app| {
                                                scan_cb(ToolsAction::ScanFriends, win, app)
                                            },
                                        )
                                    })
                                    .child(if scanning { "扫描中…" } else { "开始扫描好友" }),
                            ),
                    )
                })
                // 已扫描态：操作栏 + 列表
                .when(self.cleaner_has_scanned, |this| {
                    let scan_cb = act_scan.clone();
                    let scanning = self.cleaner_scanning;
                    this.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(10.0))
                            // 顶行：重新扫描 + 250px 筛选下拉 + 170px 搜索框
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap(px(10.0))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(10.0))
                                            .child(
                                                div()
                                                    .px(px(12.0))
                                                    .py(px(5.0))
                                                    .rounded_md()
                                                    .bg(Theme::ACCENT_PRIMARY())
                                                    .text_size(px(12.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(gpui::white())
                                                    .cursor(if scanning {
                                                        CursorStyle::OperationNotAllowed
                                                    } else {
                                                        CursorStyle::PointingHand
                                                    })
                                                    .when(!scanning, |this| {
                                                        this.hover(|s| s.opacity(0.85)).on_mouse_down(
                                                            MouseButton::Left,
                                                            move |_ev, win, app| {
                                                                scan_cb(ToolsAction::ScanFriends, win, app)
                                                            },
                                                        )
                                                    })
                                                    .child(if scanning { "扫描中…" } else { "重新扫描好友" }),
                                            )
                                            .child(generic_select_dropdown(
                                                "dropdown-cleaner-filter".into(),
                                                250.0,
                                                filter_label.clone(),
                                                self.open_cleaner_filter_dropdown,
                                                filter_menu,
                                                move |_ev, win, app| {
                                                    act_toggle_filter(
                                                        ToolsAction::ToggleDropdownMenu("cleaner_filter"),
                                                        win,
                                                        app,
                                                    )
                                                },
                                            )),
                                    )
                                    .child(lazy_text_field(
                                        "input-cleaner-search",
                                        &search_query,
                                        "搜索游戏名/备注...",
                                        170.0,
                                        32,
                                        &search_focus,
                                        window,
                                        move |next, window, _app| {
                                            *change_cell.borrow_mut() = next;
                                            window.refresh();
                                        },
                                        move |window, _app| {
                                            clear_cell.borrow_mut().clear();
                                            window.refresh();
                                        },
                                        |_text: &str, _window: &mut Window, _app: &mut App| {},
                                    )),
                            )
                            // 次行：全选、清空勾选、批量删除按钮
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(8.0))
                                            .child(
                                                div()
                                                    .px(px(10.0))
                                                    .py(px(4.0))
                                                    .rounded_md()
                                                    .border_1()
                                                    .border_color(Theme::CARD_BORDER())
                                                    .bg(Theme::CARD_BG())
                                                    .text_size(px(11.5))
                                                    .text_color(Theme::TEXT_SECONDARY())
                                                    .cursor_pointer()
                                                    .hover(|s| s.bg(Theme::HOVER_BG()))
                                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                                        act_select_all(ToolsAction::SelectAllCleanerFriends, win, app)
                                                    })
                                                    .child("勾选当前列表"),
                                            )
                                            .child(
                                                div()
                                                    .px(px(10.0))
                                                    .py(px(4.0))
                                                    .rounded_md()
                                                    .border_1()
                                                    .border_color(Theme::CARD_BORDER())
                                                    .bg(Theme::CARD_BG())
                                                    .text_size(px(11.5))
                                                    .text_color(Theme::TEXT_SECONDARY())
                                                    .cursor_pointer()
                                                    .hover(|s| s.bg(Theme::HOVER_BG()))
                                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                                        act_clear_sel(ToolsAction::ClearSelectedCleanerFriends, win, app)
                                                    })
                                                    .child("清空勾选"),
                                            ),
                                    )
                                    .child(danger_button(
                                        format!("批量删除选中的 {} 人", self.cleaner_selected_ids.len()),
                                        move |_ev, win, app| act_del(ToolsAction::BatchDeleteOfflineFriends, win, app),
                                    )),
                            )
                            // 好友列表（真实筛选结果）
                            .child(
                                div()
                                    .id("cleaner-friends-list-scroll")
                                    .w_full()
                                    .max_h(px(220.0))
                                    .overflow_y_scroll()
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.0))
                                    .when(filtered.is_empty(), |this| {
                                        this.child(
                                            div()
                                                .py(px(12.0))
                                                .text_size(px(12.0))
                                                .text_color(Theme::TEXT_MUTED())
                                                .child("无符合当前筛选条件的好友"),
                                        )
                                    })
                                    .children(filtered.into_iter().map(|f| {
                                        let fid = f.id.clone();
                                        let is_selected = self.cleaner_selected_ids.contains(&fid);
                                        let act_check = on_action.clone();
                                        let activity_color = match f.activity {
                                            FriendActivity::Online => Theme::WIN_TEXT(),
                                            FriendActivity::Hidden => Theme::TEXT_MUTED(),
                                            _ => Theme::LOSS_TEXT(),
                                        };

                                        div()
                                            .w_full()
                                            .px(px(12.0))
                                            .py(px(6.0))
                                            .rounded_md()
                                            .border_1()
                                            .border_color(if is_selected {
                                                Theme::SIDEBAR_ACTIVE_BORDER()
                                            } else {
                                                Theme::CARD_BORDER()
                                            })
                                            .bg(if is_selected {
                                                Theme::ACCENT_LIGHT()
                                            } else {
                                                Theme::CARD_BG()
                                            })
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .cursor_pointer()
                                            .hover(|s| s.border_color(Theme::ACCENT_PRIMARY()))
                                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                                act_check(ToolsAction::ToggleSelectCleanerFriend(fid.clone()), win, app)
                                            })
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(10.0))
                                                    .child(
                                                        div()
                                                            .size(px(16.0))
                                                            .rounded(px(3.0))
                                                            .border_1()
                                                            .border_color(if is_selected {
                                                                Theme::ACCENT_PRIMARY()
                                                            } else {
                                                                Theme::CARD_BORDER()
                                                            })
                                                            .bg(if is_selected {
                                                                Theme::ACCENT_PRIMARY()
                                                            } else {
                                                                Theme::CARD_BG()
                                                            })
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .text_size(px(10.0))
                                                            .text_color(gpui::white())
                                                            .child(if is_selected { "✓" } else { "" }),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(12.5))
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(Theme::TEXT_PRIMARY())
                                                            .child(f.name.clone()),
                                                    )
                                                    .when(!f.note.is_empty(), |s| {
                                                        s.child(
                                                            div()
                                                                .text_size(px(11.0))
                                                                .text_color(Theme::TEXT_MUTED())
                                                                .child(format!("({})", f.note)),
                                                        )
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.5))
                                                    .text_color(activity_color)
                                                    .child(f.activity_label(now)),
                                            )
                                    })),
                            ),
                    )
                }),
        )
    }
}

/// 首次点击时惰性创建焦点句柄的单行输入框。
///
/// gpui 0.2 的 `FocusHandle` 只能由 `App` 创建，而 `ToolsView::new()` 没有 `App`，
/// 因此先渲染同尺寸占位，点击时创建句柄并立即聚焦；`Window::focus` 后显式 `refresh()`
/// 触发重绘，下一帧改为真实 `text_field`（沿用统一样式）。
#[allow(clippy::too_many_arguments)]
pub(super) fn lazy_text_field(
    id: &'static str,
    text: &str,
    placeholder: &str,
    width: f32,
    max_chars: usize,
    focus_cell: &Rc<OnceCell<FocusHandle>>,
    window: &Window,
    on_change: impl Fn(String, &mut Window, &mut App) + 'static + Clone,
    on_clear: impl Fn(&mut Window, &mut App) + 'static + Clone,
    on_submit: impl Fn(&str, &mut Window, &mut App) + 'static + Clone,
) -> AnyElement {
    if let Some(handle) = focus_cell.get().cloned() {
        // [P1-4] 聚焦态实时接入：不再传死 false，边框高亮与光标竖线随焦点变化
        let is_focused = handle.is_focused(window);
        return div()
            .w(px(width))
            .child(text_field(
                id,
                text,
                placeholder,
                px(width),
                &handle,
                is_focused,
                TextFieldOptions {
                    max_chars: Some(max_chars),
                    ..Default::default()
                },
                on_change,
                on_clear,
                on_submit,
            ))
            .into_any_element();
    }

    let cell = focus_cell.clone();
    div()
        .id(id)
        .w(px(width))
        .h(px(28.0))
        .rounded(px(10.0))
        .bg(Theme::CARD_BG())
        .border_1()
        .border_color(Theme::CARD_BORDER())
        .hover(|s| s.border_color(Theme::ACCENT_PRIMARY()))
        .px(px(10.0))
        .flex()
        .items_center()
        .cursor_text()
        .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
            let handle = cell.get_or_init(|| app.focus_handle()).clone();
            window.focus(&handle);
            // [P1-4] 首次点击创建句柄后立即重绘，下一帧切到真实 text_field 并显示聚焦态
            window.refresh();
        })
        .child(
            div()
                .text_size(px(12.5))
                .text_color(Theme::TEXT_MUTED())
                .child(if text.is_empty() {
                    placeholder.to_string()
                } else {
                    text.to_string()
                }),
        )
        .into_any_element()
}
