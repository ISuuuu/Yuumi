//! OP.GG 符文出装详情
//!
//! 对照 Vue `src/components/opgg/BuildDetailPanel.vue`，按 Vue 的渲染顺序拼接：
//! 英雄摘要 → 召唤师技能 → 符文 → 技能加点 → 应用出装按钮 → 装备 → 克制关系。
//! 每个模块随返回数据独立显隐，稀疏详情下只剩有数据的区块。

use crate::components::lcu_image::LcuImage;
use crate::opgg::format::{champion_icon, games_count, pct, spell_icon};
use crate::opgg::{items, runes, skill_order};
use crate::opgg::{tier_badge_color, tier_title_bg, tier_title_border, Palette, RuneCatalog};
use crate::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::collections::HashMap;
use std::rc::Rc;
use yuumi_core::lcu::game_data::GameDataAssets;
use yuumi_core::opgg::{ChampionBuild, CounterEntry, Stats};

/// 详情渲染所需的上下文
pub struct BuildDetailContext<'a> {
    pub build: &'a ChampionBuild,
    pub champion_id: Option<i32>,
    pub position: Option<&'static str>,
    pub champion_names: &'a HashMap<i32, String>,
    pub assets: Option<&'a GameDataAssets>,
    pub catalog: &'a RuneCatalog,
    pub rune_index: usize,
    pub applying_items: bool,
    pub on_select_rune: Rc<dyn Fn(usize, &mut Window, &mut App)>,
    pub on_apply_rune: Rc<dyn Fn(&mut Window, &mut App)>,
    pub on_apply_items: Rc<dyn Fn(&mut Window, &mut App)>,
}

/// 通用卡片外框
fn card() -> Div {
    div()
        .mx(px(14.0))
        .my(px(10.0))
        .px(px(16.0))
        .py(px(12.0))
        .bg(Theme::CARD_BG)
        .border_1()
        .border_color(Theme::CARD_BORDER)
        .rounded(px(8.0))
        .shadow_sm()
}

/// 单列统计（标签 + 数值）
fn stat_column(label: &'static str, value: String) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(1.0))
        .child(
            div()
                .text_size(px(10.4))
                .text_color(Theme::TEXT_MUTED)
                .child(label),
        )
        .child(
            div()
                .text_size(px(13.12))
                .font_weight(FontWeight::BOLD)
                .text_color(Theme::TEXT_PRIMARY)
                .child(value),
        )
}

fn stat_divider() -> impl IntoElement {
    div().w(px(1.0)).h(px(24.0)).bg(Theme::CARD_BORDER)
}

/// 英雄摘要标题栏
fn title_bar(ctx: &BuildDetailContext<'_>) -> Option<AnyElement> {
    let summary = ctx.build.summary.as_ref()?;
    let stats: Option<&Stats> = summary.stats_for(ctx.position);
    let champion_id = summary.champion_id.or(ctx.champion_id);
    let name = champion_id
        .and_then(|id| ctx.champion_names.get(&id).cloned())
        .or_else(|| summary.name.clone())
        .unwrap_or_else(|| "英雄".to_string());
    let tier = stats.and_then(|s| s.tier.clone());

    let mut bar = div()
        .mx(px(14.0))
        .my(px(10.0))
        .px(px(18.0))
        .py(px(14.0))
        .flex()
        .items_center()
        .gap(px(12.0))
        .bg(tier_title_bg(tier.as_deref()))
        .border_1()
        .border_color(tier_title_border())
        .rounded(px(8.0))
        .shadow_sm();

    if let Some(id) = champion_id {
        bar = bar.child(
            LcuImage::new(champion_icon(id))
                .size(px(46.0))
                .circle()
                .border_color(Theme::CARD_BORDER)
                .border_width(px(2.0)),
        );
    }

    bar = bar.child(
        div()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .min_w(px(60.0))
            .child(
                div()
                    .text_size(px(15.2))
                    .font_weight(FontWeight::BOLD)
                    .text_color(Theme::TEXT_PRIMARY)
                    .child(name),
            )
            .children(ctx.position.map(|position| {
                div()
                    .text_size(px(11.2))
                    .text_color(Theme::TEXT_MUTED)
                    .child(position.to_string())
            })),
    );

    bar = bar.child(
        div()
            .ml_auto()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(stat_column(
                "胜率",
                pct(stats.and_then(|s| s.win_rate)),
            ))
            .child(stat_divider())
            .child(stat_column(
                "登场率",
                pct(stats.and_then(|s| s.pick_rate)),
            ))
            .child(stat_divider())
            .child(stat_column(
                "禁用率",
                pct(stats.and_then(|s| s.ban_rate)),
            )),
    );

    if let Some(tier) = tier {
        bar = bar.child(
            div().ml(px(8.0)).child(
                div()
                    .px(px(10.0))
                    .py(px(2.0))
                    .rounded(px(4.0))
                    .bg(tier_badge_color(Some(tier.as_str())))
                    .text_size(px(13.12))
                    .font_weight(FontWeight::EXTRA_BOLD)
                    .text_color(rgb(0xffffff))
                    .child(tier),
            ),
        );
    }

    Some(bar.into_any_element())
}

