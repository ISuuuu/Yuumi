//! 公告与更新日志右下角浮动通知弹窗 (对照 Vue `src/components/NoticePopup.vue`)
//!
//! 纯白水晶极光风格：固定于右下角 (320px 宽)，带毛玻璃底色、投影与关闭按钮；
//! 展示最新更新动态与注意事项，不打断主窗口路由状态。

use crate::components::icon::icon;
use crate::theme::Theme;
use gpui::*;

pub fn notice_popup(
    title: &'static str,
    content: &'static str,
    on_close: impl Fn(&mut Window, &mut App) + 'static + Clone,
) -> impl IntoElement {
    let close_cb = on_close;

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
        .child(
            div()
                .p(px(14.0))
                .flex()
                .flex_col()
                .gap(px(8.0))
                .text_size(px(12.5))
                .text_color(Theme::TEXT_SECONDARY)
                .line_height(relative(1.4))
                .child(content),
        )
}
