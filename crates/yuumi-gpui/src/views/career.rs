use crate::components::dropdown::{dropdown_item, dropdown_menu, dropdown_trigger};
use crate::components::icon::icon;
use crate::components::lcu_image::{LcuImage, LcuImageShape};
use crate::components::text_field::{text_field, TextFieldOptions};
use crate::match_model::{
    compute_match_stats, position_to_zh, top_champions, top_positions, MatchItem,
};
use crate::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use yuumi_core::parsers::game_info::{rank_tier_label, RankedQueueEntry, RankedStats};
use yuumi_core::parsers::loot::{EssenceBalances, LootInventoryItem};
use yuumi_core::parsers::mastery::ChampionMasteryItem;
use yuumi_core::parsers::summoner::SummonerInfo;

/// 生成一段圆弧的描边路径（角度以正右方为 0°，顺时针为正）
///
/// GPUI 的 `PathBuilder::stroke` 只有直线与贝塞尔，圆弧按 64 段折线逼近；
/// 104px 环上每段弦高误差小于 0.1px，视觉上等价于真弧。
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

/// 排位表单元格：按权重等分整行宽度
fn ranked_cell(
    value: String,
    width: f32,
    color: Hsla,
    size: f32,
    medium: bool,
) -> impl IntoElement {
    let mut cell = div()
        .w(px(width))
        .text_size(px(size))
        .text_color(color)
        .whitespace_nowrap()
        .overflow_hidden()
        .child(value);
    if medium {
        cell = cell.font_weight(FontWeight::MEDIUM);
    }
    cell
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
        "未定级".to_string()
    };

    [
        label.to_string(),
        total.to_string(),
        if total == 0 {
            "—".to_string()
        } else {
            format!("{:.1}%", entry.win_rate())
        },
        entry.wins.to_string(),
        entry.losses.to_string(),
        rank_label,
        if entry.is_placed() {
            entry.league_points.to_string()
        } else {
            "—".to_string()
        },
        rank_label_or_dash(&entry.highest_tier, &entry.highest_division),
        rank_label_or_dash(
            &entry.previous_season_end_tier,
            &entry.previous_season_end_division,
        ),
    ]
}

/// 段位标签，未定级时返回破折号（用于「赛季最高」「上赛季结算」列）
fn rank_label_or_dash(tier: &str, division: &str) -> String {
    let label = rank_tier_label(tier, division);
    if label == "未定级" {
        "—".to_string()
    } else {
        label
    }
}

/// 近期统计横条中的单个指标块
fn bar_stat(label: &'static str, value: String, value_color: Hsla) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(2.0))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(Theme::TEXT_MUTED)
                .child(label),
        )
        .child(
            div()
                .text_size(px(14.0))
                .font_weight(FontWeight::BOLD)
                .text_color(value_color)
                .child(value),
        )
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CareerTab {
    Matches,
    Mastery,
    Loot,
}

/// 近期统计横条右侧的浮层
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CareerPopup {
    QueueMenu,
    Teammates,
}

/// 生涯页队列下拉选项（对照 Vue MatchHistoryTab.vue 的 QUEUE_OPTIONS）
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
        label: "单双排位",
        id: Some(420),
    },
    QueueOption {
        label: "灵活排位",
        id: Some(440),
    },
    QueueOption {
        label: "匹配模式",
        id: Some(430),
    },
    QueueOption {
        label: "极地大乱斗",
        id: Some(450),
    },
    QueueOption {
        label: "海克斯大乱斗",
        id: Some(2400),
    },
    QueueOption {
        label: "经典海斗",
        id: Some(2450),
    },
];

/// 横条动作
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CareerBarAction {
    TogglePopup(CareerPopup),
    ClosePopup,
    SelectQueue(Option<i32>),
}

/// 熟练度宝箱状态筛选（对照 Vue `chestFilterOptions`）
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
            Self::Eligible => "可获取宝箱",
            Self::Granted => "已获取宝箱",
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

/// 熟练度等级区间筛选（对照 Vue `levelFilterOptions`）
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
            Self::Lv10Plus => "Lv.10+",
            Self::Lv7To9 => "Lv.7-9",
            Self::Lv4To6 => "Lv.4-6",
            Self::Lv1To3 => "Lv.1-3",
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

/// 熟练度排序字段（对照 Vue `sortOptions`）
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
            Self::LastPlay => "最近对局",
        }
    }
}

/// 熟练度筛选行里的三个 n-select（Vue ChampionMasteryTab.vue:235/242/252）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MasteryDropdown {
    Chest,
    Level,
    Sort,
}

/// 熟练度筛选行触发的单一动作
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MasteryFilterAction {
    Chest(ChestFilter),
    Level(LevelFilter),
    Sort(MasterySort),
    ToggleSortDir,
    ToggleDropdown(MasteryDropdown),
    CloseDropdown,
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
    /// 复制 Riot ID 后的短暂反馈态
    pub copied: bool,
    /// 本人排位数据（单排 / 灵活）
    pub ranked: Option<RankedStats>,
    pub mastery_list: Vec<ChampionMasteryItem>,
    /// 熟练度页签的筛选与排序状态
    pub mastery_query: String,
    pub mastery_chest: ChestFilter,
    pub mastery_level: LevelFilter,
    pub mastery_sort: MasterySort,
    pub mastery_sort_desc: bool,
    /// 熟练度筛选行当前展开的下拉
    pub mastery_dropdown: Option<MasteryDropdown>,
    /// 近期统计横条：队列筛选（None = 全部）与当前浮层
    pub selected_queue: Option<i32>,
    pub popup: Option<CareerPopup>,
    pub essences: EssenceBalances,
    pub loot_items: Vec<LootInventoryItem>,
}

