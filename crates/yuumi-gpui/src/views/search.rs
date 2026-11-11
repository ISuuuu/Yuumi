//! 战绩查询主视图 (SearchView)
//!
//! 1:1 复刻 Vue 原版：
//! - `src/views/Search.vue`
//! - `src/components/search/MiniMatchList.vue`
//! - `src/components/search/MatchListItem.vue`
//! - `src/components/search/MatchDetailPanel.vue`
//! - `src/components/ReplayButton.vue`
//!
//! 严格依照 [05-5-GPUI_VUE_PAGE_PARITY.md] S01~S12 规范实现。

use crate::components::card::Card;
use crate::components::icon::icon;
use crate::components::lcu_image::{LcuImage, LcuImageShape};
use crate::components::text_field::search_input_field;
use crate::match_model::{GameDetail, GameDetailPlayer, GameDetailTeam, MatchItem};
use crate::theme::Theme;
use crate::views::career::format_local_month_day;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::collections::HashMap;

/// 队列筛选选项定义（S05 对照 Vue QUEUE_OPTIONS）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchQueueOption {
    pub id: Option<i32>,
    pub label: &'static str,
}

pub const SEARCH_QUEUE_OPTIONS: [SearchQueueOption; 7] = [
    SearchQueueOption {
        id: None,
        label: "全部",
    },
    SearchQueueOption {
        id: Some(2400),
        label: "海克斯大乱斗",
    },
    SearchQueueOption {
        id: Some(2450),
        label: "经典海斗",
    },
    SearchQueueOption {
        id: Some(450),
        label: "极地大乱斗",
    },
    SearchQueueOption {
        id: Some(430),
        label: "匹配模式",
    },
    SearchQueueOption {
        id: Some(420),
        label: "单双排位",
    },
    SearchQueueOption {
        id: Some(440),
        label: "灵活排位",
    },
];

/// 地图状态图标路径映射 (对照 Vue MatchDetailPanel 计算逻辑)
pub fn get_map_icon_url(map_id: u32, queue_id: i32, win: bool) -> &'static str {
    if map_id == 11 {
        if win {
            "/images/sr-victory.png"
        } else {
            "/images/sr-defeat.png"
        }
    } else if map_id == 12 {
        if win {
            "/images/ha-victory.png"
        } else {
            "/images/ha-defeat.png"
        }
    } else if map_id == 30 || queue_id == 1700 {
        if win {
            "/images/arena-victory.png"
        } else {
            "/images/arena-defeat.png"
        }
    } else {
        if win {
            "/images/other-victory.png"
        } else {
            "/images/other-defeat.png"
        }
    }
}

/// 格式化大整数为带逗号的千分位字符串 (对齐 Vue 原版 toLocaleString)
pub fn format_num(val: i64) -> String {
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

/// 队列 ID 转中文模式名 (对齐 Vue 原版 getQueueName)
pub fn queue_id_to_name(queue_id: i32, fallback_map: &str) -> String {
    match queue_id {
        400 => "征召模式".to_string(),
        420 => "单双排位".to_string(),
        430 => "匹配模式".to_string(),
        440 => "灵活排位".to_string(),
        450 => "极地大乱斗".to_string(),
        2400 => "海克斯大乱斗".to_string(),
        2450 => "经典海斗".to_string(),
        900 => "无限火力".to_string(),
        1010 => "随机无限火力".to_string(),
        1020 => "克隆模式".to_string(),
        1300 => "极限闪击".to_string(),
        1700 | 1710 => "斗魂竞技场".to_string(),
        4300 | 4310 => "经典模式".to_string(),
        480 | 490 => "快速模式".to_string(),
        0 => "自定义模式".to_string(),
        _ => {
            if !fallback_map.is_empty() {
                fallback_map.to_string()
            } else {
                "自定义模式".to_string()
            }
        }
    }
}

/// 战绩查询主视图模型
pub struct SearchView {
    /// 搜索输入框文本
    pub search_query: String,
    /// 选中的队列过滤 (None: 全部, Some(2400), Some(450), Some(420) 等)
    pub active_queue_filter: Option<i32>,
    /// 历史对局全量列表
    pub matches: Vec<MatchItem>,
    /// 当前分页页码（1-based，默认 1，每页精确 10 局）
    pub current_page: usize,
    /// 每页展示条数
    pub matches_per_page: usize,
    /// 当前选中的对局 ID
    pub selected_game_id: Option<u64>,
    /// 对局详情缓存 (game_id -> GameDetail)
    pub details_cache: HashMap<u64, GameDetail>,
    /// 最近一次成功载入的详情，切换对局时用作占位，避免右侧先清空再重绘造成的闪烁
    pub last_detail: Option<GameDetail>,
    /// 详情加载状态
    pub is_loading_detail: bool,
    /// 请求序号：防止快速点击不同对局时，旧请求结果回写新对局
    pub select_request_id: usize,
    /// 是否正在进行网络搜索召唤师/战绩
    pub is_searching: bool,
    /// 是否正在翻页加载中
    pub is_page_switching: bool,
    /// 后台静默刷新状态（对局结束重搜：顶部 2px 细进度条，不遮挡面板内容）
    pub is_background_refreshing: bool,
    /// 搜索失败或空态错误提示
    pub search_error: Option<String>,
    /// 详情加载失败错误提示
    pub detail_error: Option<String>,
    /// 搜索世代 ID：防止快速并发搜索时旧响应回填覆盖新结果
    pub search_generation: usize,
    /// 当前查询到的召唤师完整显示名 (Name#Tag)
    pub current_summoner_name: Option<String>,
    /// 当前查询到的召唤师 PUUID
    pub current_summoner_puuid: Option<String>,
    /// 当前查询到的召唤师 ID
    pub current_summoner_id: u64,
    /// 队列筛选下拉是否展开
    pub queue_menu_open: bool,
    /// 搜索历史记录（最新在前，去重，上限 20 条）
    pub search_history: Vec<String>,
    /// 历史下拉是否展开
    pub show_history: bool,
    /// 输入框自上次提交搜索后是否被编辑过（未编辑时历史下拉展示全部记录）
    pub search_query_dirty: bool,
    /// 是否开启对局上传开关
    pub upload_enabled: bool,
    /// 是否在配置中配置了 UploadApiUrl（有配置才渲染复选框）
    pub has_upload_api_url: bool,
    /// 外部跳转期待选中的对局 ID
    pub target_select_game_id: Option<u64>,
    /// 复制状态反馈提示
    pub copied_feedback: Option<u64>,
    /// 对局详情玩家段位 (puuid -> 段位)，受「对局信息显示段位」开关控制按需拉取
    pub participant_ranks: HashMap<String, ParticipantRankInfo>,
    /// 段位请求序号：防止旧详情的段位结果回写当前选中详情
    pub rank_request_id: usize,
    /// 键盘焦点句柄
    pub focus_handle: FocusHandle,
}

/// 对局详情玩家行段位徽章信息
#[derive(Debug, Clone)]
pub struct ParticipantRankInfo {
    /// 段位英文（用于挑选内置图标），例如 "DIAMOND"
    pub tier: String,
    /// 段位中文标签，例如 "钻石 I"
    pub rank_text: String,
}

/// 本地持久化搜索历史路径
fn get_history_file_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|p| p.join("Yuumi").join("search_history.json"))
}

fn load_history_from_disk() -> Vec<String> {
    if let Some(path) = get_history_file_path() {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(list) = serde_json::from_str::<Vec<String>>(&content) {
                return list;
            }
        }
    }
    Vec::new()
}

fn save_history_to_disk(list: &[String]) {
    if let Some(path) = get_history_file_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(content) = serde_json::to_string_pretty(list) {
            let _ = std::fs::write(path, content);
        }
    }
}

