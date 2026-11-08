use gpui::*;
use crate::theme::Theme;

pub struct BenchOverlayView {
    pub bench_champions: Vec<(&'static str, &'static str)>,
    pub selected_idx: Option<usize>,
}

impl BenchOverlayView {
    pub fn new() -> Self {
        Self {
            bench_champions: vec![
                ("悠米", "Yuumi"),
                ("金克丝", "Jinx"),
                ("亚索", "Yasuo"),
                ("阿狸", "Ahri"),
                ("李青", "LeeSin"),
            ],
            selected_idx: None,
        }
    }
}

impl Render for BenchOverlayView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let champs = self.bench_champions.clone();
        let selected = self.selected_idx;

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgba(0x0f172acc)) // 游戏内悬浮深色高对比半透底色
            .border_1()
            .border_color(Theme::AURORA_CYAN)
            .rounded_xl()
            .p_3()
            .shadow_lg()
            // 顶部小标题栏
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    .mb_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .size(px(8.0))
                                    .rounded_full()
                                    .bg(Theme::AURORA_CYAN),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xffffff))
                                    .child("大乱斗板凳席 (置顶透明)"),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::AURORA_CYAN)
                            .child("点击一键交换"),
                    ),
            )
            // 板凳英雄 5 槽位横向排开
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .children(champs.iter().enumerate().map(|(idx, (name, en_name))| {
                        let is_active = selected == Some(idx);
                        div()
                            .id(ElementId::NamedInteger("bench-slot".into(), idx as u64))
                            .size(px(54.0))
                            .rounded_lg()
                            .bg(if is_active {
                                rgba(0x00f5d440)
                            } else {
                                rgba(0x2d3748aa)
                            })
                            .border_2()
                            .border_color(if is_active {
                                Theme::AURORA_CYAN
                            } else {
                                Theme::CARD_BORDER
                            })
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(|s| s.bg(rgba(0x00f5d425)).border_color(Theme::AURORA_CYAN))
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _ev, _window, cx| {
                                this.selected_idx = Some(idx);
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xffffff))
                                    .child(*name),
                            )
                            .child(
                                div()
                                    .text_size(px(9.0))
                                    .text_color(Theme::TEXT_MUTED)
                                    .child(*en_name),
                            )
                    })),
            )
    }
}
