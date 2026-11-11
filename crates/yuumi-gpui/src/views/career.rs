use crate::components::dropdown::{dropdown_item, dropdown_menu, dropdown_trigger};
use crate::components::icon::icon;
use crate::components::lcu_image::{LcuImage, LcuImageShape};
use crate::components::offline_state::render_lcu_offline_state;
use crate::components::text_field::{text_field, TextFieldOptions};
use crate::match_model::{
    compute_match_stats, position_to_zh, top_champions, top_positions, MatchItem,
};
use crate::theme::Theme;
use crate::views::search::queue_id_to_name;
use gpui::prelude::FluentBuilder;
use gpui::*;
use yuumi_core::parsers::game_info::{rank_tier_label, RankedQueueEntry, RankedStats};
use yuumi_core::parsers::loot::{EssenceBalances, LootInventoryItem};
use yuumi_core::parsers::loot_actions::LootActionKind;
use yuumi_core::parsers::mastery::ChampionMasteryItem;
use yuumi_core::parsers::summoner::SummonerInfo;
use yuumi_core::parsers::teammates::RecentTeammate;

/// 生成一段圆弧的描边路径（角度以正右方为 0°，顺时针为正）
fn arc_path(
    center: Point<Pixels>,
    radius: f32,
    start_deg: f32,
    sweep_deg: f32,
    stroke_width: f32,
) -> Option<Path<Pixels>> {
    const STEPS: usize = 64;
    let cx = f32::from(center.x);
    let cy = f32::from(center.y);
    let point_at = |deg: f32| {
        let rad = deg.to_radians();
        point(px(cx + radius * rad.cos()), px(cy + radius * rad.sin()))
    };

    let mut builder = PathBuilder::stroke(px(stroke_width));
    builder.move_to(point_at(start_deg));
    for step in 1..=STEPS {
        let deg = start_deg + sweep_deg * (step as f32 / STEPS as f32);
        builder.line_to(point_at(deg));
    }
    builder.build().ok()
}

/// 排位表单元格
fn ranked_cell(
    value: String,
    width: f32,
    color: Hsla,
    size: f32,
    weight: FontWeight,
) -> impl IntoElement {
    div()
        .w(px(width))
        .text_size(px(size))
        .font_weight(weight)
        .text_color(color)
        .whitespace_nowrap()
        .overflow_hidden()
        .child(value)
}

/// 单行排位数据（缺失队列按空数据占位）
fn ranked_row(label: &str, entry: Option<&RankedQueueEntry>) -> [String; 9] {
    let placeholder = RankedQueueEntry::default();
    let entry = entry.unwrap_or(&placeholder);
    let total = entry.total_games();

    let rank_label = if entry.is_placed() {
        rank_tier_label(&entry.tier, &entry.division)
    } else if entry.is_provisional {
        "定级中".to_string()
    } else {
        "--".to_string()
    };

    [
        label.to_string(),
        total.to_string(),
        if total == 0 {
            "--".to_string()
        } else {
            format!("{:.0}%", entry.win_rate())
        },
        entry.wins.to_string(),
        entry.losses.to_string(),
        rank_label,
        if entry.is_placed() {
            entry.league_points.to_string()
        } else {
            "0".to_string()
        },
        rank_label_or_dash(&entry.highest_tier, &entry.highest_division),
        rank_label_or_dash(
            &entry.previous_season_end_tier,
            &entry.previous_season_end_division,
        ),
    ]
}

/// 段位标签，未定级时返回破折号
fn rank_label_or_dash(tier: &str, division: &str) -> String {
    let label = rank_tier_label(tier, division);
    if label == "未定级" || tier.is_empty() || tier == "NONE" {
        "--".to_string()
    } else {
        label
    }
}

fn format_num(val: i64) -> String {
    let s = val.to_string();
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    for (i, &ch) in chars.iter().enumerate() {
        result.push(ch);
        let rem = len - 1 - i;
        if rem > 0 && rem.is_multiple_of(3) {
            result.push(',');
        }
    }
    result
}

fn format_local_time(ts_ms: i64) -> String {
    format_local_by_pattern(ts_ms, "%m-%d %H:%M")
}

/// epoch 毫秒 → 本地 `YYYY/MM/DD HH:mm`（对齐 Vue `formatLocalDateTime`）
///
/// `MatchDisplay::short_time` 由 Rust 侧按 UTC 生成，直接展示会早 8 小时，
/// 因此面向用户的时间一律从 `time_stamp` 派生。
fn format_local_datetime(ts_ms: i64) -> String {
    format_local_by_pattern(ts_ms, "%Y/%m/%d %H:%M")
}

/// epoch 毫秒 → 本地 `MM-DD`（对齐 Vue `formatLocalMonthDay`）
pub(crate) fn format_local_month_day(ts_ms: i64) -> String {
    format_local_by_pattern(ts_ms, "%m-%d")
}

