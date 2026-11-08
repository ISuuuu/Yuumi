use crate::components::lcu_image::LcuImage;
use crate::match_model::{compute_match_stats, position_to_zh, MatchItem};
use crate::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use yuumi_core::parsers::loot::{EssenceBalances, LootInventoryItem};
use yuumi_core::parsers::mastery::ChampionMasteryItem;
use yuumi_core::parsers::summoner::SummonerInfo;

fn format_num(val: i64) -> String {
    let s = val.to_string();
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    for (i, &ch) in chars.iter().enumerate() {
        result.push(ch);
        let rem = len - 1 - i;
        if rem > 0 && rem % 3 == 0 {
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

impl CareerTab {
    pub fn label(&self) -> &'static str {
        match self {
            CareerTab::Matches => "对局历史",
            CareerTab::Mastery => "英雄熟练度",
            CareerTab::Loot => "战利品管理",
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
    pub mastery_list: Vec<ChampionMasteryItem>,
    pub essences: EssenceBalances,
    pub loot_items: Vec<LootInventoryItem>,
}

impl Default for CareerView {
    fn default() -> Self {
        Self {
            active_tab: CareerTab::Matches,
            summoner: None,
            mastery_list: Self::mock_masteries(),
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
    pub fn render(
        &self,
        matches: &[MatchItem],
        stats: &CareerHeaderStats,
        on_tab_change: impl Fn(CareerTab, &mut Window, &mut App) + 'static + Clone,
        on_toggle_match: impl Fn(usize, &mut Window, &mut App) + 'static + Clone,
        on_refresh: impl Fn(&mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let active_tab = self.active_tab;

        div()
            .w_full()
            .h_full()
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            // 顶部召唤师身份与 Tab 胶囊导航栏
            .child(self.render_summoner_header(stats, on_tab_change, on_refresh))
            // 主体内容区域（根据当前 Sub-Tab 切换）
            .child(
                div()
                    .w_full()
                    .flex_1()
                    .id("career-tab-content-scroll")
                    .overflow_y_scroll()
                    .when(active_tab == CareerTab::Matches, |this| {
                        this.child(Self::render_matches_tab(matches, stats, on_toggle_match))
                    })
                    .when(active_tab == CareerTab::Mastery, |this| {
                        this.child(self.render_mastery_tab())
                    })
                    .when(active_tab == CareerTab::Loot, |this| {
                        this.child(self.render_loot_tab())
                    }),
            )
    }

    /// 渲染顶部召唤师身份与 Sub-Tab 切换胶囊
    fn render_summoner_header(
        &self,
        stats: &CareerHeaderStats,
        on_tab_change: impl Fn(CareerTab, &mut Window, &mut App) + 'static + Clone,
        on_refresh: impl Fn(&mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let name_str = self
            .summoner
            .as_ref()
            .map(|s| s.full_name())
            .unwrap_or_else(|| "悠米 #GPUI".to_string());
        let level_str = self
            .summoner
            .as_ref()
            .and_then(|s| s.summoner_level)
            .map(|l| format!("Lv. {}", l))
            .unwrap_or_else(|| "Lv. 428".to_string());

        let icon_path = self
            .summoner
            .as_ref()
            .map(|s| s.icon_path())
            .unwrap_or_else(|| "/lol-game-data/assets/v1/profile-icons/29.jpg".to_string());

        div()
            .w_full()
            .bg(Theme::CARD_BG)
            .border_1()
            .border_color(Theme::CARD_BORDER)
            .rounded_xl()
            .p_4()
            .shadow_sm()
            .flex()
            .items_center()
            .justify_between()
            // 左侧：头像 + 召唤师名 + 等级
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(
                        // 头像带极光光环
                        div()
                            .relative()
                            .size(px(58.0))
                            .rounded_xl()
                            .border_2()
                            .border_color(Theme::AURORA_CYAN)
                            .overflow_hidden()
                            .child(
                                LcuImage::new(icon_path)
                                    .size(px(58.0))
                                    .shape(crate::components::lcu_image::LcuImageShape::Rounded)
                                    .placeholder("悠"),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .bottom_0()
                                    .left_0()
                                    .right_0()
                                    .bg(rgba(0x000000aa))
                                    .py(px(1.0))
                                    .flex()
                                    .justify_center()
                                    .text_xs()
                                    .text_color(rgb(0xffffff))
                                    .child(level_str),
                            ),
                    )
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
                                    .child(
                                        div()
                                            .text_lg()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(Theme::TEXT_PRIMARY)
                                            .child(name_str),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py(px(2.0))
                                            .rounded_md()
                                            .bg(Theme::BADGE_BG)
                                            .text_xs()
                                            .text_color(Theme::AURORA_BLUE)
                                            .child("当前召唤师"),
                                    ),
                            )
                            .child(div().text_xs().text_color(Theme::TEXT_MUTED).child(format!(
                                "近 {} 场胜率 {:.1}% · KDA {:.2}:1",
                                stats.total_games, stats.win_rate, stats.avg_kda_ratio
                            ))),
                    ),
            )
            // 右侧：三大 Sub-Tab 极光胶囊切换器
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .p_1()
                    .bg(Theme::BADGE_BG)
                    .rounded_xl()
                    .border_1()
                    .border_color(Theme::CARD_BORDER)
                    .children(
                        [CareerTab::Matches, CareerTab::Mastery, CareerTab::Loot]
                            .into_iter()
                            .map(|tab| {
                                let is_active = self.active_tab == tab;
                                let cb = on_tab_change.clone();
                                div()
                                    .px_4()
                                    .py_2()
                                    .rounded_lg()
                                    .cursor_pointer()
                                    .text_sm()
                                    .font_weight(if is_active {
                                        FontWeight::BOLD
                                    } else {
                                        FontWeight::NORMAL
                                    })
                                    .bg(if is_active {
                                        Theme::CARD_BG
                                    } else {
                                        transparent_black()
                                    })
                                    .text_color(if is_active {
                                        Theme::ACCENT_PRIMARY
                                    } else {
                                        Theme::TEXT_SECONDARY
                                    })
                                    .border_1()
                                    .border_color(if is_active {
                                        Theme::AURORA_CYAN
                                    } else {
                                        transparent_black()
                                    })
                                    .when(is_active, |s| s.shadow_sm())
                                    .hover(|s| s.bg(rgba(0xffffff66)))
                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                        cb(tab, win, app);
                                    })
                                    .child(tab.label())
                            }),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .rounded_lg()
                            .cursor_pointer()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .bg(Theme::CARD_BG)
                            .border_1()
                            .border_color(Theme::CARD_BORDER)
                            .text_color(Theme::AURORA_BLUE)
                            .hover(|s| s.bg(rgba(0xffffff88)).border_color(Theme::AURORA_CYAN))
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                on_refresh(win, app);
                            })
                            .child("🔄 刷新"),
                    ),
            )
    }

    /// Tab 1: 对局历史与战绩概览
    fn render_matches_tab(
        matches: &[MatchItem],
        stats: &CareerHeaderStats,
        on_toggle_match: impl Fn(usize, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let is_empty = matches.is_empty();

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_4()
            // 战绩指标统计卡片
            .child(Self::render_header(stats))
            // 紧凑战绩列表或空态引导
            .child(if is_empty {
                div()
                    .w_full()
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
                    .child(
                        div().text_xs().text_color(Theme::TEXT_MUTED).child(
                            "请在英雄联盟客户端完成对局，或点击右上角「🔄 刷新」拉取最新战绩",
                        ),
                    )
            } else {
                div().w_full().flex().flex_col().children(
                    matches
                        .iter()
                        .enumerate()
                        .map(|(idx, m)| Self::render_match_card(m, idx, on_toggle_match.clone())),
                )
            })
    }

    /// Tab 2: 英雄熟练度与里程碑网格
    fn render_mastery_tab(&self) -> impl IntoElement {
        let total_points: i64 = self.mastery_list.iter().map(|m| m.champion_points).sum();
        let total_champs = self.mastery_list.len();
        let chests_count = self.mastery_list.iter().filter(|m| m.chest_granted).count();

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_4()
            // 熟练度概览指标卡片
            .child(
                div()
                    .w_full()
                    .grid()
                    .grid_cols(3)
                    .gap_3()
                    .child(
                        div()
                            .p_3()
                            .rounded_xl()
                            .bg(Theme::CARD_BG)
                            .border_1()
                            .border_color(Theme::CARD_BORDER)
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::TEXT_MUTED)
                                    .child("熟练度总点数"),
                            )
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::ACCENT_PRIMARY)
                                    .child(format!("{} pts", format_num(total_points))),
                            ),
                    )
                    .child(
                        div()
                            .p_3()
                            .rounded_xl()
                            .bg(Theme::CARD_BG)
                            .border_1()
                            .border_color(Theme::CARD_BORDER)
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::TEXT_MUTED)
                                    .child("已成就有英雄"),
                            )
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::AURORA_BLUE)
                                    .child(format!("{} 位英雄", total_champs)),
                            ),
                    )
                    .child(
                        div()
                            .p_3()
                            .rounded_xl()
                            .bg(Theme::CARD_BG)
                            .border_1()
                            .border_color(Theme::CARD_BORDER)
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::TEXT_MUTED)
                                    .child("赛段宝箱进度"),
                            )
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(crate::theme::GOLD)
                                    .child(format!("已获取 {} 个", chests_count)),
                            ),
                    ),
            )
            // 英雄熟练度卡片网格（4 列）
            .child(div().w_full().grid().grid_cols(4).gap_3().children(
                self.mastery_list.iter().map(|m| {
                    div()
                        .p_3()
                        .rounded_xl()
                        .bg(Theme::CARD_BG)
                        .border_1()
                        .border_color(Theme::CARD_BORDER)
                        .hover(|s| s.border_color(Theme::AURORA_CYAN).bg(rgba(0xffffff88)))
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
                                        .child(
                                            LcuImage::new(m.champion_icon_url.clone())
                                                .size(px(44.0))
                                                .shape(crate::components::lcu_image::LcuImageShape::Rounded)
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
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(Theme::TEXT_PRIMARY)
                                                .child(m.champion_name.clone()),
                                        )
                                        .child(
                                            div().text_xs().text_color(Theme::TEXT_MUTED).child(
                                                format!("{} 点", format_num(m.champion_points)),
                                            ),
                                        ),
                                ),
                        )
                        // 赛段里程碑等级与宝箱标签
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
                                    div().text_color(Theme::TEXT_SECONDARY).child(format!(
                                        "赛段里程碑 {}",
                                        m.champion_season_milestone
                                    )),
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
                                            "已领宝箱"
                                        } else {
                                            "可获宝箱"
                                        }),
                                ),
                        )
                }),
            ))
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
    pub fn render_header(stats: &CareerHeaderStats) -> impl IntoElement {
        div()
            .w_full()
            .bg(Theme::CARD_BG)
            .border_1()
            .border_color(Theme::CARD_BORDER)
            .rounded_xl()
            .p_4()
            .mb_2()
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
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::TEXT_PRIMARY)
                            .child("近期对局整体胜率看板"),
                    )
                    .child(div().text_xs().text_color(Theme::TEXT_MUTED).child(format!(
                        "共统计近 {} 场比赛 · 包括排位、匹配与大乱斗",
                        stats.total_games
                    ))),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_6()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(if stats.win_rate >= 50.0 {
                                        Theme::WIN_TEXT
                                    } else {
                                        Theme::LOSS_TEXT
                                    })
                                    .child(format!("{:.1}%", stats.win_rate)),
                            )
                            .child(div().text_xs().text_color(Theme::TEXT_SECONDARY).child(
                                format!(
                                    "{} 胜 {} 负 ({} 重开)",
                                    stats.wins, stats.losses, stats.remakes
                                ),
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child(format!("{:.2}:1 KDA", stats.avg_kda_ratio)),
                            )
                            .child(div().text_xs().text_color(Theme::TEXT_MUTED).child(format!(
                                "{:.1} / {:.1} / {:.1}",
                                stats.avg_kills, stats.avg_deaths, stats.avg_assists
                            ))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(Theme::TEXT_SECONDARY)
                            .child("分路胜率偏好: 中单 40% · 下路 30% · 上单 20% · 辅助 10%"),
                    )
                    .child(
                        div()
                            .h(px(6.0))
                            .w(px(180.0))
                            .rounded_full()
                            .bg(Theme::BADGE_BG)
                            .overflow_hidden()
                            .flex()
                            .child(div().w_2_5().h_full().bg(Theme::AURORA_CYAN))
                            .child(div().w_1_4().h_full().bg(Theme::AURORA_BLUE))
                            .child(div().w_1_5().h_full().bg(Theme::ACCENT_PRIMARY)),
                    ),
            )
    }

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
            .mb_2()
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