/// 召唤师技能：并排最多两组
fn summoner_spells(ctx: &BuildDetailContext<'_>) -> Option<AnyElement> {
    if ctx.build.summoner_spells.is_empty() {
        return None;
    }
    let pairs = ctx
        .build
        .summoner_spells
        .iter()
        .take(2)
        .map(|entry| {
            div()
                .flex_1()
                .flex()
                .items_center()
                .gap(px(12.0))
                .px(px(12.0))
                .py(px(6.0))
                .bg(Palette::spell_pair_bg())
                .border_1()
                .border_color(Theme::CARD_BORDER)
                .rounded(px(6.0))
                .child(
                    div()
                        .flex()
                        .gap(px(4.0))
                        .children(entry.ids.iter().take(2).map(|spell_id| {
                            LcuImage::new(spell_icon(ctx.assets, *spell_id))
                                .size(px(28.0))
                                .border_color(Theme::CARD_BORDER)
                        })),
                )
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap(px(1.0))
                        .child(
                            div()
                                .text_size(px(12.48))
                                .font_weight(FontWeight::BOLD)
                                .text_color(Theme::TEXT_PRIMARY)
                                .child(pct(entry.win_rate)),
                        )
                        .child(
                            div()
                                .text_size(px(9.92))
                                .text_color(Theme::TEXT_MUTED)
                                .child(games_count(entry.play)),
                        ),
                )
                .child(
                    div()
                        .text_right()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(Theme::TEXT_SECONDARY)
                        .child(pct(entry.pick_rate)),
                )
        });

    Some(card().child(div().flex().gap(px(12.0)).children(pairs)).into_any_element())
}

/// 克制 / 被克双列
fn counters(ctx: &BuildDetailContext<'_>) -> Option<AnyElement> {
    if ctx.build.counters.is_empty() {
        return None;
    }
    let strong = ctx.build.strong_counters();
    let weak = ctx.build.weak_counters();

    fn column(
        title: &'static str,
        entries: Vec<&CounterEntry>,
        names: &HashMap<i32, String>,
    ) -> Div {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(
                div()
                    .mb(px(4.0))
                    .text_size(px(11.2))
                    .font_weight(FontWeight::BOLD)
                    .text_color(Theme::TEXT_MUTED)
                    .child(title),
            )
            .children(entries.into_iter().map(|entry| {
                let name = names
                    .get(&entry.champion_id)
                    .cloned()
                    .or_else(|| entry.name.clone())
                    .unwrap_or_else(|| entry.champion_id.to_string());
                let rate = entry.rate;
                div()
                    .h(px(28.0))
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(5.0))
                            .child(
                                LcuImage::new(champion_icon(entry.champion_id))
                                    .size(px(22.0))
                                    .circle()
                                    .border_color(Theme::CARD_BORDER),
                            )
                            .child(
                                div()
                                    .text_size(px(11.52))
                                    .text_color(Theme::TEXT_SECONDARY)
                                    .child(name),
                            ),
                    )
                    .child(
                        div()
                            .ml_auto()
                            .min_w(px(45.0))
                            .text_center()
                            .text_size(px(9.92))
                            .text_color(Theme::TEXT_MUTED)
                            .child(games_count(entry.play)),
                    )
                    .child(
                        div()
                            .min_w(px(40.0))
                            .text_right()
                            .text_size(px(12.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(match rate {
                                Some(value) if value >= 0.5 => Palette::win_text(),
                                _ => Palette::loss_text(),
                            })
                            .child(pct(rate)),
                    )
            }))
    }

    Some(
        card()
            .child(
                div()
                    .flex()
                    .child(column("克制", strong, ctx.champion_names))
                    .child(
                        div()
                            .w(px(1.0))
                            .mx(px(8.0))
                            .bg(Theme::CARD_BORDER),
                    )
                    .child(column("被克制", weak, ctx.champion_names)),
            )
            .into_any_element(),
    )
}

/// 应用出装按钮栏
fn apply_items_bar(ctx: &BuildDetailContext<'_>) -> Option<AnyElement> {
    if !ctx.build.has_item_data() {
        return None;
    }
    let applying = ctx.applying_items;
    let on_apply = ctx.on_apply_items.clone();
    Some(
        div()
            .flex()
            .justify_end()
            .mx(px(14.0))
            .mt(px(12.0))
            .mb(px(-2.0))
            .child(
                div()
                    .id("opgg-apply-items")
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(14.0))
                    .py(px(6.0))
                    .bg(if applying {
                        Palette::tab_active_hover()
                    } else {
                        Palette::tab_active()
                    })
                    .rounded(px(6.0))
                    .shadow_sm()
                    .text_size(px(12.48))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0xffffff))
                    .when(applying, |style| style.opacity(0.6))
                    .when(!applying, |style| {
                        style
                            .cursor_pointer()
                            .hover(|hover| hover.bg(Palette::tab_active_hover()))
                            .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                                on_apply(window, app)
                            })
                    })
                    .child(if applying {
                        "正在应用出装..."
                    } else {
                        "一键应用出装到游戏推荐页"
                    }),
            )
            .into_any_element(),
    )
}

/// 详情主体
pub fn build_detail(ctx: BuildDetailContext<'_>) -> AnyElement {
    let rune_block = runes::runes_section(
        &ctx.build.runes,
        ctx.rune_index,
        ctx.catalog,
        ctx.assets,
        ctx.on_select_rune.clone(),
        ctx.on_apply_rune.clone(),
    );
    let skill_block = skill_order::skill_order_section(&ctx.build.skills, &ctx.build.skill_masteries);
    let item_block = items::items_section(
        &ctx.build.starter_items,
        &ctx.build.core_items,
        &ctx.build.boots,
        &ctx.build.last_items,
        ctx.assets,
    );

    div()
        .flex()
        .flex_col()
        .pb(px(10.0))
        .children(title_bar(&ctx))
        .children(summoner_spells(&ctx))
        .children(rune_block)
        .children(skill_block)
        .children(apply_items_bar(&ctx))
        .children(item_block)
        .children(counters(&ctx))
        .into_any_element()
}