impl Default for CareerView {
    fn default() -> Self {
        Self {
            active_tab: CareerTab::Matches,
            summoner: None,
            copied: false,
            ranked: None,
            mastery_list: Self::mock_masteries(),
            mastery_query: String::new(),
            mastery_chest: ChestFilter::All,
            mastery_level: LevelFilter::All,
            mastery_sort: MasterySort::Points,
            mastery_sort_desc: true,
            mastery_dropdown: None,
            selected_queue: None,
            popup: None,
            essences: EssenceBalances {
                blue_essence: 32680,
                orange_essence: 8450,
                mythic_essence: 165,
            },
            loot_items: Self::mock_loots(),
        }
    }
}

impl CareerView {
    pub fn new() -> Self {
        Self::default()
    }

    fn mock_masteries() -> Vec<ChampionMasteryItem> {
        vec![
            ChampionMasteryItem {
                champion_id: 350,
                champion_name: "悠米".to_string(),
                champion_title: "魔法猫咪".to_string(),
                champion_icon_url: "/lol-game-data/assets/v1/champion-icons/350.png".to_string(),
                champion_level: 10,
                champion_points: 352400,
                champion_points_since_last_level: 52400,
                champion_points_until_next_level: 100000,
                champion_season_milestone: 4,
                milestone_grades: vec!["S+".into(), "S".into(), "S-".into()],
                chest_granted: true,
                eligible_for_chest: false,
                last_play_time: 1700000000,
            },
            ChampionMasteryItem {
                champion_id: 157,
                champion_name: "亚索".to_string(),
                champion_title: "疾风剑豪".to_string(),
                champion_icon_url: "/lol-game-data/assets/v1/champion-icons/157.png".to_string(),
                champion_level: 7,
                champion_points: 218900,
                champion_points_since_last_level: 18900,
                champion_points_until_next_level: 80000,
                champion_season_milestone: 2,
                milestone_grades: vec!["S".into()],
                chest_granted: false,
                eligible_for_chest: true,
                last_play_time: 1699900000,
            },
            ChampionMasteryItem {
                champion_id: 103,
                champion_name: "阿狸".to_string(),
                champion_title: "九尾妖狐".to_string(),
                champion_icon_url: "/lol-game-data/assets/v1/champion-icons/103.png".to_string(),
                champion_level: 6,
                champion_points: 125600,
                champion_points_since_last_level: 5600,
                champion_points_until_next_level: 40000,
                champion_season_milestone: 1,
                milestone_grades: vec!["A+".into()],
                chest_granted: false,
                eligible_for_chest: true,
                last_play_time: 1699800000,
            },
            ChampionMasteryItem {
                champion_id: 222,
                champion_name: "金克丝".to_string(),
                champion_title: "暴走萝莉".to_string(),
                champion_icon_url: "/lol-game-data/assets/v1/champion-icons/222.png".to_string(),
                champion_level: 5,
                champion_points: 84300,
                champion_points_since_last_level: 4300,
                champion_points_until_next_level: 21600,
                champion_season_milestone: 1,
                milestone_grades: vec!["S-".into()],
                chest_granted: true,
                eligible_for_chest: false,
                last_play_time: 1699500000,
            },
        ]
    }

    fn mock_loots() -> Vec<LootInventoryItem> {
        vec![
            LootInventoryItem {
                loot_id: "CHEST_hextech".to_string(),
                name: "海克斯科技宝箱".to_string(),
                item_desc: "开启获得随机皮肤碎片、英雄碎片或精粹".to_string(),
                loot_type: "CHEST".to_string(),
                count: 3,
                value: 0,
                disenchant_value: 0,
                tile_path: None,
                is_openable: true,
            },
            LootInventoryItem {
                loot_id: "CHEST_masterwork".to_string(),
                name: "杰作宝箱".to_string(),
                item_desc: "开启必定获得高品质皮肤碎片或神话精萃".to_string(),
                loot_type: "CHEST".to_string(),
                count: 2,
                value: 0,
                disenchant_value: 0,
                tile_path: None,
                is_openable: true,
            },
            LootInventoryItem {
                loot_id: "CHAMPION_RENTAL_350".to_string(),
                name: "魔法猫咪 悠米 碎片".to_string(),
                item_desc: "可升级为永久英雄或分解为蓝色精粹".to_string(),
                loot_type: "CHAMPION_RENTAL".to_string(),
                count: 2,
                value: 6300,
                disenchant_value: 1260,
                tile_path: None,
                is_openable: false,
            },
            LootInventoryItem {
                loot_id: "SKIN_RENTAL_157001".to_string(),
                name: "西部牛仔 亚索 皮肤碎片".to_string(),
                item_desc: "可使用橙色精粹升级或分解为橙色精粹".to_string(),
                loot_type: "SKIN_RENTAL".to_string(),
                count: 1,
                value: 6900,
                disenchant_value: 1050,
                tile_path: None,
                is_openable: false,
            },
        ]
    }

    /// 渲染整个生涯页面
    ///
    /// 版面顺序对照 Vue `Career.vue`：居中限宽内容列 → 资料卡 → 卡外页签 → 当前页签内容。
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        matches: &[MatchItem],
        stats: &CareerHeaderStats,
        on_tab_change: impl Fn(CareerTab, &mut Window, &mut App) + 'static + Clone,
        on_toggle_match: impl Fn(usize, &mut Window, &mut App) + 'static + Clone,
        on_refresh: impl Fn(&mut Window, &mut App) + 'static + Clone,
        on_copy_name: impl Fn(&mut Window, &mut App) + 'static + Clone,
        on_history: impl Fn(&mut Window, &mut App) + 'static + Clone,
        mastery_focus: &FocusHandle,
        mastery_search_focused: bool,
        on_mastery_query: impl Fn(String, &mut Window, &mut App) + 'static + Clone,
        on_mastery_filter: impl Fn(MasteryFilterAction, &mut Window, &mut App) + 'static + Clone,
        on_career_bar: impl Fn(CareerBarAction, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let active_tab = self.active_tab;

        div()
            .w_full()
            .px(px(24.0))
            .py(px(16.0))
            .flex()
            .flex_col()
            .items_center()
            .child(
                div()
                    .w_full()
                    .max_w(px(1000.0))
                    .flex()
                    .flex_col()
                    // 顶部召唤师资料卡（三段式）
                    .child(self.render_summoner_header(
                        on_refresh.clone(),
                        on_copy_name,
                        on_history,
                    ))
                    // 卡外侧栏页签
                    .child(self.render_tabs(on_tab_change))
                    // 主体内容区域（根据当前 Sub-Tab 切换）
                    .child(
                        div()
                            .w_full()
                            .grid()
                            .grid_cols(1)
                            .pt(px(16.0))
                            .when(active_tab == CareerTab::Matches, |this| {
                                this.child(self.render_matches_tab(
                                    matches,
                                    stats,
                                    on_toggle_match,
                                    on_career_bar,
                                ))
                            })
                            .when(active_tab == CareerTab::Mastery, |this| {
                                this.child(self.render_mastery_tab(
                                    mastery_focus,
                                    mastery_search_focused,
                                    on_mastery_query,
                                    on_mastery_filter,
                                    on_refresh,
                                ))
                            })
                            .when(active_tab == CareerTab::Loot, |this| {
                                this.child(self.render_loot_tab())
                            }),
                    ),
            )
    }

