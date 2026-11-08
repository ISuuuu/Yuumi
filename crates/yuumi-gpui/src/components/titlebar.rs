use gpui::*;
use crate::theme::Theme;

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

    pub fn render(
        &self,
        _window: &mut Window,
        on_minimize: impl Fn(&mut Window, &mut App) + 'static + Clone,
        on_close: impl Fn(&mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let title = self.title.clone();
        let close_cb = on_close.clone();
        let min_cb = on_minimize.clone();

        div()
            .h(px(38.0))
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .bg(Theme::CARD_BG)
            .border_b_1()
            .border_color(Theme::CARD_BORDER)
            // 左侧：Logo 与应用标题
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        // 极光渐变小圆点
                        div()
                            .size(px(12.0))
                            .rounded_full()
                            .bg(Theme::AURORA_CYAN)
                            .border_1()
                            .border_color(Theme::AURORA_BLUE),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(Theme::TEXT_PRIMARY)
                            .child(title),
                    )
                    .child(
                        // 状态小药丸
                        div()
                            .px_2()
                            .py(px(1.0))
                            .rounded_full()
                            .bg(Theme::ACCENT_LIGHT)
                            .text_xs()
                            .text_color(Theme::ACCENT_PRIMARY)
                            .child("GPUI 原型 v0.1.0"),
                    ),
            )
            // 右侧：最小化与关闭按钮
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .id("btn-minimize")
                            .px_3()
                            .py_1()
                            .rounded_sm()
                            .text_sm()
                            .text_color(Theme::TEXT_SECONDARY)
                            .hover(|s| s.bg(rgba(0x00000010)))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_ev, window, cx| {
                                min_cb(window, cx);
                            })
                            .child("—"),
                    )
                    .child(
                        div()
                            .id("btn-close")
                            .px_3()
                            .py_1()
                            .rounded_sm()
                            .text_sm()
                            .text_color(Theme::TEXT_SECONDARY)
                            .hover(|s| s.bg(rgba(0xe53e3e20)).text_color(rgb(0xe53e3e)))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_ev, window, cx| {
                                close_cb(window, cx);
                            })
                            .child("✕"),
                    ),
            )
    }
}
