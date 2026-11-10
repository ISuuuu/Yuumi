//! 云顶之弈视图：生涯（段位 + 近期战绩）、阵容推荐、海克斯强化三个页签。
//!
//! 对照 Vue `src/views/TFT.vue` 与 `src/components/tft/*`：
//! - 生涯页：单排 / 狂暴 / 双人 / 汇总四张卡 + 八人对局卡，点击展开结算榜；
//! - 阵容推荐：OP.GG 热门阵容卡片、详情棋盘（4×7）、上一套 / 下一套、复制阵容码；
//! - 海克斯：搜索、等级筛选（银 / 金 / 棱彩）、刷新、加载更多。
//!
//! 本文件只负责渲染与本地交互状态，数据请求在 `app_window::tft`。

use crate::components::lcu_image::LcuImage;
use crate::components::text_field::{text_field, TextFieldOptions};
use crate::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::rc::Rc;
use yuumi_core::parsers::tft::{
    MetaDeck, MetaDeckUnit, TftAugmentInfo, TftMatch, TftMatchSummary, TftRankStats, TftTrait,
    TftUnit,
};

/// 顶部页签
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TftTab {
    Career,
    Comps,
    Augments,
}

impl TftTab {
    pub fn label(&self) -> &'static str {
        match self {
            TftTab::Career => "云顶战绩",
            TftTab::Comps => "阵容推荐",
            TftTab::Augments => "海克斯强化",
        }
    }

    const ALL: [TftTab; 3] = [TftTab::Career, TftTab::Comps, TftTab::Augments];
}

/// 海克斯等级筛选
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AugTierFilter {
    All,
    Silver,
    Gold,
    Prismatic,
}

impl AugTierFilter {
    fn matches(&self, tier: i32) -> bool {
        match self {
            AugTierFilter::All => true,
            AugTierFilter::Silver => tier == 1,
            AugTierFilter::Gold => tier == 2,
            AugTierFilter::Prismatic => tier == 3,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            AugTierFilter::All => "全部",
            AugTierFilter::Silver => "银色",
            AugTierFilter::Gold => "金色",
            AugTierFilter::Prismatic => "棱彩",
        }
    }
}

/// 视图动作（由主窗分发到数据层）
#[derive(Debug, Clone, PartialEq)]
pub enum TftAction {
    SwitchTab(TftTab),
    /// 战绩刷新 / 失败重试
    RefreshCareer,
    OpenMatch(u64),
    CloseMatchDetail,
    RefreshComps,
    OpenDeck(usize),
    PrevDeck,
    NextDeck,
    CloseDeck,
    CopyTeamCode(String),
    RefreshAugments,
    LoadMoreAugments,
    SetAugmentTier(AugTierFilter),
    QueryAugments(String),
    ClearAugmentQuery,
}

/// 每页海克斯条数（对照 Vue 的 displayLimit 步长）
const AUGMENT_PAGE_SIZE: usize = 40;

pub struct TftView {
    pub active_tab: TftTab,
    pub loading: bool,
    pub error: String,
    pub ranked: Option<TftRankStats>,
    pub summary: Option<TftMatchSummary>,
    /// 客户端版本（`major.minor`）
    pub client_version: String,
    /// 正在查看的对局 gameId（None 表示未展开详情）
    pub detail_game_id: Option<u64>,

    pub comps_loading: bool,
    pub comps_error: String,
    pub decks: Vec<MetaDeck>,
    /// 详情浮层中的阵容下标
    pub deck_detail: Option<usize>,
    pub aug_loading: bool,
    pub aug_error: String,
    pub augments: Vec<TftAugmentInfo>,
    pub aug_query: String,
    pub aug_tier: AugTierFilter,
    pub aug_limit: usize,

    career_scroll: ScrollHandle,
    detail_scroll: ScrollHandle,
    comps_scroll: ScrollHandle,
    deck_scroll: ScrollHandle,
    augments_scroll: ScrollHandle,
}

impl Default for TftView {
    fn default() -> Self {
        Self::new()
    }
}

impl TftView {
    pub fn new() -> Self {
        Self {
            active_tab: TftTab::Career,
            loading: false,
            error: String::new(),
            ranked: None,
            summary: None,
            client_version: String::new(),
            detail_game_id: None,
            comps_loading: false,
            comps_error: String::new(),
            decks: Vec::new(),
            deck_detail: None,
            aug_loading: false,
            aug_error: String::new(),
            augments: Vec::new(),
            aug_query: String::new(),
            aug_tier: AugTierFilter::All,
            aug_limit: AUGMENT_PAGE_SIZE,
            career_scroll: ScrollHandle::new(),
            detail_scroll: ScrollHandle::new(),
            comps_scroll: ScrollHandle::new(),
            deck_scroll: ScrollHandle::new(),
            augments_scroll: ScrollHandle::new(),
        }
    }

    /// 当前筛选后的海克斯列表长度
    fn filtered_augments(&self) -> Vec<&TftAugmentInfo> {
        let query = self.aug_query.trim().to_lowercase();
        self.augments
            .iter()
            .filter(|augment| self.aug_tier.matches(augment.tier))
            .filter(|augment| {
                if query.is_empty() {
                    return true;
                }
                augment.name.to_lowercase().contains(&query)
                    || augment.desc.to_lowercase().contains(&query)
                    || augment.api_name.to_lowercase().contains(&query)
            })
            .collect()
    }

    fn selected_match(&self) -> Option<&TftMatch> {
        let game_id = self.detail_game_id?;
        self.summary
            .as_ref()?
            .matches
            .iter()
            .find(|entry| entry.game_id == game_id)
    }

    /// 点击选局时复位详情滚动位置
    pub fn select_match(&mut self, game_id: u64) {
        self.detail_game_id = Some(game_id);
        self.detail_scroll.set_offset(Point::default());
    }

    pub fn close_match(&mut self) {
        self.detail_game_id = None;
    }