    /// 渲染资料卡：左侧弹性留空 / 中间资料组 / 右侧动作，保证资料组始终居中
    fn render_summoner_header(
        &self,
        on_refresh: impl Fn(&mut Window, &mut App) + 'static + Clone,
        on_copy_name: impl Fn(&mut Window, &mut App) + 'static + Clone,
        on_history: impl Fn(&mut Window, &mut App) + 'static + Clone,
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
            .unwrap_or(0)
            .min(100);
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

        div()
            .w_full()
            .bg(Theme::CARD_BG)
            .border_1()
            .border_color(Theme::CARD_BORDER)
            .rounded_xl()
            .px(px(24.0))
            .py(px(24.0))
            .shadow_sm()
            .flex()
            .items_center()
            // 左：弹性留空（与右侧等宽，使中间资料组以卡片中心线居中）
            .child(div().flex_1())
            // 中：头像进度环 + 昵称 / TagLine 资料组
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
                            .flex_col()
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
                                            .text_color(Theme::TEXT_PRIMARY)
                                            .child(game_name),
                                    )
                                    .when(is_private, |this| {
                                        this.child(icon("icons/lock.svg", 17.0, Theme::GOLD))
                                    })
                                    .child({
                                        let cb = on_copy_name.clone();
                                        div()
                                            .size(px(28.0))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .rounded_md()
                                            .border_1()
                                            .border_color(Theme::CARD_BORDER)
                                            .bg(Theme::HOVER_BG)
                                            .cursor_pointer()
                                            .hover(|s| {
                                                s.bg(Theme::HOVER_BG_STRONG)
                                                    .border_color(Theme::ACCENT_PRIMARY)
                                            })
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                move |_ev, win, app| cb(win, app),
                                            )
                                            .child(icon("icons/copy.svg", 14.0, Theme::TEXT_MUTED))
                                    })
                                    .when(copied, |this| {
                                        this.child(
                                            div()
                                                .text_size(px(12.0))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(Theme::WIN_TEXT)
                                                .child("已复制"),
                                        )
                                    }),
                            )
                            .when_some(tag_line, |this, tag| {
                                this.child(
                                    div()
                                        .text_size(px(13.0))
                                        .text_color(Theme::TEXT_MUTED)
                                        .child(format!("# {}", tag)),
                                )
                            }),
                    ),
            )
            // 右：竖排动作按钮（刷新 → 历史战绩）
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_end()
                    .gap(px(8.0))
                    .child(Self::action_button("刷新", on_refresh))
                    .child(Self::action_button("历史战绩", on_history)),
            )
    }

    /// 头像等级进度环：104×104，300° 弧、缺口朝下，等级贴在环下缘
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
                                window.paint_path(fill, Theme::ACCENT_PRIMARY);
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
                    .text_color(Theme::TEXT_PRIMARY)
                    .child(level.to_string()),
            )
    }

    /// 资料卡右侧的竖排动作按钮
    fn action_button(
        label: &'static str,
        on_click: impl Fn(&mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        div()
            .px(px(16.0))
            .py(px(6.0))
            .rounded_md()
            .border_1()
            .border_color(Theme::CARD_BORDER)
            .bg(Theme::CARD_BG)
            .text_size(px(13.0))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(Theme::TEXT_PRIMARY)
            .cursor_pointer()
            .hover(|s| {
                s.bg(Theme::CARD_BG_HOVER)
                    .border_color(Theme::ACCENT_PRIMARY)
            })
            .on_mouse_down(MouseButton::Left, move |_ev, win, app| on_click(win, app))
            .child(label)
    }

    /// 卡外页签行：左对齐，顺序为 生涯战绩 → 战利品 → 英雄熟练度
    fn render_tabs(
        &self,
        on_tab_change: impl Fn(CareerTab, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        div()
            .w_full()
            .flex()
            .items_center()
            .gap_2()
            .mt(px(12.0))
            .pb(px(8.0))
            .border_b_1()
            .border_color(Theme::CARD_BORDER)
            .children(
                [CareerTab::Matches, CareerTab::Loot, CareerTab::Mastery]
                    .into_iter()
                    .map(|tab| {
                        let is_active = self.active_tab == tab;
                        let cb = on_tab_change.clone();
                        div()
                            .px(px(16.0))
                            .py(px(6.0))
                            .rounded_md()
                            .border_1()
                            .cursor_pointer()
                            .text_size(px(14.0))
                            .font_weight(FontWeight::BOLD)
                            .bg(if is_active {
                                Theme::ACCENT_LIGHT
                            } else {
                                transparent_black()
                            })
                            .text_color(if is_active {
                                Theme::ACCENT_PRIMARY
                            } else {
                                Theme::TEXT_MUTED
                            })
                            .border_color(if is_active {
                                Theme::SIDEBAR_ACTIVE_BORDER
                            } else {
                                transparent_black()
                            })
                            .when(!is_active, |this| {
                                this.hover(|s| {
                                    s.bg(Theme::CARD_BG_HOVER).text_color(Theme::ACCENT_PRIMARY)
                                })
                            })
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                cb(tab, win, app);
                            })
                            .child(tab.label())
                    }),
            )
    }

    /// Tab 1: 对局历史与战绩概览
    fn render_matches_tab(
        &self,
        matches: &[MatchItem],
        stats: &CareerHeaderStats,
        on_toggle_match: impl Fn(usize, &mut Window, &mut App) + 'static + Clone,
        on_career_bar: impl Fn(CareerBarAction, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        // 队列筛选只作用于战绩列表，统计横条仍基于全量（对照 Vue filteredMatches）
        let shown: Vec<&MatchItem> = matches
            .iter()
            .filter(|m| {
                self.selected_queue
                    .is_none_or(|qid| m.display.queue_id == qid)
            })
            .collect();
        let is_empty = shown.is_empty();

        // 单列 grid：子项由网格轨道拉伸到整行宽度
        // （GPUI 里 `w_full` 在非定宽父级下会退化为 auto，故整页统一用 grid 轨道传递宽度）
        div()
            .min_w(px(0.0))
            .grid()
            .grid_cols(1)
            .gap_4()
            // 排位表（单排 / 灵活）
            .child(self.render_ranked_card())
            // 近期统计横条
            .child(self.render_recent_bar(matches, stats, on_career_bar))
            // 紧凑战绩列表或空态引导
            .child(if is_empty {
                div()
                    .bg(Theme::CARD_BG)
                    .border_1()
                    .border_color(Theme::CARD_BORDER)
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
                            .text_color(Theme::TEXT_PRIMARY)
                            .child("暂无近期的对局历史记录"),
                    )
                    .child(div().text_xs().text_color(Theme::TEXT_MUTED).child(
                        "请在英雄联盟客户端完成对局，或点击资料卡右侧的「刷新」拉取最新战绩",
                    ))
            } else {
                div().flex().flex_col().gap_2().children(
                    shown
                        .into_iter()
                        .enumerate()
                        .map(|(idx, m)| Self::render_match_card(m, idx, on_toggle_match.clone())),
                )
            })
    }

    /// 排位表：类型 → 总场次 → 胜率 → 胜场 → 负场 → 段位 → 胜点 → 赛季最高 → 上赛季结算
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
                ranked_row("单排", stats.queue_map.get("RANKED_SOLO_5x5")),
                ranked_row("灵活", stats.queue_map.get("RANKED_FLEX_SR")),
            ],
            None => Vec::new(),
        };

        div()
            .bg(Theme::CARD_BG)
            .border_1()
            .border_color(Theme::CARD_BORDER)
            .rounded_xl()
            .px(px(16.0))
            .py(px(12.0))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .py(px(6.0))
                    .children(COLUMNS.iter().map(|(label, weight)| {
                        ranked_cell(label.to_string(), *weight, Theme::TEXT_MUTED, 11.5, false)
                    })),
            )
            .when(rows.is_empty(), |this| {
                this.child(
                    div()
                        .py(px(10.0))
                        .text_size(px(12.5))
                        .text_color(Theme::TEXT_MUTED)
                        .child("排位数据读取中…"),
                )
            })
            .children(rows.into_iter().enumerate().map(|(idx, values)| {
                let mut row = div()
                    .flex()
                    .items_center()
                    .py(px(8.0))
                    .border_t_1()
                    .border_color(Theme::CARD_BORDER);
                if idx == 0 {
                    row = row.border_t_1().border_color(Theme::CARD_BORDER);
                }
                row.children(
                    values
                        .into_iter()
                        .zip(COLUMNS.iter())
                        .map(|(value, (_, weight))| {
                            ranked_cell(value, *weight, Theme::TEXT_PRIMARY, 13.0, true)
                        }),
                )
            }))
    }

    /// 近期统计横条：局数 / 胜负 / KDA + 常玩位置 + 近期英雄头像
    /// 近期统计横条：局数/胜负/KDA + 常玩位置 + 近期英雄，右侧为队列下拉与最近队友
    fn render_recent_bar(
        &self,
        matches: &[MatchItem],
        stats: &CareerHeaderStats,
        on_bar: impl Fn(CareerBarAction, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let positions = top_positions(matches, 3);
        let champions = top_champions(matches, 5);
        let queue_label = QUEUE_OPTIONS
            .iter()
            .find(|o| o.id == self.selected_queue)
            .map(|o| o.label)
            .unwrap_or("全部");

        div()
            .relative()
            .bg(Theme::CARD_BG)
            .border_1()
            .border_color(Theme::CARD_BORDER)
            .rounded_xl()
            .px(px(16.0))
            .py(px(12.0))
            .flex()
            .items_center()
            .gap(px(18.0))
            .child(bar_stat(
                "近期对局",
                format!("{} 场", stats.total_games),
                Theme::TEXT_PRIMARY,
            ))
            .child(bar_stat(
                "胜负",
                format!("{} 胜 {} 负", stats.wins, stats.losses),
                if stats.win_rate >= 50.0 {
                    Theme::WIN_TEXT
                } else {
                    Theme::LOSS_TEXT
                },
            ))
            .child(bar_stat(
                "KDA",
                format!("{:.2} : 1", stats.avg_kda_ratio),
                Theme::TEXT_PRIMARY,
            ))
            .child(div().w(px(1.0)).h(px(28.0)).bg(Theme::CARD_BORDER))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .when(positions.is_empty(), |this| {
                        this.child(
                            div()
                                .text_size(px(12.0))
                                .text_color(Theme::TEXT_MUTED)
                                .child("暂无分路数据"),
                        )
                    })
                    .children(positions.into_iter().map(|(label, count)| {
                        div()
                            .px(px(8.0))
                            .py(px(3.0))
                            .rounded_md()
                            .bg(Theme::HOVER_BG)
                            .text_size(px(12.0))
                            .text_color(Theme::TEXT_SECONDARY)
                            .child(format!("{} {}", label, count))
                    })),
            )
            .child(div().w(px(1.0)).h(px(28.0)).bg(Theme::CARD_BORDER))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .when(champions.is_empty(), |this| {
                        this.child(
                            div()
                                .text_size(px(12.0))
                                .text_color(Theme::TEXT_MUTED)
                                .child("暂无英雄数据"),
                        )
                    })
                    .children(champions.into_iter().map(|(name, icon_url, _count)| {
                        div()
                            .id(SharedString::from(format!("recent-champ-{}", name)))
                            .child(
                                LcuImage::new(icon_url)
                                    .size(px(26.0))
                                    .shape(LcuImageShape::Circle)
                                    .placeholder("英"),
                            )
                    })),
            )
            .child(div().flex_1())
            // 触发器 1：队列下拉
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
            // 触发器 2：最近队友下拉
            .child({
                let on_toggle = on_bar.clone();
                dropdown_trigger(
                    "career-teammates-trigger",
                    "最近队友".to_string(),
                    self.popup == Some(CareerPopup::Teammates),
                    teammates_placeholder(),
                    move |win, app| {
                        on_toggle(
                            CareerBarAction::TogglePopup(CareerPopup::Teammates),
                            win,
                            app,
                        )
                    },
                )
            })
    }

    /// Tab 2: 英雄熟练度与里程碑网格
    /// Tab 2: 英雄熟练度（四张统计卡 → 筛选行 → 四列网格）
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

        // 筛选与排序（对照 Vue `filteredMasteryList`）
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
                MasterySort::Points => a.champion_points.cmp(&b.champion_points),
                MasterySort::Level => a
                    .champion_level
                    .cmp(&b.champion_level)
                    .then(a.champion_points.cmp(&b.champion_points)),
                MasterySort::Milestone => a
                    .champion_season_milestone
                    .cmp(&b.champion_season_milestone)
                    .then(a.champion_points.cmp(&b.champion_points)),
                MasterySort::LastPlay => a
                    .last_play_time
                    .cmp(&b.last_play_time)
                    .then(a.champion_points.cmp(&b.champion_points)),
            };
            if self.mastery_sort_desc {
                ord.reverse()
            } else {
                ord
            }
        });

        div()
            .min_w(px(0.0))
            .grid()
            .grid_cols(1)
            .gap_4()
            // 四张统计卡
            .child(
                div()
                    .grid()
                    .grid_cols(4)
                    .gap_3()
                    .child(mastery_stat(
                        "熟练英雄总数",
                        format!("{} 位", total_champs),
                        "已记录熟练度的英雄".to_string(),
                        Theme::TEXT_PRIMARY,
                    ))
                    .child(mastery_stat(
                        "熟练度总分",
                        format!("{} 分", format_num(total_points)),
                        "全部英雄累计".to_string(),
                        Theme::ACCENT_PRIMARY,
                    ))
                    .child(mastery_stat(
                        "最高等级英雄",
                        top.map(|m| format!("Lv.{} {}", m.champion_level, m.champion_name))
                            .unwrap_or_else(|| "--".to_string()),
                        top.map(|m| format!("{} 分", format_num(m.champion_points)))
                            .unwrap_or_default(),
                        Theme::AURORA_BLUE,
                    ))
                    .child(mastery_stat(
                        "里程碑宝箱",
                        format!("已获取 {}", granted),
                        format!("另有 {} 个可获取", eligible),
                        crate::theme::GOLD,
                    )),
            )
            // 筛选行：左 搜索 / 宝箱 / 等级，右 排序 / 升降序 / 刷新
            .child(
                div()
                    .bg(Theme::CARD_BG)
                    .border_1()
                    .border_color(Theme::CARD_BORDER)
                    .rounded_xl()
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .flex_wrap()
                            .gap_2()
                            .child({
                                let cb = on_query.clone();
                                let clear = on_query.clone();
                                text_field(
                                    "mastery-search",
                                    &self.mastery_query,
                                    "搜索英雄名 / ID",
                                    px(200.0),
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
                            // 宝箱状态 n-select
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
                                    dropdown_menu(150.0, items),
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
                            // 等级 n-select
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
                            .flex_wrap()
                            .gap_2()
                            .child(div().flex_1())
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(Theme::TEXT_MUTED)
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
                                    dropdown_menu(140.0, items),
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
                                filter_chip(
                                    if self.mastery_sort_desc {
                                        "降序 ↓"
                                    } else {
                                        "升序 ↑"
                                    },
                                    false,
                                    move |win, app| {
                                        cb(MasteryFilterAction::ToggleSortDir, win, app)
                                    },
                                )
                            })
                            .child({
                                div()
                                    .px(px(10.0))
                                    .py(px(5.0))
                                    .rounded_md()
                                    .border_1()
                                    .border_color(Theme::CARD_BORDER)
                                    .bg(Theme::CARD_BG)
                                    .text_size(px(12.0))
                                    .text_color(Theme::TEXT_SECONDARY)
                                    .cursor_pointer()
                                    .hover(|s| s.border_color(Theme::ACCENT_PRIMARY))
                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                        on_refresh(win, app)
                                    })
                                    .child("刷新")
                            }),
                    ),
            )
            // 英雄卡片网格（默认四列）
            .child(if list.is_empty() {
                div()
                    .bg(Theme::CARD_BG)
                    .border_1()
                    .border_color(Theme::CARD_BORDER)
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
                            .text_color(Theme::TEXT_PRIMARY)
                            .child("没有符合条件的英雄"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::TEXT_MUTED)
                            .child("请调整搜索词、宝箱状态或等级筛选"),
                    )
            } else {
                div()
                    .grid()
                    .grid_cols(4)
                    .gap_3()
                    .children(list.into_iter().map(mastery_card))
            })
    }

    /// Tab 3: 战利品管理与精粹资产网格
    fn render_loot_tab(&self) -> impl IntoElement {
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_4()
            // 顶部精粹资产卡片
            .child(
                div()
                    .w_full()
                    .p_4()
                    .rounded_xl()
                    .bg(Theme::CARD_BG)
                    .border_1()
                    .border_color(Theme::CARD_BORDER)
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_8()
                            // 蓝色精粹
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .size(px(36.0))
                                            .rounded_lg()
                                            .bg(rgba(0x00bbf922))
                                            .border_1()
                                            .border_color(Theme::AURORA_BLUE)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(Theme::AURORA_BLUE)
                                            .child("蓝"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(Theme::TEXT_MUTED)
                                                    .child("蓝色精粹"),
                                            )
                                            .child(
                                                div()
                                                    .text_lg()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(Theme::TEXT_PRIMARY)
                                                    .child(format_num(
                                                        self.essences.blue_essence as i64,
                                                    )),
                                            ),
                                    ),
                            )
                            // 橙色精粹
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .size(px(36.0))
                                            .rounded_lg()
                                            .bg(rgba(0xff9e0022))
                                            .border_1()
                                            .border_color(rgb(0xff9e00))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(0xff9e00))
                                            .child("橙"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(Theme::TEXT_MUTED)
                                                    .child("橙色精粹"),
                                            )
                                            .child(
                                                div()
                                                    .text_lg()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(Theme::TEXT_PRIMARY)
                                                    .child(format_num(
                                                        self.essences.orange_essence as i64,
                                                    )),
                                            ),
                                    ),
                            )
                            // 神话精萃
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .size(px(36.0))
                                            .rounded_lg()
                                            .bg(rgba(0x7209b722))
                                            .border_1()
                                            .border_color(rgb(0x7209b7))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(0x7209b7))
                                            .child("神"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(Theme::TEXT_MUTED)
                                                    .child("神话精萃"),
                                            )
                                            .child(
                                                div()
                                                    .text_lg()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(Theme::TEXT_PRIMARY)
                                                    .child(format_num(
                                                        self.essences.mythic_essence as i64,
                                                    )),
                                            ),
                                    ),
                            ),
                    )
                    // 操作按钮组
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .px_4()
                                    .py_2()
                                    .rounded_lg()
                                    .bg(Theme::ACCENT_LIGHT)
                                    .border_1()
                                    .border_color(Theme::AURORA_CYAN)
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::ACCENT_PRIMARY)
                                    .cursor_pointer()
                                    .child("一键智能开箱"),
                            )
                            .child(
                                div()
                                    .px_4()
                                    .py_2()
                                    .rounded_lg()
                                    .bg(Theme::BADGE_BG)
                                    .border_1()
                                    .border_color(Theme::CARD_BORDER)
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(Theme::TEXT_SECONDARY)
                                    .cursor_pointer()
                                    .child("分解重复碎片"),
                            ),
                    ),
            )
            // 战利品网格展示（4 列）
            .child(
                div()
                    .w_full()
                    .grid()
                    .grid_cols(4)
                    .gap_3()
                    .children(self.loot_items.iter().map(|item| {
                        div()
                            .p_3()
                            .rounded_xl()
                            .bg(Theme::CARD_BG)
                            .border_1()
                            .border_color(Theme::CARD_BORDER)
                            .hover(|s| s.border_color(Theme::AURORA_BLUE).bg(rgba(0xffffff88)))
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(Theme::TEXT_PRIMARY)
                                            .child(item.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py(px(1.0))
                                            .rounded_md()
                                            .bg(Theme::BADGE_BG)
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(Theme::AURORA_BLUE)
                                            .child(format!("x{}", item.count)),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::TEXT_MUTED)
                                    .child(item.item_desc.clone()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .pt_2()
                                    .border_t_1()
                                    .border_color(Theme::CARD_BORDER)
                                    .child(div().text_xs().text_color(Theme::TEXT_SECONDARY).child(
                                        if item.disenchant_value > 0 {
                                            format!("分解: +{} 精粹", item.disenchant_value)
                                        } else {
                                            "不可分解".to_string()
                                        },
                                    ))
                                    .child(
                                        div()
                                            .px_3()
                                            .py_1()
                                            .rounded_md()
                                            .cursor_pointer()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .bg(if item.is_openable {
                                                Theme::ACCENT_LIGHT
                                            } else {
                                                Theme::BADGE_BG
                                            })
                                            .text_color(if item.is_openable {
                                                Theme::ACCENT_PRIMARY
                                            } else {
                                                Theme::TEXT_SECONDARY
                                            })
                                            .child(if item.is_openable {
                                                "开启"
                                            } else {
                                                "升级"
                                            }),
                                    ),
                            )
                    })),
            )
    }

    /// 渲染顶部统计面板（兼容原有 Matches 列表内部调用）
    /// 渲染单条紧凑战绩卡片（112px 黄金行高）
    pub fn render_match_card(
        m: &MatchItem,
        idx: usize,
        on_toggle: impl Fn(usize, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let toggle_cb = on_toggle.clone();

        let (bg_color, border_color, bar_color, win_text_color, win_label) = if m.display.remake {
            (
                Theme::REMAKE_BG,
                Theme::REMAKE_BORDER,
                Theme::REMAKE_BAR,
                Theme::REMAKE_TEXT,
                "重开",
            )
        } else if m.display.win {
            (
                Theme::WIN_BG,
                Theme::WIN_BORDER,
                Theme::WIN_BAR,
                Theme::WIN_TEXT,
                "胜利",
            )
        } else {
            (
                Theme::LOSS_BG,
                Theme::LOSS_BORDER,
                Theme::LOSS_BAR,
                Theme::LOSS_TEXT,
                "失败",
            )
        };

        let champion_name = m.display.name.clone();
        let champion_icon_url = m.display.champion_icon_url.clone();
        let spell1_icon_url = m.display.spell1_icon_url.clone();
        let spell2_icon_url = m.display.spell2_icon_url.clone();
        let queue_name = m.display.map.clone();
        let duration_str = m.display.duration.clone();
        let time_str = m.display.short_time.clone();
        let kda_ratio = m.display.kda.clone();
        let cs_str = format!("{} 刀", m.display.cs);
        let gold_str = format!("{:.1}k", m.display.gold as f32 / 1000.0);
        let damage_str = format!("{:.1}k", m.display.total_damage as f32 / 1000.0);
        let pos_str = position_to_zh(m.display.position.as_deref());
        let mut items = [0i32; 7];
        for (i, &it) in m.display.item_ids.iter().take(7).enumerate() {
            items[i] = it;
        }
        let item_urls = m.display.item_icon_urls.clone();
        let is_expanded = m.expanded;
        let champ_level = m.display.champ_level;
        let kills = m.display.kills;
        let deaths = m.display.deaths;
        let assists = m.display.assists;
        let game_id = m.display.game_id;
        let detail_line = format!(
            "对局编号: {} · 输出 {:.1}k · 治疗 {:.1}k · 补刀 {} · 金币 {:.1}k",
            game_id,
            m.display.total_damage as f32 / 1000.0,
            m.display.total_heal as f32 / 1000.0,
            m.display.cs,
            m.display.gold as f32 / 1000.0
        );
        let augment_line = if m.display.augment_names.is_empty() {
            None
        } else {
            Some(format!(
                "海克斯强化: {}",
                m.display.augment_names.join(" · ")
            ))
        };

        div()
            .w_full()
            .flex()
            .flex_col()
            .rounded_xl()
            .border_1()
            .border_color(border_color)
            .bg(bg_color)
            .shadow_sm()
            .overflow_hidden()
            .child(
                div()
                    .h(px(112.0))
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pr_4()
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_ev, window, cx| {
                        toggle_cb(idx, window, cx);
                    })
                    // 1. 左侧胜负竖条与英雄模块
                    .child(
                        div()
                            .h_full()
                            .flex()
                            .items_center()
                            .child(div().w(px(4.0)).h_full().bg(bar_color))
                            .child(
                                div()
                                    .px_3()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .relative()
                                            .size(px(52.0))
                                            .rounded_lg()
                                            .border_1()
                                            .border_color(border_color)
                                            .overflow_hidden()
                                            .child(
                                                LcuImage::new(champion_icon_url)
                                                    .size(px(52.0))
                                                    .shape(crate::components::lcu_image::LcuImageShape::Rounded)
                                                    .placeholder(champion_name),
                                            )
                                            .child(
                                                div()
                                                    .absolute()
                                                    .bottom(px(1.0))
                                                    .right(px(1.0))
                                                    .px_1()
                                                    .rounded_sm()
                                                    .bg(rgba(0x000000bb))
                                                    .text_xs()
                                                    .text_color(rgb(0xffffff))
                                                    .child(format!("{}", champ_level)),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.0))
                                            .child(
                                                LcuImage::new(spell1_icon_url)
                                                    .size(px(22.0))
                                                    .shape(crate::components::lcu_image::LcuImageShape::Rounded)
                                                    .placeholder("D"),
                                            )
                                            .child(
                                                LcuImage::new(spell2_icon_url)
                                                    .size(px(22.0))
                                                    .shape(crate::components::lcu_image::LcuImageShape::Rounded)
                                                    .placeholder("F"),
                                            ),
                                    ),
                            ),
                    )
                    // 2. 模式、胜负与对局时间
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .w(px(120.0))
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child(queue_name),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(win_text_color)
                                            .child(win_label),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(Theme::TEXT_SECONDARY)
                                            .child(duration_str),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::TEXT_MUTED)
                                    .child(format!("{} · {}", time_str, pos_str)),
                            ),
                    )
                    // 3. KDA 与评级
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .w(px(130.0))
                            .items_start()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child(format!("{} / {} / {}", kills, deaths, assists)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(Theme::TEXT_SECONDARY)
                                    .child(kda_ratio),
                            ),
                    )
                    // 4. 补刀、经济与输出
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .w(px(110.0))
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child(cs_str),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::TEXT_MUTED)
                                    .child(format!("金币 {}", gold_str)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::TEXT_MUTED)
                                    .child(format!("输出 {}", damage_str)),
                            ),
                    )
                    // 5. 装备网格（6 装备 + 1 饰品）
                    .child(div().flex().items_center().gap(px(4.0)).children(
                        items.iter().enumerate().map(|(slot_idx, &item_id)| {
                            let is_trinket = slot_idx == 6;
                            let is_empty = item_id == 0;
                            let item_url = item_urls.get(slot_idx).cloned().unwrap_or_default();

                            LcuImage::new(item_url)
                                .size(px(26.0))
                                .item_slot()
                                .border_color(if is_trinket {
                                    Theme::AURORA_CYAN
                                } else {
                                    Theme::ITEM_SLOT_BORDER
                                })
                                .placeholder(if is_empty {
                                    "".to_string()
                                } else {
                                    format!("{}", item_id % 100)
                                })
                        }),
                    ))
                    // 6. 右侧展开按钮
                    .child(
                        div()
                            .p_2()
                            .rounded_md()
                            .hover(|s| s.bg(rgba(0x00000010)))
                            .text_sm()
                            .text_color(Theme::TEXT_SECONDARY)
                            .child(if is_expanded { "▲" } else { "▼" }),
                    ),
            )
            // 展开态详情抽屉
            .when(is_expanded, |this| {
                this.child(
                    div()
                        .w_full()
                        .p_3()
                        .border_t_1()
                        .border_color(border_color)
                        .bg(Theme::CARD_BG)
                        .flex()
                        .items_center()
                        .justify_between()
                        .text_xs()
                        .text_color(Theme::TEXT_SECONDARY)
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(div().child(detail_line))
                                .when_some(augment_line, |this, text| {
                                    this.child(div().text_color(Theme::TEXT_MUTED).child(text))
                                }),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(
                                    div()
                                        .px_2()
                                        .py_1()
                                        .rounded_md()
                                        .bg(Theme::BADGE_BG)
                                        .cursor_pointer()
                                        .child("下载回放"),
                                )
                                .child(
                                    div()
                                        .px_2()
                                        .py_1()
                                        .rounded_md()
                                        .bg(Theme::ACCENT_LIGHT)
                                        .text_color(Theme::ACCENT_PRIMARY)
                                        .cursor_pointer()
                                        .child("查看完整图表"),
                                ),
                        ),
                )
            })
    }
}

