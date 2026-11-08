use crate::asset_loader::global_asset_loader;
use crate::components::badge::{Badge, BadgeVariant};
use crate::components::card::Card;
use crate::theme::{self, Theme};
use gpui::prelude::FluentBuilder;
use gpui::*;
use yuumi_core::parsers::saved_player::{create_mock_saved_players, SavedPlayer, RECOMMENDED_TAGS};

/// 标签颜色映射
fn tag_badge_colors(tag: &str) -> (Hsla, Hsla, Hsla) {
    match tag {
        "大腿" => (Theme::WIN_BG, Theme::WIN_TEXT, Theme::WIN_BORDER), // 翡翠绿
        "避坑" | "演员/挂机" => (Theme::LOSS_BG, Theme::LOSS_TEXT, Theme::LOSS_BORDER), // 珊瑚红 / 绯红
        "绝活" => (
            Theme::ACCENT_LIGHT,
            Theme::ACCENT_PRIMARY,
            Theme::AURORA_CYAN.opacity(0.6),
        ), // 极光蓝青
        "置顶" => (
            Theme::PREMADE_COLOR_2.opacity(0.15),
            Theme::GOLD,
            Theme::GOLD.opacity(0.4),
        ), // 琥珀金
        _ => (Theme::BADGE_BG, Theme::TEXT_SECONDARY, Theme::CARD_BORDER),
    }
}

/// 队列名称显示映射
fn queue_type_display_name(queue_type: &str) -> &'static str {
    match queue_type {
        "420" => "单双排位",
        "440" => "灵活排位",
        "430" => "匹配模式",
        "450" => "极地大乱斗",
        "2400" | "2450" => "海克斯大乱斗",
        "1700" | "1710" => "斗魂竞技场",
        "900" | "1010" => "无限火力",
        "0" => "自定义对局",
        _ => "其他对局",
    }
}

/// 路人集主视图
pub struct SavedPlayersView {
    /// 完整路人记录列表
    pub players: Vec<SavedPlayer>,
    /// 当前选中的标签过滤，例如 "全部"、"置顶"、"大腿" 等
    pub active_tag_filter: String,
    /// 搜索过滤词（按召唤师名、标签、备注检索）
    pub search_query: String,
    /// 当前展开查看相遇历史的玩家 PUUID
    pub expanded_puuid: Option<String>,
    /// 正在编辑标签或备注的玩家 PUUID
    pub editing_puuid: Option<String>,
    /// 编辑输入临时缓冲
    pub editing_tag_buffer: String,
    pub editing_notes_buffer: String,
}

impl Default for SavedPlayersView {
    fn default() -> Self {
        Self::new()
    }
}

impl SavedPlayersView {
    pub fn new() -> Self {
        Self {
            players: create_mock_saved_players(),
            active_tag_filter: "全部".to_string(),
            search_query: String::new(),
            expanded_puuid: None,
            editing_puuid: None,
            editing_tag_buffer: String::new(),
            editing_notes_buffer: String::new(),
        }
    }

    /// 使用自定义数据初始化
    pub fn with_players(mut self, players: Vec<SavedPlayer>) -> Self {
        self.players = players;
        self
    }

    /// 过滤后的路人记录列表
    pub fn filtered_players(&self) -> Vec<&SavedPlayer> {
        let q = self.search_query.trim().to_lowercase();
        let mut list: Vec<&SavedPlayer> = self
            .players
            .iter()
            .filter(|p| {
                // 1. 标签过滤
                let match_tag = match self.active_tag_filter.as_str() {
                    "全部" => true,
                    "置顶" => p.is_pinned,
                    target => p.tag.as_deref() == Some(target),
                };

                if !match_tag {
                    return false;
                }

                // 2. 搜索词过滤
                if q.is_empty() {
                    return true;
                }

                p.summoner_name.to_lowercase().contains(&q)
                    || p.tag_line.to_lowercase().contains(&q)
                    || p.notes.to_lowercase().contains(&q)
                    || p.tag
                        .as_ref()
                        .map(|t| t.to_lowercase().contains(&q))
                        .unwrap_or(false)
            })
            .collect();

        // 排序规则：置顶排前，同级按最近相遇时间倒序
        list.sort_by(|a, b| {
            b.is_pinned
                .cmp(&a.is_pinned)
                .then_with(|| b.last_met_timestamp.cmp(&a.last_met_timestamp))
        });

        list
    }

