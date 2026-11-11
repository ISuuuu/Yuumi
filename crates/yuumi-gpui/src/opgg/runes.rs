//! OP.GG 符文推荐区块
//!
//! 对照 Vue `src/components/opgg/RunesSection.vue`：左侧主系 / 副系 / 属性碎片三列，
//! 右侧最多三套方案切换与「设为当前符文页」按钮。
//!
//! 视觉近似说明：Vue 对未选中的符文用 `filter: grayscale(1)` 去色，
//! GPUI 无 CSS 滤镜能力，这里以 `opacity(0.35)`（碎片 0.3）降低存在感，
//! 选中态仍按 Vue 的琥珀描边 + 浅底 + 高亮还原。

use crate::components::lcu_image::LcuImage;
use crate::opgg::format::{games_count, pct, rune_icon};
use crate::opgg::{Palette, PerkStyle, RuneCatalog};
use crate::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::rc::Rc;
use yuumi_core::lcu::game_data::GameDataAssets;
use yuumi_core::opgg::RunePreset;

/// 卡片外框（与详情页其余卡片一致：margin 10 14 / padding 12 16）
fn card() -> Div {
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
}

/// 单个符文格
fn rune_cell(
    perk_id: i32,
    size: f32,
    active: bool,
    catalog: &RuneCatalog,
    assets: Option<&GameDataAssets>,
) -> impl IntoElement {
    let icon_size = size * 0.8;
    // tooltip 回调要求 'static，提示文本预先算好再移入闭包
    let tooltip_text = catalog.tooltip(perk_id);
    let icon = rune_icon(assets, catalog, perk_id);
    div()
        .id(SharedString::from(format!("opgg-rune-{perk_id}")))
        .flex_none()
        .size(px(size))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(if active {
            rgba(0xe5a93b1a)
        } else {
            rgba(0x0000000d)
        })
        .border_1()
        .border_color(if active {
            Palette::rune_active()
        } else {
            rgba(0x00000000).into()
        })
        .opacity(if active { 1.0 } else { 0.35 })
        .when(active, |style| style.shadow_sm())
        .tooltip(move |_window, cx| {
            let text = tooltip_text.clone();
            cx.new(|_| TooltipLabel { text }).into()
        })
        .child(
            LcuImage::new(icon)
                .size(px(icon_size))
                .circle()
                .border_width(px(0.0)),
        )
}

/// 属性碎片格
fn shard_cell(
    perk_id: i32,
    active: bool,
    catalog: &RuneCatalog,
    assets: Option<&GameDataAssets>,
) -> impl IntoElement {
    let tooltip_text = catalog.tooltip(perk_id);
    let icon = rune_icon(assets, catalog, perk_id);
    div()
        .id(SharedString::from(format!("opgg-shard-{perk_id}")))
        .flex_none()
        .size(px(18.0))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(if active {
            rgba(0x51cf6626)
        } else {
            rgba(0x00000014)
        })
        .border_1()
        .border_color(if active {
            Palette::shard_active()
        } else {
            rgba(0x00000000).into()
        })
        .opacity(if active { 1.0 } else { 0.3 })
        .tooltip(move |_window, cx| {
            let text = tooltip_text.clone();
            cx.new(|_| TooltipLabel { text }).into()
        })
        .child(
            LcuImage::new(icon)
                .size(px(14.0))
                .circle()
                .border_width(px(0.0)),
        )
}

/// 符文悬停提示文本（对照 Vue 的 `title` 属性：名称 + 说明）
struct TooltipLabel {
    text: String,
}

impl Render for TooltipLabel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .max_w(px(280.0))
            .px(px(8.0))
            .py(px(6.0))
            .bg(rgb(0x0f172a))
            .rounded(px(6.0))
            .text_size(px(11.5))
            .text_color(rgb(0xf8fafc))
            .child(self.text.clone())
    }
}

/// 符文树列的插槽区
fn slot_rows(
    style: &PerkStyle,
    slot_start: usize,
    slot_end: usize,
    active_ids: &[i32],
    keystone_first: bool,
    catalog: &RuneCatalog,
    assets: Option<&GameDataAssets>,
) -> impl IntoElement {
    let slots: Vec<&crate::opgg::PerkSlot> = style
        .slots
        .iter()
        .enumerate()
        .filter(|(index, _)| *index >= slot_start && *index < slot_end)
        .map(|(_, slot)| slot)
        .collect();

    div().flex().flex_col().gap(px(10.0)).children(
        slots.into_iter().enumerate().map(|(offset, slot)| {
            let is_keystone = keystone_first && offset == 0;
            let size = if is_keystone { 38.0 } else { 28.0 };
            div()
                .flex()
                .items_center()
                .justify_center()
                .gap(px(8.0))
                .children(slot.perks.iter().map(|perk_id| {
                    rune_cell(
                        *perk_id,
                        size,
                        active_ids.contains(perk_id),
                        catalog,
                        assets,
                    )
                }))
        }),
    )
}