/// 熟练度统计卡
fn mastery_stat(
    label: &'static str,
    value: String,
    sub: String,
    value_color: Hsla,
) -> impl IntoElement {
    div()
        .p_3()
        .rounded_xl()
        .bg(Theme::CARD_BG)
        .border_1()
        .border_color(Theme::CARD_BORDER)
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_xs().text_color(Theme::TEXT_MUTED).child(label))
        .child(
            div()
                .text_lg()
                .font_weight(FontWeight::BOLD)
                .text_color(value_color)
                .whitespace_nowrap()
                .overflow_hidden()
                .child(value),
        )
        .child(
            div()
                .text_xs()
                .text_color(Theme::TEXT_MUTED)
                .whitespace_nowrap()
                .overflow_hidden()
                .child(sub),
        )
}

/// 筛选行的小分段按钮
fn filter_chip(
    label: &'static str,
    active: bool,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .px(px(10.0))
        .py(px(5.0))
        .rounded_md()
        .border_1()
        .border_color(if active {
            Theme::SIDEBAR_ACTIVE_BORDER
        } else {
            Theme::CARD_BORDER
        })
        .bg(if active {
            Theme::ACCENT_LIGHT
        } else {
            Theme::CARD_BG
        })
        .text_size(px(12.0))
        .font_weight(if active {
            FontWeight::BOLD
        } else {
            FontWeight::NORMAL
        })
        .text_color(if active {
            Theme::ACCENT_PRIMARY
        } else {
            Theme::TEXT_MUTED
        })
        .cursor_pointer()
        .hover(|s| s.border_color(Theme::ACCENT_PRIMARY))
        .on_mouse_down(MouseButton::Left, move |_ev, win, app| on_click(win, app))
        .child(label)
}