impl SearchView {
    pub fn new(cx: &mut App) -> Self {
        let history = load_history_from_disk();
        Self {
            search_query: String::new(),
            active_queue_filter: None,
            matches: Vec::new(),
            current_page: 1,
            matches_per_page: 10,
            selected_game_id: None,
            details_cache: HashMap::new(),
            last_detail: None,
            is_loading_detail: false,
            select_request_id: 0,
            is_searching: false,
            is_page_switching: false,
            is_background_refreshing: false,
            search_error: None,
            detail_error: None,
            search_generation: 0,
            current_summoner_name: None,
            current_summoner_puuid: None,
            current_summoner_id: 0,
            queue_menu_open: false,
            search_history: history,
            show_history: false,
            search_query_dirty: false,
            upload_enabled: true,
            has_upload_api_url: false,
            target_select_game_id: None,
            copied_feedback: None,
            participant_ranks: HashMap::new(),
            rank_request_id: 0,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn with_matches(mut self, matches: Vec<MatchItem>) -> Self {
        if let Some(first) = matches.first() {
            self.selected_game_id = Some(first.display.game_id);
        }
        self.matches = matches;
        self
    }

    pub fn set_search_query(&mut self, query: impl Into<String>) {
        self.search_query = query.into();
    }

    pub fn set_searching(&mut self, is_searching: bool) {
        self.is_searching = is_searching;
        if is_searching {
            self.search_error = None;
        }
    }

    pub fn set_search_error(&mut self, err: impl Into<String>) {
        self.search_error = Some(err.into());
        self.is_searching = false;
        self.is_background_refreshing = false;
        self.is_page_switching = false;
    }

    /// 保存搜索历史项（去重、最新在前、上限 20 条，并持久化到本地）
    pub fn save_to_history(&mut self, entry: &str) {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            return;
        }
        self.search_history.retain(|item| item != trimmed);
        self.search_history.insert(0, trimmed.to_string());
        if self.search_history.len() > 20 {
            self.search_history.truncate(20);
        }
        save_history_to_disk(&self.search_history);
    }

    /// 移除单条搜索历史
    pub fn remove_from_history(&mut self, entry: &str) {
        self.search_history.retain(|item| item != entry);
        save_history_to_disk(&self.search_history);
    }

    /// 填充搜索结果并定位对局
    pub fn set_search_results(
        &mut self,
        summoner_name: String,
        puuid: String,
        summoner_id: u64,
        matches: Vec<MatchItem>,
    ) {
        self.save_to_history(&summoner_name);
        self.current_summoner_name = Some(summoner_name);
        self.current_summoner_puuid = Some(puuid);
        self.current_summoner_id = summoner_id;
        self.is_searching = false;
        self.is_background_refreshing = false;
        self.search_error = None;
        self.current_page = 1;

        if let Some(target) = self.target_select_game_id.take() {
            if matches.iter().any(|m| m.display.game_id == target) {
                self.selected_game_id = Some(target);
            } else {
                self.selected_game_id = matches.first().map(|m| m.display.game_id);
            }
        } else {
            self.selected_game_id = matches.first().map(|m| m.display.game_id);
        }

        self.matches = matches;
    }

    /// 添加单局详情缓存
    pub fn insert_game_detail(&mut self, game_id: u64, detail: GameDetail) {
        // 记录最近一次成功载入的详情：切换对局时缓存尚未命中的这段窗口里，
        // 继续沿用上一份内容渲染，避免右侧区域先空白再重绘的闪烁
        self.last_detail = Some(detail.clone());
        self.details_cache.insert(game_id, detail);
    }

    /// 获取符合队列筛选的全量战绩列表
    pub fn filtered_matches(&self) -> Vec<&MatchItem> {
        match self.active_queue_filter {
            None | Some(-1) => self.matches.iter().collect(),
            Some(qid) => self
                .matches
                .iter()
                .filter(|m| m.display.queue_id == qid)
                .collect(),
        }
    }

    /// 当前页切片对局、总数与是否有下一页
    pub fn paged_filtered_matches(&self) -> (Vec<&MatchItem>, usize, bool) {
        let all = self.filtered_matches();
        let total = all.len();
        let page_size = self.matches_per_page;
        let start = (self.current_page.saturating_sub(1)) * page_size;
        let end = (start + page_size).min(total);
        let items = if start < total {
            all[start..end].to_vec()
        } else {
            Vec::new()
        };
        let has_more = total > end;
        (items, total, has_more)
    }

    /// 匹配当前输入词的历史记录过滤项
    pub fn filtered_history(&self) -> Vec<&str> {
        let q = self.search_query.trim().to_lowercase();
        if q.is_empty() {
            return self.search_history.iter().map(String::as_str).collect();
        }

        // 自上次提交搜索后未编辑输入（如查询失败后重新点击聚焦），则展示全部历史记录
        if !self.search_query_dirty {
            return self.search_history.iter().map(String::as_str).collect();
        }

        // 如果输入框内容和当前展示的召唤师姓名完全一致（代表没有进行新的输入，仅仅是点击聚焦），则展示全部历史记录
        if let Some(ref current_name) = self.current_summoner_name {
            let cn = current_name.trim().to_lowercase();
            if cn == q {
                return self.search_history.iter().map(String::as_str).collect();
            }
            if let Some((name_part, _)) = cn.split_once('#') {
                if name_part.trim().to_lowercase() == q {
                    return self.search_history.iter().map(String::as_str).collect();
                }
            }
        }

        self.search_history
            .iter()
            .filter(|item| item.to_lowercase().contains(&q))
            .map(String::as_str)
            .collect()
    }

    /// 渲染战绩查询完整视图
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        is_connected: bool,
        on_select_match: impl Fn(u64, &mut Window, &mut App) + 'static + Clone,
        on_filter_queue: impl Fn(Option<i32>, &mut Window, &mut App) + 'static + Clone,
        on_search: impl Fn(&str, &mut Window, &mut App) + 'static + Clone,
        on_input_change: impl Fn(String, &mut Window, &mut App) + 'static + Clone,
        is_search_focused: bool,
        on_open_history: impl Fn(&mut Window, &mut App) + 'static + Clone,
        on_toggle_queue_menu: impl Fn(bool, &mut Window, &mut App) + 'static + Clone,
        on_delete_history: impl Fn(&str, &mut Window, &mut App) + 'static + Clone,
        on_close_history: impl Fn(&mut Window, &mut App) + 'static + Clone,
        on_prev_page: impl Fn(&mut Window, &mut App) + 'static + Clone,
        on_next_page: impl Fn(&mut Window, &mut App) + 'static + Clone,
        on_career_click: impl Fn(&mut Window, &mut App) + 'static + Clone,
        on_toggle_upload: impl Fn(bool, &mut Window, &mut App) + 'static + Clone,
        on_copy_game_id: impl Fn(u64, &mut Window, &mut App) + 'static + Clone,
        on_player_click: impl Fn(String, u64, String, &mut Window, &mut App) + 'static + Clone,
        on_replay: impl Fn(u64, &mut Window, &mut App) + 'static + Clone,
        on_retry_detail: impl Fn(u64, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        // S01: 离线时复用离线状态组件
        if !is_connected {
            return div()
                .flex()
                .flex_col()
                .size_full()
                .bg(Theme::BG_APP())
                .items_center()
                .justify_center()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_3()
                        .child(div().text_size(px(48.0)).child("🐱"))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(Theme::TEXT_MUTED())
                                .child("英雄联盟客户端未连接，请先启动游戏客户端"),
                        ),
                )
                .into_any_element();
        }

