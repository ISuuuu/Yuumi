//! OP.GG 装备推荐区块
//!
//! 对照 Vue `src/components/opgg/ItemsSection.vue`：
//! 上半区并排展示出门装与鞋子（各最多三组），中间为核心装备图标链（最多五组），
//! 下半区为可选装备横排（最多十项）。

use crate::components::lcu_image::LcuImage;
use crate::opgg::format::{games_count, item_icon, pct};
use crate::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use yuumi_core::lcu::game_data::GameDataAssets;
use yuumi_core::opgg::IdWinRate;

/// 单个装备图标
fn item_icon_element(assets: Option<&GameDataAssets>, item_id: i32) -> LcuImage {
    LcuImage::new(item_icon(assets, item_id))
        .size(px(26.0))
        .border_color(Theme::CARD_BORDER())
}

/// 出门装/鞋子条目：图标 + 胜率 + 场次
fn group_entry(assets: Option<&GameDataAssets>, entry: &IdWinRate) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(4.0))
        .children(
            entry
                .ids
                .iter()
                .map(|item_id| item_icon_element(assets, *item_id)),
        )
        .child(
            div()
                .ml_auto()
                .text_size(px(11.52))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(Theme::TEXT_PRIMARY())
                .child(pct(entry.win_rate)),
        )
        .child(
            div()
                .min_w(px(50.0))
                .text_size(px(9.92))
                .text_color(Theme::TEXT_MUTED())
                .child(games_count(entry.play)),
        )
}

fn group_column(assets: Option<&GameDataAssets>, groups: &[IdWinRate]) -> impl IntoElement {
    div().flex_1().flex().flex_col().gap(px(4.0)).children(
        groups
            .iter()
            .take(3)
            .map(|entry| group_entry(assets, entry)),
    )
}

fn horizontal_divider() -> impl IntoElement {
    div().h(px(1.0)).my(px(6.0)).bg(Theme::CARD_BORDER())
}

/// 装备区块；四类装备数据都为空时不渲染
pub fn items_section(
    starter_items: &[IdWinRate],
    core_items: &[IdWinRate],
    boots: &[IdWinRate],
    last_items: &[IdWinRate],
    assets: Option<&GameDataAssets>,
) -> Option<AnyElement> {
    if starter_items.is_empty()
        && core_items.is_empty()
        && boots.is_empty()
        && last_items.is_empty()
    {
        return None;
    }

    let top_row = (!starter_items.is_empty() || !boots.is_empty()).then(|| {
        div()
            .flex()
            .child(group_column(assets, starter_items))
            .child(div().w(px(1.0)).mx(px(8.0)).bg(Theme::CARD_BORDER()))
            .child(group_column(assets, boots))
    });

    let core_rows = core_items.iter().take(5).map(|entry| {
        let chain = div().flex().items_center().gap(px(3.0)).children(
            entry.ids.iter().enumerate().flat_map(|(index, item_id)| {
                let mut children: Vec<AnyElement> =
                    vec![item_icon_element(assets, *item_id).into_any_element()];
                if index + 1 < entry.ids.len() {
                    children.push(
                        div()
                            .mx(px(2.0))
                            .text_size(px(11.2))
                            .text_color(Theme::TEXT_MUTED())
                            .child("›")
                            .into_any_element(),
                    );
                }
                children
            }),
        );

        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .py(px(3.0))
            .child(chain)
            .child(
                div()
                    .ml_auto()
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(11.52))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(Theme::TEXT_PRIMARY())
                            .child(pct(entry.win_rate)),
                    )
                    .child(
                        div()
                            .text_size(px(9.92))
                            .text_color(Theme::TEXT_MUTED())
                            .child(games_count(entry.play)),
                    ),
            )
            .child(
                div()
                    .min_w(px(40.0))
                    .text_right()
                    .text_size(px(11.52))
                    .font_weight(FontWeight::BOLD)
                    .text_color(Theme::TEXT_SECONDARY())
                    .child(pct(entry.pick_rate)),
            )
    });

    let last_row = (!last_items.is_empty()).then(|| {
        div()
            .flex()
            .flex_wrap()
            .gap(px(4.0))
            .children(last_items.iter().take(10).filter_map(|entry| {
                entry
                    .ids
                    .first()
                    .map(|item_id| item_icon_element(assets, *item_id))
            }))
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
            .children(top_row)
            .when(!core_items.is_empty(), |el| {
                el.child(horizontal_divider()).children(core_rows)
            })
            .when(!last_items.is_empty(), |el| {
                el.child(horizontal_divider()).children(last_row)
            })
            .into_any_element(),
    )
}
