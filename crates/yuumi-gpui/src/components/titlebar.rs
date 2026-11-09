//! 自定义标题栏（对照 Vue `src/components/layout/CustomTitleBar.vue`）
//!
//! 复刻要点：高 38、左侧 Logo 20×20 与 Yummi 标题、连接且处于对局阶段时的中文阶段徽章、
//! 右侧 32×26 的最小化 / 最大化（还原）/ 关闭按钮；空白区负责拖拽与双击最大化。

use crate::assets::logo_image;
use crate::components::icon::icon;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;

pub const APP_NAME: &str = "Yummi";

/// 游戏阶段英文枚举 → 旧版中文文案（`src/locales/zh-CN.json` 的 `phase.*`）
pub fn phase_label(phase: &str) -> Option<&'static str> {
    match phase {
        "Lobby" => Some("房间组队中"),
        "Matchmaking" => Some("匹配中"),
        "CheckedIntoTournament" => Some("锦标赛签到"),
        "ReadyCheck" => Some("确认对局"),
        "ChampSelect" => Some("选择英雄"),
        "GameStart" => Some("游戏加载"),
        "FailedToLaunch" => Some("启动失败"),
        "InProgress" => Some("游戏中"),
        "Reconnect" => Some("重新连接"),
        "WaitingForStats" => Some("等待数据"),
        "PreEndOfGame" => Some("结算中"),
        "EndOfGame" => Some("对局结束"),
        "TerminatedInError" | "TerminatedByError" => Some("对局异常终止"),
        "TerminatedInSeries" => Some("系列赛终止"),
        "WatchInProgress" => Some("观战中"),
        _ => None,
    }
}

pub struct TitleBar {
    pub title: SharedString,
    #[allow(dead_code)]
    pub is_maximized: bool,
}

impl TitleBar {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            is_maximized: false,
        }
    }

    /// 渲染标题栏
    ///
    /// - `show_back`：历史栈非空时展示返回键（对照 Vue 的 `pageHistory.length > 0`）
    /// - `phase`：连接态下的真实游戏阶段，`None` 表示不展示阶段徽章
    pub fn render(
        &self,
        show_back: bool,
        phase: Option<&'static str>,
        window: &mut Window,
        on_back: impl Fn(&mut Window, &mut App) + 'static + Clone,
        on_minimize: impl Fn(&mut Window, &mut App) + 'static + Clone,
        on_close: impl Fn(&mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let title = self.title.clone();
        let back_cb = on_back;
        let min_cb = on_minimize;
        let max_window = window.is_maximized();
        let close_cb = on_close;

        div()
            .flex()
            .items_center()
            .justify_between()
            .h(Theme::TITLEBAR_HEIGHT)
            .flex_shrink_0()
            .px(px(8.0))
            .bg(Theme::TITLEBAR_BG)
            .border_b_1()
            .border_color(Theme::CARD_BORDER)
            // 原生托管拖拽区：利用 GPUI 的 WindowControlArea::Drag
            // Windows 底层 hit test 命中 HTCAPTION，由系统原生处理平滑拖拽、Aero Snap 与双击最大化/还原，
            // 彻底避免在 GPUI 事件回调持有 AppCell 借用时同步调用 SendMessageW 造成 "RefCell already borrowed" 冲突。
            .window_control_area(WindowControlArea::Drag)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .when(show_back, |el| {
                        el.child(
                            div()
                                .id("titlebar-back")
                                .occlude()
                                .flex()
                                .items_center()
                                .justify_center()
                                .w(px(32.0))
                                .h(px(26.0))
                                .rounded(px(4.0))
                                .hover(|s| s.bg(Theme::HOVER_BG))
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                                    app.stop_propagation();
                                    back_cb(window, app);
                                })
                                .child(icon("icons/back.svg", 14.0, Theme::TEXT_MUTED)),
                        )
                    })
                    .child(match logo_image() {
                        Some(logo) => div()
                            .flex_none()
                            .w(px(20.0))
                            .h(px(20.0))
                            .ml(px(4.0))
                            .child(img(logo).size_full().object_fit(ObjectFit::Contain)),
                        None => div().flex_none().w(px(20.0)).h(px(20.0)).ml(px(4.0)),
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .ml(px(2.0))
                            .child(
                                div()
                                    .text_size(px(12.8))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child(title),
                            )
                            .when_some(phase, |el, label| {
                                el.child(
                                    div()
                                        .ml(px(6.0))
                                        .px(px(6.0))
                                        .py(px(1.0))
                                        .rounded(px(4.0))
                                        .bg(Theme::ACCENT_LIGHT)
                                        .text_size(px(12.5))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(Theme::ACCENT_PRIMARY)
                                        .child(format!("· {}", label)),
                                )
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .child(
                        div()
                            .id("btn-minimize")
                            .occlude()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(32.0))
                            .h(px(26.0))
                            .rounded(px(4.0))
                            .hover(|s| s.bg(Theme::HOVER_BG))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                                app.stop_propagation();
                                min_cb(window, app);
                            })
                            .child(icon("icons/minimize.svg", 14.0, Theme::TEXT_MUTED)),
                    )
                    .child(
                        div()
                            .id("btn-maximize")
                            .occlude()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(32.0))
                            .h(px(26.0))
                            .rounded(px(4.0))
                            .hover(|s| s.bg(Theme::HOVER_BG))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                                app.stop_propagation();
                                window.zoom_window();
                            })
                            .child(icon(
                                if max_window {
                                    "icons/restore.svg"
                                } else {
                                    "icons/maximize.svg"
                                },
                                14.0,
                                Theme::TEXT_MUTED,
                            )),
                    )
                    .child(
                        div()
                            .id("btn-close")
                            .occlude()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(32.0))
                            .h(px(26.0))
                            .rounded(px(4.0))
                            .hover(|s| s.bg(Theme::LOSS_BAR))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                                app.stop_propagation();
                                close_cb(window, app);
                            })
                            .child(icon("icons/close.svg", 14.0, Theme::TEXT_MUTED)),
                    ),
            )
    }
}
