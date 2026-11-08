use gpui::*;
use crate::components::badge::Badge;
use crate::components::card::Card;
use crate::theme;

/// 已保存的路人记录
#[derive(Debug, Clone)]
pub struct SavedPlayerItem {
    pub summoner_name: String,
    pub tag: String,
    pub encounter_count: u32,
    pub note: String,
    pub is_pinned: bool,
}

/// 路人集主视图
pub struct SavedPlayersView {
    pub players: Vec<SavedPlayerItem>,
}

impl SavedPlayersView {
    pub fn new() -> Self {
        Self {
            players: vec![
                SavedPlayerItem {
                    summoner_name: "稳健打野#CN1".to_string(),
                    tag: "大腿".to_string(),
                    encounter_count: 5,
                    note: "控龙意识极好，多帮下路".to_string(),
                    is_pinned: true,
                },
                SavedPlayerItem {
                    summoner_name: "搞心态上单#CN2".to_string(),
                    tag: "避雷".to_string(),
                    encounter_count: 3,
                    note: "容易红温挂机，不要互动".to_string(),
                    is_pinned: false,
                },
                SavedPlayerItem {
                    summoner_name: "好脾气辅助#CN1".to_string(),
                    tag: "好友候选".to_string(),
                    encounter_count: 2,
                    note: "软辅玩得很好，开麦指挥温和".to_string(),
                    is_pinned: false,
                },
            ],
        }
    }

    pub fn render(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme::BG_MAIN)
            .p_4()
            .gap_4()
            .overflow_hidden()
            // 顶部操作栏与统计
            .child(
                Card::new()
                    .p(px(14.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme::TEXT_PRIMARY)
                                            .child("路人集管理"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme::TEXT_MUTED)
                                            .child(format!("共记录 {} 位玩家", self.players.len())),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(Badge::new("全部标签").ghost())
                                    .child(Badge::new("置顶玩家").ghost())
                                    .child(Badge::new("导入 / 导出").ghost()),
                            ),
                    ),
            )
            // 玩家网格列表
            .child(
                div()
                    .id("saved-players-scroll")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .gap_3()
                    .overflow_y_scroll()
                    .children(self.players.iter().map(|p| {
                        let tag_color = match p.tag.as_str() {
                            "大腿" => theme::WIN_PRIMARY,
                            "避雷" => theme::LOSS_PRIMARY,
                            _ => theme::PRIMARY,
                        };

                        Card::new()
                            .p_4()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .child(if p.is_pinned {
                                                        div()
                                                            .text_xs()
                                                            .text_color(theme::GOLD)
                                                            .child("★")
                                                    } else {
                                                        div()
                                                    })
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(theme::TEXT_PRIMARY)
                                                            .child(p.summoner_name.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .px_2()
                                                            .py_0p5()
                                                            .rounded(px(4.0))
                                                            .bg(tag_color.opacity(0.15))
                                                            .text_xs()
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .text_color(tag_color)
                                                            .child(p.tag.clone()),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(theme::TEXT_SECONDARY)
                                                    .child(p.note.clone()),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_3()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(theme::TEXT_MUTED)
                                                    .child(format!("相遇 {} 局", p.encounter_count)),
                                            )
                                            .child(
                                                div()
                                                    .px_3()
                                                    .py_1()
                                                    .bg(theme::BG_CARD_HOVER)
                                                    .border_1()
                                                    .border_color(theme::BORDER_SECONDARY)
                                                    .rounded(px(6.0))
                                                    .text_xs()
                                                    .text_color(theme::TEXT_PRIMARY)
                                                    .cursor_pointer()
                                                    .child("查看战绩"),
                                            ),
                                    ),
                            )
                    })),
            )
    }
}