    pub fn select_deck(&mut self, index: usize) {
        self.deck_detail = Some(index);
        self.deck_scroll.set_offset(Point::default());
    }

    pub fn close_deck(&mut self) {
        self.deck_detail = None;
    }

    pub fn shift_deck(&mut self, forward: bool) {
        let Some(current) = self.deck_detail else {
            return;
        };
        if self.decks.is_empty() {
            return;
        }
        let next = if forward {
            (current + 1).min(self.decks.len() - 1)
        } else {
            current.saturating_sub(1)
        };
        self.deck_detail = Some(next);
        self.deck_scroll.set_offset(Point::default());
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        is_connected: bool,
        augment_focus: &FocusHandle,
        augment_focus_active: bool,
        on_action: impl Fn(TftAction, &mut Window, &mut App) + 'static + Clone,
    ) -> AnyElement {
        if !is_connected {
            return self.render_offline();
        }

        let dispatch = on_action.clone();
        let tab_click = on_action.clone();
        let tabs =
            div()
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .mt(px(12.0))
                .mb(px(16.0))
                .pb(px(8.0))
                .border_b_1()
                .border_color(Theme::CARD_BORDER)
                .child(div().flex().items_center().gap(px(8.0)).children(
                    TftTab::ALL.into_iter().map(|tab| {
                        let active = self.active_tab == tab;
                        let cb = tab_click.clone();
                        div()
                            .id(SharedString::from(format!("tft-tab-{}", tab.label())))
                            .px(px(16.0))
                            .py(px(6.0))
                            .rounded(px(8.0))
                            .border_1()
                            .cursor_pointer()
                            .text_size(px(14.0))
                            .font_weight(FontWeight::BOLD)
                            .bg(if active {
                                Theme::ACCENT_ALPHA_10
                            } else {
                                gpui::transparent_black()
                            })
                            .text_color(if active {
                                Theme::ACCENT_PRIMARY
                            } else {
                                Theme::TEXT_MUTED
                            })
                            .border_color(if active {
                                Theme::ACCENT_ALPHA_20
                            } else {
                                gpui::transparent_black()
                            })
                            .when(!active, |this| {
                                this.hover(|style| {
                                    style
                                        .bg(Theme::CARD_BG_HOVER)
                                        .text_color(Theme::ACCENT_PRIMARY)
                                })
                            })
                            .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                                cb(TftAction::SwitchTab(tab), window, app);
                            })
                            .child(tab.label())
                    }),
                ))
                .child(div().flex().items_center().gap(px(8.0)).when(
                    !self.client_version.is_empty(),
                    |this| {
                        this.child(
                            div()
                                .px(px(8.0))
                                .py(px(2.0))
                                .rounded(px(12.0))
                                .bg(Theme::ACCENT_ALPHA_10)
                                .border_1()
                                .border_color(Theme::ACCENT_ALPHA_20)
                                .text_size(px(11.5))
                                .font_weight(FontWeight::BOLD)
                                .text_color(Theme::ACCENT_PRIMARY)
                                .child(format!("v{}", self.client_version)),
                        )
                    },
                ));

        let body = match self.active_tab {
            TftTab::Career => self.render_career(on_action.clone()),
            TftTab::Comps => self.render_comps(on_action.clone()),
            TftTab::Augments => {
                self.render_augments(augment_focus, augment_focus_active, on_action.clone())
            }
        };

        div()
            .relative()
            .size_full()
            .overflow_hidden()
            .flex()
            .flex_col()
            .items_center()
            .px(px(24.0))
            .pt(px(16.0))
            .child(
                div()
                    .relative()
                    .w_full()
                    .max_w(px(1000.0))
                    .h_full()
                    .flex()
                    .flex_col()
                    .min_h_0()
                    .overflow_hidden()
                    .child(tabs)
                    .child(
                        div()
                            .w_full()
                            .flex_1()
                            .min_h_0()
                            .overflow_hidden()
                            .child(body),
                    )
                    .when_some(self.selected_match().cloned(), |this, entry| {
                        this.child(self.render_match_detail(&entry, dispatch.clone()))
                    })
                    .when_some(
                        self.deck_detail.and_then(|index| self.decks.get(index)),
                        |this, deck| this.child(self.render_deck_detail(deck, dispatch.clone())),
                    ),
            )
            .into_any_element()
    }

    fn render_offline(&self) -> AnyElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_size(px(48.0))
                    .line_height(px(48.0))
                    .mb(px(16.0))
                    .child("♟️"),
            )
            .child(
                div()
                    .text_size(px(15.0))
                    .line_height(px(24.0))
                    .text_color(Theme::TEXT_MUTED)
                    .child("请先启动英雄联盟客户端"),
            )
            .into_any_element()
    }

    // ─── 生涯页 ───

    fn render_career(
        &self,
        on_action: impl Fn(TftAction, &mut Window, &mut App) + 'static + Clone,
    ) -> AnyElement {
        let retry = on_action.clone();
        let open_match = on_action.clone();

        let header = self.render_rank_header();

        let content: AnyElement = if self.loading && self.summary.is_none() {
            centered_state("正在加载云顶战绩...", None, None)
        } else if !self.error.is_empty() && self.summary.is_none() {
            centered_state(
                self.error.as_str(),
                Some("重试"),
                Some(Rc::new(move |window: &mut Window, app: &mut App| {
                    retry(TftAction::RefreshCareer, window, app);
                })),
            )
        } else {
            match self.summary.as_ref() {
                Some(summary) if summary.matches.is_empty() => {
                    centered_state("近期没有云顶之弈对局", None, None)
                }
                Some(summary) => {
                    let mut list = div()
                        .id("tft-career-scroll")
                        .w_full()
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .track_scroll(&self.career_scroll)
                        .flex()
                        .flex_col()
                        .gap(px(10.0))
                        .pb(px(24.0));
                    for entry in &summary.matches {
                        list = list.child(self.render_match_card(entry, open_match.clone()));
                    }
                    list.into_any_element()
                }
                None => centered_state("等待云顶数据...", None, None),
            }
        };

        div()
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .min_h_0()
            .child(header)
            .when(!self.error.is_empty() && self.summary.is_some(), |this| {
                this.child(
                    div()
                        .w_full()
                        .pb(px(8.0))
                        .text_size(px(12.0))
                        .text_color(Theme::LOSS_TEXT)
                        .child(format!("刷新失败，展示上次数据: {}", self.error)),
                )
            })
            .child(content)
            .into_any_element()
    }

    fn render_rank_header(&self) -> Div {
        let (solo_tier, solo_lp, solo_wins, solo_losses) = match self.ranked.as_ref() {
            Some(stats) => (
                tier_label(&stats.solo_tier, &stats.solo_division),
                stats.solo_lp,
                stats.solo_wins,
                stats.solo_losses,
            ),
            None => (tier_label("UNRANKED", "NA"), 0, 0, 0),
        };
        let (turbo_tier, turbo_rating, turbo_wins) = match self.ranked.as_ref() {
            Some(stats) => (
                tier_label(&stats.turbo_tier, "NA"),
                stats.turbo_rating,
                stats.turbo_wins,
            ),
            None => (tier_label("NONE", "NA"), 0, 0),
        };
        let (double_tier, double_lp, double_wins, double_losses) = match self.ranked.as_ref() {
            Some(stats) => (
                tier_label(&stats.double_tier, &stats.double_division),
                stats.double_lp,
                stats.double_wins,
                stats.double_losses,
            ),
            None => (tier_label("UNRANKED", "NA"), 0, 0, 0),
        };
        let (top4_rate, win_rate, avg_placement, total_games) = match self.summary.as_ref() {
            Some(summary) => (
                summary.top4_rate,
                summary.win_rate,
                summary.avg_placement,
                summary.total_games,
            ),
            None => (0.0, 0.0, 0.0, 0),
        };

        div()
            .w_full()
            .flex()
            .gap(px(12.0))
            .mb(px(14.0))
            .child(rank_card(
                "单排",
                Theme::ACCENT_PRIMARY,
                &solo_tier,
                &format!("{solo_lp} LP"),
                &format!("胜: {solo_wins}    负: {solo_losses}"),
            ))
            .child(rank_card(
                "狂暴",
                Theme::GOLD,
                &turbo_tier,
                &format!("{turbo_rating} 评分"),
                &format!("胜场: {turbo_wins}"),
            ))
            .child(rank_card(
                "双人",
                Theme::WIN_TEXT,
                &double_tier,
                &format!("{double_lp} LP"),
                &format!("胜: {double_wins}    负: {double_losses}"),
            ))
            .child(
                div()
                    .flex_1()
                    .px(px(14.0))
                    .py(px(12.0))
                    .rounded(px(10.0))
                    .bg(Theme::CARD_BG)
                    .border_1()
                    .border_color(Theme::CARD_BORDER)
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(Theme::TEXT_MUTED)
                            .child("近期战绩汇总"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(14.0))
                            .child(summary_stat(
                                "前四率",
                                &format!("{top4_rate:.1}%"),
                                Theme::WIN_TEXT,
                            ))
                            .child(summary_stat(
                                "登顶率",
                                &format!("{win_rate:.1}%"),
                                Theme::GOLD,
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(14.0))
                            .child(summary_stat(
                                "平均名次",
                                &format!("#{avg_placement:.2}"),
                                Theme::TEXT_PRIMARY,
                            ))
                            .child(summary_stat(
                                "总场次",
                                &total_games.to_string(),
                                Theme::TEXT_PRIMARY,
                            )),
                    ),
            )
    }

    fn render_match_card(
        &self,
        entry: &TftMatch,
        on_select: impl Fn(TftAction, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let game_id = entry.game_id;
        let accent = placement_accent(entry.placement);
        let visible_traits = top_traits(&entry.traits, 4);

        div()
            .id(SharedString::from(format!("tft-match-{game_id}")))
            .w_full()
            .flex()
            .items_center()
            .gap(px(16.0))
            .px(px(14.0))
            .py(px(12.0))
            .rounded(px(10.0))
            .bg(Theme::CARD_BG)
            .border_1()
            .border_color(Theme::CARD_BORDER)
            .cursor_pointer()
            .hover(|style| {
                style
                    .bg(Theme::CARD_BG_HOVER)
                    .border_color(Theme::BORDER_HOVER)
            })
            .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                on_select(TftAction::OpenMatch(game_id), window, app);
            })
            // 左侧：名次、时长、队列、经济
            .child(
                div()
                    .w(px(150.0))
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(16.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(accent)
                                    .child(format!("#{}", entry.placement)),
                            )
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(Theme::TEXT_MUTED)
                                    .child(entry.duration_str.clone()),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(Theme::TEXT_MUTED)
                            .child(format!("{} · {}", entry.queue_name, entry.time_str)),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(6.0))
                            .child(pill(&format!("Lv.{}", entry.level)))
                            .child(pill(&format!("金币 {}", entry.gold_left))),
                    ),
            )
            // 中间：棋子与装备
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .flex()
                    .flex_wrap()
                    .gap(px(8.0))
                    .children(entry.units.iter().map(unit_chip)),
            )
            // 右侧：羁绊
            .child(
                div()
                    .w(px(180.0))
                    .flex_none()
                    .flex()
                    .flex_wrap()
                    .justify_end()
                    .gap(px(4.0))
                    .children(visible_traits.into_iter().map(|trait_value| {
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .px(px(6.0))
                            .py(px(2.0))
                            .rounded(px(6.0))
                            .bg(Theme::ACCENT_ALPHA_10)
                            .border_1()
                            .border_color(Theme::ACCENT_ALPHA_20)
                            .child(
                                div()
                                    .text_size(px(10.8))
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child(trait_value.name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(10.8))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::ACCENT_PRIMARY)
                                    .child(trait_value.num_units.to_string()),
                            )
                    })),
            )
    }

    fn render_match_detail(
        &self,
        entry: &TftMatch,
        on_action: impl Fn(TftAction, &mut Window, &mut App) + 'static + Clone,
    ) -> Div {
        let close = on_action.clone();
        let rows = entry.participants.clone();

        overlay_shell()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.0))
                            .child(
                                div()
                                    .px(px(8.0))
                                    .py(px(2.0))
                                    .rounded(px(6.0))
                                    .bg(Theme::ACCENT_ALPHA_10)
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::ACCENT_PRIMARY)
                                    .child(format!("第 {} 名", entry.placement)),
                            )
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child("对局结算榜 (8人对决)"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(Theme::TEXT_MUTED)
                                    .child(format!("{} • {}", entry.time_str, entry.duration_str)),
                            ),
                    )
                    .child(close_button(move |window, app| {
                        close(TftAction::CloseMatchDetail, window, app);
                    })),
            )
            .child(
                div()
                    .id("tft-match-detail-scroll")
                    .flex_1()
                    .min_h_0()
                    .mt(px(12.0))
                    .overflow_y_scroll()
                    .track_scroll(&self.detail_scroll)
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .children(rows.into_iter().map(|participant| {
                        let is_top4 = participant.placement <= 4;
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .gap(px(12.0))
                            .px(px(12.0))
                            .py(px(8.0))
                            .rounded(px(8.0))
                            .bg(if participant.is_self {
                                Theme::ACCENT_ALPHA_10
                            } else {
                                Theme::BADGE_BG
                            })
                            .border_1()
                            .border_color(if is_top4 {
                                Theme::WIN_BORDER
                            } else {
                                Theme::CARD_BORDER
                            })
                            .child(
                                div()
                                    .w(px(150.0))
                                    .flex_none()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(6.0))
                                            .child(
                                                div()
                                                    .text_size(px(12.5))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(placement_accent(
                                                        participant.placement,
                                                    ))
                                                    .child(format!("#{}", participant.placement)),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.0))
                                                    .text_color(Theme::TEXT_PRIMARY)
                                                    .child(participant.summoner_name.clone()),
                                            )
                                            .when(participant.is_self, |this| {
                                                this.child(pill("我"))
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.8))
                                            .text_color(Theme::TEXT_MUTED)
                                            .child(format!(
                                                "Lv.{} · 金币 {} · 伤害 {}",
                                                participant.level,
                                                participant.gold_left,
                                                participant.total_damage_to_players
                                            )),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .flex()
                                    .flex_wrap()
                                    .gap(px(6.0))
                                    .children(participant.units.iter().map(unit_chip)),
                            )
                            .child(
                                div()
                                    .w(px(190.0))
                                    .flex_none()
                                    .flex()
                                    .flex_wrap()
                                    .justify_end()
                                    .gap(px(4.0))
                                    .children(
                                        top_traits(&participant.traits, 6)
                                            .into_iter()
                                            .map(trait_pill),
                                    ),
                            )
                    })),
            )
    }

    // ─── 阵容推荐 ───

    fn render_comps(
        &self,
        on_action: impl Fn(TftAction, &mut Window, &mut App) + 'static + Clone,
    ) -> AnyElement {
        let refresh = on_action.clone();
        let retry = on_action.clone();
        let open_deck = on_action.clone();

        let version = self.decks_version_label();
        let header = div()
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .mb(px(12.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(Theme::TEXT_MUTED)
                            .child("OP.GG 实时热门阵容，点击卡片查看站位与装备"),
                    )
                    .when_some(version, |this, text| {
                        this.child(
                            div()
                                .px(px(8.0))
                                .py(px(2.0))
                                .rounded(px(10.0))
                                .bg(Theme::ACCENT_ALPHA_10)
                                .text_size(px(11.0))
                                .text_color(Theme::ACCENT_PRIMARY)
                                .child(text),
                        )
                    }),
            )
            .child(action_pill("刷新", move |window, app| {
                refresh(TftAction::RefreshComps, window, app);
            }));

        let content: AnyElement = if self.comps_loading && self.decks.is_empty() {
            centered_state("正在加载 OP.GG 阵容...", None, None)
        } else if !self.comps_error.is_empty() && self.decks.is_empty() {
            centered_state(
                self.comps_error.as_str(),
                Some("重试"),
                Some(Rc::new(move |window: &mut Window, app: &mut App| {
                    retry(TftAction::RefreshComps, window, app);
                })),
            )
        } else if self.decks.is_empty() {
            centered_state("暂无热门阵容数据", None, None)
        } else {
            let mut grid = div()
                .id("tft-comps-scroll")
                .w_full()
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(&self.comps_scroll)
                .flex()
                .flex_wrap()
                .gap(px(12.0))
                .pb(px(24.0));
            for (index, deck) in self.decks.iter().enumerate() {
                let cb = open_deck.clone();
                grid = grid.child(deck_card(index, deck, move |window, app| {
                    cb(TftAction::OpenDeck(index), window, app);
                }));
            }
            grid.into_any_element()
        };

        div()
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .min_h_0()
            .child(header)
            .when(
                !self.comps_error.is_empty() && !self.decks.is_empty(),
                |this| this.child(error_banner(&self.comps_error)),
            )
            .child(content)
            .into_any_element()
    }

    fn decks_version_label(&self) -> Option<String> {
        let deck = self.decks.first()?;
        let set = deck.team_code.find("TFTSet").and_then(|index| {
            let tail = &deck.team_code[index + "TFTSet".len()..];
            let digits: String = tail.chars().take_while(char::is_ascii_digit).collect();
            (!digits.is_empty()).then(|| format!("Set {digits}"))
        });
        Some(match set {
            Some(set) => format!("OP.GG {set}"),
            None => "OP.GG 实时版本".to_string(),
        })
    }

    fn render_deck_detail(
        &self,
        deck: &MetaDeck,
        on_action: impl Fn(TftAction, &mut Window, &mut App) + 'static + Clone,
    ) -> Div {
        let index = self.deck_detail.unwrap_or(0);
        let total = self.decks.len();
        let close = on_action.clone();
        let prev = on_action.clone();
        let next = on_action.clone();
        let copy = on_action.clone();
        let team_code = deck.team_code.clone();

        let mut header = div()
            .flex()
            .items_center()
            .gap(px(10.0))
            .child(
                div()
                    .text_size(px(15.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(Theme::TEXT_PRIMARY)
                    .child(deck.name.clone()),
            )
            .when(deck.cost > 0, |this| {
                this.child(pill(&format!("{}费", deck.cost)))
            });
        for badge in deck.badges.iter().take(3) {
            header = header.child(pill(badge));
        }

        let mut nav = div().flex().items_center().gap(px(8.0));
        if total > 1 {
            nav = nav
                .child(nav_pill("◀ 上一个", index > 0, move |window, app| {
                    prev(TftAction::PrevDeck, window, app);
                }))
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(Theme::TEXT_MUTED)
                        .child(format!("{} / {}", index + 1, total)),
                )
                .child(nav_pill(
                    "下一个 ▶",
                    index + 1 < total,
                    move |window, app| {
                        next(TftAction::NextDeck, window, app);
                    },
                ));
        }
        if !team_code.is_empty() {
            nav = nav.child(action_pill("复制阵容代码", move |window, app| {
                copy(TftAction::CopyTeamCode(team_code.clone()), window, app);
            }));
        }

        let board = self.render_board(deck);
        let side = render_deck_side(deck);

        overlay_shell()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(header)
                    .child(div().flex().items_center().gap(px(8.0)).child(nav).child(
                        close_button(move |window, app| {
                            close(TftAction::CloseDeck, window, app);
                        }),
                    )),
            )
            .child(
                div()
                    .id("tft-deck-detail-scroll")
                    .flex_1()
                    .min_h_0()
                    .mt(px(12.0))
                    .overflow_y_scroll()
                    .track_scroll(&self.deck_scroll)
                    .flex()
                    .gap(px(16.0))
                    .child(board)
                    .child(side),
            )
    }

    fn render_board(&self, deck: &MetaDeck) -> Div {
        // 棋盘 7 列 × 4 行；OP.GG 坐标 y=1 为后排、y=4 为前排
        let mut cells: Vec<Option<&MetaDeckUnit>> = vec![None; 28];
        let mut unplaced: Vec<&MetaDeckUnit> = Vec::new();
        for unit in &deck.units {
            match unit.cell {
                Some((x, y)) if (1..=7).contains(&x) && (1..=4).contains(&y) => {
                    let index = ((4 - y) * 7 + (x - 1)) as usize;
                    if cells.get(index).is_some_and(|slot| slot.is_none()) {
                        cells[index] = Some(unit);
                    } else {
                        unplaced.push(unit);
                    }
                }
                _ => unplaced.push(unit),
            }
        }
        // 部分阵容的 cell 为 null：按「前排优先、从左补位」的固定顺序落子，
        // 避免整块棋盘空白（Vue 有前排/后排智能站位，这里是有意的简化）
        for unit in unplaced {
            let mut placed = false;
            for slot in cells.iter_mut() {
                if slot.is_none() {
                    *slot = Some(unit);
                    placed = true;
                    break;
                }
            }
            if !placed {
                break;
            }
        }

        let mut board = div()
            .w(px(392.0))
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(4.0));
        for row in 0..4 {
            let mut line = div().flex().gap(px(4.0));
            for column in 0..7 {
                let unit = cells.get(row * 7 + column).copied().flatten();
                line = line.child(board_cell(unit));
            }
            board = board.child(line);
        }
        board.child(
            div()
                .mt(px(6.0))
                .text_size(px(10.8))
                .text_color(Theme::TEXT_MUTED)
                .child("C: 阵容核心 · P1/P2/P3: 装备分配优先级"),
        )
    }

    // ─── 海克斯 ───

    fn render_augments(
        &self,
        focus: &FocusHandle,
        focus_active: bool,
        on_action: impl Fn(TftAction, &mut Window, &mut App) + 'static + Clone,
    ) -> AnyElement {
        let query = on_action.clone();
        let clear = on_action.clone();
        let tier = on_action.clone();
        let refresh = on_action.clone();
        let retry = on_action.clone();
        let more = on_action.clone();

        let filtered = self.filtered_augments();
        let total = filtered.len();
        let shown = self.aug_limit.min(total);

        let mut tier_row = div().flex().items_center().gap(px(6.0));
        for filter in [
            AugTierFilter::All,
            AugTierFilter::Silver,
            AugTierFilter::Gold,
            AugTierFilter::Prismatic,
        ] {
            let active = self.aug_tier == filter;
            let count = self
                .augments
                .iter()
                .filter(|augment| filter.matches(augment.tier))
                .count();
            let cb = tier.clone();
            tier_row = tier_row.child(
                div()
                    .id(SharedString::from(format!(
                        "tft-augment-tier-{}",
                        filter.label()
                    )))
                    .px(px(10.0))
                    .py(px(3.0))
                    .rounded(px(6.0))
                    .border_1()
                    .cursor_pointer()
                    .text_size(px(11.5))
                    .bg(if active {
                        Theme::ACCENT_ALPHA_10
                    } else {
                        Theme::CARD_BG
                    })
                    .border_color(if active {
                        Theme::ACCENT_ALPHA_20
                    } else {
                        Theme::CARD_BORDER
                    })
                    .text_color(if active {
                        Theme::ACCENT_PRIMARY
                    } else {
                        Theme::TEXT_SECONDARY
                    })
                    .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                        cb(TftAction::SetAugmentTier(filter), window, app);
                    })
                    .child(format!("{} ({count})", filter.label())),
            );
        }

        let toolbar = div()
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(12.0))
            .mb(px(12.0))
            .child(div().w(px(300.0)).flex_none().child(text_field(
                "tft-augment-search",
                &self.aug_query,
                "搜索海克斯名称或效果描述...",
                px(300.0),
                focus,
                focus_active,
                TextFieldOptions::default(),
                move |text, window, app| {
                    query(TftAction::QueryAugments(text), window, app);
                },
                move |window, app| {
                    clear(TftAction::ClearAugmentQuery, window, app);
                },
                |_text, _window, _app| {},
            )))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(tier_row)
                    .child(action_pill("刷新", move |window, app| {
                        refresh(TftAction::RefreshAugments, window, app);
                    })),
            );

        let content: AnyElement = if self.aug_loading && self.augments.is_empty() {
            centered_state("正在加载海克斯强化百科...", None, None)
        } else if !self.aug_error.is_empty() && self.augments.is_empty() {
            centered_state(
                self.aug_error.as_str(),
                Some("重试加载"),
                Some(Rc::new(move |window: &mut Window, app: &mut App| {
                    retry(TftAction::RefreshAugments, window, app);
                })),
            )
        } else if total == 0 {
            centered_state("没有找到匹配的海克斯强化", None, None)
        } else {
            let mut grid = div()
                .id("tft-augments-scroll")
                .w_full()
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(&self.augments_scroll)
                .flex()
                .flex_wrap()
                .gap(px(10.0))
                .pb(px(24.0));
            for augment in filtered.iter().take(shown) {
                grid = grid.child(augment_card(augment));
            }
            if shown < total {
                let cb = more.clone();
                grid = grid.child(div().w_full().flex().justify_center().pt(px(6.0)).child(
                    action_pill(
                        &format!("已展示 {shown} / {total} 项，点击加载更多"),
                        move |window, app| {
                            cb(TftAction::LoadMoreAugments, window, app);
                        },
                    ),
                ));
            }
            grid.into_any_element()
        };

        div()
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .min_h_0()
            .child(toolbar)
            .when(
                !self.aug_error.is_empty() && !self.augments.is_empty(),
                |this| this.child(error_banner(&self.aug_error)),
            )
            .child(content)
            .into_any_element()
    }
}

