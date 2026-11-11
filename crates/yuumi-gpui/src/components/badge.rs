use crate::theme::Theme;
use gpui::*;

#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum BadgeVariant {
    Default,
    Primary,
    Win,
    Loss,
    Ghost,
}

pub struct Badge {
    text: SharedString,
    variant: BadgeVariant,
}

impl Badge {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            variant: BadgeVariant::Default,
        }
    }

    pub fn variant(mut self, variant: BadgeVariant) -> Self {
        self.variant = variant;
        self
    }

    #[allow(dead_code)]
    pub fn ghost(mut self) -> Self {
        self.variant = BadgeVariant::Ghost;
        self
    }
}

impl IntoElement for Badge {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        let (bg_color, text_color, border_color) = match self.variant {
            BadgeVariant::Default => (
                Theme::BADGE_BG(),
                Theme::TEXT_SECONDARY(),
                Theme::CARD_BORDER(),
            ),
            BadgeVariant::Primary => (
                Theme::ACCENT_PRIMARY().opacity(0.18),
                Theme::ACCENT_PRIMARY(),
                Theme::ACCENT_PRIMARY().opacity(0.4),
            ),
            BadgeVariant::Win => (Theme::WIN_BG(), Theme::WIN_TEXT(), Theme::WIN_BORDER()),
            BadgeVariant::Loss => (Theme::LOSS_BG(), Theme::LOSS_TEXT(), Theme::LOSS_BORDER()),
            BadgeVariant::Ghost => (
                gpui::transparent_black(),
                Theme::TEXT_SECONDARY(),
                Theme::CARD_BORDER(),
            ),
        };

        div()
            .flex()
            .items_center()
            .justify_center()
            .px_2()
            .py_0p5()
            .rounded(px(4.0))
            .bg(bg_color)
            .border_1()
            .border_color(border_color)
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(text_color)
            .child(self.text)
    }
}