        let (paged_matches, total_count, has_more) = self.paged_filtered_matches();
        let selected_game_id = self
            .selected_game_id
            .filter(|id| paged_matches.iter().any(|m| m.display.game_id == *id))
            .or_else(|| paged_matches.first().map(|m| m.display.game_id));

        // 仅在当前有效列表非空时允许显示详情或回退，零场对局时呈现空态
        let current_detail = if paged_matches.is_empty() {
            None
        } else {
            selected_game_id
                .and_then(|id| self.details_cache.get(&id))
                .or(self.last_detail.as_ref())
        };

        div()
            .id("search-view-root")
            .size_full()
            .flex()
            .flex_col()
            .bg(Theme::BG_APP())
            .px(px(24.0))
            .py(px(16.0))
            .overflow_hidden()
            .child(
                div()
                    .w_full()
                    .max_w(px(1380.0))
                    .mx_auto()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .overflow_hidden()
                    // 1. 顶部搜索工具栏 (S01, S02, S03, S04, S05)
                    .child(self.render_search_bar(
                        on_search.clone(),
                        on_input_change.clone(),
                        is_search_focused,
                        on_open_history,
                        on_delete_history,
                        on_close_history,
                        on_career_click,
                        on_filter_queue,
                        on_toggle_queue_menu,
                        on_toggle_upload,
                    ))
                    // 2. 搜索错误提示条
                    .when_some(self.search_error.as_ref(), |this, err| {
                        this.child(
                            div()
                                .mb(px(12.0))
                                .px_3()
                                .py_1p5()
                                .rounded(Theme::RADIUS_SM)
                                .bg(Theme::LOSS_BG())
                                .border_1()
                                .border_color(Theme::LOSS_BORDER())
                                .text_xs()
                                .text_color(Theme::LOSS_TEXT())
                                .child(err.clone()),
                        )
                    })
                    // 3. 分栏对局面板容器 (S06: 180px + 16px + 剩余宽度)
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            // 后台静默刷新细进度条 (2px)
                            .when(self.is_background_refreshing, |this| {
                                this.child(
                                    div()
                                        .absolute()
                                        .top_0()
                                        .left_0()
                                        .right_0()
                                        .h(px(2.0))
                                        .bg(Theme::ACCENT_PRIMARY()),
                                )
                            })
                            // 双栏布局网格
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .size_full()
                                    .gap(px(16.0))
                                    .overflow_hidden()
                                    // 左侧：180px 迷你卡片列表 + 分页条 (S06, S07, S08)
                                    .child(self.render_left_match_list(
                                        paged_matches,
                                        total_count,
                                        has_more,
                                        selected_game_id,
                                        on_select_match,
                                        on_prev_page,
                                        on_next_page,
                                    ))
                                    // 右侧：对局详情面板 (S09, S10, S11, S12)
                                    .child(self.render_detail_panel(
                                        current_detail,
                                        selected_game_id,
                                        on_copy_game_id,
                                        on_player_click,
                                        on_replay.clone(),
                                        on_retry_detail.clone(),
                                    )),
                            )
                            // 搜索中半透明全屏遮罩
                            .when(self.is_searching, |this| {
                                this.child(
                                    div()
                                        .absolute()
                                        .inset_0()
                                        .bg(hsla(0.0, 0.0, 1.0, 0.45))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(
                                            div()
                                                .px_8()
                                                .py_6()
                                                .rounded(Theme::RADIUS_LG)
                                                .bg(Theme::CARD_BG_HOVER())
                                                .border_1()
                                                .border_color(Theme::CARD_BORDER())
                                                .shadow_md()
                                                .flex()
                                                .flex_col()
                                                .items_center()
                                                .gap_3()
                                                .child(div().text_xl().child("⏳"))
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .font_weight(FontWeight::BOLD)
                                                        .text_color(Theme::TEXT_PRIMARY())
                                                        .child("正在查询战绩历史..."),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(Theme::TEXT_MUTED())
                                                        .child(
                                                            "正在通过 LCU 拉取召唤师档案与近期对局",
                                                        ),
                                                ),
                                        ),
                                )
                            }),
                    ),
            )
            .into_any_element()
    }

    /// 顶部搜索栏：弹性搜索框(最大 580px) + 生涯 + 130px 队列下拉 + 条件上传
    #[allow(clippy::too_many_arguments)]
    fn render_search_bar(
        &self,
        on_search: impl Fn(&str, &mut Window, &mut App) + 'static + Clone,
        on_input_change: impl Fn(String, &mut Window, &mut App) + 'static + Clone,
        is_search_focused: bool,
        on_open_history: impl Fn(&mut Window, &mut App) + 'static + Clone,
        on_delete_history: impl Fn(&str, &mut Window, &mut App) + 'static + Clone,
        on_close_history: impl Fn(&mut Window, &mut App) + 'static + Clone,
        on_career_click: impl Fn(&mut Window, &mut App) + 'static + Clone,
        on_filter_queue: impl Fn(Option<i32>, &mut Window, &mut App) + 'static + Clone,
        on_toggle_queue_menu: impl Fn(bool, &mut Window, &mut App) + 'static + Clone,
        on_toggle_upload: impl Fn(bool, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let is_disabled = self.is_searching || self.is_background_refreshing;
        let query_text = self.search_query.clone();
        let history_matches = self.filtered_history();
        let is_history_open =
            self.show_history && !self.is_searching && !history_matches.is_empty();
        let close_hist_out = on_close_history.clone();

        div().w_full().mb(px(19.2)).child(
            Card::new().py(px(10.0)).px(px(16.0)).child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .w_full()
                    // 1. 弹性搜索输入框包装器 (max-width 580px, 占满剩余弹性宽度，含历史面板)
                    .child(
                        div()
                            .id("search-input-wrapper")
                            .relative()
                            .flex_1()
                            .max_w(px(580.0))
                            .when(is_history_open, |w| {
                                w.on_mouse_down_out(move |_ev, win, app| {
                                    close_hist_out(win, app);
                                })
                            })
                            .child({
                                let on_submit = on_search.clone();
                                let on_change = on_input_change.clone();
                                div().id("search-input-click-area").on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                    if !is_disabled {
                                        on_open_history(win, app);
                                    }
                                }).child(search_input_field(
                                    "search-bar-input",
                                    &query_text,
                                    "输入召唤师名称进行搜索...",
                                    px(580.0),
                                    &self.focus_handle,
                                    is_search_focused,
                                    is_disabled,
                                    self.is_searching,
                                    move |val, win, app| on_change(val, win, app),
                                    move |win, app| {
                                        on_input_change(String::new(), win, app);
                                    },
                                    move |val, win, app| on_submit(val, win, app),
                                ))
                            })
                            // 搜索历史下拉浮层 (5.2: show_history 唯一开关，自然换行，顶接缝底圆角，260px上限)
                            .when(is_history_open, |this| {
                                let del_cb = on_delete_history.clone();
                                let choose_cb = on_search.clone();
                                let close_cb = on_close_history.clone();
                                this.child(
                                    deferred(
                                        div()
                                            .id("search-history-dropdown")
                                            .absolute()
                                            .top(px(28.0))
                                            .left_0()
                                            .right_0()
                                            .occlude()
                                            .bg(Theme::CARD_BG_HOVER())
                                            .border_1()
                                            .border_t_0()
                                            .border_color(Theme::CARD_BORDER())
                                            .rounded_b(px(8.0))
                                            .shadow_lg()
                                            .p(px(12.0))
                                            .max_h(px(260.0))
                                            .overflow_y_scroll()
                                            .flex()
                                            .flex_col()
                                            .gap(px(10.0))
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .pb(px(6.0))
                                                    .border_b_1()
                                                    .border_color(Theme::CARD_BORDER())
                                                    .child(
                                                        div()
                                                            .text_size(px(11.5))
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(Theme::TEXT_MUTED())
                                                            .child("🕐 搜索历史"),
                                                    ),
                                            )
                                            .child(div().flex().flex_wrap().gap(px(8.0)).children(
                                                history_matches.into_iter().map(|item| {
                                                    let item_val = item.to_string();
                                                    let item_for_del = item.to_string();
                                                    let choose = choose_cb.clone();
                                                    let del = del_cb.clone();
                                                    let close_on_choose = close_cb.clone();
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap(px(6.0))
                                                        .px(px(10.0))
                                                        .py(px(4.0))
                                                        .rounded(px(14.0))
                                                        .bg(Theme::HOVER_BG())
                                                        .border_1()
                                                        .border_color(Theme::CARD_BORDER())
                                                        .cursor_pointer()
                                                        .max_w(px(260.0))
                                                        .hover(|s| {
                                                            s.bg(Theme::ACCENT_LIGHT())
                                                                .border_color(Theme::ACCENT_PRIMARY())
                                                        })
                                                        .on_mouse_down(
                                                            MouseButton::Left,
                                                            move |_ev, win, app| {
                                                                close_on_choose(win, app);
                                                                choose(&item_val, win, app);
                                                            },
                                                        )
                                                        .child(
                                                            div()
                                                                .overflow_hidden()
                                                                .whitespace_nowrap()
                                                                .text_size(px(12.5))
                                                                .font_weight(FontWeight::SEMIBOLD)
                                                                .text_color(Theme::TEXT_PRIMARY())
                                                                .child(item.to_string()),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex_none()
                                                                .size(px(14.0))
                                                                .rounded_full()
                                                                .flex()
                                                                .items_center()
                                                                .justify_center()
                                                                .text_size(px(10.0))
                                                                .text_color(Theme::TEXT_MUTED())
                                                                .hover(|s| {
                                                                    s.bg(Theme::LOSS_BG()).text_color(
                                                                        Theme::LOSS_TEXT(),
                                                                    )
                                                                })
                                                                .on_mouse_down(
                                                                    MouseButton::Left,
                                                                    move |_ev, _win, app| {
                                                                        app.stop_propagation();
                                                                        del(
                                                                            &item_for_del,
                                                                            _win,
                                                                            app,
                                                                        );
                                                                    },
                                                                )
                                                                .child("✕"),
                                                        )
                                                }),
                                            )),
                                    )
                                    .with_priority(3),
                                )
                            }),
                    )
                    // 2. 生涯按钮 (S04: 高28px, 圆角10px, 字号14px, 左右各10px内边距, 常规字重)
                    .child(
                        div()
                            .id("search-career-btn")
                            .h(px(28.0))
                            .px(px(10.0))
                            .rounded(px(10.0))
                            .bg(Theme::CARD_BG())
                            .border_1()
                            .border_color(Theme::CARD_BORDER())
                            .text_color(Theme::TEXT_PRIMARY())
                            .hover(|s| {
                                s.border_color(Theme::ACCENT_PRIMARY())
                                    .text_color(Theme::ACCENT_PRIMARY())
                            })
                            .active(|s| {
                                s.border_color(Theme::ACCENT_HOVER())
                                    .text_color(Theme::ACCENT_HOVER())
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                on_career_click(win, app);
                            })
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_weight(FontWeight::NORMAL)
                                    .child("生涯"),
                            ),
                    )
                    // 3. 队列筛选下拉 (S05: 130px 宽, 对齐 Vue Naive UI n-select small, 与历史互斥)
                    .child(self.render_search_queue_select(
                        on_filter_queue,
                        on_toggle_queue_menu,
                        on_close_history,
                    ))
                    // 4. 上传复选框 (S01: 仅配置了 UploadApiUrl 时显示)
                    .when(self.has_upload_api_url, |this| {
                        let is_checked = self.upload_enabled;
                        let toggle_cb = on_toggle_upload.clone();
                        this.child(
                            div()
                                .id("search-upload-checkbox")
                                .flex()
                                .items_center()
                                .gap_2()
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                    toggle_cb(!is_checked, win, app);
                                })
                                .child(
                                    div()
                                        .size(px(14.0))
                                        .rounded(px(3.0))
                                        .border_1()
                                        .border_color(if is_checked {
                                            Theme::ACCENT_PRIMARY()
                                        } else {
                                            Theme::CARD_BORDER()
                                        })
                                        .bg(if is_checked {
                                            Theme::ACCENT_PRIMARY()
                                        } else {
                                            Theme::CARD_BG()
                                        })
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_xs()
                                        .text_color(Theme::TEXT_ON_PRIMARY())
                                        .child(if is_checked { "✓" } else { "" }),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(Theme::TEXT_MUTED())
                                        .child("Upload matches"),
                                ),
                        )
                    }),
            ),
        )
    }

    /// 战绩查询专用队列下拉 (1:1 复刻 Vue Search.vue n-select size="small" 130px)
    fn render_search_queue_select(
        &self,
        on_filter_queue: impl Fn(Option<i32>, &mut Window, &mut App) + 'static + Clone,
        on_toggle_queue_menu: impl Fn(bool, &mut Window, &mut App) + 'static + Clone,
        on_close_history: impl Fn(&mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let is_open = self.queue_menu_open;
        let on_toggle = on_toggle_queue_menu.clone();
        let on_close_hist = on_close_history;
        let current_filter = self.active_queue_filter;
        let label = SEARCH_QUEUE_OPTIONS
            .iter()
            .find(|o| o.id == current_filter)
            .map(|o| o.label)
            .unwrap_or("全部")
            .to_string();

        let on_close_out = on_toggle_queue_menu.clone();
        let on_select_cb = on_filter_queue.clone();
        let on_close_select = on_toggle_queue_menu;

        let close_on_click_out = on_close_out;
        div()
            .id("search-queue-select-wrapper")
            .w(px(130.0))
            .relative()
            .when(is_open, |w| {
                w.on_mouse_down_out(move |_ev, win, app| {
                    close_on_click_out(false, win, app);
                })
            })
            .child(
                div()
                    .id("search-queue-select-trigger")
                    .w(px(130.0))
                    .h(px(28.0))
                    .rounded(px(10.0))
                    .bg(Theme::CARD_BG())
                    .border_1()
                    .border_color(if is_open {
                        Theme::ACCENT_PRIMARY()
                    } else {
                        Theme::CARD_BORDER()
                    })
                    .map(|mut el| {
                        if is_open {
                            el.style().box_shadow = Some(vec![BoxShadow {
                                color: Theme::ACCENT_ALPHA_20(),
                                offset: point(px(0.0), px(0.0)),
                                blur_radius: px(0.0),
                                spread_radius: px(2.0),
                            }]);
                        }
                        el
                    })
                    .hover(|s| {
                        if !is_open {
                            s.border_color(Theme::ACCENT_PRIMARY())
                        } else {
                            s
                        }
                    })
                    .flex()
                    .items_center()
                    .justify_between()
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                        if !is_open {
                            on_close_hist(win, app);
                        }
                        on_toggle(!is_open, win, app);
                    })
                    .child(
                        div()
                            .pl(px(12.0))
                            .text_size(px(14.0))
                            .font_weight(FontWeight::NORMAL)
                            .text_color(Theme::TEXT_PRIMARY())
                            .child(label),
                    )
                    .child(
                        div()
                            .pr(px(8.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                svg()
                                    .path(if is_open {
                                        "icons/chevron-up.svg"
                                    } else {
                                        "icons/chevron-down.svg"
                                    })
                                    .flex_none()
                                    .size(px(16.0))
                                    .text_color(Theme::TEXT_MUTED()),
                            ),
                    ),
            )
            .when(is_open, |this| {
                this.child(
                    deferred(
                        div()
                            .id("search-queue-select-menu")
                            .absolute()
                            .top_full()
                            .left_0()
                            .mt(px(4.0))
                            .min_w(px(130.0))
                            .w_full()
                            .py(px(4.0))
                            .occlude()
                            .bg(Theme::CARD_BG_HOVER())
                            .border_1()
                            .border_color(Theme::CARD_BORDER())
                            .rounded(px(8.0))
                            .shadow_md()
                            .children(SEARCH_QUEUE_OPTIONS.into_iter().map(|option| {
                                let is_selected = current_filter == option.id;
                                let select = on_select_cb.clone();
                                let close = on_close_select.clone();
                                div()
                                    .h(px(28.0))
                                    .px(px(12.0))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(Theme::HOVER_BG()))
                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                        select(option.id, win, app);
                                        close(false, win, app);
                                    })
                                    .child(
                                        div()
                                            .text_size(px(14.0))
                                            .font_weight(FontWeight::NORMAL)
                                            .text_color(if is_selected {
                                                Theme::ACCENT_PRIMARY()
                                            } else {
                                                Theme::TEXT_PRIMARY()
                                            })
                                            .child(option.label),
                                    )
                                    .when(is_selected, |row| {
                                        row.child(
                                            div()
                                                .text_size(px(14.0))
                                                .text_color(Theme::ACCENT_PRIMARY())
                                                .child("✓"),
                                        )
                                    })
                            })),
                    )
                    .with_priority(2),
                )
            })
    }

    /// 左侧对局列表与翻页控制（固定 180px 宽度，S06, S07, S08）
    #[allow(clippy::too_many_arguments)]
    fn render_left_match_list(
        &self,
        paged_matches: Vec<&MatchItem>,
        total_count: usize,
        has_more: bool,
        selected_game_id: Option<u64>,
        on_select_match: impl Fn(u64, &mut Window, &mut App) + 'static + Clone,
        on_prev_page: impl Fn(&mut Window, &mut App) + 'static + Clone,
        on_next_page: impl Fn(&mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let is_loading_page = self.is_page_switching || self.is_background_refreshing;

        div()
            .w(px(180.0))
            .flex_none()
            .flex()
            .flex_col()
            .h_full()
            .overflow_hidden()
            // 列表主容器（内部独立滚动）
            .child(
                div()
                    .id("mini-matches-scroll-container")
                    .flex_1()
                    .overflow_y_scroll()
                    .pr_1()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    // 情况 A: 已有战绩卡片正常展现 (S07: 10 张等高划分填满左栏，不足 10 张用透明 slot 保持同等高度)
                    .when(!paged_matches.is_empty(), |this| {
                        let select_cb = on_select_match.clone();
                        let count = paged_matches.len();
                        let remaining = 10usize.saturating_sub(count);
                        this.children(paged_matches.clone().into_iter().map(|item| {
                            let gid = item.display.game_id;
                            let is_selected = Some(gid) == selected_game_id;
                            let win = item.display.win;
                            let cb = select_cb.clone();

                            div()
                                .id(ElementId::NamedInteger("mini-card".into(), gid))
                                .flex_1()
                                .min_h(px(58.0))
                                .w_full()
                                .rounded(px(8.0))
                                .px_2p5()
                                .py_2()
                                .cursor_pointer()
                                // 选中态带 shadow_sm()，胜负底色需预合成到页面底色，避免阴影透进卡片
                                .bg(if win {
                                    Theme::over_page(Theme::WIN_BG())
                                } else {
                                    Theme::over_page(Theme::LOSS_BG())
                                })
                                .border_1()
                                .border_color(if is_selected {
                                    if win {
                                        Theme::WIN_TEXT()
                                    } else {
                                        Theme::LOSS_BORDER()
                                    }
                                } else if win {
                                    Theme::WIN_BORDER()
                                } else {
                                    Theme::LOSS_BORDER()
                                })
                                .when(is_selected, |s| s.border_2().shadow_sm())
                                .hover(|s| {
                                    s.bg(if win {
                                        Theme::over_page(Theme::WIN_BG_HOVER())
                                    } else {
                                        Theme::over_page(Theme::LOSS_BG_HOVER())
                                    })
                                })
                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                    cb(gid, win, app);
                                })
                                .flex()
                                .items_center()
                                .gap(px(10.0))
                                // 头像 36x36 (S07)
                                .child(
                                    div()
                                        .size(px(36.0))
                                        .flex_none()
                                        .rounded(px(18.0))
                                        .overflow_hidden()
                                        .border_1()
                                        .border_color(Theme::CARD_BORDER())
                                        .child(
                                            LcuImage::new(item.display.champion_icon_url.clone())
                                                .size(px(36.0))
                                                .shape(LcuImageShape::Circle),
                                        ),
                                )
                                // 右侧上下两行信息 (S07: 第一行模式名，第二行日期与KDA)
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.0))
                                        .flex()
                                        .flex_col()
                                        .justify_between()
                                        .h_full()
                                        .child(
                                            div()
                                                .text_size(px(12.5))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(Theme::TEXT_PRIMARY())
                                                .overflow_hidden()
                                                .whitespace_nowrap()
                                                .child(queue_id_to_name(
                                                    item.display.queue_id,
                                                    &item.display.map,
                                                )),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(11.5))
                                                .text_color(Theme::TEXT_MUTED())
                                                .whitespace_nowrap()
                                                .flex()
                                                .items_center()
                                                .gap(px(8.0))
                                                .child(format_local_month_day(
                                                    item.display.time_stamp as i64,
                                                ))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .child(format!("{}/", item.display.kills))
                                                        .child(
                                                            div()
                                                                .text_color(Theme::DEATH_RED())
                                                                .font_weight(FontWeight::BOLD)
                                                                .child(format!(
                                                                    "{}",
                                                                    item.display.deaths
                                                                )),
                                                        )
                                                        .child(format!(
                                                            "/{}",
                                                            item.display.assists
                                                        )),
                                                ),
                                        ),
                                )
                        }))
                        .children((0..remaining).map(|i| {
                            div()
                                .id(ElementId::NamedInteger("placeholder-card".into(), i as u64))
                                .flex_1()
                                .min_h(px(58.0))
                                .w_full()
                        }))
                    })
                    // 情况 B: 已有召唤师但筛选后当前队列无局 (明确空态)
                    .when(
                        self.current_summoner_name.is_some() && paged_matches.is_empty(),
                        |this| {
                            this.child(
                                div()
                                    .py_8()
                                    .px_3()
                                    .rounded(px(8.0))
                                    .bg(Theme::CARD_BG())
                                    .border_1()
                                    .border_color(Theme::CARD_BORDER())
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .justify_center()
                                    .gap_2()
                                    .child(div().text_xl().child("🔍"))
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(Theme::TEXT_PRIMARY())
                                            .child("当前筛选条件下没有对局"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(Theme::TEXT_MUTED())
                                            .child("可切回「全部」模式重试"),
                                    ),
                            )
                        },
                    )
                    // 情况 C: 初始未查询骨架卡片框 (S06: 10 张等高骨架卡框填满容器)
                    // 仅在没有可展示对局时占位，避免已有数据时骨架条与真实卡片并存
                    .when(
                        self.current_summoner_name.is_none() && paged_matches.is_empty(),
                        |this| {
                            this.children((0..10).map(|i| {
                                div()
                                    .id(ElementId::NamedInteger("skeleton-card".into(), i as u64))
                                    .flex_1()
                                    .min_h(px(58.0))
                                    .w_full()
                                    .rounded(px(8.0))
                                    .bg(Theme::SKELETON_BG())
                                    .border_1()
                                    .border_color(Theme::SKELETON_BORDER())
                            }))
                        },
                    ),
            )
            // 分页控制栏 (S06, S08: 独立于滚动列表之外)
            .child(
                div()
                    .mt(px(8.0))
                    .h(px(38.0))
                    .w_full()
                    .rounded(px(6.0))
                    .bg(Theme::CARD_BG())
                    .border_1()
                    .border_color(Theme::CARD_BORDER())
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    // 分页条只依赖实际结果数：自动带出的战绩没有搜索词，同样需要翻页
                    .when(total_count > 0, |this| {
                        let can_prev = self.current_page > 1 && !is_loading_page;
                        let can_next = has_more && !is_loading_page;
                        let prev_cb = on_prev_page.clone();
                        let next_cb = on_next_page.clone();

                        this.child(
                            div()
                                .id("btn-prev-page")
                                .size(px(26.0))
                                .rounded(px(4.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .when(can_prev, |s| {
                                    s.hover(|h| h.bg(Theme::HOVER_BG())).on_mouse_down(
                                        MouseButton::Left,
                                        move |_ev, win, app| {
                                            prev_cb(win, app);
                                        },
                                    )
                                })
                                .text_color(if can_prev {
                                    Theme::TEXT_PRIMARY()
                                } else {
                                    Theme::TEXT_MUTED()
                                })
                                .child("◀"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(Theme::TEXT_PRIMARY())
                                .child(if is_loading_page {
                                    "⏳".to_string()
                                } else {
                                    format!("{}", self.current_page)
                                }),
                        )
                        .child(
                            div()
                                .id("btn-next-page")
                                .size(px(26.0))
                                .rounded(px(4.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .when(can_next, |s| {
                                    s.hover(|h| h.bg(Theme::HOVER_BG())).on_mouse_down(
                                        MouseButton::Left,
                                        move |_ev, win, app| {
                                            next_cb(win, app);
                                        },
                                    )
                                })
                                .text_color(if can_next {
                                    Theme::TEXT_PRIMARY()
                                } else {
                                    Theme::TEXT_MUTED()
                                })
                                .child("▶"),
                        )
                    })
                    // 无结果时的占位
                    .when(total_count == 0, |this| {
                        this.child(div().text_xs().text_color(Theme::TEXT_MUTED()).child("—"))
                    }),
            )
    }

    /// 右侧对局详情面板 (S09, S10, S11, S12)
    fn render_detail_panel(
        &self,
        detail: Option<&GameDetail>,
        selected_game_id: Option<u64>,
        on_copy_game_id: impl Fn(u64, &mut Window, &mut App) + 'static + Clone,
        on_player_click: impl Fn(String, u64, String, &mut Window, &mut App) + 'static + Clone,
        on_replay: impl Fn(u64, &mut Window, &mut App) + 'static + Clone,
        on_retry_detail: impl Fn(u64, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let is_loading = self.is_loading_detail;

        // 外层保留卡片容器：详情内容通常短于视口高度，卡片底能把剩余高度填满，
        // 否则 Bannner 与队伍块下方会直接露出页面底色，看起来像「底部缺一块」
        Card::new().flex_1().overflow_hidden().child(match detail {
            Some(d) => div()
                .id("match-detail-panel-content")
                .flex()
                .flex_col()
                .size_full()
                .min_h(px(640.0))
                .p(px(12.0))
                .gap_3()
                .overflow_y_scroll()
                // 1. 头部大 Banner (S09)
                .child(self.render_detail_banner(d, on_copy_game_id, on_replay))
                // 2. 队伍详细数据 (S10, S11: 蓝方 -> 红方)
                .child(self.render_team_table(&d.blue, d.queue_id, on_player_click.clone()))
                .child(self.render_team_table(&d.red, d.queue_id, on_player_click))
                .into_any_element(),
            None => {
                let retry_cb = on_retry_detail;
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .size_full()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(if self.detail_error.is_some() {
                                Theme::LOSS_TEXT()
                            } else {
                                Theme::TEXT_PRIMARY()
                            })
                            .child(if is_loading {
                                "正在拉取对局详细数据...".to_string()
                            } else if let Some(ref err) = self.detail_error {
                                format!("对局详情载入失败: {}", err)
                            } else if let Some(gid) = selected_game_id {
                                format!("已选对局 ID: {}，等待详情载入", gid)
                            } else {
                                "当前队列暂无对局记录".to_string()
                            }),
                    )
                    .when(self.detail_error.is_some() && selected_game_id.is_some(), |this| {
                        let gid = selected_game_id.unwrap();
                        this.child(
                            div()
                                .px_3()
                                .py_1p5()
                                .rounded(px(6.0))
                                .bg(Theme::CARD_BG())
                                .border_1()
                                .border_color(Theme::CARD_BORDER())
                                .hover(|s| s.bg(Theme::CARD_BG_HOVER()).border_color(Theme::ACCENT_PRIMARY()))
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(Theme::TEXT_PRIMARY())
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                    retry_cb(gid, win, app);
                                })
                                .child("重试载入详情"),
                        )
                    })
                    .into_any_element()
            }
        })
    }

    /// 详情横幅 (S09: 54x54 地图图标 + 胜负大字 + 元信息 + 回放与复制按钮)
    fn render_detail_banner(
        &self,
        d: &GameDetail,
        on_copy_game_id: impl Fn(u64, &mut Window, &mut App) + 'static + Clone,
        on_replay: impl Fn(u64, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let win = d.win;
        let gid = d.game_id;
        let map_icon = get_map_icon_url(d.map_id, d.queue_id, win);
        let copy_cb = on_copy_game_id;
        let copied = self.copied_feedback == Some(gid);

        div()
            .w_full()
            .rounded(px(8.0))
            .p_3()
            .bg(if win { Theme::WIN_BG() } else { Theme::LOSS_BG() })
            .border_1()
            .border_color(if win {
                Theme::WIN_BORDER()
            } else {
                Theme::LOSS_BORDER()
            })
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    // 地图图标 54x54 (S09)
                    .child(
                        div()
                            .size(px(54.0))
                            .rounded(px(6.0))
                            .overflow_hidden()
                            .border_1()
                            .border_color(Theme::CARD_BORDER())
                            .child(
                                LcuImage::new(map_icon.to_string())
                                    .size(px(54.0))
                                    .shape(LcuImageShape::Rounded),
                            ),
                    )
                    // 胜负标题与同行元信息 (S09)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(20.0))
                                    .font_weight(FontWeight::EXTRA_BOLD)
                                    .text_color(if win {
                                        Theme::WIN_TEXT()
                                    } else {
                                        Theme::LOSS_TEXT()
                                    })
                                    .child(if win { "胜利" } else { "失败" }),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(Theme::TEXT_MUTED())
                                    .child(format!(
                                        "{} · {} · {} · {} · Game ID: {}",
                                        d.map_name, d.queue_name, d.duration, d.date, d.game_id
                                    )),
                            ),
                    ),
            )
            // 右侧操作区：回放按钮与复制 Game ID 按钮 (S09)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    // 回放按钮（对齐 Vue ReplayButton 的带文字样式与状态机）
                    .child({
                        let replay_cb = on_replay.clone();
                        let replay_state = crate::replay_store::get_replay_state(gid);
                        div()
                            .id("btn-replay")
                            .h(px(30.0))
                            .px_2()
                            .rounded(px(6.0))
                            .bg(Theme::CARD_BG())
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
                            .hover(|s| {
                                s.border_color(Theme::ACCENT_PRIMARY())
                                    .bg(Theme::CARD_BG_HOVER())
                            })
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                app.stop_propagation();
                                replay_cb(gid, win, app);
                            })
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(icon(
                                match replay_state {
                                    crate::match_model::ReplayState::Watch => "icons/play.svg",
                                    crate::match_model::ReplayState::Error => "icons/refresh.svg",
                                    _ => "icons/play.svg",
                                },
                                12.0,
                                match replay_state {
                                    crate::match_model::ReplayState::Watch => Theme::WIN_TEXT(),
                                    crate::match_model::ReplayState::Error => Theme::LOSS_TEXT(),
                                    crate::match_model::ReplayState::Downloading
                                    | crate::match_model::ReplayState::Checking => {
                                        Theme::ACCENT_PRIMARY()
                                    }
                                    _ => Theme::TEXT_MUTED(),
                                },
                            ))
                            .child(
                                div()
                                    .text_size(px(12.2))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(match replay_state {
                                        crate::match_model::ReplayState::Watch => Theme::WIN_TEXT(),
                                        crate::match_model::ReplayState::Error => Theme::LOSS_TEXT(),
                                        crate::match_model::ReplayState::Downloading
                                        | crate::match_model::ReplayState::Checking => {
                                            Theme::ACCENT_PRIMARY()
                                        }
                                        _ => Theme::TEXT_MUTED(),
                                    })
                                    .child(replay_state.label()),
                            )
                    })
                    // 复制 Game ID 按钮 (30x30)
                    .child(
                        div()
                            .id("btn-copy-game-id")
                            .size(px(30.0))
                            .rounded(px(6.0))
                            .bg(Theme::CARD_BG())
                            .border_1()
                            .border_color(Theme::CARD_BORDER())
                            .hover(|s| {
                                s.bg(Theme::CARD_BG_HOVER())
                                    .border_color(Theme::ACCENT_PRIMARY())
                            })
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                copy_cb(gid, win, app);
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(copied, |this| {
                                this.child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(Theme::WIN_TEXT())
                                        .child("✓"),
                                )
                            })
                            .when(!copied, |this| {
                                this.child(icon("icons/copy.svg", 14.0, Theme::TEXT_PRIMARY()))
                            }),
                    ),
            )
    }

    /// 队伍卡片与玩家数据表 (S10, S11)
    fn render_team_table(
        &self,
        team: &GameDetailTeam,
        queue_id: i32,
        on_player_click: impl Fn(String, u64, String, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let win = team.win;
        let is_blue = team.team_id == 100;
        let is_aram_hex = queue_id == 2400 || queue_id == 2450;

        div()
            .w_full()
            .rounded(px(8.0))
            .border_1()
            .border_color(if win {
                Theme::WIN_BORDER()
            } else {
                Theme::LOSS_BORDER()
            })
            .bg(Theme::CARD_BG())
            .overflow_hidden()
            .flex()
            .flex_col()
            // 1. 队头资源概览条 (S10)
            .child(
                div()
                    .w_full()
                    .px(px(16.0))
                    .py(px(8.0))
                    .bg(if win { Theme::WIN_BG() } else { Theme::LOSS_BG() })
                    .border_b_1()
                    .border_color(if win {
                        Theme::WIN_BORDER()
                    } else {
                        Theme::LOSS_BORDER()
                    })
                    .flex()
                    .items_center()
                    .justify_between()
                    // 左侧胜负标签与 6 个团队目标图标 (S10)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            // 胜负标签
                            .child(
                                div()
                                    .text_size(px(13.6))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(if win {
                                        Theme::WIN_TEXT()
                                    } else {
                                        Theme::LOSS_TEXT()
                                    })
                                    .child(if win { "胜方" } else { "败方" }),
                            )
                            // 6 项团队目标小图标与数值统计
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(10.0))
                                    .text_size(px(12.8))
                                    .text_color(Theme::TEXT_MUTED())
                                    .child(self.render_obj_icon("/images/kills.png", team.kills))
                                    .child(self.render_obj_icon(
                                        if is_blue {
                                            "/images/tower-100.png"
                                        } else {
                                            "/images/tower-200.png"
                                        },
                                        team.tower_kills,
                                    ))
                                    .child(self.render_obj_icon(
                                        if is_blue {
                                            "/images/inhibitor-100.png"
                                        } else {
                                            "/images/inhibitor-200.png"
                                        },
                                        team.inhibitor_kills,
                                    ))
                                    .child(self.render_obj_icon(
                                        if is_blue {
                                            "/images/baron-100.png"
                                        } else {
                                            "/images/baron-200.png"
                                        },
                                        team.baron_kills,
                                    ))
                                    .child(self.render_obj_icon(
                                        if is_blue {
                                            "/images/dragon-100.png"
                                        } else {
                                            "/images/dragon-200.png"
                                        },
                                        team.dragon_kills,
                                    ))
                                    .child(self.render_obj_icon(
                                        if is_blue {
                                            "/images/herald-100.png"
                                        } else {
                                            "/images/herald-200.png"
                                        },
                                        team.rift_herald_kills,
                                    )),
                            ),
                    )
                    // 右侧列头标签：装备、KDA、补刀、金币、伤害 (对齐 Vue 原版)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .text_size(px(11.5))
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::TEXT_MUTED())
                            .child(div().w(px(210.0)).text_center().child("装备"))
                            .child(div().w(px(70.0)).text_center().child("KDA"))
                            .child(div().w(px(42.0)).text_center().child("补刀"))
                            .child(div().w(px(55.0)).text_right().child("金币"))
                            .child(div().w(px(60.0)).text_right().child("伤害")),
                    ),
            )
            // 2. 玩家列表 (S11)
            .child(
                div().flex().flex_col().children(
                    team.players.iter().map(|p| {
                        self.render_player_row(p, win, is_aram_hex, on_player_click.clone())
                    }),
                ),
            )
    }

    /// 渲染团队目标图标项
    fn render_obj_icon(&self, icon_url: &'static str, count: i32) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(
                LcuImage::new(icon_url.to_string())
                    .size(px(16.0))
                    .shape(LcuImageShape::ItemSlot),
            )
            .child(format!("{}", count))
    }

    /// 渲染玩家行 (S11)
    fn render_player_row(
        &self,
        p: &GameDetailPlayer,
        win: bool,
        is_aram_hex: bool,
        on_player_click: impl Fn(String, u64, String, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let is_self = self
            .current_summoner_puuid
            .as_deref()
            .map(|id| id == p.puuid)
            .unwrap_or(false);

        let click_cb = on_player_click;
        let puuid_val = p.puuid.clone();
        let sid_val = p.summoner_id;
        let name_val = p.name.clone();
        let pure_name = p.name.split('#').next().unwrap_or(&p.name).to_string();

        // 段位徽章（按需拉取后展示；未拉到或未定级时不渲染）
        let rank_info = self.participant_ranks.get(&p.puuid).cloned();
        let rank_icon_path = rank_info
            .as_ref()
            .and_then(|info| crate::asset_loader::AssetLoader::get_ranked_icon_path(&info.tier));

        let self_highlight_color = if win {
            Theme::WIN_TEXT()
        } else {
            Theme::LOSS_TEXT()
        };

        div()
            .w_full()
            .h(px(52.0))
            .px(px(14.0))
            .border_b_1()
            .border_color(Theme::CARD_BORDER())
            .bg(if is_self {
                if win {
                    Theme::WIN_BG()
                } else {
                    Theme::LOSS_BG()
                }
            } else {
                Theme::CARD_BG()
            })
            .hover(|s| s.bg(Theme::CARD_BG_HOVER()))
            .flex()
            .items_center()
            .justify_between()
            // 左半区：头像(40x40)+技能(18x18)+符文(20x20) + 昵称
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    // 头像与角标等级 (width 96px, 对齐 Vue 原版)
                    .child(
                        div()
                            .w(px(96.0))
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .relative()
                                    .size(px(40.0))
                                    .flex_none()
                                    .child(
                                        LcuImage::new(p.champion_icon_url.clone())
                                            .size(px(40.0))
                                            .shape(LcuImageShape::Circle),
                                    )
                                    // 贴底等级角标 14x14
                                    .child(
                                        div()
                                            .absolute()
                                            .bottom(px(-2.0))
                                            .right(px(-2.0))
                                            .size(px(14.0))
                                            .rounded_full()
                                            .bg(Theme::TEXT_PRIMARY())
                                            .border_1()
                                            .border_color(Theme::CARD_BG())
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_size(px(9.0))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(Theme::CARD_BG())
                                            .child(format!("{}", p.level)),
                                    ),
                            )
                            // 技能与符文 (S11)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(3.0))
                                    // 两个 18x18 技能纵向排列
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.0))
                                            .child(
                                                div()
                                                    .size(px(18.0))
                                                    .rounded(px(2.0))
                                                    .border_1()
                                                    .border_color(Theme::CARD_BORDER())
                                                    .overflow_hidden()
                                                    .child(
                                                        LcuImage::new(p.spell1_icon_url.clone())
                                                            .size(px(18.0)),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .size(px(18.0))
                                                    .rounded(px(2.0))
                                                    .border_1()
                                                    .border_color(Theme::CARD_BORDER())
                                                    .overflow_hidden()
                                                    .child(
                                                        LcuImage::new(p.spell2_icon_url.clone())
                                                            .size(px(18.0)),
                                                    ),
                                            ),
                                    )
                                    // 符文图标 (20x20，非海克斯队列展示)
                                    .when(!is_aram_hex && !p.rune_icon_url.is_empty(), |this| {
                                        this.child(
                                            div()
                                                .size(px(20.0))
                                                .rounded_full()
                                                .overflow_hidden()
                                                .child(
                                                    LcuImage::new(p.rune_icon_url.clone())
                                                        .size(px(20.0))
                                                        .shape(LcuImageShape::Circle),
                                                ),
                                        )
                                    }),
                            ),
                    )
                    // 玩家昵称 (max-w 170px, 纯名字, 可点击跳转生涯)
                    .child(
                        div()
                            .max_w(px(170.0))
                            .min_w(px(0.0))
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                        click_cb(
                                            puuid_val.clone(),
                                            sid_val,
                                            name_val.clone(),
                                            win,
                                            app,
                                        );
                                    })
                                    .text_size(px(12.8))
                                    .font_weight(if is_self {
                                        FontWeight::BOLD
                                    } else {
                                        FontWeight::SEMIBOLD
                                    })
                                    .text_color(if is_self {
                                        self_highlight_color
                                    } else {
                                        Theme::TEXT_PRIMARY()
                                    })
                                    .hover(|s| s.text_color(Theme::ACCENT_PRIMARY()))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .child(pure_name),
                            )
                            // 段位徽章：图标(13px) + 中文标签 (对齐 Vue row-rank-badge)
                            .when_some(rank_info, |this, info| {
                                this.child(
                                    div()
                                        .flex_none()
                                        .flex()
                                        .items_center()
                                        .gap(px(2.0))
                                        .when(rank_icon_path.is_some(), |this| {
                                            this.child(
                                                LcuImage::new(
                                                    rank_icon_path.unwrap().to_string(),
                                                )
                                                .size(px(13.0)),
                                            )
                                        })
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .text_color(Theme::TEXT_MUTED())
                                                .child(info.rank_text),
                                        ),
                                )
                            }),
                    ),
            )
            .child(div().flex_1())
            // 右半区：装备(6格+1饰品)+KDA+补刀+金币+伤害 (严格对齐队头列宽)
            .child(
                div()
                    .flex()
                    .items_center()
                    // 装备槽 (6格 + 1饰品槽，每个 28x28，S11)
                    .child(
                        div()
                            .w(px(210.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(px(3.0))
                            // 6 格物品
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(1.0))
                                    .children((0..6).map(|idx| {
                                        let url =
                                            p.item_icon_urls.get(idx).cloned().unwrap_or_default();
                                        div()
                                            .size(px(28.0))
                                            .rounded(px(3.0))
                                            .bg(Theme::HOVER_BG())
                                            .border_1()
                                            .border_color(Theme::CARD_BORDER())
                                            .overflow_hidden()
                                            .when(!url.is_empty(), |this| {
                                                this.child(
                                                    LcuImage::new(url)
                                                        .size(px(28.0))
                                                        .shape(LcuImageShape::ItemSlot),
                                                )
                                            })
                                    })),
                            )
                            // 1 格饰品 (28x28, 黄色边框)
                            .child(
                                div()
                                    .size(px(28.0))
                                    .rounded(px(3.0))
                                    .bg(rgba(0xf59e0b0d))
                                    .border_1()
                                    .border_color(rgba(0xf59e0b4c))
                                    .overflow_hidden()
                                    .when(!p.ward_icon_url.is_empty(), |this| {
                                        this.child(
                                            LcuImage::new(p.ward_icon_url.clone())
                                                .size(px(28.0))
                                                .shape(LcuImageShape::ItemSlot),
                                        )
                                    }),
                            ),
                    )
                    // KDA (70px, 居中)
                    .child(
                        div()
                            .w(px(70.0))
                            .text_center()
                            .text_size(px(12.8))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .font_weight(if is_self {
                                        FontWeight::BOLD
                                    } else {
                                        FontWeight::NORMAL
                                    })
                                    .text_color(if is_self {
                                        self_highlight_color
                                    } else {
                                        Theme::TEXT_PRIMARY()
                                    })
                                    .child(format!("{}/", p.kills)),
                            )
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::DEATH_RED())
                                    .child(format!("{}", p.deaths)),
                            )
                            .child(
                                div()
                                    .font_weight(if is_self {
                                        FontWeight::BOLD
                                    } else {
                                        FontWeight::NORMAL
                                    })
                                    .text_color(if is_self {
                                        self_highlight_color
                                    } else {
                                        Theme::TEXT_PRIMARY()
                                    })
                                    .child(format!("/{}", p.assists)),
                            ),
                    )
                    // 补刀 (42px, 居中)
                    .child(
                        div()
                            .w(px(42.0))
                            .text_center()
                            .text_size(px(12.8))
                            .font_weight(if is_self {
                                FontWeight::BOLD
                            } else {
                                FontWeight::NORMAL
                            })
                            .text_color(if is_self {
                                self_highlight_color
                            } else {
                                Theme::TEXT_MUTED()
                            })
                            .child(format!("{}", p.cs)),
                    )
                    // 金币 (55px, 右对齐, 纯千分位格式化数字)
                    .child(
                        div()
                            .w(px(55.0))
                            .text_right()
                            .text_size(px(12.8))
                            .font_weight(if is_self {
                                FontWeight::BOLD
                            } else {
                                FontWeight::NORMAL
                            })
                            .text_color(if is_self {
                                self_highlight_color
                            } else {
                                Theme::TEXT_MUTED()
                            })
                            .child(format_num(p.gold as i64)),
                    )
                    // 伤害 (60px, 右对齐, 纯千分位加粗数字)
                    .child(
                        div()
                            .w(px(60.0))
                            .text_right()
                            .text_size(px(12.8))
                            .font_weight(FontWeight::BOLD)
                            .text_color(if is_self {
                                self_highlight_color
                            } else {
                                Theme::TEXT_PRIMARY()
                            })
                            .child(format_num(p.damage as i64)),
                    ),
            )
    }
}