// ─── 小部件 ───

fn rank_card(title: &str, accent: Hsla, tier: &str, lp: &str, sub: &str) -> Div {
    div()
        .flex_1()
        .px(px(14.0))
        .py(px(12.0))
        .rounded(px(10.0))
        .bg(Theme::CARD_BG)
        .border_1()
        .border_color(Theme::CARD_BORDER)
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(
                    div()
                        .px(px(6.0))
                        .py(px(1.0))
                        .rounded(px(10.0))
                        .bg(Theme::ACCENT_ALPHA_10)
                        .text_size(px(10.5))
                        .font_weight(FontWeight::BOLD)
                        .text_color(accent)
                        .child(title.to_string()),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(Theme::TEXT_MUTED)
                        .child("云顶段位"),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(15.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(Theme::TEXT_PRIMARY)
                        .child(tier.to_string()),
                )
                .child(
                    div()
                        .text_size(px(11.5))
                        .font_weight(FontWeight::BOLD)
                        .text_color(Theme::ACCENT_PRIMARY)
                        .child(lp.to_string()),
                ),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(Theme::TEXT_MUTED)
                .child(sub.to_string()),
        )
}

fn summary_stat(label: &str, value: &str, color: Hsla) -> Div {
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .text_size(px(10.5))
                .text_color(Theme::TEXT_MUTED)
                .child(label.to_string()),
        )
        .child(
            div()
                .text_size(px(13.5))
                .font_weight(FontWeight::BOLD)
                .text_color(color)
                .child(value.to_string()),
        )
}