/// 符文区块；数据不足（缺少方案或主副系）时整块不渲染
pub fn runes_section(
    runes: &[RunePreset],
    rune_index: usize,
    catalog: &RuneCatalog,
    assets: Option<&GameDataAssets>,
    on_select: Rc<dyn Fn(usize, &mut Window, &mut App)>,
    on_apply: Rc<dyn Fn(&mut Window, &mut App)>,
) -> Option<AnyElement> {
    let active = runes.get(rune_index).or_else(|| runes.first())?;
    let primary = catalog.style(active.primary_page_id)?;
    let secondary = catalog.style(active.secondary_page_id)?;

    let left = div()
        .flex()
        .flex_1()
        .gap(px(20.0))
        .justify_around()
        .items_start()
        // 主系：四个插槽（第一个为基石）
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .child(
                    div()
                        .h(px(36.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .mb(px(12.0))
                        .child(
                            LcuImage::new(primary.icon_path.clone())
                                .size(px(24.0))
                                .border_width(px(0.0)),
                        ),
                )
                .child(slot_rows(
                    primary,
                    0,
                    4,
                    &active.primary_rune_ids,
                    true,
                    catalog,
                    assets,
                )),
        )
        // 副系：跳过基石
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .child(
                    div()
                        .h(px(36.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .mb(px(12.0))
                        .child(
                            LcuImage::new(secondary.icon_path.clone())
                                .size(px(24.0))
                                .border_width(px(0.0)),
                        ),
                )
                .child(slot_rows(
                    secondary,
                    1,
                    4,
                    &active.secondary_rune_ids,
                    false,
                    catalog,
                    assets,
                )),
        )
        // 属性碎片：主系末三个插槽
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .child(div().h(px(36.0)).mb(px(12.0)))
                .child(
                    div().flex().flex_col().gap(px(10.0)).children(
                        primary
                            .slots
                            .iter()
                            .skip(4)
                            .take(3)
                            .map(|slot| {
                                div().flex().items_center().justify_center().gap(px(8.0)).children(
                                    slot.perks.iter().map(|perk_id| {
                                        shard_cell(
                                            *perk_id,
                                            active.stat_mod_ids.contains(perk_id),
                                            catalog,
                                            assets,
                                        )
                                    }),
                                )
                            }),
                    ),
                ),
        );

    let presets = div().flex().flex_col().gap(px(6.0)).children(
        runes
            .iter()
            .take(3)
            .enumerate()
            .map(|(index, preset)| {
                let is_active = index == rune_index;
                let select = on_select.clone();
                div()
                    .id(SharedString::from(format!("opgg-rune-preset-{index}")))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(8.0))
                    .py(px(6.0))
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(if is_active {
                        Theme::ACCENT_PRIMARY()
                    } else {
                        Theme::CARD_BORDER()
                    })
                    .bg(if is_active {
                        Theme::ACCENT_LIGHT()
                    } else {
                        Theme::BG_APP()
                    })
                    .cursor_pointer()
                    .hover(|style| style.border_color(Theme::ACCENT_PRIMARY()))
                    .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                        select(index, window, app)
                    })
                    .child(
                        LcuImage::new(rune_icon(assets, catalog, preset.primary_page_id))
                            .size(px(20.0))
                            .circle()
                            .border_width(px(0.0)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap(px(1.0))
                            .child(
                                div()
                                    .text_size(px(11.52))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY())
                                    .child(pct(preset.win_rate)),
                            )
                            .child(
                                div()
                                    .text_size(px(9.6))
                                    .text_color(Theme::TEXT_MUTED())
                                    .child(games_count(preset.play)),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(11.52))
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::TEXT_SECONDARY())
                            .child(pct(preset.pick_rate)),
                    )
            }),
    );

    let apply_button = div()
        .id("opgg-apply-rune")
        .w_full()
        .py(px(8.0))
        .bg(Palette::tab_active())
        .rounded(px(6.0))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(0xffffff))
        .cursor_pointer()
        .hover(|style| style.bg(Palette::tab_active_hover()))
        .on_mouse_down(MouseButton::Left, move |_ev, window, app| on_apply(window, app))
        .child("设为当前符文页");

    Some(
        card()
            .child(
                div()
                    .flex()
                    .gap(px(16.0))
                    .p(px(16.0))
                    .bg(Palette::rune_tree_bg())
                    .border_1()
                    .border_color(Theme::CARD_BORDER())
                    .rounded(px(6.0))
                    .child(left)
                    .child(
                        div()
                            .w(px(170.0))
                            .flex_none()
                            .pl(px(16.0))
                            .border_l_1()
                            .border_color(Theme::CARD_BORDER())
                            .flex()
                            .flex_col()
                            .justify_between()
                            .gap(px(12.0))
                            .child(presets)
                            .child(apply_button),
                    ),
            )
            .into_any_element(),
    )
}