fn format_local_by_pattern(ts_ms: i64, pattern: &str) -> String {
    if ts_ms <= 0 {
        return String::new();
    }
    let millis = if ts_ms < 10_000_000_000 {
        ts_ms * 1000
    } else {
        ts_ms
    };
    chrono::DateTime::from_timestamp_millis(millis)
        .map(|dt| {
            let local: chrono::DateTime<chrono::Local> = dt.into();
            local.format(pattern).to_string()
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CareerTab {
    Matches,
    Loot,
    Mastery,
}

impl CareerTab {
    pub fn label(&self) -> &'static str {
        match self {
            CareerTab::Matches => "生涯战绩",
            CareerTab::Loot => "战利品",
            CareerTab::Mastery => "英雄熟练度",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CareerPopup {
    QueueMenu,
    Teammates,
}

#[derive(Debug, Clone, Copy)]
pub struct QueueOption {
    pub label: &'static str,
    pub id: Option<i32>,
}

pub const QUEUE_OPTIONS: [QueueOption; 7] = [
    QueueOption {
        label: "全部",
        id: None,
    },
    QueueOption {
        label: "海克斯大乱斗",
        id: Some(2400),
    },
    QueueOption {
        label: "经典海斗",
        id: Some(2450),
    },
    QueueOption {
        label: "极地大乱斗",
        id: Some(450),
    },
    QueueOption {
        label: "匹配模式",
        id: Some(430),
    },
    QueueOption {
        label: "单双排位",
        id: Some(420),
    },
    QueueOption {
        label: "灵活排位",
        id: Some(440),
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CareerBarAction {
    TogglePopup(CareerPopup),
    ClosePopup,
    SelectQueue(Option<i32>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChestFilter {
    All,
    Eligible,
    Granted,
}

impl ChestFilter {
    pub const ALL: [Self; 3] = [Self::All, Self::Eligible, Self::Granted];

    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "全部宝箱状态",
            Self::Eligible => "可获取宝箱 📦",
            Self::Granted => "已获取宝箱 🎁",
        }
    }

    pub fn matches(&self, m: &ChampionMasteryItem) -> bool {
        match self {
            Self::All => true,
            Self::Eligible => m.eligible_for_chest && !m.chest_granted,
            Self::Granted => m.chest_granted,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelFilter {
    All,
    Lv10Plus,
    Lv7To9,
    Lv4To6,
    Lv1To3,
}

impl LevelFilter {
    pub const ALL: [Self; 5] = [
        Self::All,
        Self::Lv10Plus,
        Self::Lv7To9,
        Self::Lv4To6,
        Self::Lv1To3,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "全部等级",
            Self::Lv10Plus => "Lv. 10 及以上",
            Self::Lv7To9 => "Lv. 7 - 9",
            Self::Lv4To6 => "Lv. 4 - 6",
            Self::Lv1To3 => "Lv. 1 - 3",
        }
    }

    pub fn matches(&self, level: i32) -> bool {
        match self {
            Self::All => true,
            Self::Lv10Plus => level >= 10,
            Self::Lv7To9 => (7..=9).contains(&level),
            Self::Lv4To6 => (4..=6).contains(&level),
            Self::Lv1To3 => (1..=3).contains(&level),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MasterySort {
    Points,
    Level,
    Milestone,
    LastPlay,
}

impl MasterySort {
    pub const ALL: [Self; 4] = [Self::Points, Self::Level, Self::Milestone, Self::LastPlay];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Points => "熟练度分数",
            Self::Level => "熟练度等级",
            Self::Milestone => "赛段里程碑",
            Self::LastPlay => "最近对局时间",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MasteryDropdown {
    Chest,
    Level,
    Sort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MasteryFilterAction {
    Chest(ChestFilter),
    Level(LevelFilter),
    Sort(MasterySort),
    ToggleSortDir,
    ToggleDropdown(MasteryDropdown),
    CloseDropdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LootDropdown {
    Type,
    Owned,
    ValueBasis,
    Operator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LootTypeFilter {
    All,
    Champion,
    Skin,
    Emote,
    WardSkin,
    SummonerIcon,
    Eternal,
    Material,
}

impl LootTypeFilter {
    pub const ALL: [Self; 8] = [
        Self::All,
        Self::Champion,
        Self::Skin,
        Self::Emote,
        Self::WardSkin,
        Self::SummonerIcon,
        Self::Eternal,
        Self::Material,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "全部",
            Self::Champion => "英雄",
            Self::Skin => "皮肤",
            Self::Emote => "表情",
            Self::WardSkin => "守卫",
            Self::SummonerIcon => "图标",
            Self::Eternal => "永恒星碑",
            Self::Material => "材料/宝箱",
        }
    }

    pub fn key(&self) -> &'static str {
        match self {
            Self::All => "ALL",
            Self::Champion => "CHAMPION",
            Self::Skin => "SKIN",
            Self::Emote => "EMOTE",
            Self::WardSkin => "WARDSKIN",
            Self::SummonerIcon => "SUMMONERICON",
            Self::Eternal => "ETERNAL",
            Self::Material => "MATERIAL",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LootOwnedFilter {
    All,
    Owned,
    NotOwned,
}

impl LootOwnedFilter {
    pub const ALL: [Self; 3] = [Self::All, Self::Owned, Self::NotOwned];

    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "全部状态",
            Self::Owned => "已拥有",
            Self::NotOwned => "未拥有",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LootValueBasis {
    Disenchant,
    Store,
}

impl LootValueBasis {
    pub const ALL: [Self; 2] = [Self::Disenchant, Self::Store];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Disenchant => "分解价值",
            Self::Store => "商店价值",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LootOperator {
    Le,
    Ge,
    Eq,
}

impl LootOperator {
    pub const ALL: [Self; 3] = [Self::Le, Self::Ge, Self::Eq];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Le => "小于等于 (<=)",
            Self::Ge => "大于等于 (>=)",
            Self::Eq => "等于 (=)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LootQuantityAction {
    Open,
    Craft,
}

impl LootQuantityAction {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Open => "开启",
            Self::Craft => "合成",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LootModal {
    Quantity {
        loot_id: String,
        name: String,
        owned: i32,
        quantity: i32,
        need_key: bool,
        key_count: i32,
        key_name: Option<String>,
        tile_path: Option<String>,
        action: LootQuantityAction,
        recipe_name: String,
        ingredients: Vec<String>,
    },
    Confirm {
        action_kind: LootActionKind,
        title: String,
        message: String,
        selected: usize,
        gain_blue: i64,
        gain_orange: i64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LootProgress {
    pub current: usize,
    pub total: usize,
    pub success: usize,
    pub current_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LootAction {
    ToggleDropdown(LootDropdown),
    CloseDropdown,
    SetType(LootTypeFilter),
    SetOwned(LootOwnedFilter),
    SetValueBasis(LootValueBasis),
    SetOperator(LootOperator),
    SetValueInput(String),
    ClearValueInput,
    ToggleSelect(String),
    SelectAllFiltered,
    ClearSelection,
    OpenQuantity(LootModal),
    OpenConfirm(LootModal),
    Confirm(LootModal),
    CloseModal,
    CloseProgress,
}

#[derive(Debug, Clone)]
pub struct CareerHeaderStats {
    pub total_games: usize,
    pub wins: usize,
    pub losses: usize,
    pub remakes: usize,
    pub win_rate: f32,
    pub avg_kda_ratio: f32,
    pub avg_kills: f32,
    pub avg_deaths: f32,
    pub avg_assists: f32,
}

impl CareerHeaderStats {
    pub fn compute(matches: &[MatchItem]) -> Self {
        let stats = compute_match_stats(matches);
        let total_games = stats.total_matches;
        if total_games == 0 {
            return Self {
                total_games: 0,
                wins: 0,
                losses: 0,
                remakes: 0,
                win_rate: 0.0,
                avg_kda_ratio: 0.0,
                avg_kills: 0.0,
                avg_deaths: 0.0,
                avg_assists: 0.0,
            };
        }

        let mut total_k = 0;
        let mut total_d = 0;
        let mut total_a = 0;

        for m in matches {
            total_k += m.display.kills;
            total_d += m.display.deaths;
            total_a += m.display.assists;
        }

        let avg_kills = total_k as f32 / total_games as f32;
        let avg_deaths = total_d as f32 / total_games as f32;
        let avg_assists = total_a as f32 / total_games as f32;

        Self {
            total_games,
            wins: stats.win_count,
            losses: stats.loss_count,
            remakes: stats.remake_count,
            win_rate: stats.win_rate as f32,
            avg_kda_ratio: stats.avg_kda as f32,
            avg_kills,
            avg_deaths,
            avg_assists,
        }
    }
}

#[derive(Clone)]
pub struct CareerView {
    pub active_tab: CareerTab,
    pub summoner: Option<SummonerInfo>,
    pub is_viewing_other: bool,
    pub copied: bool,
    pub ranked: Option<RankedStats>,
    pub mastery_list: Vec<ChampionMasteryItem>,
    pub mastery_query: String,
    pub mastery_chest: ChestFilter,
    pub mastery_level: LevelFilter,
    pub mastery_sort: MasterySort,
    pub mastery_sort_desc: bool,
    pub mastery_dropdown: Option<MasteryDropdown>,
    pub selected_queue: Option<i32>,
    pub popup: Option<CareerPopup>,
    pub recent_teammates: Vec<RecentTeammate>,
    pub essences: EssenceBalances,
    pub loot_items: Vec<LootInventoryItem>,
    pub loot_filter_type: LootTypeFilter,
    pub loot_filter_owned: LootOwnedFilter,
    pub loot_filter_value_basis: LootValueBasis,
    pub loot_filter_operator: LootOperator,
    pub loot_value_input: String,
    pub loot_dropdown: Option<LootDropdown>,
    pub loot_selected: Vec<String>,
    pub loot_modal: Option<LootModal>,
    pub loot_progress: Option<LootProgress>,
    pub loot_last_quantity: i32,
    pub matches_scroll_handle: ScrollHandle,
    pub loot_scroll_handle: ScrollHandle,
    pub mastery_scroll_handle: ScrollHandle,
}

impl Default for CareerView {
    fn default() -> Self {
        Self {
            active_tab: CareerTab::Matches,
            summoner: None,
            is_viewing_other: false,
            copied: false,
            ranked: None,
            mastery_list: Vec::new(),
            mastery_query: String::new(),
            mastery_chest: ChestFilter::All,
            mastery_level: LevelFilter::All,
            mastery_sort: MasterySort::Points,
            mastery_sort_desc: true,
            mastery_dropdown: None,
            selected_queue: None,
            popup: None,
            recent_teammates: Vec::new(),
            essences: EssenceBalances::default(),
            loot_items: Vec::new(),
            loot_filter_type: LootTypeFilter::Champion,
            loot_filter_owned: LootOwnedFilter::All,
            loot_filter_value_basis: LootValueBasis::Disenchant,
            loot_filter_operator: LootOperator::Le,
            loot_value_input: String::new(),
            loot_dropdown: None,
            loot_selected: Vec::new(),
            loot_modal: None,
            loot_progress: None,
            loot_last_quantity: 1,
            matches_scroll_handle: ScrollHandle::new(),
            loot_scroll_handle: ScrollHandle::new(),
            mastery_scroll_handle: ScrollHandle::new(),
        }
    }
}

impl CareerView {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn filtered_loot(&self) -> Vec<&LootInventoryItem> {
        let threshold = self.loot_value_input.trim().parse::<i64>().ok();
        let mut list: Vec<&LootInventoryItem> = self
            .loot_items
            .iter()
            .filter(|item| {
                let cat = loot_category(item);
                let id = item.loot_id.to_uppercase();
                let ty = item.loot_type.to_uppercase();

                let type_ok = match self.loot_filter_type {
                    LootTypeFilter::All => {
                        !(cat == "CURRENCY"
                            || id.starts_with("CURRENCY")
                            || (id.starts_with("MATERIAL_KEY") && !id.contains("FRAGMENT"))
                            || cat == "CHEST"
                            || ty == "CHEST"
                            || id.starts_with("CHEST_"))
                    }
                    LootTypeFilter::Eternal => cat == "ETERNAL" || ty.contains("STATSTONE"),
                    LootTypeFilter::Material => {
                        matches!(cat, "MATERIAL" | "CHEST" | "ORB" | "CURRENCY")
                    }
                    other => cat == other.key(),
                };
                if !type_ok {
                    return false;
                }

                let owned = item.item_status.eq_ignore_ascii_case("OWNED");
                let owned_ok = match self.loot_filter_owned {
                    LootOwnedFilter::All => true,
                    LootOwnedFilter::Owned => owned,
                    LootOwnedFilter::NotOwned => !owned,
                };
                if !owned_ok {
                    return false;
                }

                if let Some(t) = threshold {
                    let cmp = match self.loot_filter_value_basis {
                        LootValueBasis::Disenchant => item.disenchant_value as i64,
                        LootValueBasis::Store => item.value as i64,
                    };
                    let value_ok = match self.loot_filter_operator {
                        LootOperator::Le => cmp <= t,
                        LootOperator::Ge => cmp >= t,
                        LootOperator::Eq => cmp == t,
                    };
                    if !value_ok {
                        return false;
                    }
                }
                true
            })
            .collect();
        list.sort_by_key(|item| item.disenchant_value);
        list
    }

    pub fn render_offline_state() -> impl IntoElement {
        render_lcu_offline_state(None)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        matches: &[MatchItem],
        stats: &CareerHeaderStats,
        is_connected: bool,
        on_tab_change: impl Fn(CareerTab, &mut Window, &mut App) + 'static + Clone,
        on_select_match: impl Fn(u64, &mut Window, &mut App) + 'static + Clone,
        on_refresh: impl Fn(&mut Window, &mut App) + 'static + Clone,
        on_copy_name: impl Fn(&mut Window, &mut App) + 'static + Clone,
        on_history: impl Fn(&mut Window, &mut App) + 'static + Clone,
        on_back_to_me: impl Fn(&mut Window, &mut App) + 'static + Clone,
        on_click_teammate: impl Fn(RecentTeammate, &mut Window, &mut App) + 'static + Clone,
        mastery_focus: &FocusHandle,
        mastery_search_focused: bool,
        on_mastery_query: impl Fn(String, &mut Window, &mut App) + 'static + Clone,
        on_mastery_filter: impl Fn(MasteryFilterAction, &mut Window, &mut App) + 'static + Clone,
        on_career_bar: impl Fn(CareerBarAction, &mut Window, &mut App) + 'static + Clone,
        loot_value_focus: &FocusHandle,
        loot_value_focused: bool,
        on_loot_action: impl Fn(LootAction, &mut Window, &mut App) + 'static + Clone,
        on_replay: impl Fn(u64, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        if !is_connected {
            return Self::render_offline_state().into_any_element();
        }

        if self.summoner.is_none() {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(px(14.0))
                        .text_color(Theme::TEXT_MUTED())
                        .child("正在加载召唤师资料..."),
                )
                .into_any_element();
        }

        let active_tab = self.active_tab;
        let is_viewing_other = self.is_viewing_other;
        let loot_refresh = on_refresh.clone();

        div()
            .size_full()
            .overflow_hidden()
            .flex()
            .flex_col()
            .items_center()
            .px(px(24.0))
            .pt(px(16.0))
            .child(
                div()
                    .w_full()
                    .max_w(px(1000.0))
                    .h_full()
                    .flex()
                    .flex_col()
                    .min_h_0()
                    .overflow_hidden()
                    .child(self.render_summoner_header(
                        on_refresh.clone(),
                        on_copy_name,
                        on_history,
                        on_back_to_me,
                    ))
                    .when(!is_viewing_other, |this| {
                        this.child(self.render_tabs(on_tab_change))
                    })
                    .child(
                        div()
                            .w_full()
                            .flex_1()
                            .min_h_0()
                            .overflow_hidden()
                            .when(
                                active_tab == CareerTab::Matches || is_viewing_other,
                                |this| {
                                    this.child(self.render_matches_tab(
                                        matches,
                                        stats,
                                        on_select_match,
                                        on_career_bar,
                                        on_click_teammate,
                                        on_replay,
                                    ))
                                },
                            )
                            .when(
                                active_tab == CareerTab::Mastery && !is_viewing_other,
                                |this| {
                                    this.child(self.render_mastery_tab(
                                        mastery_focus,
                                        mastery_search_focused,
                                        on_mastery_query,
                                        on_mastery_filter,
                                        on_refresh,
                                    ))
                                },
                            )
                            .when(active_tab == CareerTab::Loot && !is_viewing_other, |this| {
                                this.child(self.render_loot_tab(
                                    loot_refresh,
                                    loot_value_focus,
                                    loot_value_focused,
                                    on_loot_action,
                                ))
                            }),
                    ),
            )
            .into_any_element()
    }

    fn render_summoner_header(
        &self,
        on_refresh: impl Fn(&mut Window, &mut App) + 'static + Clone,
        on_copy_name: impl Fn(&mut Window, &mut App) + 'static + Clone,
        on_history: impl Fn(&mut Window, &mut App) + 'static + Clone,
        on_back_to_me: impl Fn(&mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let game_name = self
            .summoner
            .as_ref()
            .and_then(|s| s.game_name.clone().filter(|n| !n.is_empty()))
            .or_else(|| {
                self.summoner
                    .as_ref()
                    .and_then(|s| s.display_name.clone())
                    .filter(|n| !n.is_empty())
            })
            .unwrap_or_else(|| "召唤师".to_string());
        let tag_line = self
            .summoner
            .as_ref()
            .and_then(|s| s.tag_line.clone())
            .filter(|t| !t.is_empty());
        let level = self
            .summoner
            .as_ref()
            .and_then(|s| s.summoner_level)
            .unwrap_or(0);
        let progress = self
            .summoner
            .as_ref()
            .and_then(|s| s.percent_complete_for_next_level)
            .unwrap_or(0);
        let icon_path = self
            .summoner
            .as_ref()
            .map(|s| s.icon_path())
            .unwrap_or_else(|| "/lol-game-data/assets/v1/profile-icons/29.jpg".to_string());
        let is_private = self
            .summoner
            .as_ref()
            .and_then(|s| s.privacy.as_deref())
            .is_some_and(|p| p.eq_ignore_ascii_case("PRIVATE"));
        let copied = self.copied;
        let is_viewing_other = self.is_viewing_other;

        div()
            .w(px(1000.0))
            .bg(Theme::over_page(Theme::CARD_BG()))
            .border_1()
            .border_color(Theme::CARD_BORDER())
            .rounded_xl()
            .p(px(24.0))
            .shadow_sm()
            .flex()
            .items_center()
            .mb(px(24.0))
            .flex_none()
            .child(div().flex_1())
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(20.0))
                    .child(Self::render_level_ring(icon_path, level, progress))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .gap(px(4.0))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(6.0))
                                            .child(
                                                div()
                                                    .text_size(px(26.0))
                                                    .font_weight(FontWeight::EXTRA_BOLD)
                                                    .text_color(Theme::TEXT_PRIMARY())
                                                    .child(game_name),
                                            )
                                            .when(is_private, |this| {
                                                this.child(icon(
                                                    "icons/lock.svg",
                                                    17.0,
                                                    Theme::GOLD(),
                                                ))
                                            })
                                            .when(copied, |this| {
                                                this.child(
                                                    div()
                                                        .text_size(px(12.0))
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(Theme::WIN_TEXT())
                                                        .child("✓ 已复制"),
                                                )
                                            }),
                                    )
                                    .when_some(tag_line, |this, tag| {
                                        this.child(
                                            div()
                                                .text_size(px(13.6))
                                                .text_color(Theme::TEXT_MUTED())
                                                .child(format!("# {}", tag)),
                                        )
                                    }),
                            )
                            .child({
                                let cb = on_copy_name.clone();
                                div()
                                    .size(px(28.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(Theme::CARD_BORDER())
                                    .bg(Theme::HOVER_BG())
                                    .cursor_pointer()
                                    .hover(|s| {
                                        s.bg(Theme::HOVER_BG_STRONG())
                                            .border_color(Theme::ACCENT_PRIMARY())
                                    })
                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                        cb(win, app)
                                    })
                                    .child(icon("icons/copy.svg", 14.0, Theme::TEXT_MUTED()))
                            }),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_end()
                    .gap(px(8.0))
                    .when(is_viewing_other, |this| {
                        this.child(Self::action_button_styled(
                            "返回我的生涯",
                            Theme::ACCENT_PRIMARY(),
                            Theme::ACCENT_LIGHT(),
                            on_back_to_me,
                        ))
                    })
                    .child(Self::action_button("刷新", on_refresh))
                    .child(Self::action_button("历史战绩", on_history)),
            )
    }

    fn render_level_ring(icon_path: String, level: u32, progress: u32) -> impl IntoElement {
        div()
            .relative()
            .flex_none()
            .size(px(104.0))
            .child(
                canvas(
                    |bounds, _window, _cx| bounds,
                    move |bounds, _, window, _cx| {
                        let size = f32::from(bounds.size.width).min(f32::from(bounds.size.height));
                        let radius = size * 0.45;
                        let stroke = size * 0.05;
                        let center = bounds.center();
                        if let Some(track) = arc_path(center, radius, 120.0, 300.0, stroke) {
                            window.paint_path(track, rgb(0xe2e8f0));
                        }
                        let sweep = 300.0 * (progress.min(100) as f32 / 100.0);
                        if sweep > 0.5 {
                            if let Some(fill) = arc_path(center, radius, 120.0, sweep, stroke) {
                                window.paint_path(fill, Theme::ACCENT_PRIMARY());
                            }
                        }
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size(px(104.0)),
            )
            .child(
                div()
                    .absolute()
                    .top(px(13.0))
                    .left(px(13.0))
                    .size(px(78.0))
                    .rounded_full()
                    .overflow_hidden()
                    .child(
                        LcuImage::new(icon_path)
                            .size(px(78.0))
                            .shape(LcuImageShape::Circle)
                            .placeholder("悠"),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .text_size(px(11.5))
                    .font_weight(FontWeight::BOLD)
                    .line_height(px(12.0))
                    .text_color(Theme::TEXT_PRIMARY())
                    .child(level.to_string()),
            )
    }

    fn action_button(
        label: &'static str,
        on_click: impl Fn(&mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        div()
            .px(px(16.0))
            .py(px(6.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(Theme::CARD_BORDER())
            .bg(Theme::CARD_BG())
            .text_size(px(13.1))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(Theme::TEXT_PRIMARY())
            .cursor_pointer()
            .hover(|s| {
                s.bg(Theme::CARD_BG_HOVER())
                    .border_color(Theme::ACCENT_PRIMARY())
            })
            .on_mouse_down(MouseButton::Left, move |_ev, win, app| on_click(win, app))
            .child(label)
    }

    fn action_button_styled(
        label: &'static str,
        border_color: Hsla,
        bg_color: Hsla,
        on_click: impl Fn(&mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        div()
            .px(px(16.0))
            .py(px(6.0))
            .rounded_md()
            .border_1()
            .border_color(border_color)
            .bg(bg_color)
            .text_size(px(13.0))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(Theme::ACCENT_PRIMARY())
            .cursor_pointer()
            .hover(|s| s.bg(Theme::HOVER_BG_STRONG()))
            .on_mouse_down(MouseButton::Left, move |_ev, win, app| on_click(win, app))
            .child(label)
    }

    fn render_tabs(
        &self,
        on_tab_change: impl Fn(CareerTab, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        div()
            .w(px(1000.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            .mt(px(12.0))
            .mb(px(16.0))
            .pb(px(8.0))
            .border_b_1()
            .border_color(Theme::CARD_BORDER())
            .flex_none()
            .children(
                [CareerTab::Matches, CareerTab::Loot, CareerTab::Mastery]
                    .into_iter()
                    .map(|tab| {
                        let is_active = self.active_tab == tab;
                        let cb = on_tab_change.clone();
                        div()
                            .px(px(16.0))
                            .py(px(6.0))
                            .rounded(px(8.0))
                            .border_1()
                            .cursor_pointer()
                            .text_size(px(14.1))
                            .font_weight(FontWeight::BOLD)
                            // 选中底色预合成到页面底色，Mica 开启时与关闭时保持同一像素
                            .bg(if is_active {
                                Theme::over_page(Theme::ACCENT_ALPHA_10())
                            } else {
                                transparent_black()
                            })
                            .text_color(if is_active {
                                Theme::ACCENT_PRIMARY()
                            } else {
                                Theme::TEXT_MUTED()
                            })
                            .border_color(if is_active {
                                Theme::ACCENT_ALPHA_20()
                            } else {
                                transparent_black()
                            })
                            .when(!is_active, |this| {
                                this.hover(|s| {
                                    s.bg(Theme::CARD_BG_HOVER())
                                        .text_color(Theme::ACCENT_PRIMARY())
                                })
                            })
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                cb(tab, win, app);
                            })
                            .child(tab.label())
                    }),
            )
    }

    fn render_matches_tab(
        &self,
        matches: &[MatchItem],
        stats: &CareerHeaderStats,
        on_select_match: impl Fn(u64, &mut Window, &mut App) + 'static + Clone,
        on_career_bar: impl Fn(CareerBarAction, &mut Window, &mut App) + 'static + Clone,
        on_click_teammate: impl Fn(RecentTeammate, &mut Window, &mut App) + 'static + Clone,
        on_replay: impl Fn(u64, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let shown: Vec<&MatchItem> = matches
            .iter()
            .filter(|m| {
                self.selected_queue
                    .is_none_or(|qid| m.display.queue_id == qid)
            })
            .collect();
        let is_empty = shown.is_empty();

        div()
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .overflow_hidden()
            // 2. 段位表 (1000px)
            .child(self.render_ranked_card())
            // 3. 近期对局横条 (1000px)
            .child(self.render_recent_bar(
                matches,
                stats,
                on_career_bar,
                on_click_teammate,
            ))
            // 4. 对局明细列表 (1000px)
            .child(
                div()
                    .id("career-scroll-area")
                    .w(px(1000.0))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.matches_scroll_handle)
                    .mt(px(16.0))
                    .child(if is_empty {
                        div()
                            .w(px(1000.0))
                            .bg(Theme::CARD_BG())
                            .border_1()
                            .border_color(Theme::CARD_BORDER())
                            .rounded_xl()
                            .p_8()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(Theme::TEXT_PRIMARY())
                                    .child("暂无近期的对局历史记录"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::TEXT_MUTED())
                                    .child("请在英雄联盟客户端完成对局，或点击资料卡右侧的「刷新」拉取最新战绩"),
                            )
                            .into_any_element()
                    } else {
                        let replay_cb = on_replay.clone();
                        div()
                            .w(px(1000.0))
                            .flex()
                            .flex_col()
                            .gap(px(10.0))
                            .pb(px(24.0))
                            .children(
                                shown
                                    .into_iter()
                                    .map(move |m| Self::render_match_card(m, on_select_match.clone(), replay_cb.clone())),
                            )
                            .into_any_element()
                    }),
            )
    }

    fn render_ranked_card(&self) -> impl IntoElement {
        const COLUMNS: [(&str, f32); 9] = [
            ("类型", 96.0),
            ("总场次", 76.0),
            ("胜率", 76.0),
            ("胜场", 64.0),
            ("负场", 64.0),
            ("段位", 160.0),
            ("胜点", 72.0),
            ("赛季最高", 180.0),
            ("上赛季结算", 180.0),
        ];

        let rows: Vec<[String; 9]> = match self.ranked.as_ref() {
            Some(stats) => vec![
                ranked_row("单双排位", stats.queue_map.get("RANKED_SOLO_5x5")),
                ranked_row("灵活排位", stats.queue_map.get("RANKED_FLEX_SR")),
            ],
            None => vec![ranked_row("单双排位", None), ranked_row("灵活排位", None)],
        };

        div()
            .w(px(1000.0))
            .flex_none()
            // 表体近白底预合成到页面底色；表头的 HOVER_BG 继续叠在这层之上
            .bg(Theme::over_page(Theme::CARD_BG()))
            .border_1()
            .border_color(Theme::CARD_BORDER())
            .rounded_xl()
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .rounded_t(px(11.0))
                    .items_center()
                    .px(px(16.0))
                    .py(px(6.0))
                    .bg(Theme::HOVER_BG())
                    .border_b_1()
                    .border_color(Theme::CARD_BORDER())
                    .children(COLUMNS.iter().map(|(label, width)| {
                        ranked_cell(
                            label.to_string(),
                            *width,
                            Theme::TEXT_MUTED(),
                            13.1,
                            FontWeight::SEMIBOLD,
                        )
                    })),
            )
            .children(rows.into_iter().enumerate().map(|(idx, values)| {
                let is_last = idx + 1 == 2;
                let mut row = div().flex().items_center().px(px(16.0)).py(px(8.0));
                if !is_last {
                    row = row.border_b_1().border_color(Theme::CARD_BORDER());
                }
                row.children(values.into_iter().zip(COLUMNS.iter()).enumerate().map(
                    |(col, (value, (_, width)))| {
                        let (color, weight) = match col {
                            0 => (Theme::TEXT_PRIMARY(), FontWeight::SEMIBOLD),
                            // 段位列在 Vue 侧为品牌青加粗
                            5 => (Theme::ACCENT_PRIMARY(), FontWeight::BOLD),
                            _ => (Theme::TEXT_PRIMARY(), FontWeight::NORMAL),
                        };
                        ranked_cell(value, *width, color, 13.1, weight)
                    },
                ))
            }))
    }

    fn render_recent_bar(
        &self,
        matches: &[MatchItem],
        stats: &CareerHeaderStats,
        on_bar: impl Fn(CareerBarAction, &mut Window, &mut App) + 'static + Clone,
        on_click_teammate: impl Fn(RecentTeammate, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let positions = top_positions(matches, 2);
        let champions = top_champions(matches, 6);
        let queue_label = QUEUE_OPTIONS
            .iter()
            .find(|o| o.id == self.selected_queue)
            .map(|o| o.label)
            .unwrap_or("全部");
        // 分路占比以「有分路信息的对局」为分母（对齐 Vue computeAggregatedPositions）
        let valid_total = matches
            .iter()
            .filter(|m| position_to_zh(m.display.position.as_deref()) != "全能")
            .count()
            .max(1);
        let sum_kills: i32 = matches.iter().map(|m| m.display.kills).sum();
        let sum_deaths: i32 = matches.iter().map(|m| m.display.deaths).sum();
        let sum_assists: i32 = matches.iter().map(|m| m.display.assists).sum();

        div()
            .w(px(1000.0))
            .flex_none()
            .relative()
            // Vue .recent-summary-bar 用 --settings-collapse-bg（浅灰蓝），并预合成到页面底色
            .bg(Theme::over_page(Theme::SETTINGS_COLLAPSE_BG()))
            .border_1()
            .border_color(Theme::CARD_BORDER())
            .rounded(px(8.0))
            .px(px(16.0))
            .py(px(10.0))
            .mt(px(16.0))
            .flex()
            .items_center()
            .gap(px(16.0))
            // 近期对局汇总文本流（对齐 Vue .summary-text）
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .text_size(px(13.1))
                    .text_color(Theme::TEXT_MUTED())
                    .whitespace_nowrap()
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::TEXT_PRIMARY())
                            .mr(px(4.0))
                            .child(format!("近期对局（最近 {} 场）", stats.total_games)),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(Theme::WIN_TEXT())
                            .child(format!("胜: {}", stats.wins)),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(Theme::LOSS_TEXT())
                            .child(format!("负: {}", stats.losses)),
                    )
                    .child(
                        div()
                            .text_color(Theme::TEXT_MUTED())
                            .ml(px(8.0))
                            .child("KDA:"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(3.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(Theme::TEXT_PRIMARY())
                            .child(format!("{} /", sum_kills))
                            .child(
                                div()
                                    .text_color(Theme::DEATH_RED())
                                    .child(sum_deaths.to_string()),
                            )
                            .child(format!("/ {}", sum_assists)),
                    )
                    .child(
                        div()
                            .text_color(Theme::TEXT_MUTED())
                            .child(format!("({:.1})", stats.avg_kda_ratio)),
                    ),
            )
            // 常玩位置胶囊
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(8.0))
                    .py(px(3.0))
                    .rounded(px(6.0))
                    .bg(Theme::HOVER_BG())
                    .border_1()
                    .border_color(Theme::CARD_BORDER())
                    .when(positions.is_empty(), |this| {
                        this.child(
                            div()
                                .text_size(px(11.5))
                                .text_color(Theme::TEXT_MUTED())
                                .child("常玩位置: 暂无"),
                        )
                    })
                    .when(!positions.is_empty(), |this| {
                        this.child(
                            div()
                                .text_size(px(11.5))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(Theme::TEXT_MUTED())
                                .child("常玩位置:"),
                        )
                    })
                    .children(positions.into_iter().map(|(label, count)| {
                        div()
                            .flex()
                            .items_center()
                            .gap(px(3.0))
                            .text_size(px(11.5))
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY())
                                    .child(label),
                            )
                            .child(
                                div()
                                    .text_color(Theme::TEXT_MUTED())
                                    .child(format!("{}%", count * 100 / valid_total)),
                            )
                    })),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .when(champions.is_empty(), |this| {
                        this.child(
                            div()
                                .text_size(px(11.5))
                                .text_color(Theme::TEXT_MUTED())
                                .child("暂无英雄数据"),
                        )
                    })
                    .children(champions.into_iter().map(|(name, icon_url, count)| {
                        div()
                            .id(SharedString::from(format!("recent-champ-{}", name)))
                            .relative()
                            .size(px(28.0))
                            .child(
                                LcuImage::new(icon_url)
                                    .size(px(28.0))
                                    .shape(LcuImageShape::Circle)
                                    .placeholder("英"),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .bottom(px(-2.0))
                                    .right(px(-2.0))
                                    .px(px(3.0))
                                    .rounded_full()
                                    .bg(Theme::CARD_BG())
                                    .border_1()
                                    .border_color(Theme::CARD_BORDER())
                                    .text_size(px(9.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY())
                                    .child(count.to_string()),
                            )
                    })),
            )
            .child(div().flex_1())
            // 动作 1：最近队友（在队列下拉之前）
            .child({
                let on_toggle = on_bar.clone();
                dropdown_trigger(
                    "career-teammates-trigger",
                    "最近队友".to_string(),
                    self.popup == Some(CareerPopup::Teammates),
                    self.render_teammates_menu(on_click_teammate),
                    move |win, app| {
                        on_toggle(
                            CareerBarAction::TogglePopup(CareerPopup::Teammates),
                            win,
                            app,
                        )
                    },
                )
            })
            // 动作 2：队列下拉（最右侧）
            .child({
                let on_toggle = on_bar.clone();
                let items = QUEUE_OPTIONS.into_iter().map(|option| {
                    let cb = on_bar.clone();
                    dropdown_item(
                        option.label,
                        self.selected_queue == option.id,
                        move |win, app| cb(CareerBarAction::SelectQueue(option.id), win, app),
                    )
                });
                dropdown_trigger(
                    "career-queue-trigger",
                    queue_label.to_string(),
                    self.popup == Some(CareerPopup::QueueMenu),
                    dropdown_menu(150.0, items),
                    move |win, app| {
                        on_toggle(
                            CareerBarAction::TogglePopup(CareerPopup::QueueMenu),
                            win,
                            app,
                        )
                    },
                )
            })
    }

    fn render_teammates_menu(
        &self,
        on_click_teammate: impl Fn(RecentTeammate, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let empty = self.recent_teammates.is_empty();
        div()
            .id("teammates-menu-scroll")
            .w(px(420.0))
            .max_h(px(400.0))
            .rounded(px(12.0))
            .p(px(12.0))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pb(px(4.0))
                    .border_b_1()
                    .border_color(Theme::CARD_BORDER())
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::TEXT_PRIMARY())
                            .child("最近队友"),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(Theme::TEXT_MUTED())
                            .child("近 20 场同队数据"),
                    ),
            )
            .child(if empty {
                div()
                    .py(px(24.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(12.0))
                    .text_color(Theme::TEXT_MUTED())
                    .child("暂无最近队友数据")
                    .into_any_element()
            } else {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .children(self.recent_teammates.iter().map(|tm| {
                        let tm_clone = tm.clone();
                        let click_cb = on_click_teammate.clone();
                        let time_str = format_local_time(tm.last_play_time as i64);

                        div()
                            .p(px(8.0))
                            .rounded_lg()
                            .border_1()
                            .border_color(Theme::CARD_BORDER())
                            .bg(Theme::CARD_BG())
                            .hover(|s| {
                                s.bg(Theme::HOVER_BG())
                                    .border_color(Theme::ACCENT_PRIMARY())
                            })
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                click_cb(tm_clone.clone(), win, app);
                            })
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(10.0))
                                    .child(
                                        LcuImage::new(tm.icon.clone())
                                            .size(px(36.0))
                                            .shape(LcuImageShape::Circle)
                                            .placeholder("友".to_string()),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.0))
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_1()
                                                    .child(
                                                        div()
                                                            .text_size(px(12.5))
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .text_color(Theme::TEXT_PRIMARY())
                                                            .child(tm.name.clone()),
                                                    )
                                                    .when_some(tm.tag.as_ref(), |this, tag| {
                                                        this.child(
                                                            div()
                                                                .px(px(4.0))
                                                                .py(px(1.0))
                                                                .rounded_sm()
                                                                .bg(Theme::BADGE_BG())
                                                                .text_size(px(10.0))
                                                                .text_color(Theme::ACCENT_PRIMARY())
                                                                .child(tag.clone()),
                                                        )
                                                    }),
                                            )
                                            .when(!time_str.is_empty(), |this| {
                                                this.child(
                                                    div()
                                                        .text_size(px(10.5))
                                                        .text_color(Theme::TEXT_MUTED())
                                                        .child(time_str),
                                                )
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .text_size(px(11.5))
                                    .child(
                                        div()
                                            .text_color(Theme::TEXT_MUTED())
                                            .child(format!("总: {}", tm.total)),
                                    )
                                    .child(
                                        div()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(Theme::WIN_TEXT())
                                            .child(format!("胜: {}", tm.wins)),
                                    )
                                    .child(
                                        div()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(Theme::LOSS_TEXT())
                                            .child(format!("负: {}", tm.losses)),
                                    ),
                            )
                    }))
                    .into_any_element()
            })
    }

    pub fn render_match_card(
        m: &MatchItem,
        on_select: impl Fn(u64, &mut Window, &mut App) + 'static + Clone,
        on_replay: impl Fn(u64, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let (bg_color, border_color, win_text_color, win_label) = if m.display.remake {
            (
                Theme::REMAKE_BG(),
                Theme::REMAKE_BORDER(),
                Theme::REMAKE_TEXT(),
                "重开",
            )
        } else if m.display.win {
            (
                Theme::WIN_BG(),
                Theme::WIN_BORDER(),
                Theme::WIN_TEXT(),
                "胜利",
            )
        } else {
            (
                Theme::LOSS_BG(),
                Theme::LOSS_BORDER(),
                Theme::LOSS_TEXT(),
                "失败",
            )
        };
        // 半透明胜负底色先合成到页面底色，避免 GPUI 在背景之下绘制的阴影透进卡片内部
        let bg_color = Theme::over_page(bg_color);

        let champion_name = m.display.name.clone();
        let champion_icon_url = m.display.champion_icon_url.clone();
        let spell1_icon_url = m.display.spell1_icon_url.clone();
        let spell2_icon_url = m.display.spell2_icon_url.clone();
        let rune_icon_url = m.display.rune_icon_url.clone();
        let queue_name = queue_id_to_name(m.display.queue_id, &m.display.map);
        let map_name = m.display.map.clone();
        let duration_str = m.display.duration.clone();
        let time_str = format_local_datetime(m.display.time_stamp as i64);
        let kda_ratio = m.display.kda.clone();
        let cs_str = m.display.cs.to_string();
        let gold_str = format_num(m.display.gold as i64);
        let pos_str = position_to_zh(m.display.position.as_deref());
        let item_urls = m.display.item_icon_urls.clone();
        let champ_level = m.display.champ_level;
        let kills = m.display.kills;
        let deaths = m.display.deaths;
        let assists = m.display.assists;
        let game_id = m.display.game_id;
        let queue_id = m.display.queue_id;

        let kda_val = kda_ratio.parse::<f32>().unwrap_or(0.0);
        let kda_color = if kda_val >= 5.0 {
            hsla(330.0 / 360.0, 0.80, 0.50, 1.0)
        } else if kda_val >= 3.0 {
            hsla(217.0 / 360.0, 0.90, 0.55, 1.0)
        } else {
            Theme::TEXT_MUTED()
        };

        let select_cb = on_select.clone();
        // 回放按钮仅卡片 hover 时浮现（对齐 Vue .replay-action-panel 的 opacity 过渡）
        let group_id = SharedString::from(format!("match-card-{}", game_id));

        div()
            .w(px(1000.0))
            .h(px(82.0))
            .flex_shrink_0()
            .rounded(px(10.0))
            .border_1()
            .border_color(border_color)
            .bg(bg_color)
            .px(px(20.0))
            .py(px(14.0))
            .shadow_sm()
            .group(group_id.clone())
            .hover(|s| s.shadow_md())
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                select_cb(game_id, win, app);
            })
            .flex()
            .items_center()
            // 1. 英雄面板 (头像、等级、技能、符文)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .min_w(px(100.0))
                    .flex_shrink_0()
                    .child(
                        div()
                            .relative()
                            .size(px(52.0))
                            .child(
                                LcuImage::new(champion_icon_url)
                                    .size(px(52.0))
                                    .shape(LcuImageShape::Circle)
                                    .placeholder(champion_name),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .bottom(px(-2.0))
                                    .right(px(-2.0))
                                    .size(px(18.0))
                                    .rounded_full()
                                    .bg(Theme::CARD_BG())
                                    .border_1()
                                    .border_color(Theme::CARD_BORDER())
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY())
                                    .child(champ_level.to_string()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(3.0))
                            .items_center()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .child(
                                        div()
                                            .size(px(22.0))
                                            .rounded(px(3.0))
                                            .border_1()
                                            .border_color(Theme::CARD_BORDER())
                                            .overflow_hidden()
                                            .child(
                                                LcuImage::new(spell1_icon_url)
                                                    .size(px(22.0))
                                                    .placeholder("D"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .size(px(22.0))
                                            .rounded(px(3.0))
                                            .border_1()
                                            .border_color(Theme::CARD_BORDER())
                                            .overflow_hidden()
                                            .child(
                                                LcuImage::new(spell2_icon_url)
                                                    .size(px(22.0))
                                                    .placeholder("F"),
                                            ),
                                    ),
                            )
                            .when(queue_id != 2400 && queue_id != 2450, |this| {
                                this.child(
                                    div()
                                        .size(px(22.0))
                                        .rounded_full()
                                        .border_1()
                                        .border_color(Theme::CARD_BORDER())
                                        .overflow_hidden()
                                        .child(
                                            LcuImage::new(rune_icon_url)
                                                .size(px(22.0))
                                                .shape(LcuImageShape::Circle)
                                                .placeholder("符"),
                                        ),
                                )
                            }),
                    ),
            )
            // 2. 胜负面板 (胜负、位置、模式、时长)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .min_w(px(90.0))
                    .flex_shrink_0()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(px(16.0))
                                    .line_height(px(20.0))
                                    .font_weight(FontWeight::EXTRA_BOLD)
                                    .text_color(win_text_color)
                                    .child(win_label),
                            )
                            .when(pos_str != "全能", |this| {
                                this.child(
                                    div()
                                        .px(px(4.0))
                                        .py(px(1.0))
                                        .rounded_sm()
                                        .bg(Theme::BADGE_BG())
                                        .text_size(px(10.0))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(Theme::TEXT_SECONDARY())
                                        .child(pos_str),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(1.0))
                            .mt(px(2.0))
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .line_height(px(17.0))
                                    .text_color(Theme::TEXT_MUTED())
                                    .child(queue_name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .line_height(px(15.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Theme::TEXT_MUTED())
                                    .child(duration_str),
                            ),
                    ),
            )
            // 3. KDA 面板 (杀/死/助 + 倍率)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .min_w(px(105.0))
                    .flex_shrink_0()
                    .child(
                        div().text_size(px(16.0)).line_height(px(20.0)).child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(Theme::TEXT_PRIMARY())
                                        .child(kills.to_string()),
                                )
                                .child(div().text_color(Theme::TEXT_MUTED()).child("/"))
                                .child(
                                    div()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(Theme::LOSS_TEXT())
                                        .child(deaths.to_string()),
                                )
                                .child(div().text_color(Theme::TEXT_MUTED()).child("/"))
                                .child(
                                    div()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(Theme::TEXT_PRIMARY())
                                        .child(assists.to_string()),
                                ),
                        ),
                    )
                    .child(
                        div()
                            .mt(px(2.0))
                            .text_size(px(13.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(kda_color)
                            .child(format!("{} KDA", kda_ratio)),
                    ),
            )
            // 4. 补刀面板 (靠左)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .min_w(px(44.0))
                    .flex_shrink_0()
                    .mr(px(6.0))
                    .child(icon("icons/cs.svg", 14.0, Theme::TEXT_MUTED()))
                    .child(
                        div()
                            .text_size(px(13.6))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(Theme::TEXT_MUTED())
                            .child(cs_str),
                    ),
            )
            // 5. 装备面板 (6件常规装备 + 独立饰品 + 海克斯强化)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .min_w(px(0.0))
                    .when(
                        (queue_id == 2400 || queue_id == 2450)
                            && !m.display.augment_icon_urls.is_empty(),
                        |this| {
                            this.child(div().flex().gap(px(2.0)).mr(px(4.0)).children(
                                m.display.augment_icon_urls.iter().map(|url| {
                                    div()
                                        .size(px(22.0))
                                        .rounded(px(4.0))
                                        .border_1()
                                        .border_color(rgba(0xa855f772))
                                        .bg(rgba(0x9333ea1e))
                                        .overflow_hidden()
                                        .child(
                                            LcuImage::new(url.clone())
                                                .size(px(22.0))
                                                .placeholder("海"),
                                        )
                                }),
                            ))
                        },
                    )
                    .child(div().flex().gap(px(2.0)).children((0..6).map(|idx| {
                        let url = item_urls.get(idx).cloned().unwrap_or_default();
                        div()
                            .size(px(28.0))
                            .rounded(px(4.0))
                            .bg(Theme::HOVER_BG())
                            .border_1()
                            .border_color(Theme::CARD_BORDER())
                            .overflow_hidden()
                            .when(!url.is_empty(), |this| {
                                this.child(LcuImage::new(url).size(px(28.0)).placeholder(""))
                            })
                    })))
                    .child(
                        div()
                            .size(px(28.0))
                            .rounded(px(4.0))
                            .bg(rgba(0xf59e0b0d))
                            .border_1()
                            .border_color(rgba(0xf59e0b4c))
                            .ml(px(2.0))
                            .overflow_hidden()
                            .when_some(
                                item_urls.get(6).cloned().filter(|u| !u.is_empty()),
                                |this, url| {
                                    this.child(LcuImage::new(url).size(px(28.0)).placeholder("眼"))
                                },
                            ),
                    ),
            )
            // 6. 金币面板 (固定右侧，ml_auto)
            .child(
                div()
                    .ml_auto()
                    .mr(px(24.0))
                    .min_w(px(62.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(px(13.6))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(Theme::TEXT_MUTED())
                            .child(gold_str),
                    )
                    .child(icon("icons/gold.svg", 14.0, Theme::GOLD())),
            )
            // 7. 时间面板 (地图与日期时间)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_end()
                    .min_w(px(80.0))
                    .flex_shrink_0()
                    .whitespace_nowrap()
                    .text_size(px(13.1))
                    .text_color(Theme::TEXT_MUTED())
                    .when(!map_name.is_empty(), |this| {
                        this.child(div().child(map_name))
                    })
                    .child(div().mt(px(4.0)).child(time_str)),
            )
            // 8. 回放按钮 (点击阻止卡片事件冒泡，接通真实回放服务)
            .child({
                let replay_cb = on_replay;
                let replay_state = crate::replay_store::get_replay_state(game_id);
                div()
                    .ml(px(14.0))
                    .flex_shrink_0()
                    .opacity(0.0)
                    .group_hover(group_id.clone(), |s| s.opacity(1.0))
                    .child(
                        div()
                            .size(px(30.0))
                            .rounded_md()
                            .border_1()
                            .border_color(match replay_state {
                                crate::match_model::ReplayState::Watch => Theme::WIN_BORDER(),
                                crate::match_model::ReplayState::Downloading
                                | crate::match_model::ReplayState::Checking => {
                                    Theme::ACCENT_PRIMARY()
                                }
                                crate::match_model::ReplayState::Error => Theme::LOSS_BORDER(),
                                _ => Theme::CARD_BORDER(),
                            })
                            .bg(Theme::CARD_BG())
                            .hover(|s| {
                                s.bg(Theme::HOVER_BG())
                                    .border_color(Theme::ACCENT_PRIMARY())
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                app.stop_propagation();
                                replay_cb(game_id, win, app);
                            })
                            .child(icon(
                                match replay_state {
                                    crate::match_model::ReplayState::Watch => "icons/play.svg",
                                    crate::match_model::ReplayState::Error => "icons/refresh.svg",
                                    _ => "icons/play.svg",
                                },
                                13.0,
                                match replay_state {
                                    crate::match_model::ReplayState::Watch => Theme::WIN_TEXT(),
                                    crate::match_model::ReplayState::Error => Theme::LOSS_TEXT(),
                                    crate::match_model::ReplayState::Downloading
                                    | crate::match_model::ReplayState::Checking => {
                                        Theme::ACCENT_PRIMARY()
                                    }
                                    _ => Theme::TEXT_MUTED(),
                                },
                            )),
                    )
            })
    }

    fn render_mastery_tab(
        &self,
        focus: &FocusHandle,
        search_focused: bool,
        on_query: impl Fn(String, &mut Window, &mut App) + 'static + Clone,
        on_filter: impl Fn(MasteryFilterAction, &mut Window, &mut App) + 'static + Clone,
        on_refresh: impl Fn(&mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let total_points: i64 = self.mastery_list.iter().map(|m| m.champion_points).sum();
        let total_champs = self.mastery_list.len();
        let granted = self.mastery_list.iter().filter(|m| m.chest_granted).count();
        let eligible = self
            .mastery_list
            .iter()
            .filter(|m| m.eligible_for_chest && !m.chest_granted)
            .count();
        let top = self.mastery_list.iter().max_by(|a, b| {
            a.champion_level
                .cmp(&b.champion_level)
                .then(a.champion_points.cmp(&b.champion_points))
        });

        let query = self.mastery_query.trim().to_lowercase();
        let mut list: Vec<&ChampionMasteryItem> = self
            .mastery_list
            .iter()
            .filter(|m| {
                let text_ok = query.is_empty()
                    || m.champion_name.to_lowercase().contains(&query)
                    || m.champion_id.to_string().contains(&query);
                text_ok
                    && self.mastery_chest.matches(m)
                    && self.mastery_level.matches(m.champion_level)
            })
            .collect();
        list.sort_by(|a, b| {
            let ord = match self.mastery_sort {
                MasterySort::Points => a
                    .champion_points
                    .cmp(&b.champion_points)
                    .then(a.champion_id.cmp(&b.champion_id)),
                MasterySort::Level => a
                    .champion_level
                    .cmp(&b.champion_level)
                    .then(a.champion_points.cmp(&b.champion_points))
                    .then(a.champion_id.cmp(&b.champion_id)),
                MasterySort::Milestone => a
                    .champion_season_milestone
                    .cmp(&b.champion_season_milestone)
                    .then(a.champion_points.cmp(&b.champion_points))
                    .then(a.champion_id.cmp(&b.champion_id)),
                MasterySort::LastPlay => a
                    .last_play_time
                    .cmp(&b.last_play_time)
                    .then(a.champion_points.cmp(&b.champion_points))
                    .then(a.champion_id.cmp(&b.champion_id)),
            };
            if self.mastery_sort_desc {
                ord.reverse()
            } else {
                ord
            }
        });

        div()
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .overflow_hidden()
            .child(
                div()
                    .w(px(1000.0))
                    .flex_none()
                    .grid()
                    .grid_cols(4)
                    .gap_3()
                    .child(mastery_stat(
                        "熟练英雄总数",
                        format!("{} 位", total_champs),
                        "累计拥有熟练度记录".to_string(),
                        Theme::TEXT_PRIMARY(),
                    ))
                    .child(mastery_stat(
                        "熟练度总分",
                        format!("{} 分", format_num(total_points)),
                        "全英雄点数总和".to_string(),
                        Theme::WIN_TEXT(),
                    ))
                    .child(mastery_stat(
                        "最高等级英雄",
                        top.map(|m| format!("{} Lv.{}", m.champion_name, m.champion_level))
                            .unwrap_or_else(|| "--".to_string()),
                        top.map(|m| format!("{} 分", format_num(m.champion_points)))
                            .unwrap_or_default(),
                        Theme::ACCENT_PRIMARY(),
                    ))
                    .child(mastery_stat(
                        "里程碑宝箱",
                        format!("{} / +{} 可得", granted, eligible),
                        "本赛段里程碑宝箱追踪".to_string(),
                        Theme::GOLD(),
                    )),
            )
            .child(
                div()
                    .w(px(1000.0))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.0))
                    .px(px(16.0))
                    .py(px(10.0))
                    .rounded_xl()
                    .bg(Theme::CARD_BG())
                    .border_1()
                    .border_color(Theme::CARD_BORDER())
                    .mt(px(14.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child({
                                let cb = on_query.clone();
                                let clear = on_query.clone();
                                text_field(
                                    "mastery-search",
                                    &self.mastery_query,
                                    "搜索英雄名称...",
                                    px(180.0),
                                    focus,
                                    search_focused,
                                    TextFieldOptions {
                                        max_chars: Some(24),
                                        ..Default::default()
                                    },
                                    move |text, win, app| cb(text, win, app),
                                    move |win, app| clear(String::new(), win, app),
                                    |_text, _win, _app| {},
                                )
                            })
                            .child({
                                let cb = on_filter.clone();
                                let items = ChestFilter::ALL.into_iter().map(|value| {
                                    let item_cb = on_filter.clone();
                                    dropdown_item(
                                        value.label(),
                                        self.mastery_chest == value,
                                        move |win, app| {
                                            item_cb(MasteryFilterAction::Chest(value), win, app)
                                        },
                                    )
                                });
                                dropdown_trigger(
                                    "mastery-chest-select",
                                    self.mastery_chest.label().to_string(),
                                    self.mastery_dropdown == Some(MasteryDropdown::Chest),
                                    dropdown_menu(130.0, items),
                                    move |win, app| {
                                        cb(
                                            MasteryFilterAction::ToggleDropdown(
                                                MasteryDropdown::Chest,
                                            ),
                                            win,
                                            app,
                                        )
                                    },
                                )
                            })
                            .child({
                                let cb = on_filter.clone();
                                let items = LevelFilter::ALL.into_iter().map(|value| {
                                    let item_cb = on_filter.clone();
                                    dropdown_item(
                                        value.label(),
                                        self.mastery_level == value,
                                        move |win, app| {
                                            item_cb(MasteryFilterAction::Level(value), win, app)
                                        },
                                    )
                                });
                                dropdown_trigger(
                                    "mastery-level-select",
                                    self.mastery_level.label().to_string(),
                                    self.mastery_dropdown == Some(MasteryDropdown::Level),
                                    dropdown_menu(130.0, items),
                                    move |win, app| {
                                        cb(
                                            MasteryFilterAction::ToggleDropdown(
                                                MasteryDropdown::Level,
                                            ),
                                            win,
                                            app,
                                        )
                                    },
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Theme::TEXT_MUTED())
                                    .child("排序:"),
                            )
                            .child({
                                let cb = on_filter.clone();
                                let items = MasterySort::ALL.into_iter().map(|value| {
                                    let item_cb = on_filter.clone();
                                    dropdown_item(
                                        value.label(),
                                        self.mastery_sort == value,
                                        move |win, app| {
                                            item_cb(MasteryFilterAction::Sort(value), win, app)
                                        },
                                    )
                                });
                                dropdown_trigger(
                                    "mastery-sort-select",
                                    self.mastery_sort.label().to_string(),
                                    self.mastery_dropdown == Some(MasteryDropdown::Sort),
                                    dropdown_menu(130.0, items),
                                    move |win, app| {
                                        cb(
                                            MasteryFilterAction::ToggleDropdown(
                                                MasteryDropdown::Sort,
                                            ),
                                            win,
                                            app,
                                        )
                                    },
                                )
                            })
                            .child({
                                let cb = on_filter.clone();
                                let desc = self.mastery_sort_desc;
                                div()
                                    .size(px(28.0))
                                    .rounded_md()
                                    .border_1()
                                    .border_color(Theme::CARD_BORDER())
                                    .bg(Theme::CARD_BG())
                                    .hover(|s| {
                                        s.bg(Theme::HOVER_BG())
                                            .border_color(Theme::ACCENT_PRIMARY())
                                    })
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                        cb(MasteryFilterAction::ToggleSortDir, win, app)
                                    })
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .text_color(Theme::TEXT_PRIMARY())
                                            .child(if desc { "↓" } else { "↑" }),
                                    )
                            })
                            .child({
                                let cb = on_refresh.clone();
                                div()
                                    .size(px(28.0))
                                    .rounded_md()
                                    .border_1()
                                    .border_color(Theme::CARD_BORDER())
                                    .bg(Theme::CARD_BG())
                                    .hover(|s| {
                                        s.bg(Theme::HOVER_BG())
                                            .border_color(Theme::ACCENT_PRIMARY())
                                    })
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                        cb(win, app)
                                    })
                                    .child(
                                        div()
                                            .text_size(px(14.0))
                                            .text_color(Theme::TEXT_PRIMARY())
                                            .child("↻"),
                                    )
                            }),
                    ),
            )
            .child(
                div()
                    .id("mastery-scroll-area")
                    .w(px(1000.0))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.mastery_scroll_handle)
                    .mt(px(14.0))
                    .child(if list.is_empty() {
                        div()
                            .w(px(1000.0))
                            .bg(Theme::CARD_BG())
                            .border_1()
                            .border_color(Theme::CARD_BORDER())
                            .rounded_xl()
                            .p_8()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(Theme::TEXT_PRIMARY())
                                    .child("未找到匹配的英雄"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::TEXT_MUTED())
                                    .child("尝试清除搜索条件或刷新客户端数据"),
                            )
                            .into_any_element()
                    } else {
                        div()
                            .w(px(1000.0))
                            .grid()
                            .grid_cols(4)
                            .gap(px(14.0))
                            .pb(px(24.0))
                            .children(list.into_iter().map(mastery_card))
                            .into_any_element()
                    }),
            )
    }

    fn render_loot_tab(
        &self,
        on_refresh: impl Fn(&mut Window, &mut App) + 'static + Clone,
        value_focus: &FocusHandle,
        value_focused: bool,
        on_action: impl Fn(LootAction, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let key_count: i32 = self
            .loot_items
            .iter()
            .filter(|i| i.loot_id == "MATERIAL_key" || i.loot_id == "MATERIAL_key_premium")
            .map(|i| i.count)
            .sum();

        let materials: Vec<&LootInventoryItem> =
            self.loot_items.iter().filter(|i| i.is_openable).collect();

        let filtered = self.filtered_loot();
        let has_filtered = !filtered.is_empty();

        let mut gain_blue: i64 = 0;
        let mut gain_orange: i64 = 0;
        for id in &self.loot_selected {
            if let Some(item) = self.loot_items.iter().find(|i| &i.loot_id == id) {
                let gain = item.disenchant_value as i64 * item.count as i64;
                if loot_category(item) == "CHAMPION" {
                    gain_blue += gain;
                } else {
                    gain_orange += gain;
                }
            }
        }

        let has_selection = !self.loot_selected.is_empty();
        let selected_items: Vec<&LootInventoryItem> = self
            .loot_items
            .iter()
            .filter(|i| self.loot_selected.contains(&i.loot_id))
            .collect();
        let mut can_reroll = false;
        let mut can_upgrade = false;
        let mut has_upgrade_cost = false;
        let mut has_unlock = false;
        for item in &selected_items {
            let cat = loot_category(item);
            let excluded = matches!(cat, "MATERIAL" | "CHEST" | "ORB" | "CURRENCY");
            let owned = item.item_status.eq_ignore_ascii_case("OWNED");
            if !item.loot_type.to_uppercase().contains("STATSTONE") && !excluded {
                can_reroll = true;
            }
            if !owned && !excluded {
                can_upgrade = true;
            }
            if item.upgrade_essence_cost > 0 {
                has_upgrade_cost = true;
            } else {
                has_unlock = true;
            }
        }
        can_reroll &= has_selection;
        can_upgrade &= has_selection;
        let upgrade_label = if !has_selection || (has_upgrade_cost && has_unlock) {
            "升级 / 解锁".to_string()
        } else if has_unlock {
            "解锁永久".to_string()
        } else {
            "升级永久".to_string()
        };

        div()
            .id("loot-tab-scroll")
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.loot_scroll_handle)
            .relative()
            .gap(px(20.0))
            .pb(px(24.0))
            // 区块一：可开启战利品（材料）
            .child(
                div()
                    .w(px(1000.0))
                    .flex_none()
                    .p(px(16.0))
                    .rounded_xl()
                    .bg(Theme::CARD_BG())
                    .border_1()
                    .border_color(Theme::CARD_BORDER())
                    .flex()
                    .flex_col()
                    .gap_3()
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
                                            .text_size(px(16.0))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(Theme::TEXT_PRIMARY())
                                            .child("📦 可开启战利品"),
                                    )
                                    .child(Self::action_button("刷新", on_refresh.clone()))
                                    .when(!materials.is_empty(), |this| {
                                        this.child(
                                            div()
                                                .px(px(8.0))
                                                .py(px(3.0))
                                                .rounded_md()
                                                .bg(Theme::BADGE_BG())
                                                .text_size(px(12.0))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(Theme::GOLD())
                                                .child(format!("🔑 x{}", key_count)),
                                        )
                                    }),
                            )
                            .when(!materials.is_empty(), |this| {
                                let action_cb = on_action.clone();
                                let total_keys = key_count;
                                let can_smart_open = total_keys > 0;
                                let confirm_modal = LootModal::Confirm {
                                    action_kind: LootActionKind::SmartOpenAll,
                                    title: "一键开启全部战利品".to_string(),
                                    message: format!(
                                        "将自动开启所有拥有钥匙的宝箱与法球，当前可用钥匙 x{}。",
                                        total_keys
                                    ),
                                    selected: materials.len(),
                                    gain_blue: 0,
                                    gain_orange: 0,
                                };
                                this.child(
                                    div()
                                        .px(px(14.0))
                                        .py(px(6.0))
                                        .rounded_md()
                                        .border_1()
                                        .border_color(if can_smart_open {
                                            Theme::ACCENT_PRIMARY()
                                        } else {
                                            Theme::CARD_BORDER()
                                        })
                                        .bg(if can_smart_open {
                                            Theme::ACCENT_LIGHT()
                                        } else {
                                            Theme::BADGE_BG()
                                        })
                                        .text_size(px(13.0))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(if can_smart_open {
                                            Theme::ACCENT_PRIMARY()
                                        } else {
                                            Theme::TEXT_MUTED()
                                        })
                                        .when(can_smart_open, |el| {
                                            el.cursor_pointer().on_mouse_down(
                                                MouseButton::Left,
                                                move |_ev, win, app| {
                                                    action_cb(
                                                        LootAction::OpenConfirm(
                                                            confirm_modal.clone(),
                                                        ),
                                                        win,
                                                        app,
                                                    )
                                                },
                                            )
                                        })
                                        .child("一键开启全部"),
                                )
                            }),
                    )
                    .child(if materials.is_empty() {
                        div()
                            .py(px(24.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(12.5))
                            .text_color(Theme::TEXT_MUTED())
                            .child("点击刷新以加载可开启战利品")
                            .into_any_element()
                    } else {
                        div()
                            .grid()
                            .grid_cols(3)
                            .gap(px(12.0))
                            .children(materials.into_iter().map(|item| {
                                let action_cb = on_action.clone();
                                let is_frag = item.loot_id == "MATERIAL_key_fragment";
                                let need_key = item.loot_id.starts_with("CHEST_")
                                    && !item.loot_id.contains("masterwork");
                                let quantity_modal = LootModal::Quantity {
                                    loot_id: item.loot_id.clone(),
                                    name: item.name.clone(),
                                    owned: item.count,
                                    quantity: 1,
                                    need_key,
                                    key_count,
                                    key_name: Some("符文熔炉钥匙".to_string()),
                                    tile_path: item.tile_path.clone(),
                                    action: if is_frag {
                                        LootQuantityAction::Craft
                                    } else {
                                        LootQuantityAction::Open
                                    },
                                    recipe_name: if is_frag {
                                        "MATERIAL_key_fragment_forge".to_string()
                                    } else {
                                        format!("{}_open", item.loot_id)
                                    },
                                    ingredients: vec![item.loot_id.clone()],
                                };

                                div()
                                    .p(px(14.0))
                                    .rounded_xl()
                                    .bg(Theme::CARD_BG())
                                    .border_1()
                                    .border_color(Theme::CARD_BORDER())
                                    .hover(|s| s.border_color(Theme::ACCENT_PRIMARY()))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap(px(12.0))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(10.0))
                                            .min_w(px(0.0))
                                            .flex_1()
                                            .child(
                                                LcuImage::new(
                                                    item.tile_path.clone().unwrap_or_default(),
                                                )
                                                .size(px(44.0))
                                                .shape(LcuImageShape::Rounded)
                                                .placeholder("箱"),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .gap(px(2.0))
                                                    .min_w(px(0.0))
                                                    .child(
                                                        div()
                                                            .text_size(px(13.0))
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(Theme::TEXT_PRIMARY())
                                                            .overflow_hidden()
                                                            .whitespace_nowrap()
                                                            .child(item.name.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(11.5))
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .text_color(Theme::ACCENT_PRIMARY())
                                                            .child(format!("拥有 x{}", item.count)),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .items_end()
                                            .gap(px(6.0))
                                            .flex_none()
                                            .child(
                                                div()
                                                    .px(px(6.0))
                                                    .py(px(1.5))
                                                    .rounded_sm()
                                                    .bg(if is_frag {
                                                        rgba(0x10b9811e)
                                                    } else if need_key {
                                                        rgba(0xd977061e)
                                                    } else {
                                                        rgba(0x2563eb1e)
                                                    })
                                                    .text_size(px(10.5))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(if is_frag {
                                                        Theme::WIN_TEXT()
                                                    } else if need_key {
                                                        Theme::GOLD()
                                                    } else {
                                                        Theme::ACCENT_PRIMARY()
                                                    })
                                                    .child(if is_frag {
                                                        "3合1合成"
                                                    } else if need_key {
                                                        "需要钥匙"
                                                    } else {
                                                        "无需钥匙"
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .px(px(12.0))
                                                    .py(px(4.0))
                                                    .rounded_md()
                                                    .border_1()
                                                    .border_color(Theme::ACCENT_PRIMARY())
                                                    .bg(Theme::ACCENT_LIGHT())
                                                    .text_size(px(12.0))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(Theme::ACCENT_PRIMARY())
                                                    .cursor_pointer()
                                                    .hover(|s| s.bg(Theme::HOVER_BG_STRONG()))
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        move |_ev, win, app| {
                                                            action_cb(
                                                                LootAction::OpenQuantity(
                                                                    quantity_modal.clone(),
                                                                ),
                                                                win,
                                                                app,
                                                            )
                                                        },
                                                    )
                                                    .child(if is_frag {
                                                        "合成"
                                                    } else {
                                                        "开启"
                                                    }),
                                            ),
                                    )
                            }))
                            .into_any_element()
                    }),
            )
            // 区块二：碎片库存管理
            .child(
                div()
                    .w(px(1000.0))
                    .flex_none()
                    .p(px(16.0))
                    .rounded_xl()
                    .bg(Theme::CARD_BG())
                    .border_1()
                    .border_color(Theme::CARD_BORDER())
                    .flex()
                    .flex_col()
                    .gap_3()
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
                                            .text_size(px(16.0))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(Theme::TEXT_PRIMARY())
                                            .child("💎 碎片库存管理"),
                                    )
                                    .child(Self::action_button("刷新", on_refresh)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .px(px(10.0))
                                    .py(px(4.0))
                                    .rounded_full()
                                    .bg(Theme::BADGE_BG())
                                    .border_1()
                                    .border_color(Theme::CARD_BORDER())
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(Theme::ACCENT_PRIMARY())
                                            .child(format!(
                                                "🔵 {}",
                                                format_num(self.essences.blue_essence as i64)
                                            )),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(Theme::GOLD())
                                            .child(format!(
                                                "🟠 {}",
                                                format_num(self.essences.orange_essence as i64)
                                            )),
                                    ),
                            ),
                    )
                    // 水平筛选栏（单行紧凑对齐，不折行）
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .px(px(12.0))
                            .py(px(8.0))
                            .rounded_lg()
                            .bg(Theme::HOVER_BG())
                            .border_1()
                            .border_color(Theme::CARD_BORDER())
                            .child(loot_filter_label("碎片类型"))
                            .child({
                                let cb = on_action.clone();
                                let items = LootTypeFilter::ALL.into_iter().map(|value| {
                                    let item_cb = on_action.clone();
                                    dropdown_item(
                                        value.label(),
                                        self.loot_filter_type == value,
                                        move |win, app| {
                                            item_cb(LootAction::SetType(value), win, app)
                                        },
                                    )
                                });
                                dropdown_trigger(
                                    "loot-type-select",
                                    self.loot_filter_type.label().to_string(),
                                    self.loot_dropdown == Some(LootDropdown::Type),
                                    dropdown_menu(115.0, items),
                                    move |win, app| {
                                        cb(LootAction::ToggleDropdown(LootDropdown::Type), win, app)
                                    },
                                )
                            })
                            .child(loot_filter_label("拥有状态"))
                            .child({
                                let cb = on_action.clone();
                                let items = LootOwnedFilter::ALL.into_iter().map(|value| {
                                    let item_cb = on_action.clone();
                                    dropdown_item(
                                        value.label(),
                                        self.loot_filter_owned == value,
                                        move |win, app| {
                                            item_cb(LootAction::SetOwned(value), win, app)
                                        },
                                    )
                                });
                                dropdown_trigger(
                                    "loot-owned-select",
                                    self.loot_filter_owned.label().to_string(),
                                    self.loot_dropdown == Some(LootDropdown::Owned),
                                    dropdown_menu(100.0, items),
                                    move |win, app| {
                                        cb(
                                            LootAction::ToggleDropdown(LootDropdown::Owned),
                                            win,
                                            app,
                                        )
                                    },
                                )
                            })
                            .child(loot_filter_label("价值基准"))
                            .child({
                                let cb = on_action.clone();
                                let items = LootValueBasis::ALL.into_iter().map(|value| {
                                    let item_cb = on_action.clone();
                                    dropdown_item(
                                        value.label(),
                                        self.loot_filter_value_basis == value,
                                        move |win, app| {
                                            item_cb(LootAction::SetValueBasis(value), win, app)
                                        },
                                    )
                                });
                                dropdown_trigger(
                                    "loot-basis-select",
                                    self.loot_filter_value_basis.label().to_string(),
                                    self.loot_dropdown == Some(LootDropdown::ValueBasis),
                                    dropdown_menu(115.0, items),
                                    move |win, app| {
                                        cb(
                                            LootAction::ToggleDropdown(LootDropdown::ValueBasis),
                                            win,
                                            app,
                                        )
                                    },
                                )
                            })
                            .child(loot_filter_label("价值"))
                            .child({
                                let cb = on_action.clone();
                                let items = LootOperator::ALL.into_iter().map(|value| {
                                    let item_cb = on_action.clone();
                                    dropdown_item(
                                        value.label(),
                                        self.loot_filter_operator == value,
                                        move |win, app| {
                                            item_cb(LootAction::SetOperator(value), win, app)
                                        },
                                    )
                                });
                                dropdown_trigger(
                                    "loot-operator-select",
                                    self.loot_filter_operator.label().to_string(),
                                    self.loot_dropdown == Some(LootDropdown::Operator),
                                    dropdown_menu(95.0, items),
                                    move |win, app| {
                                        cb(
                                            LootAction::ToggleDropdown(LootDropdown::Operator),
                                            win,
                                            app,
                                        )
                                    },
                                )
                            })
                            .child({
                                let cb = on_action.clone();
                                let clear_cb = on_action.clone();
                                text_field(
                                    "loot-value-input",
                                    &self.loot_value_input,
                                    "不限",
                                    px(90.0),
                                    value_focus,
                                    value_focused,
                                    TextFieldOptions {
                                        max_chars: Some(9),
                                        ..Default::default()
                                    },
                                    move |text, win, app| {
                                        cb(LootAction::SetValueInput(text), win, app)
                                    },
                                    move |win, app| clear_cb(LootAction::ClearValueInput, win, app),
                                    |_text, _win, _app| {},
                                )
                            }),
                    )
                    // 碎片网格
                    .child(if filtered.is_empty() {
                        div()
                            .py(px(32.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(12.5))
                            .text_color(Theme::TEXT_MUTED())
                            .child("暂无符合筛选条件的碎片")
                            .into_any_element()
                    } else {
                        div()
                            .grid()
                            .grid_cols(4)
                            .gap_3()
                            .children(filtered.into_iter().map(|item| {
                                let selected = self.loot_selected.contains(&item.loot_id);
                                let cb = on_action.clone();
                                let id = item.loot_id.clone();
                                let cat = loot_category(item);
                                let owned = item.item_status.eq_ignore_ascii_case("OWNED");
                                let essence_color = if cat == "CHAMPION" {
                                    Theme::ACCENT_PRIMARY()
                                } else {
                                    Theme::GOLD()
                                };
                                div()
                                    .relative()
                                    .p(px(12.0))
                                    .rounded_xl()
                                    .bg(if selected {
                                        Theme::ACCENT_LIGHT()
                                    } else {
                                        Theme::CARD_BG()
                                    })
                                    .border_1()
                                    .border_color(if selected {
                                        Theme::ACCENT_PRIMARY()
                                    } else {
                                        Theme::CARD_BORDER()
                                    })
                                    .cursor_pointer()
                                    .hover(|s| s.border_color(Theme::ACCENT_PRIMARY()))
                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                        cb(LootAction::ToggleSelect(id.clone()), win, app)
                                    })
                                    .when(selected, |this| {
                                        this.child(
                                            div()
                                                .absolute()
                                                .top(px(-6.0))
                                                .right(px(-6.0))
                                                .size(px(18.0))
                                                .rounded_full()
                                                .bg(Theme::ACCENT_PRIMARY())
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .text_size(px(11.0))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(hsla(0.0, 0.0, 1.0, 1.0))
                                                .child("✓"),
                                        )
                                    })
                                    .child(
                                        div()
                                            .flex()
                                            .gap(px(10.0))
                                            .child(
                                                LcuImage::new(
                                                    item.tile_path.clone().unwrap_or_default(),
                                                )
                                                .size(px(46.0))
                                                .shape(LcuImageShape::Rounded)
                                                .placeholder("碎"),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .justify_between()
                                                    .min_w(px(0.0))
                                                    .flex_1()
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .items_center()
                                                            .justify_between()
                                                            .child(
                                                                div()
                                                                    .text_size(px(12.5))
                                                                    .font_weight(FontWeight::BOLD)
                                                                    .text_color(
                                                                        Theme::TEXT_PRIMARY(),
                                                                    )
                                                                    .overflow_hidden()
                                                                    .whitespace_nowrap()
                                                                    .child(item.item_desc.clone()),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_size(px(11.0))
                                                                    .font_weight(
                                                                        FontWeight::SEMIBOLD,
                                                                    )
                                                                    .text_color(
                                                                        Theme::ACCENT_PRIMARY(),
                                                                    )
                                                                    .child(format!(
                                                                        "x{}",
                                                                        item.count
                                                                    )),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .items_center()
                                                            .gap(px(6.0))
                                                            .child(
                                                                div()
                                                                    .px(px(6.0))
                                                                    .py(px(1.0))
                                                                    .rounded_sm()
                                                                    .bg(Theme::BADGE_BG())
                                                                    .text_size(px(10.0))
                                                                    .font_weight(
                                                                        FontWeight::SEMIBOLD,
                                                                    )
                                                                    .text_color(if owned {
                                                                        Theme::WIN_TEXT()
                                                                    } else {
                                                                        Theme::TEXT_MUTED()
                                                                    })
                                                                    .child(if owned {
                                                                        "已拥有"
                                                                    } else {
                                                                        "未拥有"
                                                                    }),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_size(px(11.0))
                                                                    .font_weight(FontWeight::BOLD)
                                                                    .text_color(essence_color)
                                                                    .child(format!(
                                                                        "{} {}",
                                                                        if cat == "CHAMPION" {
                                                                            "🔵"
                                                                        } else {
                                                                            "🟠"
                                                                        },
                                                                        item.disenchant_value
                                                                    )),
                                                            ),
                                                    ),
                                            ),
                                    )
                            }))
                            .into_any_element()
                    })
                    // 底部浮动批量操作栏
                    .when(has_filtered, |this| {
                        let action_cb = on_action.clone();
                        let selected_count = self.loot_selected.len();
                        let confirm_disenchant = LootModal::Confirm {
                            action_kind: LootActionKind::Disenchant,
                            title: "批量分解确认".to_string(),
                            message: format!(
                                "确定要分解选中的 {} 项战利品碎片吗？",
                                selected_count
                            ),
                            selected: selected_count,
                            gain_blue,
                            gain_orange,
                        };
                        let confirm_reroll = LootModal::Confirm {
                            action_kind: LootActionKind::Reroll,
                            title: "重铸确认".to_string(),
                            message: format!(
                                "确定要重铸选中的 {} 项碎片吗？（每 3 个碎片合成 1 个永久物品）",
                                selected_count
                            ),
                            selected: selected_count,
                            gain_blue: 0,
                            gain_orange: 0,
                        };
                        let confirm_upgrade = LootModal::Confirm {
                            action_kind: LootActionKind::Upgrade,
                            title: "批量升级 / 解锁确认".to_string(),
                            message: format!(
                                "确定要升级 / 解锁选中的 {} 项战利品吗？",
                                selected_count
                            ),
                            selected: selected_count,
                            gain_blue: 0,
                            gain_orange: 0,
                        };

                        this.child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .flex_wrap()
                                .gap_3()
                                .p(px(10.0))
                                .rounded_lg()
                                .bg(Theme::CARD_BG_HOVER())
                                .border_1()
                                .border_color(Theme::CARD_BORDER())
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child({
                                            let cb = action_cb.clone();
                                            Self::action_button("全选", move |win, app| {
                                                cb(LootAction::SelectAllFiltered, win, app)
                                            })
                                        })
                                        .child({
                                            let cb = action_cb.clone();
                                            Self::action_button("清空", move |win, app| {
                                                cb(LootAction::ClearSelection, win, app)
                                            })
                                        })
                                        .child(
                                            div()
                                                .text_size(px(12.5))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(Theme::TEXT_PRIMARY())
                                                .child(format!("已选 {} 项", selected_count)),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .when(has_selection, |this| {
                                            this.child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .child(
                                                        div()
                                                            .text_size(px(12.0))
                                                            .text_color(Theme::TEXT_MUTED())
                                                            .child("预计精粹"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(12.0))
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(Theme::ACCENT_PRIMARY())
                                                            .child(format!(
                                                                "🔵 +{}",
                                                                format_num(gain_blue)
                                                            )),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(12.0))
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(Theme::GOLD())
                                                            .child(format!(
                                                                "🟠 +{}",
                                                                format_num(gain_orange)
                                                            )),
                                                    ),
                                            )
                                        })
                                        .child({
                                            let cb = action_cb.clone();
                                            let modal = confirm_disenchant.clone();
                                            div()
                                                .px(px(12.0))
                                                .py(px(5.0))
                                                .rounded_md()
                                                .border_1()
                                                .border_color(if has_selection {
                                                    Theme::ACCENT_PRIMARY()
                                                } else {
                                                    Theme::CARD_BORDER()
                                                })
                                                .bg(if has_selection {
                                                    Theme::ACCENT_LIGHT()
                                                } else {
                                                    Theme::BADGE_BG()
                                                })
                                                .text_size(px(12.5))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(if has_selection {
                                                    Theme::ACCENT_PRIMARY()
                                                } else {
                                                    Theme::TEXT_MUTED()
                                                })
                                                .when(has_selection, |el| {
                                                    el.cursor_pointer().on_mouse_down(
                                                        MouseButton::Left,
                                                        move |_ev, win, app| {
                                                            cb(
                                                                LootAction::OpenConfirm(
                                                                    modal.clone(),
                                                                ),
                                                                win,
                                                                app,
                                                            )
                                                        },
                                                    )
                                                })
                                                .child("分解")
                                        })
                                        .child({
                                            let cb = action_cb.clone();
                                            let modal = confirm_reroll.clone();
                                            div()
                                                .px(px(12.0))
                                                .py(px(5.0))
                                                .rounded_md()
                                                .border_1()
                                                .border_color(if can_reroll {
                                                    Theme::ACCENT_PRIMARY()
                                                } else {
                                                    Theme::CARD_BORDER()
                                                })
                                                .bg(if can_reroll {
                                                    Theme::ACCENT_LIGHT()
                                                } else {
                                                    Theme::BADGE_BG()
                                                })
                                                .text_size(px(12.5))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(if can_reroll {
                                                    Theme::ACCENT_PRIMARY()
                                                } else {
                                                    Theme::TEXT_MUTED()
                                                })
                                                .when(can_reroll, |el| {
                                                    el.cursor_pointer().on_mouse_down(
                                                        MouseButton::Left,
                                                        move |_ev, win, app| {
                                                            cb(
                                                                LootAction::OpenConfirm(
                                                                    modal.clone(),
                                                                ),
                                                                win,
                                                                app,
                                                            )
                                                        },
                                                    )
                                                })
                                                .child("重铸")
                                        })
                                        .child({
                                            let cb = action_cb.clone();
                                            let modal = confirm_upgrade.clone();
                                            div()
                                                .px(px(12.0))
                                                .py(px(5.0))
                                                .rounded_md()
                                                .border_1()
                                                .border_color(if can_upgrade {
                                                    Theme::ACCENT_PRIMARY()
                                                } else {
                                                    Theme::CARD_BORDER()
                                                })
                                                .bg(if can_upgrade {
                                                    Theme::ACCENT_LIGHT()
                                                } else {
                                                    Theme::BADGE_BG()
                                                })
                                                .text_size(px(12.5))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(if can_upgrade {
                                                    Theme::ACCENT_PRIMARY()
                                                } else {
                                                    Theme::TEXT_MUTED()
                                                })
                                                .when(can_upgrade, |el| {
                                                    el.cursor_pointer().on_mouse_down(
                                                        MouseButton::Left,
                                                        move |_ev, win, app| {
                                                            cb(
                                                                LootAction::OpenConfirm(
                                                                    modal.clone(),
                                                                ),
                                                                win,
                                                                app,
                                                            )
                                                        },
                                                    )
                                                })
                                                .child(upgrade_label)
                                        }),
                                ),
                        )
                    }),
            )
            .children(self.loot_modal.as_ref().map(|modal| {
                deferred(
                    div()
                        .absolute()
                        .inset_0()
                        .bg(rgba(0x00000080))
                        .flex()
                        .items_center()
                        .justify_center()
                        .id("loot-modal-overlay")
                        .occlude()
                        .child(Self::render_loot_modal(modal, on_action.clone())),
                )
                .with_priority(10)
            }))
            .children(self.loot_progress.as_ref().map(|progress| {
                deferred(
                    div()
                        .absolute()
                        .inset_0()
                        .bg(rgba(0x00000080))
                        .flex()
                        .items_center()
                        .justify_center()
                        .id("loot-progress-overlay")
                        .occlude()
                        .child(Self::render_loot_progress(progress, on_action.clone())),
                )
                .with_priority(10)
            }))
    }

    fn render_loot_modal(
        modal: &LootModal,
        on_action: impl Fn(LootAction, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let cancel_cb = on_action.clone();
        let confirm_cb = on_action.clone();
        let modal_clone = modal.clone();

        match modal {
            LootModal::Quantity {
                loot_id,
                name,
                owned,
                quantity,
                need_key,
                key_count,
                tile_path,
                action,
                recipe_name,
                ingredients,
                key_name,
            } => {
                let max_qty = if *need_key {
                    (*owned).min(*key_count).max(0)
                } else {
                    (*owned).max(0)
                };
                let minus_modal = LootModal::Quantity {
                    loot_id: loot_id.clone(),
                    name: name.clone(),
                    owned: *owned,
                    quantity: (*quantity - 1).max(1),
                    need_key: *need_key,
                    key_count: *key_count,
                    key_name: key_name.clone(),
                    tile_path: tile_path.clone(),
                    action: *action,
                    recipe_name: recipe_name.clone(),
                    ingredients: ingredients.clone(),
                };
                let plus_modal = LootModal::Quantity {
                    loot_id: loot_id.clone(),
                    name: name.clone(),
                    owned: *owned,
                    quantity: (*quantity + 1).min(max_qty.max(1)),
                    need_key: *need_key,
                    key_count: *key_count,
                    key_name: key_name.clone(),
                    tile_path: tile_path.clone(),
                    action: *action,
                    recipe_name: recipe_name.clone(),
                    ingredients: ingredients.clone(),
                };
                let minus_cb = on_action.clone();
                let plus_cb = on_action.clone();
                let is_insufficient = max_qty <= 0;

                div()
                    .w(px(440.0))
                    .bg(Theme::CARD_BG())
                    .border_1()
                    .border_color(Theme::CARD_BORDER())
                    .rounded_xl()
                    .p(px(20.0))
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .gap(px(14.0))
                    .on_mouse_down(MouseButton::Left, |_ev, _, _| {})
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(15.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY())
                                    .child(format!("{} - {}", action.label(), name)),
                            )
                            .child({
                                let close_cb = cancel_cb.clone();
                                div()
                                    .size(px(24.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(Theme::HOVER_BG()))
                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                        close_cb(LootAction::CloseModal, win, app)
                                    })
                                    .child("✕")
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .p(px(10.0))
                            .rounded_lg()
                            .bg(Theme::HOVER_BG())
                            .child(
                                LcuImage::new(tile_path.clone().unwrap_or_default())
                                    .size(px(48.0))
                                    .shape(LcuImageShape::Rounded)
                                    .placeholder("宝"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(Theme::TEXT_PRIMARY())
                                            .child(name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(Theme::TEXT_MUTED())
                                            .child(format!("持有数量: x{}", owned)),
                                    ),
                            ),
                    )
                    .when(*need_key, |this| {
                        this.child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .text_size(px(12.5))
                                .child(div().text_color(Theme::TEXT_MUTED()).child("所需钥匙:"))
                                .child(
                                    div()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(Theme::GOLD())
                                        .child(format!("x{} (持有: {})", quantity, key_count)),
                                ),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Theme::TEXT_PRIMARY())
                                    .child(if *action == LootQuantityAction::Craft {
                                        "合成次数:"
                                    } else {
                                        "开启数量:"
                                    }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .size(px(28.0))
                                            .rounded_md()
                                            .border_1()
                                            .border_color(Theme::CARD_BORDER())
                                            .bg(Theme::CARD_BG())
                                            .hover(|s| s.bg(Theme::HOVER_BG()))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .cursor_pointer()
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                move |_ev, win, app| {
                                                    minus_cb(
                                                        LootAction::OpenQuantity(
                                                            minus_modal.clone(),
                                                        ),
                                                        win,
                                                        app,
                                                    )
                                                },
                                            )
                                            .child("−"),
                                    )
                                    .child(
                                        div()
                                            .w(px(40.0))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_size(px(14.0))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(Theme::TEXT_PRIMARY())
                                            .child(quantity.to_string()),
                                    )
                                    .child(
                                        div()
                                            .size(px(28.0))
                                            .rounded_md()
                                            .border_1()
                                            .border_color(Theme::CARD_BORDER())
                                            .bg(Theme::CARD_BG())
                                            .hover(|s| s.bg(Theme::HOVER_BG()))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .cursor_pointer()
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                move |_ev, win, app| {
                                                    plus_cb(
                                                        LootAction::OpenQuantity(
                                                            plus_modal.clone(),
                                                        ),
                                                        win,
                                                        app,
                                                    )
                                                },
                                            )
                                            .child("+"),
                                    ),
                            ),
                    )
                    .when(is_insufficient, |this| {
                        this.child(
                            div()
                                .text_size(px(12.0))
                                .text_color(Theme::LOSS_TEXT())
                                .child(if *need_key {
                                    "钥匙不足，无法开启"
                                } else {
                                    "碎片数量不足"
                                }),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap_2()
                            .mt(px(4.0))
                            .child(
                                div()
                                    .px(px(16.0))
                                    .py(px(6.0))
                                    .rounded_md()
                                    .border_1()
                                    .border_color(Theme::CARD_BORDER())
                                    .bg(Theme::CARD_BG())
                                    .text_size(px(13.0))
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                        cancel_cb(LootAction::CloseModal, win, app)
                                    })
                                    .child("取消"),
                            )
                            .child(
                                div()
                                    .px(px(16.0))
                                    .py(px(6.0))
                                    .rounded_md()
                                    .border_1()
                                    .border_color(if is_insufficient {
                                        Theme::CARD_BORDER()
                                    } else {
                                        Theme::ACCENT_PRIMARY()
                                    })
                                    .bg(if is_insufficient {
                                        Theme::BADGE_BG()
                                    } else {
                                        Theme::ACCENT_PRIMARY()
                                    })
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(if is_insufficient {
                                        Theme::TEXT_MUTED()
                                    } else {
                                        hsla(0.0, 0.0, 1.0, 1.0)
                                    })
                                    .when(!is_insufficient, |el| {
                                        el.cursor_pointer().on_mouse_down(
                                            MouseButton::Left,
                                            move |_ev, win, app| {
                                                confirm_cb(
                                                    LootAction::Confirm(modal_clone.clone()),
                                                    win,
                                                    app,
                                                )
                                            },
                                        )
                                    })
                                    .child(if *action == LootQuantityAction::Craft {
                                        format!("开始合成 ({})", quantity)
                                    } else {
                                        format!("开始开启 ({})", quantity)
                                    }),
                            ),
                    )
            }
            LootModal::Confirm {
                title,
                message,
                selected,
                gain_blue,
                gain_orange,
                ..
            } => div()
                .w(px(440.0))
                .bg(Theme::CARD_BG())
                .border_1()
                .border_color(Theme::CARD_BORDER())
                .rounded_xl()
                .p(px(20.0))
                .shadow_lg()
                .flex()
                .flex_col()
                .gap(px(14.0))
                .on_mouse_down(MouseButton::Left, |_ev, _, _| {})
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(15.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(Theme::TEXT_PRIMARY())
                                .child(title.clone()),
                        )
                        .child({
                            let close_cb = cancel_cb.clone();
                            div()
                                .size(px(24.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_md()
                                .cursor_pointer()
                                .hover(|s| s.bg(Theme::HOVER_BG()))
                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                    close_cb(LootAction::CloseModal, win, app)
                                })
                                .child("✕")
                        }),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .line_height(px(20.0))
                        .text_color(Theme::TEXT_PRIMARY())
                        .child(message.clone()),
                )
                .when(*gain_blue > 0 || *gain_orange > 0, |this| {
                    this.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            .p(px(10.0))
                            .rounded_lg()
                            .bg(Theme::HOVER_BG())
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .text_size(px(12.5))
                                    .child(div().text_color(Theme::TEXT_MUTED()).child("已选项目:"))
                                    .child(
                                        div()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(Theme::TEXT_PRIMARY())
                                            .child(format!("{} 项", selected)),
                                    ),
                            )
                            .when(*gain_blue > 0, |el| {
                                el.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .text_size(px(12.5))
                                        .child(
                                            div()
                                                .text_color(Theme::TEXT_MUTED())
                                                .child("预计获得蓝色精粹:"),
                                        )
                                        .child(
                                            div()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(Theme::ACCENT_PRIMARY())
                                                .child(format!("+{}", format_num(*gain_blue))),
                                        ),
                                )
                            })
                            .when(*gain_orange > 0, |el| {
                                el.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .text_size(px(12.5))
                                        .child(
                                            div()
                                                .text_color(Theme::TEXT_MUTED())
                                                .child("预计获得橙色精粹:"),
                                        )
                                        .child(
                                            div()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(Theme::GOLD())
                                                .child(format!("+{}", format_num(*gain_orange))),
                                        ),
                                )
                            }),
                    )
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap_2()
                        .mt(px(4.0))
                        .child(
                            div()
                                .px(px(16.0))
                                .py(px(6.0))
                                .rounded_md()
                                .border_1()
                                .border_color(Theme::CARD_BORDER())
                                .bg(Theme::CARD_BG())
                                .text_size(px(13.0))
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                    cancel_cb(LootAction::CloseModal, win, app)
                                })
                                .child("取消"),
                        )
                        .child(
                            div()
                                .px(px(16.0))
                                .py(px(6.0))
                                .rounded_md()
                                .bg(Theme::ACCENT_PRIMARY())
                                .text_size(px(13.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(hsla(0.0, 0.0, 1.0, 1.0))
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                    confirm_cb(LootAction::Confirm(modal_clone.clone()), win, app)
                                })
                                .child("确认执行"),
                        ),
                ),
        }
    }

    fn render_loot_progress(
        progress: &LootProgress,
        on_action: impl Fn(LootAction, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let is_done = progress.current >= progress.total;
        let pct = if progress.total == 0 {
            0.0
        } else {
            (progress.current as f32 / progress.total as f32).clamp(0.0, 1.0)
        };
        let close_cb = on_action.clone();

        div()
            .w(px(420.0))
            .bg(Theme::CARD_BG())
            .border_1()
            .border_color(Theme::CARD_BORDER())
            .rounded_xl()
            .p(px(20.0))
            .shadow_lg()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .on_mouse_down(MouseButton::Left, |_ev, _, _| {})
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(15.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::TEXT_PRIMARY())
                            .child(if is_done {
                                "战利品处理完成"
                            } else {
                                "正在处理战利品..."
                            }),
                    )
                    .when(is_done, |this| {
                        let cb = close_cb.clone();
                        this.child(
                            div()
                                .size(px(24.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_md()
                                .cursor_pointer()
                                .hover(|s| s.bg(Theme::HOVER_BG()))
                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                    cb(LootAction::CloseProgress, win, app)
                                })
                                .child("✕"),
                        )
                    }),
            )
            .child(
                div()
                    .text_size(px(12.5))
                    .text_color(Theme::TEXT_MUTED())
                    .child(format!("当前条目: {}", progress.current_name)),
            )
            .child(
                div()
                    .w_full()
                    .h(px(8.0))
                    .rounded_full()
                    .bg(Theme::BADGE_BG())
                    .overflow_hidden()
                    .child(
                        div()
                            .w(px(380.0 * pct))
                            .h(px(8.0))
                            .rounded_full()
                            .bg(Theme::ACCENT_PRIMARY()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_size(px(12.0))
                    .child(
                        div()
                            .text_color(Theme::TEXT_PRIMARY())
                            .child(format!("{}/{}", progress.current, progress.total)),
                    )
                    .child(
                        div()
                            .text_color(Theme::WIN_TEXT())
                            .child(format!("成功 {} 项", progress.success)),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::ACCENT_PRIMARY())
                            .child(format!("{:.0}%", pct * 100.0)),
                    ),
            )
            .when(is_done, |this| {
                let cb = close_cb.clone();
                this.child(
                    div()
                        .w_full()
                        .mt(px(8.0))
                        .py(px(7.0))
                        .rounded_md()
                        .bg(Theme::ACCENT_PRIMARY())
                        .text_size(px(13.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(hsla(0.0, 0.0, 1.0, 1.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            cb(LootAction::CloseProgress, win, app)
                        })
                        .child("确定"),
                )
            })
    }
}

fn mastery_stat(
    label: &'static str,
    value: String,
    sub: String,
    value_color: Hsla,
) -> impl IntoElement {
    div()
        .p(px(14.0))
        .rounded_xl()
        .bg(Theme::CARD_BG())
        .border_1()
        .border_color(Theme::CARD_BORDER())
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(Theme::TEXT_MUTED())
                .child(label),
        )
        .child(
            div()
                .text_size(px(18.0))
                .line_height(px(24.0))
                .font_weight(FontWeight::BOLD)
                .text_color(value_color)
                .whitespace_nowrap()
                .overflow_hidden()
                .child(value),
        )
        .child(
            div()
                .text_size(px(11.5))
                .text_color(Theme::TEXT_MUTED())
                .whitespace_nowrap()
                .overflow_hidden()
                .child(sub),
        )
}