fn pill(text: &str) -> Div {
    div()
        .px(px(6.0))
        .py(px(1.0))
        .rounded(px(6.0))
        .bg(Theme::BADGE_BG)
        .border_1()
        .border_color(Theme::CARD_BORDER)
        .text_size(px(10.5))
        .text_color(Theme::TEXT_SECONDARY)
        .child(text.to_string())
}

fn placement_accent(placement: i32) -> Hsla {
    if placement == 1 {
        Theme::GOLD
    } else if placement <= 4 {
        Theme::WIN_TEXT
    } else {
        Theme::LOSS_TEXT
    }
}

fn top_traits(traits: &[TftTrait], limit: usize) -> Vec<TftTrait> {
    let mut sorted: Vec<TftTrait> = traits
        .iter()
        .filter(|trait_value| trait_value.tier_current > 0 && trait_value.num_units > 0)
        .cloned()
        .collect();
    sorted.sort_by(|left, right| {
        right
            .tier_current
            .cmp(&left.tier_current)
            .then(right.num_units.cmp(&left.num_units))
    });
    sorted.truncate(limit);
    sorted
}

fn trait_pill(trait_value: TftTrait) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(4.0))
        .px(px(6.0))
        .py(px(2.0))
        .rounded(px(6.0))
        .bg(Theme::ACCENT_ALPHA_10)
        .border_1()
        .border_color(Theme::ACCENT_ALPHA_20)
        .child(
            div()
                .text_size(px(10.5))
                .text_color(Theme::TEXT_PRIMARY)
                .child(trait_value.name),
        )
        .child(
            div()
                .text_size(px(10.5))
                .font_weight(FontWeight::BOLD)
                .text_color(Theme::ACCENT_PRIMARY)
                .child(trait_value.num_units.to_string()),
        )
}

