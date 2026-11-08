use crate::asset_loader::global_asset_loader;
use crate::components::badge::{Badge, BadgeVariant};
use crate::components::card::Card;
use crate::theme::{self, Theme};
use gpui::prelude::FluentBuilder;
use gpui::*;
use yuumi_core::parsers::saved_player::{SavedPlayer, RECOMMENDED_TAGS};
use yuumi_core::storage::SavedPlayersStorage;

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

/// 路人集界面交互动作分发枚举
#[derive(Debug, Clone)]
pub enum SavedPlayersAction {
    FilterTag(String),
    TogglePin(String),
    ToggleExpand(String),
    RemovePlayer(String),
    StartEdit(String),
    SaveEdit(String),
    CancelEdit,
    SetEditTag(String),
    LoadMore,
    SearchInput(String),
    SearchSubmit,
    SearchClear,
    EditTagInput(String),
    EditNotesInput(String),
}

/// 每页展示路人数量（防止一次性渲染上千张卡片撑爆渲染管线）
pub const SAVED_PLAYERS_PAGE_SIZE: usize = 30;

pub struct SavedPlayersView {
    /// 持久化数据库存储句柄
    pub storage: Option<SavedPlayersStorage>,
    /// 当前登录召唤师 PUUID
    pub self_puuid: Option<String>,
    /// 当前已加载展示的路人记录列表（按页累加）
    pub players: Vec<SavedPlayer>,
    /// 匹配当前条件的总人数
    pub total_count: u32,
    /// 当前加载到的页码（基于 1）
    pub current_page: usize,
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
    /// 正在拉取下一页标志位（防止快速滚动时重复并发加载）
    pub is_loading_more: bool,
    /// 独立列表滚动控制与位置跟踪句柄（固定顶部后由列表独占滚动）
    pub scroll_handle: ScrollHandle,
    /// 顶部搜索框焦点句柄
    pub search_focus_handle: FocusHandle,
    /// 内联编辑标签输入框焦点句柄
    pub edit_tag_focus_handle: FocusHandle,
    /// 内联编辑备注输入框焦点句柄
    pub edit_notes_focus_handle: FocusHandle,
}

impl SavedPlayersView {
    pub fn new(cx: &mut App) -> Self {
        let storage = SavedPlayersStorage::open_default().ok();
        let (total_count, players) = if let Some(ref s) = storage {
            let total = s.count_players(None, None, None).unwrap_or(0);
            let list = s
                .query_players(None, None, None, SAVED_PLAYERS_PAGE_SIZE, 0)
                .unwrap_or_default();
            (total, list)
        } else {
            (0, Vec::new())
        };

        Self {
            storage,
            self_puuid: None,
            players,
            total_count,
            current_page: 1,
            active_tag_filter: "全部".to_string(),
            search_query: String::new(),
            expanded_puuid: None,
            editing_puuid: None,
            editing_tag_buffer: String::new(),
            editing_notes_buffer: String::new(),
            is_loading_more: false,
            scroll_handle: ScrollHandle::new(),
            search_focus_handle: cx.focus_handle(),
            edit_tag_focus_handle: cx.focus_handle(),
            edit_notes_focus_handle: cx.focus_handle(),
        }
    }

    /// 使用自定义数据初始化
    pub fn with_players(mut self, players: Vec<SavedPlayer>) -> Self {
        self.total_count = players.len() as u32;
        self.players = players;
        self
    }

    /// 重新从本地数据库加载（重置到第一页，按过滤与搜索词精准查库）
    pub fn reload(&mut self, self_puuid: Option<&str>) {
        self.is_loading_more = false;
        if let Some(ref s) = self.storage {
            let filter = if self.active_tag_filter == "全部" {
                None
            } else {
                Some(self.active_tag_filter.as_str())
            };
            let search = if self.search_query.trim().is_empty() {
                None
            } else {
                Some(self.search_query.trim())
            };

            self.current_page = 1;
            self.total_count = s.count_players(self_puuid, filter, search).unwrap_or(0);
            if let Ok(list) =
                s.query_players(self_puuid, filter, search, SAVED_PLAYERS_PAGE_SIZE, 0)
            {
                self.players = list;
            }
        }
    }