/// 单个英雄熟练度卡：头像 + 等级 / 名 + 点数 / 进度条 + 下级所需点数 / 里程碑与宝箱
fn mastery_card(m: &ChampionMasteryItem) -> impl IntoElement {
    let since = m.champion_points_since_last_level.max(0);
    let until = m.champion_points_until_next_level.max(0);
    let pct = if until <= 0 {
        100.0
    } else {
        (since as f32 / (since + until) as f32 * 100.0).clamp(0.0, 100.0)
    };

    div()
        .p_3()
        .rounded_xl()
        .bg(Theme::CARD_BG)
        .border_1()
        .border_color(Theme::CARD_BORDER)
        .hover(|s| s.border_color(Theme::AURORA_CYAN))
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .relative()
                        .size(px(44.0))
                        .rounded_lg()
                        .border_1()
                        .border_color(Theme::CARD_BORDER)
                        .overflow_hidden()
                        .flex_none()
                        .child(
                            LcuImage::new(m.champion_icon_url.clone())
                                .size(px(44.0))
                                .shape(LcuImageShape::Rounded)
                                .placeholder(format!("{}", m.champion_id)),
                        )
                        .child(
                            div()
                                .absolute()
                                .bottom_0()
                                .right_0()
                                .px_1()
                                .rounded_tl_md()
                                .bg(rgba(0x000000bb))
                                .text_xs()
                                .text_color(rgb(0xffffff))
                                .child(format!("Lv.{}", m.champion_level)),
                        ),
                )
                .child(
                    div()
                        .min_w(px(0.0))
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(Theme::TEXT_PRIMARY)
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .child(m.champion_name.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(Theme::TEXT_MUTED)
                                .child(format!("{} 点", format_num(m.champion_points))),
                        ),
                ),
        )
        .child(
            div()
                .w_full()
                .h(px(6.0))
                .rounded_full()
                .bg(Theme::BADGE_BG)
                .overflow_hidden()
                .child(
                    div()
                        .h_full()
                        .w(px(217.0 * pct / 100.0))
                        .rounded_full()
                        .bg(Theme::ACCENT_PRIMARY),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .text_xs()
                .child(
                    div()
                        .text_color(Theme::TEXT_SECONDARY)
                        .child(format!("{:.0}%", pct)),
                )
                .child(div().text_color(Theme::TEXT_MUTED).child(if until <= 0 {
                    "已达最高等级".to_string()
                } else {
                    format!("距下一级还需 {} 点", format_num(until))
                })),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .pt_1()
                .border_t_1()
                .border_color(Theme::CARD_BORDER)
                .text_xs()
                .child(
                    div()
                        .text_color(Theme::TEXT_SECONDARY)
                        .child(format!("赛段里程碑 {}", m.champion_season_milestone)),
                )
                .child(
                    div()
                        .px_2()
                        .py(px(1.0))
                        .rounded_md()
                        .bg(if m.chest_granted {
                            Theme::BADGE_BG
                        } else {
                            Theme::ACCENT_LIGHT
                        })
                        .text_color(if m.chest_granted {
                            Theme::TEXT_MUTED
                        } else {
                            Theme::ACCENT_PRIMARY
                        })
                        .child(if m.chest_granted {
                            "已获宝箱"
                        } else {
                            "可获宝箱"
                        }),
                ),
        )
}

/// 最近队友下拉：数据链路（对局详情批量聚合）未接通时的说明
fn teammates_placeholder() -> impl IntoElement {
    div()
        .w(px(240.0))
        .p_3()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight::BOLD)
                .text_color(Theme::TEXT_PRIMARY)
                .child("最近队友"),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(Theme::TEXT_MUTED)
                .child("需要按对局详情批量聚合队友数据，GPUI 服务层尚未接通"),
        )
}
