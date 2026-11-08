use crate::theme::Theme;
use gpui::*;

/// 极光半透水晶卡片
#[allow(dead_code)]
pub struct Card {
    element: Div,
}

#[allow(dead_code)]
impl Card {
    pub fn new() -> Self {
        Self {
            element: div()
                .bg(Theme::CARD_BG)
                .border_1()
                .border_color(Theme::CARD_BORDER)
                .rounded(px(8.0))
                .shadow_sm(),
        }
    }

    pub fn p(mut self, padding: Pixels) -> Self {
        self.element = self.element.p(padding);
        self
    }

    pub fn p_3(mut self) -> Self {
        self.element = self.element.p_3();
        self
    }

    pub fn p_4(mut self) -> Self {
        self.element = self.element.p_4();
        self
    }

    pub fn flex_1(mut self) -> Self {
        self.element = self.element.flex_1();
        self
    }

    pub fn overflow_hidden(mut self) -> Self {
        self.element = self.element.overflow_hidden();
        self
    }

    pub fn border_color(mut self, color: Hsla) -> Self {
        self.element = self.element.border_color(color);
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.element = self.element.child(child);
        self
    }
}

impl IntoElement for Card {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        self.element
    }
}