    /// 加载下一页数据并追加至当前列表（带防重入保护）
    pub fn load_more(&mut self) {
        if self.is_loading_more || self.players.len() >= self.total_count as usize {
            return;
        }
        self.is_loading_more = true;

        if let Some(ref s) = self.storage {
            let next_page = self.current_page + 1;
            let offset = (next_page - 1) * SAVED_PLAYERS_PAGE_SIZE;
            let filter = if self.active_tag_filter == "全部" {
                None
            } else {
                Some(self.active_tag_filter.as_str())
            };
            let search = if self.search_query.trim().is_empty() {
                None
            } else {
                Some(self.search_query.trim())
            };
            let self_p = self.self_puuid.as_deref();

            if let Ok(more) =
                s.query_players(self_p, filter, search, SAVED_PLAYERS_PAGE_SIZE, offset)
            {
                self.players.extend(more);
                self.current_page = next_page;
            }
        }

        self.is_loading_more = false;
    }

    /// 设置当前召唤师 PUUID 并刷新
    pub fn set_self_puuid(&mut self, puuid: Option<String>) {
        let self_p = puuid.clone();
        self.self_puuid = puuid;
        self.reload(self_p.as_deref());
    }

    /// 当前展示的玩家列表引用（直接基于数据库已分页排序的结果）
    pub fn displayed_players(&self) -> Vec<&SavedPlayer> {
        self.players.iter().collect()
    }

    /// 切换标签过滤
    pub fn set_filter(&mut self, tag: impl Into<String>) {
        self.active_tag_filter = tag.into();
        let self_p = self.self_puuid.clone();
        self.reload(self_p.as_deref());
    }

    /// 设置搜索词并刷新
    pub fn set_search(&mut self, query: impl Into<String>) {
        self.search_query = query.into();
        let self_p = self.self_puuid.clone();
        self.reload(self_p.as_deref());
    }

    /// 累计同局相遇总局数
    pub fn total_encounter_count(&self) -> u32 {
        self.players.iter().map(|p| p.encountered_count).sum()
    }

    /// 切换展开某个玩家的历史
    pub fn toggle_expand(&mut self, puuid: &str) {
        if self.expanded_puuid.as_deref() == Some(puuid) {
            self.expanded_puuid = None;
        } else {
            self.expanded_puuid = Some(puuid.to_string());
            // 如果历史记录为空，从持久化层查询最近 20 场真实相遇对局
            if let Some(player) = self.players.iter_mut().find(|p| p.puuid == puuid) {
                if player.encounter_history.is_empty() {
                    if let Some(ref s) = self.storage {
                        let self_p = self.self_puuid.as_deref();
                        if let Ok(history) = s.query_encountered_games(puuid, self_p, 20) {
                            player.encounter_history = history;
                        }
                    }
                }
            }
        }
    }

    /// 切换玩家置顶状态（写回数据库）
    pub fn toggle_pin(&mut self, puuid: &str) {
        let mut new_pinned = false;
        if let Some(player) = self.players.iter_mut().find(|p| p.puuid == puuid) {
            player.is_pinned = !player.is_pinned;
            new_pinned = player.is_pinned;
        }
        if let Some(ref s) = self.storage {
            let self_p = self.self_puuid.as_deref().unwrap_or("");
            let _ = s.update_pin(puuid, self_p, new_pinned);
        }
    }