fn unit_chip(unit: &TftUnit) -> Div {
    let stars: String = "★".repeat(unit.tier.clamp(0, 3) as usize);
    let mut chip = div()
        .w(px(46.0))
        .flex_none()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(1.0))
        .child(
            div()
                .h(px(10.0))
                .text_size(px(9.0))
                .text_color(Theme::GOLD)
                .child(stars),
        )
        .child(
            LcuImage::new(unit.icon_url.clone())
                .size(px(38.0))
                .placeholder(unit.name.chars().take(2).collect::<String>()),
        );
    if !unit.item_icon_urls.is_empty() {
        let mut items = div().flex().gap(px(1.0)).mt(px(1.0));
        for icon in unit.item_icon_urls.iter().take(3) {
            items = items.child(LcuImage::new(icon.clone()).size(px(12.0)).placeholder(""));
        }
        chip = chip.child(items);
    }
    chip
}

fn augment_card(augment: &TftAugmentInfo) -> Div {
    let (tier_label, tier_color) = match augment.tier {
        1 => ("银色", Theme::TEXT_MUTED),
        3 => ("棱彩", Theme::PREMADE_COLOR_3),
        _ => ("金色", Theme::GOLD),
    };
    div()
        .w(px(310.0))
        .flex()
        .items_start()
        .gap(px(10.0))
        .px(px(10.0))
        .py(px(8.0))
        .rounded(px(8.0))
        .bg(Theme::CARD_BG)
        .border_1()
        .border_color(Theme::CARD_BORDER)
        .child(
            div().w(px(34.0)).h(px(34.0)).flex_none().child(
                LcuImage::new(augment.icon_path.clone())
                    .size(px(34.0))
                    .placeholder("⚡"),
            ),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(12.5))
                                .font_weight(FontWeight::BOLD)
                                .text_color(Theme::TEXT_PRIMARY)
                                .child(augment.name.clone()),
                        )
                        .child(
                            div()
                                .px(px(6.0))
                                .py(px(1.0))
                                .rounded(px(8.0))
                                .bg(Theme::BADGE_BG)
                                .text_size(px(10.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(tier_color)
                                .child(tier_label),
                        ),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .line_height(relative(1.35))
                        .text_color(Theme::TEXT_SECONDARY)
                        .child(if augment.desc.is_empty() {
                            "暂无效果描述".to_string()
                        } else {
                            augment.desc.clone()
                        }),
                ),
        )
}