    /// 累计同局相遇总局数
    pub fn total_encounter_count(&self) -> u32 {
        self.players.iter().map(|p| p.encountered_count).sum()
    }

    /// 切换标签过滤
    pub fn set_filter(&mut self, tag: impl Into<String>) {
        self.active_tag_filter = tag.into();
    }

    /// 切换展开某个玩家的历史
    pub fn toggle_expand(&mut self, puuid: &str) {
        if self.expanded_puuid.as_deref() == Some(puuid) {
            self.expanded_puuid = None;
        } else {
            self.expanded_puuid = Some(puuid.to_string());
        }
    }

    /// 切换玩家置顶状态
    pub fn toggle_pin(&mut self, puuid: &str) {
        if let Some(player) = self.players.iter_mut().find(|p| p.puuid == puuid) {
            player.is_pinned = !player.is_pinned;
        }
    }

    /// 删除玩家记录
    pub fn remove_player(&mut self, puuid: &str) {
        self.players.retain(|p| p.puuid != puuid);
        if self.expanded_puuid.as_deref() == Some(puuid) {
            self.expanded_puuid = None;
        }
        if self.editing_puuid.as_deref() == Some(puuid) {
            self.editing_puuid = None;
        }
    }

    /// 开始编辑
    pub fn start_edit(&mut self, puuid: &str) {
        if let Some(player) = self.players.iter().find(|p| p.puuid == puuid) {
            self.editing_puuid = Some(puuid.to_string());
            self.editing_tag_buffer = player.tag.clone().unwrap_or_default();
            self.editing_notes_buffer = player.notes.clone();
        }
    }