    /// 删除玩家记录（同步从数据库移除）
    pub fn remove_player(&mut self, puuid: &str) {
        self.players.retain(|p| p.puuid != puuid);
        if self.expanded_puuid.as_deref() == Some(puuid) {
            self.expanded_puuid = None;
        }
        if self.editing_puuid.as_deref() == Some(puuid) {
            self.editing_puuid = None;
        }
        if let Some(ref s) = self.storage {
            let self_p = self.self_puuid.as_deref().unwrap_or("");
            let _ = s.delete_player(puuid, self_p);
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

    /// 快捷选择标签
    pub fn set_editing_tag(&mut self, tag: &str) {
        if tag == "清除标签" {
            self.editing_tag_buffer.clear();
        } else {
            self.editing_tag_buffer = tag.to_string();
        }
    }

    /// 保存编辑内容（写回数据库）
    pub fn save_edit(&mut self, puuid: &str) {
        let tag_opt = {
            let t = self.editing_tag_buffer.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        };
        let notes_str = self.editing_notes_buffer.trim().to_string();

        if let Some(player) = self.players.iter_mut().find(|p| p.puuid == puuid) {
            player.tag = tag_opt.clone();
            player.notes = notes_str.clone();
        }
        if let Some(ref s) = self.storage {
            let self_p = self.self_puuid.as_deref().unwrap_or("");
            let _ = s.update_tag_and_notes(puuid, self_p, tag_opt.as_deref(), &notes_str);
        }
        self.editing_puuid = None;
    }

    /// 取消编辑
    pub fn cancel_edit(&mut self) {
        self.editing_puuid = None;
    }

    // ── 渲染入口 ──

    /// 统一完整交互式面板渲染
    pub fn render_panel<F>(&self, window: &mut Window, on_action: F) -> impl IntoElement
    where
        F: Fn(SavedPlayersAction, &mut Window, &mut App) + 'static + Clone,
    {
        let displayed = self.displayed_players();
        let total_players = self.total_count as usize;
        let total_encounters = self.players.iter().map(|p| p.encountered_count).sum();

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .gap_3()
            .overflow_hidden()
            // 顶部基本信息卡片（固定在上方，绝不随滚动滚走）
            .child(div().flex_none().w_full().child(self.render_header_bar(
                total_players,
                total_encounters,
                window,
                on_action.clone(),
            )))
            // 下方独立滚动的卡片列表（独占单层平滑滚动）
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .min_h(px(0.0))
                    .overflow_hidden()
                    .child(self.render_players_content(displayed, window, on_action)),
            )
    }

    /// 无外部回调的默认渲染
    pub fn render(&self, window: &mut Window) -> impl IntoElement {
        self.render_panel(window, |_act, _win, _app| {})
    }

    /// 顶部搜索、标签胶囊与统计徽章
    fn render_header_bar<F>(
        &self,
        total_players: usize,
        total_encounters: u32,
        window: &mut Window,
        on_action: F,
    ) -> impl IntoElement
    where
        F: Fn(SavedPlayersAction, &mut Window, &mut App) + 'static + Clone,
    {
        let is_search_focused = self.search_focus_handle.is_focused(window);
        let current_search = self.search_query.clone();
        let search_focus_handle = self.search_focus_handle.clone();
        let on_action_search = on_action.clone();
        let on_action_clear = on_action.clone();

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
                // 第二行：支持真实键盘打字的搜索输入框 + 标签过滤胶囊组
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_4()
                        .child(
                            div()
                                .id("saved-players-search-box")
                                .track_focus(&search_focus_handle)
                                .flex()
                                .items_center()
                                .justify_between()
                                .bg(theme::BG_INPUT)
                                .border_1()
                                .border_color(if is_search_focused {
                                    Theme::AURORA_CYAN
                                } else {
                                    theme::BORDER_SECONDARY
                                })
                                .rounded(px(6.0))
                                .px_3()
                                .py_1()
                                .w(px(280.0))
                                .cursor_text()
                                .on_mouse_down(MouseButton::Left, move |_ev, win, _app| {
                                    win.focus(&search_focus_handle);
                                })
                                .on_key_down(move |ev: &KeyDownEvent, win, app| {
                                    let key_str = &ev.keystroke.key;
                                    if key_str == "enter" {
                                        on_action_search(
                                            SavedPlayersAction::SearchSubmit,
                                            win,
                                            app,
                                        );
                                    } else if key_str == "backspace" {
                                        let mut s = current_search.clone();
                                        s.pop();
                                        on_action_search(
                                            SavedPlayersAction::SearchInput(s),
                                            win,
                                            app,
                                        );
                                    } else if let Some(ref c) = ev.keystroke.key_char {
                                        let mut s = current_search.clone();
                                        s.push_str(c);
                                        on_action_search(
                                            SavedPlayersAction::SearchInput(s),
                                            win,
                                            app,
                                        );
                                    } else if key_str.chars().count() == 1
                                        && !ev.keystroke.modifiers.control
                                        && !ev.keystroke.modifiers.alt
                                    {
                                        let mut s = current_search.clone();
                                        s.push_str(key_str);
                                        on_action_search(
                                            SavedPlayersAction::SearchInput(s),
                                            win,
                                            app,
                                        );
                                    }
                                })
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme::TEXT_MUTED)
                                                .child("🔍"),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(if self.search_query.is_empty() {
                                                    theme::TEXT_MUTED
                                                } else {
                                                    theme::TEXT_PRIMARY
                                                })
                                                .child(if self.search_query.is_empty() {
                                                    if is_search_focused {
                                                        "|".to_string()
                                                    } else {
                                                        "搜索召唤师名 / 标签 / 备注...".to_string()
                                                    }
                                                } else {
                                                    if is_search_focused {
                                                        format!("{}|", self.search_query)
                                                    } else {
                                                        self.search_query.clone()
                                                    }
                                                }),
                                        ),
                                )
                                .when(!self.search_query.is_empty(), |this| {
                                    this.child(
                                        div()
                                            .text_xs()
                                            .text_color(Theme::TEXT_MUTED)
                                            .cursor_pointer()
                                            .hover(|s| s.text_color(Theme::TEXT_PRIMARY))
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                move |_ev, win, app| {
                                                    on_action_clear(
                                                        SavedPlayersAction::SearchClear,
                                                        win,
                                                        app,
                                                    );
                                                },
                                            )
                                            .child("✕"),
                                    )
                                }),
                        )
                        .child(div().flex().items_center().gap_2().children(
                            RECOMMENDED_TAGS.iter().map(|&tag_name| {
                                let is_active = self.active_tag_filter == tag_name;
                                let on_action_tag = on_action.clone();
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
                                        on_action_tag(
                                            SavedPlayersAction::FilterTag(tag_name.to_string()),
                                            win,
                                            app,
                                        );
                                    })
                                    .child(tag_name)
                            }),
                        )),
                ),
        )
    }

    /// 玩家列表容器
    fn render_players_content<'a, F>(
        &'a self,
        displayed: Vec<&'a SavedPlayer>,
        window: &mut Window,
        on_action: F,
    ) -> AnyElement
    where
        F: Fn(SavedPlayersAction, &mut Window, &mut App) + 'static + Clone,
    {
        if displayed.is_empty() {
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
                .child(div().text_xs().text_color(Theme::TEXT_MUTED).child(
                    "对局结束后曾同局的玩家将自动收录在此，您也可以给他们添加高亮标签与备注。",
                ))
                .into_any_element();
        }

        let has_more = self.players.len() < self.total_count as usize;
        let loaded_count = self.players.len();
        let total_count = self.total_count;
        let is_loading = self.is_loading_more;
        let on_more_click = on_action.clone();
        let on_more_wheel = on_action.clone();
        let scroll_handle = self.scroll_handle.clone();

        div()
            .id("saved-players-scroll")
            .flex()
            .flex_col()
            .size_full()
            .gap_3()
            .overflow_y_scroll()
            .track_scroll(&self.scroll_handle)
            .on_scroll_wheel(move |ev, win, app| {
                // 滚轮向下滚动且靠近底部时，直接自动异步加载下一批路人记录，无需任何鼠标移动
                if has_more && ev.delta.pixel_delta(px(20.0)).y < px(0.0) {
                    let offset_y = -scroll_handle.offset().y;
                    let max_h = scroll_handle.max_offset().height;
                    if offset_y + px(400.0) >= max_h {
                        on_more_wheel(SavedPlayersAction::LoadMore, win, app);
                    }
                }
            })
            .children(
                displayed
                    .into_iter()
                    .map(|p| self.render_player_card(p, window, on_action.clone())),
            )
            .when(has_more, |this| {
                let on_more = on_more_click.clone();
                this.child(
                    div()
                        .id("saved-players-infinite-sentinel")
                        .flex()
                        .items_center()
                        .justify_center()
                        .py_3()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            on_more(SavedPlayersAction::LoadMore, win, app);
                        })
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .px_4()
                                .py_2()
                                .rounded_lg()
                                .bg(Theme::CARD_BG)
                                .border_1()
                                .border_color(Theme::AURORA_CYAN.opacity(0.35))
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(Theme::ACCENT_PRIMARY)
                                .child(if is_loading {
                                    "⏳ 正在自动载入更多记录..."
                                } else {
                                    "⬇ 向下滚动自动加载更多记录"
                                })
                                .child(format!("({} / {} 人)", loaded_count, total_count)),
                        ),
                )
            })
            .when(!has_more && total_count > 0, |this| {
                this.child(
                    div().flex().items_center().justify_center().py_3().child(
                        div()
                            .text_xs()
                            .text_color(Theme::TEXT_MUTED)
                            .child(format!("已载入全部 {} 位路人记录", total_count)),
                    ),
                )
            })
            .into_any_element()
    }

    /// 单个极光水晶卡片渲染
    fn render_player_card<F>(
        &self,
        p: &SavedPlayer,
        window: &mut Window,
        on_action: F,
    ) -> impl IntoElement
    where
        F: Fn(SavedPlayersAction, &mut Window, &mut App) + 'static + Clone,
    {
        let is_expanded = self.expanded_puuid.as_deref() == Some(&p.puuid);
        let is_editing = self.editing_puuid.as_deref() == Some(&p.puuid);
        let loader = global_asset_loader();

        // 头像与英雄图标（纯内存高速查询，微秒级）
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
        let p_puuid_edit = p.puuid.clone();
        let p_puuid_save = p.puuid.clone();

        let on_action_pin = on_action.clone();
        let on_action_expand = on_action.clone();
        let on_action_delete = on_action.clone();
        let on_action_edit = on_action.clone();
        let on_action_save = on_action.clone();
        let on_action_cancel = on_action.clone();

        div()
            .p(px(12.0))
            .rounded_lg()
            .bg(Theme::CARD_BG)
            .border_1()
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
                                                            on_action_pin(
                                                                SavedPlayersAction::TogglePin(p_puuid_pin.clone()),
                                                                win,
                                                                app,
                                                            );
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
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        move |_ev, win, app| {
                                                            on_action_edit(
                                                                SavedPlayersAction::StartEdit(p_puuid_edit.clone()),
                                                                win,
                                                                app,
                                                            );
                                                        },
                                                    )
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
                                                            on_action_expand(
                                                                SavedPlayersAction::ToggleExpand(p_puuid_expand.clone()),
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
                                                            on_action_delete(
                                                                SavedPlayersAction::RemovePlayer(p_puuid_delete.clone()),
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
                        let p_puuid_save_clone = p_puuid_save.clone();
                        let on_save_clone = on_action_save.clone();
                        let on_cancel_clone = on_action_cancel.clone();
                        let on_action_tag_btn = on_action.clone();

                        let is_tag_focused = self.edit_tag_focus_handle.is_focused(window);
                        let is_notes_focused = self.edit_notes_focus_handle.is_focused(window);
                        let edit_tag_h = self.edit_tag_focus_handle.clone();
                        let edit_notes_h = self.edit_notes_focus_handle.clone();
                        let current_tag = self.editing_tag_buffer.clone();
                        let current_notes = self.editing_notes_buffer.clone();
                        let on_action_tag_key = on_action.clone();
                        let on_action_notes_key = on_action.clone();

                        this.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .p_2p5()
                                .rounded_lg()
                                .bg(Theme::CARD_BG_HOVER)
                                .border_1()
                                .border_color(Theme::AURORA_CYAN.opacity(0.4))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(Theme::TEXT_PRIMARY)
                                                .child("快速标签:"),
                                        )
                                        .child(
                                            div().flex().items_center().gap_1p5().children(
                                                ["大腿", "避坑", "绝活", "演员/挂机", "清除标签"].iter().map(|&t| {
                                                    let on_tag_click = on_action_tag_btn.clone();
                                                    let is_selected = self.editing_tag_buffer == t || (t == "清除标签" && self.editing_tag_buffer.is_empty());
                                                    div()
                                                        .px_2()
                                                        .py_0p5()
                                                        .rounded(px(4.0))
                                                        .bg(if is_selected { Theme::AURORA_CYAN.opacity(0.2) } else { Theme::CARD_BG })
                                                        .border_1()
                                                        .border_color(if is_selected { Theme::AURORA_CYAN } else { Theme::CARD_BORDER })
                                                        .text_xs()
                                                        .text_color(if is_selected { Theme::ACCENT_PRIMARY } else { Theme::TEXT_SECONDARY })
                                                        .cursor_pointer()
                                                        .hover(|s| s.bg(Theme::CARD_BG_HOVER))
                                                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                                            on_tag_click(SavedPlayersAction::SetEditTag(t.to_string()), win, app);
                                                        })
                                                        .child(t)
                                                }),
                                            ),
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
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(Theme::TEXT_PRIMARY)
                                                .child("标签输入:"),
                                        )
                                        .child({
                                            let h = edit_tag_h.clone();
                                            div()
                                                .id("edit-tag-input-box")
                                                .track_focus(&h)
                                                .px_2()
                                                .py_1()
                                                .w(px(140.0))
                                                .rounded(px(4.0))
                                                .bg(theme::BG_INPUT)
                                                .border_1()
                                                .border_color(if is_tag_focused {
                                                    Theme::AURORA_CYAN
                                                } else {
                                                    Theme::CARD_BORDER
                                                })
                                                .cursor_text()
                                                .on_mouse_down(MouseButton::Left, move |_ev, win, _app| {
                                                    win.focus(&h);
                                                })
                                                .on_key_down(move |ev: &KeyDownEvent, win, app| {
                                                    let key_str = &ev.keystroke.key;
                                                    if key_str == "backspace" {
                                                        let mut s = current_tag.clone();
                                                        s.pop();
                                                        on_action_tag_key(SavedPlayersAction::EditTagInput(s), win, app);
                                                    } else if let Some(ref c) = ev.keystroke.key_char {
                                                        let mut s = current_tag.clone();
                                                        s.push_str(c);
                                                        on_action_tag_key(SavedPlayersAction::EditTagInput(s), win, app);
                                                    } else if key_str.chars().count() == 1
                                                        && !ev.keystroke.modifiers.control
                                                        && !ev.keystroke.modifiers.alt
                                                    {
                                                        let mut s = current_tag.clone();
                                                        s.push_str(key_str);
                                                        on_action_tag_key(SavedPlayersAction::EditTagInput(s), win, app);
                                                    }
                                                })
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(if self.editing_tag_buffer.is_empty() {
                                                            Theme::TEXT_MUTED
                                                        } else {
                                                            Theme::TEXT_PRIMARY
                                                        })
                                                        .child(if self.editing_tag_buffer.is_empty() {
                                                            if is_tag_focused {
                                                                "|".to_string()
                                                            } else {
                                                                "自定义标签...".to_string()
                                                            }
                                                        } else {
                                                            if is_tag_focused {
                                                                format!("{}|", self.editing_tag_buffer)
                                                            } else {
                                                                self.editing_tag_buffer.clone()
                                                            }
                                                        }),
                                                )
                                        })
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(Theme::TEXT_PRIMARY)
                                                .child("备注:"),
                                        )
                                        .child({
                                            let h = edit_notes_h.clone();
                                            div()
                                                .id("edit-notes-input-box")
                                                .track_focus(&h)
                                                .flex_1()
                                                .px_2()
                                                .py_1()
                                                .rounded(px(4.0))
                                                .bg(theme::BG_INPUT)
                                                .border_1()
                                                .border_color(if is_notes_focused {
                                                    Theme::AURORA_CYAN
                                                } else {
                                                    Theme::CARD_BORDER
                                                })
                                                .cursor_text()
                                                .on_mouse_down(MouseButton::Left, move |_ev, win, _app| {
                                                    win.focus(&h);
                                                })
                                                .on_key_down(move |ev: &KeyDownEvent, win, app| {
                                                    let key_str = &ev.keystroke.key;
                                                    if key_str == "backspace" {
                                                        let mut s = current_notes.clone();
                                                        s.pop();
                                                        on_action_notes_key(SavedPlayersAction::EditNotesInput(s), win, app);
                                                    } else if let Some(ref c) = ev.keystroke.key_char {
                                                        let mut s = current_notes.clone();
                                                        s.push_str(c);
                                                        on_action_notes_key(SavedPlayersAction::EditNotesInput(s), win, app);
                                                    } else if key_str.chars().count() == 1
                                                        && !ev.keystroke.modifiers.control
                                                        && !ev.keystroke.modifiers.alt
                                                    {
                                                        let mut s = current_notes.clone();
                                                        s.push_str(key_str);
                                                        on_action_notes_key(SavedPlayersAction::EditNotesInput(s), win, app);
                                                    }
                                                })
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(if self.editing_notes_buffer.is_empty() {
                                                            Theme::TEXT_MUTED
                                                        } else {
                                                            Theme::TEXT_PRIMARY
                                                        })
                                                        .child(if self.editing_notes_buffer.is_empty() {
                                                            if is_notes_focused {
                                                                "|".to_string()
                                                            } else {
                                                                "输入玩家特征、常用英雄打法或避坑原因...".to_string()
                                                            }
                                                        } else {
                                                            if is_notes_focused {
                                                                format!("{}|", self.editing_notes_buffer)
                                                            } else {
                                                                self.editing_notes_buffer.clone()
                                                            }
                                                        }),
                                                )
                                        })
                                        .child(
                                            div()
                                                .px_3()
                                                .py_1()
                                                .rounded(px(4.0))
                                                .bg(Theme::ACCENT_PRIMARY)
                                                .text_color(hsla(0.0, 0.0, 1.0, 1.0))
                                                .text_xs()
                                                .cursor_pointer()
                                                .hover(|s| s.bg(Theme::ACCENT_PRIMARY.opacity(0.85)))
                                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                                    on_save_clone(SavedPlayersAction::SaveEdit(p_puuid_save_clone.clone()), win, app);
                                                })
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
                                                .hover(|s| s.bg(Theme::CARD_BG_HOVER))
                                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                                    on_cancel_clone(SavedPlayersAction::CancelEdit, win, app);
                                                })
                                                .child("取消"),
                                        ),
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
