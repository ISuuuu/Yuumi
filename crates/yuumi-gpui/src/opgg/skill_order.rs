//! OP.GG 技能加点区块
//!
//! 对照 Vue `src/components/opgg/SkillOrderSection.vue`：
//! 左侧主加技能序列（Q/W/E/R 彩色块 + 「›」分隔），中间升级顺序，
//! 右侧胜率与场次，末位显示登场率。

use crate::opgg::format::{games_count, pct};
use crate::theme::Theme;
use gpui::*;
use yuumi_core::opgg::{SkillMastery, SkillOrder};

/// 技能字母对应的（底色, 文字色, 描边色）
fn skill_palette(key: &str) -> (Hsla, Hsla, Hsla) {
    match key.to_ascii_uppercase().as_str() {
        "Q" => (
            rgba(0x339af014).into(),
            rgb(0x1c7ed6).into(),
            rgba(0x339af026).into(),
        ),
        "W" => (
            rgba(0x37b24d14).into(),
            rgb(0x2b8a3e).into(),
            rgba(0x37b24d26).into(),
        ),
        "E" => (
            rgba(0xf7670714).into(),
            rgb(0xd9480f).into(),
            rgba(0xf7670726).into(),
        ),
        "R" => (
            rgba(0xf03e3e14).into(),
            rgb(0xc92a2a).into(),
            rgba(0xf03e3e26).into(),
        ),
        _ => (
            rgba(0x0000000d).into(),
            Theme::TEXT_SECONDARY(),
            Theme::CARD_BORDER(),
        ),
    }
}

/// 26×26 主加技能块
fn skill_icon_box(key: &str) -> impl IntoElement {
    let (bg, text, border) = skill_palette(key);
    div()
        .flex_none()
        .w(px(26.0))
        .h(px(26.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.0))
        .bg(bg)
        .border_1()
        .border_color(border)
        .text_size(px(12.0))
        .text_color(text)
        .font_weight(if key.eq_ignore_ascii_case("R") {
            FontWeight::EXTRA_BOLD
        } else {
            FontWeight::BOLD
        })
        .child(key.to_ascii_uppercase())
}

/// 18×18 升级顺序块
fn skill_order_box(key: &str) -> impl IntoElement {
    let (bg, text, border) = skill_palette(key);
    div()
        .flex_none()
        .w(px(18.0))
        .h(px(18.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(3.0))
        .bg(bg)
        .border_1()
        .border_color(border)
        .text_size(px(9.92))
        .text_color(text)
        .child(key.to_ascii_uppercase())
}

/// 加点区块；技能与主加数据都缺失时整块不渲染
pub fn skill_order_section(
    skills: &[SkillOrder],
    masteries: &[SkillMastery],
) -> Option<AnyElement> {
    let first_skill = skills.first();
    let first_mastery = masteries.first();
    if first_skill.is_none() && first_mastery.is_none() {
        return None;
    }

    let mut main_row = div().flex().items_center().gap(px(4.0));
    if let Some(mastery) = first_mastery {
        let total = mastery.ids.len();
        for (index, key) in mastery.ids.iter().enumerate() {
            main_row = main_row.child(skill_icon_box(key));
            if index + 1 < total {
                main_row = main_row.child(
                    div()
                        .mx(px(4.0))
                        .text_size(px(13.12))
                        .font_weight(FontWeight::BOLD)
                        .text_color(Theme::TEXT_MUTED())
                        .child("›"),
                );
            }
        }
    }

    let order_row = first_skill
        .filter(|skill| !skill.order.is_empty())
        .map(|skill| {
            div()
                .flex()
                .gap(px(2.0))
                .children(skill.order.iter().map(|key| skill_order_box(key)))
        });

    let stats_row = first_skill.map(|skill| {
        div()
            .flex()
            .flex_col()
            .items_center()
            .ml_auto()
            .child(
                div()
                    .text_size(px(12.48))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(Theme::TEXT_PRIMARY())
                    .child(pct(skill.win_rate)),
            )
            .child(
                div()
                    .text_size(px(10.4))
                    .text_color(Theme::TEXT_MUTED())
                    .child(games_count(skill.play)),
            )
    });

    let pick_row = first_skill.map(|skill| {
        div()
            .min_w(px(40.0))
            .text_right()
            .text_size(px(12.0))
            .font_weight(FontWeight::BOLD)
            .text_color(Theme::TEXT_SECONDARY())
            .child(pct(skill.pick_rate))
    });

    Some(
        div()
            .mx(px(14.0))
            .my(px(10.0))
            .px(px(16.0))
            .py(px(12.0))
            .bg(Theme::CARD_BG())
            .border_1()
            .border_color(Theme::CARD_BORDER())
            .rounded(px(8.0))
            .shadow_sm()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .child(main_row)
                    .children(order_row)
                    .children(stats_row)
                    .children(pick_row),
            )
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::skill_palette;
    use crate::theme::Theme;

    // 顶层 `use gpui::*` 把 gpui_macros::test 引入了作用域，会遮蔽内置 #[test]，
    // 必须用全限定路径标注普通单测
    #[::core::prelude::v1::test]
    fn skill_palette_covers_all_four_abilities() {
        // Q/W/E/R 各自应有独立文字色，未知字母退回次级文字色
        let q = skill_palette("q").1;
        let r = skill_palette("R").1;
        assert_ne!(q, r);
        assert_eq!(skill_palette("X").1, Theme::TEXT_SECONDARY());
    }
}