    /// 保存编辑内容
    pub fn save_edit(&mut self, puuid: &str) {
        if let Some(player) = self.players.iter_mut().find(|p| p.puuid == puuid) {
            let t = self.editing_tag_buffer.trim();
            player.tag = if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            };
            player.notes = self.editing_notes_buffer.trim().to_string();
        }
        self.editing_puuid = None;
    }

    /// 取消编辑
    pub fn cancel_edit(&mut self) {
        self.editing_puuid = None;
    }

    // ── 渲染入口 ──

    /// 统一完整交互式面板渲染
    pub fn render_panel<F1, F2, F3, F4>(
        &self,
        on_filter_tag: F1,
        on_toggle_pin: F2,
        on_toggle_expand: F3,
        on_remove_player: F4,
    ) -> impl IntoElement
    where
        F1: Fn(&str, &mut Window, &mut App) + 'static + Clone,
        F2: Fn(&str, &mut Window, &mut App) + 'static + Clone,
        F3: Fn(&str, &mut Window, &mut App) + 'static + Clone,
        F4: Fn(&str, &mut Window, &mut App) + 'static + Clone,
    {
        let filtered = self.filtered_players();
        let total_players = self.players.len();
        let total_encounters = self.total_encounter_count();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme::BG_MAIN)
            .p_4()
            .gap_4()
            .overflow_hidden()
            // 顶部操作栏与统计
            .child(self.render_header_bar(total_players, total_encounters, on_filter_tag))
            // 极光水晶玩家列表 (Grid/Cards)
            .child(self.render_players_content(
                filtered,
                on_toggle_pin,
                on_toggle_expand,
                on_remove_player,
            ))
    }

    /// 无外部回调的默认渲染
    pub fn render(&self) -> impl IntoElement {
        self.render_panel(
            |_tag, _win, _app| {},
            |_puuid, _win, _app| {},
            |_puuid, _win, _app| {},
            |_puuid, _win, _app| {},
        )
    }

    /// 顶部搜索、标签胶囊与统计徽章
    fn render_header_bar<F>(
        &self,
        total_players: usize,
        total_encounters: u32,
        on_filter_tag: F,
    ) -> impl IntoElement
    where
        F: Fn(&str, &mut Window, &mut App) + 'static + Clone,
    {
        Card::new().p(px(14.0)).child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                // 第一行：标题 + 统计徽章 + 导入/导出动作
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
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(theme::TEXT_PRIMARY)
                                        .child("路人集管理"),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            Badge::new(format!("共记录 {} 位玩家", total_players))
                                                .variant(BadgeVariant::Primary),
                                        )
                                        .child(
                                            Badge::new(format!("累计相遇 {} 局", total_encounters))
                                                .variant(BadgeVariant::Default),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .px_2p5()
                                        .py_1()
                                        .rounded(px(6.0))
                                        .bg(Theme::CARD_BG)
                                        .border_1()
                                        .border_color(Theme::CARD_BORDER)
                                        .hover(|s| {
                                            s.bg(Theme::CARD_BG_HOVER)
                                                .border_color(Theme::AURORA_CYAN)
                                        })
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .gap_1()
                                        .text_xs()
                                        .text_color(Theme::TEXT_SECONDARY)
                                        .child("📥")
                                        .child("导入备份"),
                                )
                                .child(
                                    div()
                                        .px_2p5()
                                        .py_1()
                                        .rounded(px(6.0))
                                        .bg(Theme::CARD_BG)
                                        .border_1()
                                        .border_color(Theme::CARD_BORDER)
                                        .hover(|s| {
                                            s.bg(Theme::CARD_BG_HOVER)
                                                .border_color(Theme::AURORA_CYAN)
                                        })
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .gap_1()
                                        .text_xs()
                                        .text_color(Theme::TEXT_SECONDARY)
                                        .child("📤")
                                        .child("导出 JSON"),
                                ),
                        ),
                )
                // 第二行：搜索输入框 + 标签过滤胶囊组
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_4()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .bg(theme::BG_INPUT)
                                .border_1()
                                .border_color(theme::BORDER_SECONDARY)
                                .rounded(px(6.0))
                                .px_3()
                                .py_1()
                                .w(px(260.0))
                                .gap_2()
                                .child(div().text_xs().text_color(theme::TEXT_MUTED).child("🔍"))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(if self.search_query.is_empty() {
                                            theme::TEXT_MUTED
                                        } else {
                                            theme::TEXT_PRIMARY
                                        })
                                        .child(if self.search_query.is_empty() {
                                            "搜索召唤师名 / 标签 / 备注...".to_string()
                                        } else {
                                            self.search_query.clone()
                                        }),
                                ),
                        )
                        .child(div().flex().items_center().gap_2().children(
                            RECOMMENDED_TAGS.iter().map(|&tag_name| {
                                let is_active = self.active_tag_filter == tag_name;
                                let on_filter = on_filter_tag.clone();
                                div()
                                    .px_3()
                                    .py_1()
                                    .rounded(px(14.0))
                                    .cursor_pointer()
                                    .border_1()
                                    .border_color(if is_active {
                                        Theme::AURORA_CYAN
                                    } else {
                                        Theme::CARD_BORDER
                                    })
                                    .bg(if is_active {
                                        Theme::AURORA_CYAN.opacity(0.18)
                                    } else {
                                        Theme::CARD_BG
                                    })
                                    .text_xs()
                                    .font_weight(if is_active {
                                        FontWeight::BOLD
                                    } else {
                                        FontWeight::NORMAL
                                    })
                                    .text_color(if is_active {
                                        Theme::ACCENT_PRIMARY
                                    } else {
                                        Theme::TEXT_SECONDARY
                                    })
                                    .hover(|s| {
                                        s.bg(if is_active {
                                            Theme::AURORA_CYAN.opacity(0.24)
                                        } else {
                                            Theme::CARD_BG_HOVER
                                        })
                                    })
                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                        on_filter(tag_name, win, app);
                                    })
                                    .child(tag_name)
                            }),
                        )),
                ),
        )
    }

    /// 玩家列表容器
    fn render_players_content<'a, F1, F2, F3>(
        &'a self,
        filtered: Vec<&'a SavedPlayer>,
        on_toggle_pin: F1,
        on_toggle_expand: F2,
        on_remove_player: F3,
    ) -> AnyElement
    where
        F1: Fn(&str, &mut Window, &mut App) + 'static + Clone,
        F2: Fn(&str, &mut Window, &mut App) + 'static + Clone,
        F3: Fn(&str, &mut Window, &mut App) + 'static + Clone,
    {
        if filtered.is_empty() {
            return div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_2()
                .bg(Theme::CARD_BG)
                .border_1()
                .border_color(Theme::CARD_BORDER)
                .rounded_xl()
                .p_8()
                .child(div().text_3xl().child("📑"))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(Theme::TEXT_PRIMARY)
                        .child("未找到符合条件的路人玩家"),
                )
                .child(
                    div().text_xs().text_color(Theme::TEXT_MUTED).child(
                        "对局结束后曾同局的玩家将自动收录在此，您也可以给他们添加高亮标签。",
                    ),
                )
                .into_any_element();
        }

        div()
            .id("saved-players-scroll")
            .flex()
            .flex_col()
            .flex_1()
            .gap_3()
            .overflow_y_scroll()
            .children(filtered.into_iter().map(|p| {
                self.render_player_card(
                    p,
                    on_toggle_pin.clone(),
                    on_toggle_expand.clone(),
                    on_remove_player.clone(),
                )
            }))
            .into_any_element()
    }

    /// 单个极光水晶卡片渲染
    fn render_player_card<F1, F2, F3>(
        &self,
        p: &SavedPlayer,
        on_toggle_pin: F1,
        on_toggle_expand: F2,
        on_remove_player: F3,
    ) -> impl IntoElement
    where
        F1: Fn(&str, &mut Window, &mut App) + 'static + Clone,
        F2: Fn(&str, &mut Window, &mut App) + 'static + Clone,
        F3: Fn(&str, &mut Window, &mut App) + 'static + Clone,
    {
        let is_expanded = self.expanded_puuid.as_deref() == Some(&p.puuid);
        let is_editing = self.editing_puuid.as_deref() == Some(&p.puuid);
        let loader = global_asset_loader();

        // 头像与英雄图标
        let avatar_url = p.profile_icon_url();
        let avatar_img = loader.get_cached_render_image(&avatar_url);

        let champ_url = p.champion_icon_url();
        let champ_img = champ_url
            .as_ref()
            .and_then(|u| loader.get_cached_render_image(u));

        // 标签颜色
        let tag_label = p.tag.clone().unwrap_or_else(|| "未打标".to_string());
        let (tag_bg, tag_text, tag_border) = tag_badge_colors(&tag_label);

        let p_puuid_pin = p.puuid.clone();
        let p_puuid_expand = p.puuid.clone();
        let p_puuid_delete = p.puuid.clone();

        Card::new()
            .p(px(14.0))
            .border_color(if p.is_pinned {
                Theme::GOLD.opacity(0.6)
            } else {
                Theme::CARD_BORDER
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    // 卡片主行
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            // 左侧：头像 + 召唤师全名 + 标签 + 笔记
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    // 头像复合体 (召唤师头像 + 常用英雄小图标叠加)
                                    .child(
                                        div()
                                            .relative()
                                            .size(px(46.0))
                                            .rounded_lg()
                                            .bg(Theme::ACCENT_LIGHT)
                                            .border_1()
                                            .border_color(if p.is_pinned {
                                                Theme::GOLD
                                            } else {
                                                Theme::AURORA_CYAN.opacity(0.5)
                                            })
                                            .overflow_hidden()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .when_some(avatar_img, |this, img_ref| {
                                                this.child(
                                                    img(img_ref)
                                                        .size_full()
                                                        .object_fit(ObjectFit::Cover),
                                                )
                                            })
                                            .when(p.champion_id > 0, |this| {
                                                this.child(
                                                    div()
                                                        .absolute()
                                                        .bottom_0()
                                                        .right_0()
                                                        .size(px(20.0))
                                                        .rounded_sm()
                                                        .border_1()
                                                        .border_color(Theme::CARD_BG)
                                                        .bg(Theme::ITEM_SLOT_BG)
                                                        .overflow_hidden()
                                                        .when_some(champ_img, |sub, c_img| {
                                                            sub.child(
                                                                img(c_img)
                                                                    .size_full()
                                                                    .object_fit(ObjectFit::Cover),
                                                            )
                                                        }),
                                                )
                                            }),
                                    )
                                    // 姓名与标签详情
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
                                                    // 置顶星标
                                                    .when(p.is_pinned, |this| {
                                                        this.child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(Theme::GOLD)
                                                                .child("★"),
                                                        )
                                                    })
                                                    // 召唤师名
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(Theme::TEXT_PRIMARY)
                                                            .child(p.summoner_name.clone()),
                                                    )
                                                    // TagLine
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(Theme::TEXT_MUTED)
                                                            .child(if p.tag_line.is_empty() {
                                                                "#—".to_string()
                                                            } else {
                                                                format!("#{}", p.tag_line)
                                                            }),
                                                    )
                                                    // 标签徽标 Badge
                                                    .when(p.tag.is_some(), {
                                                        let tag_label = tag_label.clone();
                                                        |this| {
                                                            this.child(
                                                                div()
                                                                    .px_2()
                                                                    .py_0p5()
                                                                    .rounded(px(4.0))
                                                                    .bg(tag_bg)
                                                                    .border_1()
                                                                    .border_color(tag_border)
                                                                    .text_xs()
                                                                    .font_weight(
                                                                        FontWeight::SEMIBOLD,
                                                                    )
                                                                    .text_color(tag_text)
                                                                    .child(tag_label),
                                                            )
                                                        }
                                                    }),
                                            )
                                            // 备忘录 / 笔记显示
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(if p.notes.is_empty() {
                                                        Theme::TEXT_MUTED
                                                    } else {
                                                        Theme::TEXT_SECONDARY
                                                    })
                                                    .child(if p.notes.is_empty() {
                                                        "暂无备注".to_string()
                                                    } else {
                                                        p.notes.clone()
                                                    }),
                                            ),
                                    ),
                            )
                            // 右侧：相遇局数指示器 + 上次相遇时间 + 快捷操作按钮
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    // 相遇局数指示器
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .items_end()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(Theme::ACCENT_PRIMARY)
                                                    .child(format!(
                                                        "同局 {} 次",
                                                        p.encountered_count
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(Theme::TEXT_MUTED)
                                                    .child(p.formatted_last_met()),
                                            ),
                                    )
                                    // 操作按钮组
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_1p5()
                                            // 置顶 / 取消置顶
                                            .child(
                                                div()
                                                    .px_2()
                                                    .py_1()
                                                    .rounded(px(6.0))
                                                    .bg(if p.is_pinned {
                                                        Theme::GOLD.opacity(0.15)
                                                    } else {
                                                        Theme::CARD_BG
                                                    })
                                                    .border_1()
                                                    .border_color(if p.is_pinned {
                                                        Theme::GOLD
                                                    } else {
                                                        Theme::CARD_BORDER
                                                    })
                                                    .text_xs()
                                                    .text_color(if p.is_pinned {
                                                        Theme::GOLD
                                                    } else {
                                                        Theme::TEXT_SECONDARY
                                                    })
                                                    .cursor_pointer()
                                                    .hover(|s| s.bg(Theme::CARD_BG_HOVER))
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        move |_ev, win, app| {
                                                            on_toggle_pin(&p_puuid_pin, win, app);
                                                        },
                                                    )
                                                    .child(if p.is_pinned {
                                                        "取消置顶"
                                                    } else {
                                                        "置顶"
                                                    }),
                                            )
                                            // 编辑按钮
                                            .child(
                                                div()
                                                    .px_2()
                                                    .py_1()
                                                    .rounded(px(6.0))
                                                    .bg(Theme::CARD_BG)
                                                    .border_1()
                                                    .border_color(Theme::CARD_BORDER)
                                                    .text_xs()
                                                    .text_color(Theme::TEXT_SECONDARY)
                                                    .cursor_pointer()
                                                    .hover(|s| {
                                                        s.bg(Theme::CARD_BG_HOVER)
                                                            .text_color(Theme::ACCENT_PRIMARY)
                                                    })
                                                    .child("编辑"),
                                            )
                                            // 查看相遇对局展开按钮
                                            .child(
                                                div()
                                                    .px_2p5()
                                                    .py_1()
                                                    .rounded(px(6.0))
                                                    .bg(if is_expanded {
                                                        Theme::AURORA_CYAN.opacity(0.18)
                                                    } else {
                                                        Theme::CARD_BG
                                                    })
                                                    .border_1()
                                                    .border_color(if is_expanded {
                                                        Theme::AURORA_CYAN
                                                    } else {
                                                        Theme::CARD_BORDER
                                                    })
                                                    .text_xs()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(if is_expanded {
                                                        Theme::ACCENT_PRIMARY
                                                    } else {
                                                        Theme::TEXT_PRIMARY
                                                    })
                                                    .cursor_pointer()
                                                    .hover(|s| s.bg(Theme::CARD_BG_HOVER))
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        move |_ev, win, app| {
                                                            on_toggle_expand(
                                                                &p_puuid_expand,
                                                                win,
                                                                app,
                                                            );
                                                        },
                                                    )
                                                    .child(if is_expanded {
                                                        "收起对局 ▲"
                                                    } else {
                                                        "相遇记录 ▼"
                                                    }),
                                            )
                                            // 删除记录按钮
                                            .child(
                                                div()
                                                    .px_2()
                                                    .py_1()
                                                    .rounded(px(6.0))
                                                    .bg(Theme::CARD_BG)
                                                    .border_1()
                                                    .border_color(Theme::CARD_BORDER)
                                                    .text_xs()
                                                    .text_color(Theme::LOSS_TEXT)
                                                    .cursor_pointer()
                                                    .hover(|s| {
                                                        s.bg(Theme::LOSS_BG)
                                                            .border_color(Theme::LOSS_BORDER)
                                                    })
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        move |_ev, win, app| {
                                                            on_remove_player(
                                                                &p_puuid_delete,
                                                                win,
                                                                app,
                                                            );
                                                        },
                                                    )
                                                    .child("删除"),
                                            ),
                                    ),
                            ),
                    )
                    // 编辑浮层 / 内联表单
                    .when(is_editing, |this| {
                        this.child(
                            div()
                                .flex()
                                .items_center()
                                .gap_3()
                                .p_2p5()
                                .rounded_lg()
                                .bg(Theme::CARD_BG_HOVER)
                                .border_1()
                                .border_color(Theme::AURORA_CYAN.opacity(0.4))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(Theme::TEXT_PRIMARY)
                                        .child("标签:"),
                                )
                                .child(
                                    div()
                                        .px_2()
                                        .py_1()
                                        .rounded(px(4.0))
                                        .bg(theme::BG_INPUT)
                                        .border_1()
                                        .border_color(Theme::CARD_BORDER)
                                        .text_xs()
                                        .text_color(Theme::TEXT_PRIMARY)
                                        .child(if self.editing_tag_buffer.is_empty() {
                                            "输入标签 (如: 大腿/避坑)".to_string()
                                        } else {
                                            self.editing_tag_buffer.clone()
                                        }),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(Theme::TEXT_PRIMARY)
                                        .child("备注:"),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .px_2()
                                        .py_1()
                                        .rounded(px(4.0))
                                        .bg(theme::BG_INPUT)
                                        .border_1()
                                        .border_color(Theme::CARD_BORDER)
                                        .text_xs()
                                        .text_color(Theme::TEXT_PRIMARY)
                                        .child(if self.editing_notes_buffer.is_empty() {
                                            "输入玩家特征、常用英雄或打法风格...".to_string()
                                        } else {
                                            self.editing_notes_buffer.clone()
                                        }),
                                )
                                .child(
                                    div()
                                        .px_3()
                                        .py_1()
                                        .rounded(px(4.0))
                                        .bg(Theme::ACCENT_PRIMARY)
                                        .text_color(hsla(0.0, 0.0, 1.0, 1.0))
                                        .text_xs()
                                        .cursor_pointer()
                                        .child("保存"),
                                )
                                .child(
                                    div()
                                        .px_3()
                                        .py_1()
                                        .rounded(px(4.0))
                                        .bg(Theme::CARD_BG)
                                        .border_1()
                                        .border_color(Theme::CARD_BORDER)
                                        .text_xs()
                                        .text_color(Theme::TEXT_SECONDARY)
                                        .cursor_pointer()
                                        .child("取消"),
                                ),
                        )
                    })
                    // 展开的相遇对局历史列表
                    .when(is_expanded, |this| {
                        this.child(self.render_encounter_history(p))
                    }),
            )
    }

    /// 展开的相遇对局历史面板
    fn render_encounter_history(&self, p: &SavedPlayer) -> impl IntoElement {
        if p.encounter_history.is_empty() {
            return div()
                .p_3()
                .rounded_lg()
                .bg(Theme::CARD_BG_HOVER)
                .border_1()
                .border_color(Theme::CARD_BORDER)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_xs()
                        .text_color(Theme::TEXT_MUTED)
                        .child("暂无详细相遇对局历史记录"),
                );
        }

        let loader = global_asset_loader();

        div()
            .flex()
            .flex_col()
            .gap_1p5()
            .p_2p5()
            .rounded_lg()
            .bg(Theme::CARD_BG_HOVER)
            .border_1()
            .border_color(Theme::CARD_BORDER)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pb_1()
                    .border_b_1()
                    .border_color(Theme::CARD_BORDER)
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::TEXT_SECONDARY)
                            .child("历史同局战绩流水"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::TEXT_MUTED)
                            .child(format!("共 {} 局记录", p.encounter_history.len())),
                    ),
            )
            .children(p.encounter_history.iter().map(|rec| {
                let is_win = rec.result.as_deref() == Some("Win");
                let champ_icon = if rec.champion_id > 0 {
                    format!(
                        "/lol-game-data/assets/v1/champion-icons/{}.png",
                        rec.champion_id
                    )
                } else {
                    String::new()
                };
                let c_img = if !champ_icon.is_empty() {
                    loader.get_cached_render_image(&champ_icon)
                } else {
                    None
                };

                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .py_1()
                    .px_2()
                    .rounded(px(4.0))
                    .bg(Theme::CARD_BG)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            // 胜负状态圆点/条
                            .child(div().w(px(3.0)).h(px(14.0)).rounded_full().bg(if is_win {
                                Theme::WIN_BAR
                            } else {
                                Theme::LOSS_BAR
                            }))
                            // 对局使用的英雄小图标
                            .when(rec.champion_id > 0, |sub| {
                                sub.child(
                                    div()
                                        .size(px(20.0))
                                        .rounded_sm()
                                        .overflow_hidden()
                                        .bg(Theme::ITEM_SLOT_BG)
                                        .when_some(c_img, |this_sub, img_ref| {
                                            this_sub.child(
                                                img(img_ref)
                                                    .size_full()
                                                    .object_fit(ObjectFit::Cover),
                                            )
                                        }),
                                )
                            })
                            // 模式名称
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child(queue_type_display_name(&rec.queue_type)),
                            )
                            // Game ID
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::TEXT_MUTED)
                                    .child(format!("#{}", rec.game_id)),
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
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(if is_win {
                                        Theme::WIN_TEXT
                                    } else {
                                        Theme::LOSS_TEXT
                                    })
                                    .child(if is_win { "胜利" } else { "失败" }),
                            )
                            .child(div().text_xs().text_color(Theme::TEXT_MUTED).child({
                                let diff = chrono::Utc::now().timestamp_millis() - rec.timestamp;
                                let days = (diff / (1000 * 3600 * 24)).max(0);
                                if days == 0 {
                                    "今天".to_string()
                                } else {
                                    format!("{}天前", days)
                                }
                            })),
                    )
            }))
    }
}