fn deck_card(
    index: usize,
    deck: &MetaDeck,
    on_click: impl Fn(&mut Window, &mut App) + 'static + Clone,
) -> Stateful<Div> {
    let (win_rate, top4_rate, avg_placement) = (
        deck.win_rate.map(|value| format!("{:.1}%", value * 100.0)),
        deck.top4_rate.map(|value| format!("{:.1}%", value * 100.0)),
        deck.avg_placement.map(|value| format!("#{value:.2}")),
    );

    div()
        .id(SharedString::from(format!("tft-deck-{index}")))
        .w(px(310.0))
        .flex()
        .flex_col()
        .gap(px(8.0))
        .px(px(12.0))
        .py(px(10.0))
        .rounded(px(10.0))
        .bg(Theme::CARD_BG)
        .border_1()
        .border_color(Theme::CARD_BORDER)
        .cursor_pointer()
        .hover(|style| {
            style
                .bg(Theme::CARD_BG_HOVER)
                .border_color(Theme::BORDER_HOVER)
        })
        .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
            on_click(window, app);
        })
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .px(px(6.0))
                        .py(px(1.0))
                        .rounded(px(6.0))
                        .bg(Theme::ACCENT_ALPHA_10)
                        .text_size(px(11.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(Theme::ACCENT_PRIMARY)
                        .child(deck.tier.clone()),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .text_size(px(13.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(Theme::TEXT_PRIMARY)
                        .child(deck.name.clone()),
                )
                .when(deck.cost > 0, |this| {
                    this.child(pill(&format!("{}费", deck.cost)))
                }),
        )
        .child(
            div()
                .flex()
                .gap(px(12.0))
                .when_some(win_rate, |this, text| {
                    this.child(deck_stat("胜率", &text, Theme::WIN_TEXT))
                })
                .when_some(top4_rate, |this, text| {
                    this.child(deck_stat("Top4", &text, Theme::ACCENT_PRIMARY))
                })
                .when_some(avg_placement, |this, text| {
                    this.child(deck_stat("平均名次", &text, Theme::TEXT_PRIMARY))
                }),
        )
        .when(!deck.badges.is_empty(), |this| {
            this.child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(4.0))
                    .children(deck.badges.iter().take(3).map(|badge| pill(badge))),
            )
        })
        .child(div().flex().flex_wrap().gap(px(4.0)).children(
            deck.traits.iter().take(5).map(|trait_value| {
                pill(&format!("{} {}", trait_value.name, trait_value.num_units))
            }),
        ))
}