fn mastery_card(m: &ChampionMasteryItem) -> impl IntoElement {
    let since = m.champion_points_since_last_level.max(0);
    let until = m.champion_points_until_next_level.max(0);
    let pct = if until <= 0 {
        100.0
    } else {
        (since as f32 / (since + until) as f32 * 100.0).clamp(0.0, 100.0)
    };

    let is_eligible = m.eligible_for_chest && !m.chest_granted;

    div()
        .p(px(14.0))
        .rounded_xl()
        .bg(Theme::CARD_BG())
        .border_1()
        .border_color(if is_eligible {
            Theme::AURORA_CYAN()
        } else {
            Theme::CARD_BORDER()
        })
        .hover(|s| s.border_color(Theme::ACCENT_PRIMARY()))
        .flex()
        .flex_col()
        .gap(px(10.0))
        // 头部：头像 + 等级 / 名字 + 点数
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .child(
                    div()
                        .relative()
                        .size(px(46.0))
                        .rounded_lg()
                        .border_1()
                        .border_color(Theme::CARD_BORDER())
                        .overflow_hidden()
                        .flex_none()
                        .child(
                            LcuImage::new(m.champion_icon_url.clone())
                                .size(px(46.0))
                                .shape(LcuImageShape::Rounded)
                                .placeholder(format!("{}", m.champion_id)),
                        )
                        .child(
                            div()
                                .absolute()
                                .bottom(px(-1.0))
                                .right(px(-1.0))
                                .px(px(4.0))
                                .py(px(0.5))
                                .rounded_sm()
                                .bg(if m.champion_level >= 10 {
                                    rgba(0x2563ebdd)
                                } else {
                                    rgba(0x000000bb)
                                })
                                .text_size(px(10.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(hsla(0.0, 0.0, 1.0, 1.0))
                                .child(format!("{}", m.champion_level)),
                        ),
                )
                .child(
                    div()
                        .min_w(px(0.0))
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .child(
                            div()
                                .text_size(px(14.0))
                                .line_height(px(18.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(Theme::TEXT_PRIMARY())
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .child(m.champion_name.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(Theme::TEXT_MUTED())
                                .child(format!("{} 分", format_num(m.champion_points))),
                        ),
                ),
        )
        // 进度条与说明
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(5.0))
                .child(
                    div()
                        .w_full()
                        .h(px(6.0))
                        .rounded_full()
                        .bg(Theme::BADGE_BG())
                        .overflow_hidden()
                        .child(
                            div()
                                .h_full()
                                .w(px(213.0 * pct / 100.0))
                                .rounded_full()
                                .bg(Theme::ACCENT_PRIMARY()),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .text_size(px(11.0))
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(Theme::TEXT_PRIMARY())
                                .child(format!("{:.0}%", pct)),
                        )
                        .child(div().text_color(Theme::TEXT_MUTED()).child(if until <= 0 {
                            "已满级".to_string()
                        } else {
                            format!("距下一级还需 {}", format_num(until))
                        })),
                ),
        )
        // 里程碑与宝箱
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .pt(px(8.0))
                .border_t_1()
                .border_color(Theme::CARD_BORDER())
                .text_size(px(11.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .text_color(Theme::TEXT_SECONDARY())
                                .child(format!("里程碑 {}", m.champion_season_milestone)),
                        )
                        .when(!m.milestone_grades.is_empty(), |this| {
                            this.child(div().flex().gap(px(2.0)).children(
                                m.milestone_grades.iter().take(2).map(|g| {
                                    div()
                                        .px(px(3.0))
                                        .rounded_sm()
                                        .bg(Theme::BADGE_BG())
                                        .text_size(px(9.5))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(Theme::ACCENT_PRIMARY())
                                        .child(g.clone())
                                }),
                            ))
                        }),
                )
                .child(
                    div()
                        .px_2()
                        .py(px(1.5))
                        .rounded_md()
                        .bg(if m.chest_granted {
                            Theme::WIN_BG()
                        } else if m.eligible_for_chest {
                            Theme::ACCENT_LIGHT()
                        } else {
                            Theme::BADGE_BG()
                        })
                        .text_color(if m.chest_granted {
                            Theme::WIN_TEXT()
                        } else if m.eligible_for_chest {
                            Theme::AURORA_CYAN()
                        } else {
                            Theme::TEXT_MUTED()
                        })
                        .child(if m.chest_granted {
                            "已获宝箱 🎁"
                        } else if m.eligible_for_chest {
                            "可获宝箱 📦"
                        } else {
                            "未解锁"
                        }),
                ),
        )
}

fn loot_category(item: &LootInventoryItem) -> &'static str {
    let raw = if item.display_categories.is_empty() {
        format!("{} {}", item.loot_type, item.loot_id)
    } else {
        item.display_categories.clone()
    };
    let s = raw.to_uppercase();
    if s.contains("STATSTONE") || s.contains("ETERNAL") {
        "ETERNAL"
    } else if s.contains("CHAMPION") {
        "CHAMPION"
    } else if s.contains("WARDSKIN") || s.contains("WARD") {
        "WARDSKIN"
    } else if s.contains("SKIN") {
        "SKIN"
    } else if s.contains("EMOTE") {
        "EMOTE"
    } else if s.contains("SUMMONERICON") || s.contains("ICON") {
        "SUMMONERICON"
    } else {
        "MATERIAL"
    }
}

fn loot_filter_label(text: &'static str) -> impl IntoElement {
    div()
        .text_size(px(11.0))
        .font_weight(FontWeight::BOLD)
        .text_color(Theme::TEXT_MUTED())
        .whitespace_nowrap()
        .child(text)
}
