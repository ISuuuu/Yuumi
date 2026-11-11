use crate::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::*;

#[derive(Clone, Debug)]
pub struct SearchInput {
    pub text: String,
    pub placeholder: String,
    pub is_focused: bool,
    #[allow(dead_code)]
    pub last_submitted: Option<String>,
}

#[allow(dead_code)]
impl SearchInput {
    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            text: String::new(),
            placeholder: placeholder.into(),
            is_focused: false,
            last_submitted: None,
        }
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    pub fn clear(&mut self) {
        self.text.clear();
    }

    pub fn render(
        &self,
        on_clear: impl Fn(&mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let text = self.text.clone();
        let placeholder = self.placeholder.clone();
        let has_text = !text.is_empty();
        let is_focused = self.is_focused;

        div()
            .h(px(32.0))
            .w(px(240.0))
            .rounded_lg()
            .bg(Theme::CARD_BG())
            .border_1()
            .border_color(if is_focused {
                Theme::AURORA_CYAN()
            } else {
                Theme::CARD_BORDER()
            })
            .px_3()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_xs().text_color(Theme::TEXT_MUTED()).child("🔍"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(if has_text {
                                Theme::TEXT_PRIMARY()
                            } else {
                                Theme::TEXT_MUTED()
                            })
                            .child(if has_text { text } else { placeholder }),
                    ),
            )
            .when(has_text, |this| {
                this.child(
                    div()
                        .text_xs()
                        .text_color(Theme::TEXT_MUTED())
                        .cursor_pointer()
                        .hover(|s| s.text_color(Theme::TEXT_PRIMARY()))
                        .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                            on_clear(window, app);
                        })
                        .child("✕"),
                )
            })
    }
}
