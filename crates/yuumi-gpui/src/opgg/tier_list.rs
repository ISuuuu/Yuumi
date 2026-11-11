//! OP.GG 英雄梯队榜单
//!
//! 对照 Vue `src/components/opgg/TierListPanel.vue`：
//! 表头依次为序号、英雄、Tier、胜率、登场率、禁用率、KDA、劣势对抗；
//! 卡片行按 Tier 分级着色，整行可点进入详情。
//!
//! Tier 徽章在 Vue 中是 `tier-1..4.svg`（纯色盾牌 + 白色数字）。
//! GPUI 的 SVG 渲染只保留 alpha 蒙版并统一着色，会把白色数字一起吞掉，
//! 因此这里用「单色盾牌 + 叠加数字文本」还原同一视觉。

use crate::components::lcu_image::LcuImage;
use crate::opgg::format::{champion_icon, fmt_kda, pct};
use crate::opgg::{tier_badge_color, tier_card_bg, tier_card_border};
use crate::theme::Theme;
use gpui::*;
use std::collections::HashMap;
use yuumi_core::opgg::TierListEntry;

/// 序号列宽
const COL_RANK: f32 = 30.0;
/// Tier 列宽
const COL_TIER: f32 = 50.0;
/// 统计列宽
const COL_STAT: f32 = 65.0;
/// 克制列宽
const COL_COUNTERS: f32 = 90.0;

/// 卡片悬停底色：Vue 用 `filter: brightness(0.93)`，GPUI 无滤镜，
/// 改为把同级背景色预乘 0.93，效果等价且不穿透到窗口背景。
fn card_hover_bg(tier: Option<&str>) -> Hsla {
    match tier {
        Some("0") | Some("1") => rgb(0xbfd4e6).into(),
        Some("2") => rgb(0xbedbd9).into(),
        Some("3") => rgb(0xe3dac2).into(),
        Some("4") | Some("5") => rgb(0xd5d8dc).into(),
        _ => rgba(0x0000000a).into(),
    }
}

/// Tier 盾牌徽章；接口返回非数字分级时退回纯文本徽章
fn tier_badge(tier: Option<&str>) -> AnyElement {
    let (icon_path, digit) = match tier {
        Some("0") | Some("1") => ("icons/tier-1.svg", "1"),
        Some("2") => ("icons/tier-2.svg", "2"),
        Some("3") => ("icons/tier-3.svg", "3"),
        Some("4") | Some("5") => ("icons/tier-4.svg", "4"),
        _ => {
            return div()
                .w(px(COL_TIER))
                .flex_none()
                .text_center()
                .text_size(px(11.2))
                .font_weight(FontWeight::BOLD)
                .text_color(Theme::TEXT_SECONDARY())
                .child(tier.unwrap_or("-").to_string())
                .into_any_element();
        }
    };

    div()
        .relative()
        .size(px(24.0))
        .flex_none()
        .child(
            svg()
                .path(icon_path)
                .size_full()
                .text_color(tier_badge_color(tier)),
        )
        .child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .pb(px(1.0))
                .text_size(px(11.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xffffff))
                .child(digit),
        )
        .into_any_element()
}

fn header_cell(label: &'static str, width: f32) -> impl IntoElement {
    div().w(px(width)).flex_none().text_center().child(label)
}

/// 表头
fn header() -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .px(px(10.0))
        .py(px(6.0))
        .mb(px(3.0))
        .text_size(px(11.52))
        .font_weight(FontWeight::BOLD)
        .text_color(Theme::TEXT_SECONDARY())
        .border_b_1()
        .border_color(Theme::CARD_BORDER())
        .child(header_cell("#", COL_RANK))
        .child(div().flex_1().child("英雄"))
        .child(div().flex_1())
        .child(header_cell("Tier", COL_TIER))
        .child(header_cell("胜率", COL_STAT))
        .child(header_cell("登场率", COL_STAT))
        .child(header_cell("禁用率", COL_STAT))
        .child(header_cell("KDA", COL_STAT))
        .child(header_cell("劣势对抗", COL_COUNTERS))
}

/// 榜单主体（表头 + 卡片列表）
pub fn tier_list(
    entries: &[TierListEntry],
    champion_names: &HashMap<i32, String>,
    on_select: impl Fn(i32, &mut Window, &mut App) + 'static + Clone,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(3.0))
        .child(header())
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(3.0))
                .children(entries.iter().enumerate().map(|(index, entry)| {
                    let tier = entry.stats.tier.as_deref();
                    let name = champion_names
                        .get(&entry.champion_id)
                        .cloned()
                        .unwrap_or_else(|| entry.name.clone());
                    let champion_id = entry.champion_id;
                    let select = on_select.clone();
                    let background = tier_card_bg(tier);
                    let hover_background = card_hover_bg(tier);

                    div()
                        .id(SharedString::from(format!("opgg-tier-{champion_id}")))
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .px(px(14.0))
                        .py(px(12.0))
                        .border_1()
                        .border_color(tier_card_border())
                        .rounded(px(6.0))
                        .bg(background)
                        .text_size(px(12.48))
                        .text_color(Theme::TEXT_SECONDARY())
                        .cursor_pointer()
                        .hover(move |style| style.bg(hover_background))
                        .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                            select(champion_id, window, app)
                        })
                        // 序号
                        .child(
                            div()
                                .w(px(COL_RANK))
                                .flex_none()
                                .text_center()
                                .text_color(Theme::TEXT_MUTED())
                                .child((index + 1).to_string()),
                        )
                        .child(
                            LcuImage::new(champion_icon(champion_id))
                                .size(px(34.0))
                                .circle()
                                .border_color(Theme::CARD_BORDER())
                                .border_width(px(2.0)),
                        )
                        .child(
                            div()
                                .flex_none()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(Theme::TEXT_PRIMARY())
                                .child(name),
                        )
                        .child(div().flex_1())
                        .child(tier_badge(tier))
                        .child(stat_cell(pct(entry.stats.win_rate)))
                        .child(stat_cell(pct(entry.stats.pick_rate)))
                        .child(stat_cell(pct(entry.stats.ban_rate)))
                        .child(stat_cell(fmt_kda(entry.stats.kda)))
                        .child(
                            div()
                                .w(px(COL_COUNTERS))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .gap(px(3.0))
                                .children(entry.counters.iter().take(3).map(|counter_id| {
                                    LcuImage::new(champion_icon(*counter_id))
                                        .size(px(22.0))
                                        .circle()
                                        .border_color(Theme::CARD_BORDER())
                                })),
                        )
                })),
        )
        .into_any_element()
}

fn stat_cell(text: String) -> impl IntoElement {
    div().w(px(COL_STAT)).flex_none().text_center().child(text)
}
