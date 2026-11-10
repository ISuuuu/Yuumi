//! 公告与更新日志右下角浮动通知弹窗 (对照 Vue `src/components/NoticePopup.vue`)
//!
//! 纯白水晶极光风格：固定于右下角 (320px 宽)，带毛玻璃底色、投影与关闭按钮；
//! 支持真实加载中状态、失败重试与内容展示，不打断主窗口路由状态。

use crate::components::collapse_card::action_button;
use crate::components::icon::icon;
use crate::theme::Theme;
use gpui::*;

pub fn notice_popup(
    title: &'static str,
    loading: bool,
    has_error: bool,
    content: &str,
    on_close: impl Fn(&mut Window, &mut App) + 'static + Clone,
    on_retry: impl Fn(&mut Window, &mut App) + 'static + Clone,
) -> impl IntoElement {
    let close_cb = on_close;
    let retry_cb = on_retry;

    let body_element = if loading {
        div()
            .p(px(20.0))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(8.0))
            .child(
                div()
                    .size(px(16.0))
                    .rounded_full()
                    .border(px(2.0))
                    .border_color(Theme::ACCENT_PRIMARY),
            )
            .child(
                div()
                    .text_size(px(12.5))
                    .text_color(Theme::TEXT_MUTED)
                    .child("正在获取最新公告..."),
            )
            .into_any_element()
    } else if has_error {
        div()
            .p(px(16.0))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(10.0))
            .child(
                div()
                    .text_size(px(12.5))
                    .text_color(Theme::LOSS_TEXT)
                    .child("获取最新公告失败"),
            )
            .child(action_button("重试", false, move |_ev, win, app| {
                retry_cb(win, app);
            }))
            .into_any_element()
    } else {
        let display_content = if content.is_empty() {
            "当前版本已完整复刻 Vue 纯白水晶极光界面，Direct3D 11 原生极速渲染，内存与启动性能优化完毕。"
        } else {
            content
        };

        div()
            .id("notice-content-body")
            .p(px(14.0))
            .max_h(px(240.0))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .text_size(px(12.5))
            .text_color(Theme::TEXT_SECONDARY)
            .line_height(relative(1.4))
            .child(display_content.to_string())
            .into_any_element()
    };

    div()
        .id("notice-popup-toast")
        .occlude()
        .w(px(320.0))
        .bg(Theme::CARD_BG)
        .border_1()
        .border_color(Theme::CARD_BORDER)
        .rounded(Theme::RADIUS_LG)
        .shadow_lg()
        .overflow_hidden()
        .flex()
        .flex_col()
        // 头部标题行
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .h(px(40.0))
                .px(px(14.0))
                .border_b_1()
                .border_color(Theme::CARD_BORDER)
                .bg(Theme::HOVER_BG)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(icon("icons/bell.svg", 16.0, Theme::ACCENT_PRIMARY))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(Theme::TEXT_PRIMARY)
                                .child(title),
                        ),
                )
                .child(
                    div()
                        .id("notice-popup-close-btn")
                        .size(px(24.0))
                        .rounded(px(4.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .hover(|s| s.bg(Theme::HOVER_BG_STRONG))
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            close_cb(win, app);
                        })
                        .child(icon("icons/close.svg", 14.0, Theme::TEXT_MUTED)),
                ),
        )
        // 内容区
        .child(body_element)
}