fn deck_stat(label: &str, value: &str, color: Hsla) -> Div {
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .text_size(px(10.0))
                .text_color(Theme::TEXT_MUTED)
                .child(label.to_string()),
        )
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight::BOLD)
                .text_color(color)
                .child(value.to_string()),
        )
}

fn board_cell(unit: Option<&MetaDeckUnit>) -> Div {
    let cell = div()
        .relative()
        .size(px(52.0))
        .rounded(px(6.0))
        .bg(Theme::ITEM_SLOT_BG)
        .border_1()
        .border_color(Theme::ITEM_SLOT_BORDER)
        .flex()
        .items_center()
        .justify_center();

    let Some(unit) = unit else {
        return cell;
    };

    let mut inner = div().relative().flex().flex_col().items_center().child(
        LcuImage::new(unit.icon_url.clone())
            .size(px(40.0))
            .border_color(if unit.is_core {
                Theme::GOLD
            } else {
                Theme::CARD_BORDER
            })
            .placeholder(unit.name.chars().take(2).collect::<String>()),
    );
    if unit.tier >= 2 {
        inner = inner.child(
            div()
                .text_size(px(8.5))
                .text_color(if unit.tier >= 3 {
                    Theme::GOLD
                } else {
                    Theme::TEXT_SECONDARY
                })
                .child("★".repeat(unit.tier.min(3) as usize)),
        );
    }
    let mut cell = cell.child(inner);
    if unit.priority > 0 {
        cell = cell.child(
            div()
                .absolute()
                .top(px(1.0))
                .right(px(2.0))
                .text_size(px(8.5))
                .font_weight(FontWeight::BOLD)
                .text_color(Theme::ACCENT_PRIMARY)
                .child(format!("P{}", unit.priority)),
        );
    }
    cell
}

