//! 原生极光风格滑动开关组件 (Switch)
//! 遵循纯白水晶极光设计语言，支持 checked 状态动画与点击事件响应

use gpui::*;
use crate::theme::Theme;

pub struct Switch {
    checked: bool,
    disabled: bool,
}

impl Switch {
    pub fn new(checked: bool) -> Self {
        Self {
            checked,
            disabled: false,
        }
    }

    #[allow(dead_code)]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl IntoElement for Switch {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        let (track_bg, track_border) = if self.disabled {
            (hsla(0.0, 0.0, 0.9, 0.5), hsla(0.0, 0.0, 0.8, 0.3))
        } else if self.checked {
            (Theme::ACCENT_PRIMARY, Theme::ACCENT_PRIMARY.opacity(0.8))
        } else {
            (hsla(210.0, 0.15, 0.88, 0.8), hsla(210.0, 0.15, 0.78, 0.5))
        };

        let knob_bg = if self.disabled {
            hsla(0.0, 0.0, 0.95, 0.9)
        } else {
            gpui::white()
        };

        let mut track = div()
            .w(px(38.0))
            .h(px(20.0))
            .rounded_full()
            .bg(track_bg)
            .border_1()
            .border_color(track_border)
            .p(px(2.0))
            .flex()
            .items_center();

        if self.checked {
            track = track.justify_end();
        } else {
            track = track.justify_start();
        }

        let knob = div()
            .size(px(14.0))
            .rounded_full()
            .bg(knob_bg)
            .shadow_sm();

        track.child(knob)
    }
}