fn render_deck_side(deck: &MetaDeck) -> Div {
    let mut side = div()
        .flex_1()
        .min_w(px(0.0))
        .flex()
        .flex_col()
        .gap(px(12.0));
    if !deck.traits.is_empty() {
        side = side.child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(section_title("激活羁绊"))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(4.0))
                        .children(deck.traits.iter().map(|trait_value| {
                            pill(&format!("{} ({})", trait_value.name, trait_value.num_units))
                        })),
                ),
        );
    }
    for (label, phase) in [
        ("前期过渡", deck.early.as_ref()),
        ("中期过渡", deck.middle.as_ref()),
    ] {
        let Some(phase) = phase else {
            continue;
        };
        if phase.units.is_empty() {
            continue;
        }
        side = side.child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(section_title(&format!("{label} (等级 {})", phase.level)))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(4.0))
                        .children(phase.units.iter().map(|name| pill(name))),
                ),
        );
    }
    side
}

fn section_title(text: &str) -> Div {
    div()
        .text_size(px(12.0))
        .font_weight(FontWeight::BOLD)
        .text_color(Theme::TEXT_PRIMARY)
        .child(text.to_string())
}

fn overlay_shell() -> Div {
    div()
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .right(px(0.0))
        .bottom(px(0.0))
        .flex()
        .flex_col()
        .min_h_0()
        .p(px(16.0))
        .rounded(px(12.0))
        .bg(Theme::BG_APP)
        .border_1()
        .border_color(Theme::CARD_BORDER)
        .shadow_md()
}

fn close_button(on_click: impl Fn(&mut Window, &mut App) + 'static) -> Div {
    div()
        .px(px(10.0))
        .py(px(3.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(Theme::CARD_BORDER)
        .bg(Theme::CARD_BG)
        .text_size(px(11.5))
        .text_color(Theme::TEXT_SECONDARY)
        .cursor_pointer()
        .hover(|style| style.bg(Theme::HOVER_BG_STRONG))
        .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
            on_click(window, app)
        })
        .child("关闭")
}

fn action_pill(label: &str, on_click: impl Fn(&mut Window, &mut App) + 'static) -> Div {
    div()
        .px(px(10.0))
        .py(px(3.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(Theme::ACCENT_ALPHA_20)
        .bg(Theme::ACCENT_ALPHA_10)
        .text_size(px(11.5))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(Theme::ACCENT_PRIMARY)
        .cursor_pointer()
        .hover(|style| style.bg(Theme::ACCENT_ALPHA_20))
        .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
            on_click(window, app)
        })
        .child(label.to_string())
}

fn nav_pill(label: &str, enabled: bool, on_click: impl Fn(&mut Window, &mut App) + 'static) -> Div {
    div()
        .px(px(10.0))
        .py(px(3.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(Theme::CARD_BORDER)
        .bg(Theme::CARD_BG)
        .text_size(px(11.5))
        .text_color(if enabled {
            Theme::TEXT_PRIMARY
        } else {
            Theme::TEXT_MUTED
        })
        .when(enabled, |this| {
            this.cursor_pointer()
                .hover(|style| style.bg(Theme::HOVER_BG_STRONG))
                .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                    on_click(window, app)
                })
        })
        .when(!enabled, |this| this.opacity(0.5))
        .child(label.to_string())
}

/// 空态/错误态里的可选操作按钮回调
type StateAction = Rc<dyn Fn(&mut Window, &mut App)>;

/// 刷新失败但页面仍有上次数据时的可见提示（不覆盖已有内容）
fn error_banner(message: &str) -> Div {
    div()
        .w_full()
        .pb(px(8.0))
        .text_size(px(12.0))
        .text_color(Theme::LOSS_TEXT)
        .child(format!("刷新失败，展示上次数据: {message}"))
}

fn centered_state(
    message: &str,
    action_label: Option<&str>,
    action: Option<StateAction>,
) -> AnyElement {
    let mut container = div()
        .w_full()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(12.0))
        .child(
            div()
                .text_size(px(13.0))
                .text_color(Theme::TEXT_MUTED)
                .child(message.to_string()),
        );
    if let (Some(label), Some(action)) = (action_label, action) {
        container = container.child(
            div()
                .px(px(12.0))
                .py(px(4.0))
                .rounded(px(6.0))
                .bg(Theme::ACCENT_PRIMARY)
                .text_size(px(12.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(Theme::TEXT_ON_PRIMARY)
                .cursor_pointer()
                .hover(|style| style.opacity(0.85))
                .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                    action(window, app)
                })
                .child(label.to_string()),
        );
    }
    container.into_any_element()
}

/// 段位显示（对照 Vue `TftRankHeader.vue` 的 TIER_MAP）
fn tier_label(tier: &str, division: &str) -> String {
    let upper = tier.to_uppercase();
    let name = match upper.as_str() {
        "" | "UNRANKED" | "NONE" => return "未定级".to_string(),
        "IRON" => "坚韧黑铁",
        "BRONZE" => "英勇黄铜",
        "SILVER" => "不屈白银",
        "GOLD" => "荣耀黄金",
        "PLATINUM" => "华贵铂金",
        "EMERALD" => "流光翡翠",
        "DIAMOND" => "璀璨钻石",
        "MASTER" => "超凡大师",
        "GRANDMASTER" => "傲世宗师",
        "CHALLENGER" => "最强王者",
        "GRAY" => "灰白",
        "GREEN" => "翠绿",
        "BLUE" => "天蓝",
        "PURPLE" => "华紫",
        "ORANGE" => "耀橙",
        other => other,
    };
    if matches!(upper.as_str(), "MASTER" | "GRANDMASTER" | "CHALLENGER")
        || division == "NA"
        || division.is_empty()
    {
        name.to_string()
    } else {
        format!("{name} {division}")
    }
}
