//! 路人集 (SavedPlayers) 页面视图
//!
//! 1:1 对齐 Vue 页面 (SavedPlayers.vue)
//! 包含：
//! - P01: 1000px 限宽顶栏，标题 + 数量微标 + 说明 + 导入标记/导出标记系统文件对话框
//! - P02: 搜索输入框（打字支持、清空、回车）+ 三段筛选分段器（全部/标记玩家/多次相遇）
//! - P03: 玩家卡片（头像、英雄小图标、昵称#TagLine、tag徽标、队列徽标、相遇次数、相遇时间、折叠箭头）+ 行内编辑/删除
//! - P04: 相遇对局展开层（加载态、空态、队列+时间记录列表、加载更多）
//! - P05: 初始空态、搜索无结果空态（带清空按钮）、底部触底加载与全部加载完毕提示

use crate::asset_loader::global_asset_loader;
use crate::components::text_field::{text_field, TextFieldOptions};
use crate::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use yuumi_core::parsers::saved_player::{
    EncounteredGameRecord, SavedPlayer, TaggedPlayerExportDto,
};
use yuumi_core::storage::SavedPlayersStorage;

/// 常用筛选标签常量
pub const FILTER_OPTIONS: &[&str] = &["全部", "标记玩家", "多次相遇"];

/// 相遇历史分页大小
pub const ENCOUNTER_GAMES_PAGE_SIZE: usize = 20;

/// 列表分页大小
pub const SAVED_PLAYERS_PAGE_SIZE: usize = 30;

/// 标签颜色映射
pub(crate) fn tag_badge_colors(tag: &str) -> (Hsla, Hsla, Hsla) {
    match tag {
        "大腿" => (Theme::WIN_BG(), Theme::WIN_TEXT(), Theme::WIN_BORDER()), // 翡翠绿
        "避坑" | "演员/挂机" => (Theme::LOSS_BG(), Theme::LOSS_TEXT(), Theme::LOSS_BORDER()), // 珊瑚红 / 绯红
        "绝活" => (
            Theme::ACCENT_LIGHT(),
            Theme::ACCENT_PRIMARY(),
            Theme::AURORA_CYAN().opacity(0.6),
        ), // 极光蓝青
        "置顶" => (
            Theme::PREMADE_COLOR_2().opacity(0.15),
            Theme::GOLD(),
            Theme::GOLD().opacity(0.4),
        ), // 琥珀金
        _ => (
            Theme::AURORA_CYAN().opacity(0.15),
            Theme::ACCENT_PRIMARY(),
            Theme::AURORA_CYAN().opacity(0.35),
        ),
    }
}

/// 队列名称显示映射
fn queue_type_display_name(queue_type: &str) -> &'static str {
    match queue_type {
        "0" => "自定义模式",
        "400" => "征召模式",
        "420" => "单双排位",
        "430" => "匹配模式",
        "440" => "灵活排位",
        "450" => "极地大乱斗",
        "480" | "490" => "快速模式",
        "900" => "无限火力",
        "1010" => "随机无限火力",
        "1020" => "克隆模式",
        "1300" => "极限闪击",
        "1700" | "1710" => "斗魂竞技场",
        "2400" => "海克斯大乱斗",
        "2450" => "经典海斗",
        _ => "其他模式",
    }
}

/// 格式化相遇时间为 YYYY-MM-DD HH:mm
fn format_time(ts: i64) -> String {
    if ts <= 0 {
        return "暂无最后相遇时间".to_string();
    }
    if let Some(dt) = chrono::DateTime::from_timestamp_millis(ts) {
        let local: chrono::DateTime<chrono::Local> = chrono::DateTime::from(dt);
        local.format("%Y-%m-%d %H:%M").to_string()
    } else {
        "暂无最后相遇时间".to_string()
    }
}

/// 路人集界面交互动作分发枚举
#[derive(Debug, Clone)]
pub enum SavedPlayersAction {
    FilterTag(String),
    SearchInput(String),
    SearchSubmit,
    SearchClear,
    ToggleExpand(String),
    LoadMoreGames,
    StartEdit(String),
    EditTagInput(String),
    SaveEdit(String),
    CancelEdit,
    AskDelete(String),
    CancelDelete,
    ConfirmDelete(String),
    ExportTagged,
    ImportTagged,
    LoadMore,

    // 兼容旧接口
    TogglePin(String),
    RemovePlayer(String),
    SetEditTag(String),
    EditNotesInput(String),
}

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
    /// 当前选中的标签过滤，例如 "全部"、"标记玩家"、"多次相遇"
    pub active_tag_filter: String,
    /// 搜索过滤词（按召唤师名、标签、备注检索）
    pub search_query: String,
    /// 当前展开查看相遇历史的玩家 PUUID
    pub expanded_puuid: Option<String>,
    /// 当前展开玩家的相遇历史列表
    pub expanded_games: Vec<EncounteredGameRecord>,
    /// 当前展开玩家的相遇记录总数
    pub expanded_games_total: u32,
    /// 当前展开相遇历史加载到的页码
    pub expanded_games_page: usize,
    /// 相遇记录加载中标志
    pub is_games_loading: bool,
    /// 正在编辑标签的玩家 PUUID
    pub editing_puuid: Option<String>,
    /// 编辑标签输入临时缓冲
    pub editing_tag_buffer: String,
    pub editing_notes_buffer: String,
    /// 待确认删除的玩家 PUUID（用于弹出删除确认对话框）
    pub confirm_delete_puuid: Option<String>,
    /// 正在拉取下一页标志位
    pub is_loading_more: bool,
    /// 异步请求序列计数器（用于丢弃快速切换/输入时的过期响应）
    pub request_seq: u64,
    /// 独立列表滚动控制与位置跟踪句柄
    pub scroll_handle: ScrollHandle,
    /// 顶部搜索框焦点句柄
    pub search_focus_handle: FocusHandle,
    /// 内联编辑标签输入框焦点句柄
    pub edit_tag_focus_handle: FocusHandle,
    /// 内联编辑备注输入框焦点句柄（兼容）
    pub edit_notes_focus_handle: FocusHandle,
}

impl SavedPlayersView {
    /// [P2-1] 构造不再做任何同步 SQLite I/O：storage 打开与首查全部移至
    /// `MainWindow::init_saved_players_storage`（后台 Tokio 阻塞池），完成后回写并刷新。
    /// 首次运行时此处的建库 + WAL + 迁移检查曾直接卡住首帧渲染（P0-6 同源）。
    pub fn new(cx: &mut App) -> Self {
        Self {
            storage: None,
            self_puuid: None,
            players: Vec::new(),
            total_count: 0,
            current_page: 1,
            active_tag_filter: "全部".to_string(),
            search_query: String::new(),
            expanded_puuid: None,
            expanded_games: Vec::new(),
            expanded_games_total: 0,
            expanded_games_page: 1,
            is_games_loading: false,
            editing_puuid: None,
            editing_tag_buffer: String::new(),
            editing_notes_buffer: String::new(),
            confirm_delete_puuid: None,
            is_loading_more: false,
            request_seq: 0,
            scroll_handle: ScrollHandle::new(),
            search_focus_handle: cx.focus_handle(),
            edit_tag_focus_handle: cx.focus_handle(),
            edit_notes_focus_handle: cx.focus_handle(),
        }
    }

    /// 获取并递增请求序列号
    pub fn next_request_seq(&mut self) -> u64 {
        self.request_seq = self.request_seq.wrapping_add(1);
        self.request_seq
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

    /// 加载下一页数据并追加至当前列表
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

    /// 当前展示的玩家列表引用
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
            self.expanded_games.clear();
            self.expanded_games_total = 0;
            self.expanded_games_page = 1;
        } else {
            self.expanded_puuid = Some(puuid.to_string());
            self.expanded_games_page = 1;
            let target_self_puuid = self
                .players
                .iter()
                .find(|p| p.puuid == puuid)
                .map(|p| p.self_puuid.as_str())
                .filter(|s| !s.is_empty())
                .or(self.self_puuid.as_deref());

            if let Some(ref s) = self.storage {
                self.expanded_games_total = s
                    .count_encountered_games(puuid, target_self_puuid)
                    .unwrap_or(0);
                self.expanded_games = s
                    .query_encountered_games_paged(
                        puuid,
                        target_self_puuid,
                        ENCOUNTER_GAMES_PAGE_SIZE,
                        0,
                    )
                    .unwrap_or_default();
            }
        }
    }

    /// 展开玩家加载更多对局（失败文案交由调用方以 Toast 呈现）
    pub fn load_more_games(&mut self) -> Result<(), String> {
        if let Some(ref puuid) = self.expanded_puuid {
            let next_page = self.expanded_games_page + 1;
            let offset = (next_page - 1) * ENCOUNTER_GAMES_PAGE_SIZE;
            let target_self_puuid = self
                .players
                .iter()
                .find(|p| p.puuid == *puuid)
                .map(|p| p.self_puuid.as_str())
                .filter(|s| !s.is_empty())
                .or(self.self_puuid.as_deref());

            if let Some(ref s) = self.storage {
                match s.query_encountered_games_paged(
                    puuid,
                    target_self_puuid,
                    ENCOUNTER_GAMES_PAGE_SIZE,
                    offset,
                ) {
                    Ok(more) => {
                        self.expanded_games.extend(more);
                        self.expanded_games_page = next_page;
                    }
                    Err(e) => {
                        return Err(format!("加载更多对局失败: {}", e));
                    }
                }
            }
        }
        Ok(())
    }

    /// 切换玩家置顶状态（带完整账号作用域，DB 成功后更新 UI 与重新排序；失败文案交由调用方以 Toast 呈现）
    pub fn toggle_pin(&mut self, puuid: &str) -> Result<(), String> {
        let Some(player) = self.players.iter().find(|p| p.puuid == puuid) else {
            return Ok(());
        };
        let target_self_puuid = if !player.self_puuid.is_empty() {
            player.self_puuid.clone()
        } else {
            self.self_puuid.clone().unwrap_or_default()
        };
        let new_pinned = !player.is_pinned;

        if let Some(ref s) = self.storage {
            match s.update_pin(puuid, &target_self_puuid, new_pinned) {
                Ok(_) => {
                    if let Some(p) = self.players.iter_mut().find(|p| p.puuid == puuid) {
                        p.is_pinned = new_pinned;
                    }
                    // 重新排序：置顶在前，其次按最近相遇倒序
                    self.players.sort_by(|a, b| {
                        b.is_pinned
                            .cmp(&a.is_pinned)
                            .then_with(|| b.last_met_timestamp.cmp(&a.last_met_timestamp))
                    });
                }
                Err(e) => {
                    return Err(format!("更新置顶失败: {}", e));
                }
            }
        }
        Ok(())
    }

    /// 删除玩家记录
    pub fn remove_player(&mut self, puuid: &str) -> Result<(), String> {
        let target_self_puuid = self
            .players
            .iter()
            .find(|p| p.puuid == puuid)
            .map(|p| p.self_puuid.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.self_puuid.clone().unwrap_or_default());

        if let Some(ref s) = self.storage {
            s.delete_player(puuid, &target_self_puuid)?;
        }
        self.players.retain(|p| p.puuid != puuid);
        if self.total_count > 0 {
            self.total_count -= 1;
        }
        if self.expanded_puuid.as_deref() == Some(puuid) {
            self.expanded_puuid = None;
            self.expanded_games.clear();
        }
        if self.editing_puuid.as_deref() == Some(puuid) {
            self.editing_puuid = None;
        }
        Ok(())
    }

    /// 开始编辑
    pub fn start_edit(&mut self, puuid: &str) {
        if let Some(player) = self.players.iter().find(|p| p.puuid == puuid) {
            self.editing_puuid = Some(puuid.to_string());
            self.editing_tag_buffer = player.tag.clone().unwrap_or_default();
            self.editing_notes_buffer = player.notes.clone();
        }
    }

    /// 快捷选择标签（兼容）
    pub fn set_editing_tag(&mut self, tag: &str) {
        if tag == "清除标签" {
            self.editing_tag_buffer.clear();
        } else {
            self.editing_tag_buffer = tag.to_string();
        }
    }

    /// 保存编辑内容（带完整账号作用域，保留原有备注，成功后再退出编辑模式与更新列表；结果文案交由调用方以 Toast 呈现）
    pub fn save_edit(&mut self, puuid: &str) -> Result<(), String> {
        let tag_opt = {
            let t = self.editing_tag_buffer.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        };
        let notes = self.editing_notes_buffer.clone();

        let target_self_puuid = self
            .players
            .iter()
            .find(|p| p.puuid == puuid)
            .map(|p| p.self_puuid.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.self_puuid.clone().unwrap_or_default());

        if let Some(ref s) = self.storage {
            match s.update_tag_and_notes(puuid, &target_self_puuid, tag_opt.as_deref(), &notes) {
                Ok(_) => {
                    if let Some(player) = self.players.iter_mut().find(|p| p.puuid == puuid) {
                        player.tag = tag_opt;
                        player.notes = notes;
                    }
                    self.editing_puuid = None;
                    Ok(())
                }
                Err(e) => Err(format!("保存标记失败: {}", e)),
            }
        } else {
            Err("数据库未连接".to_string())
        }
    }

    /// 取消编辑
    pub fn cancel_edit(&mut self) {
        self.editing_puuid = None;
        self.editing_tag_buffer.clear();
    }

    /// 弹出删除确认
    pub fn ask_delete(&mut self, puuid: &str) {
        self.confirm_delete_puuid = Some(puuid.to_string());
    }

    /// 取消删除
    pub fn cancel_delete(&mut self) {
        self.confirm_delete_puuid = None;
    }

    /// 确认删除（失败显示错误反馈，成功才关闭对话框并刷新；结果文案交由调用方以 Toast 呈现）
    pub fn confirm_delete(&mut self, puuid: &str) -> Result<(), String> {
        match self.remove_player(puuid) {
            Ok(_) => {
                self.confirm_delete_puuid = None;
                let self_p = self.self_puuid.clone();
                self.reload(self_p.as_deref());
                Ok(())
            }
            Err(e) => Err(format!("删除失败: {}", e)),
        }
    }

    /// 导出标记玩家到 JSON 文件。
    /// 返回 `None` 表示用户取消了文件对话框（无提示）；`Ok(路径)` 供调用方发成功 Toast。
    pub fn export_tagged_to_file(&mut self) -> Option<Result<String, String>> {
        let s = self.storage.as_ref()?;
        match s.export_tagged_players() {
            Ok(data) => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("导出标记玩家")
                    .add_filter("JSON", &["json"])
                    .set_file_name("tagged_players.json")
                    .save_file()
                {
                    match serde_json::to_string_pretty(&data) {
                        Ok(json) => match std::fs::write(&path, json) {
                            Ok(_) => Some(Ok(path.display().to_string())),
                            Err(e) => Some(Err(format!("导出保存失败: {}", e))),
                        },
                        Err(e) => Some(Err(format!("JSON 序列化失败: {}", e))),
                    }
                } else {
                    None
                }
            }
            Err(e) => Some(Err(format!("导出查询失败: {}", e))),
        }
    }

    /// 从 JSON 文件导入标记玩家。
    /// 返回 `None` 表示用户取消了文件对话框（无提示）；`Ok(数量)` 供调用方发成功 Toast。
    pub fn import_tagged_from_file(&mut self) -> Option<Result<u32, String>> {
        let s = self.storage.as_ref()?;
        if let Some(path) = rfd::FileDialog::new()
            .set_title("导入标记玩家")
            .add_filter("JSON", &["json"])
            .pick_file()
        {
            match std::fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str::<Vec<TaggedPlayerExportDto>>(&content) {
                    Ok(records) => match s.import_tagged_players(&records) {
                        Ok(count) => {
                            let self_p = self.self_puuid.clone();
                            self.reload(self_p.as_deref());
                            Some(Ok(count))
                        }
                        Err(e) => Some(Err(format!("导入写入数据库失败: {}", e))),
                    },
                    Err(e) => Some(Err(format!("JSON 解析失败，请确认文件格式: {}", e))),
                },
                Err(e) => Some(Err(format!("读取文件失败: {}", e))),
            }
        } else {
            None
        }
    }

    // ── 渲染入口 ──

    /// 统一完整交互式面板渲染
    pub fn render_panel<F>(&self, window: &mut Window, on_action: F) -> impl IntoElement
    where
        F: Fn(SavedPlayersAction, &mut Window, &mut App) + 'static + Clone,
    {
        let displayed = self.displayed_players();

        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .items_center()
            // 居中约束内容列：max-width: 1000px; margin: 0 auto
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .max_w(px(1000.0))
                    .h_full()
                    .px_6()
                    .py_4()
                    .overflow_hidden()
                    // P01: 页面第一行标题与操作栏
                    .child(self.render_header_bar(on_action.clone()))
                    // P02: 页面第二行工具条（搜索框 + 三段筛选器）
                    .child(self.render_toolbar_row(window, on_action.clone()))
                    // 列表与空态容器
                    .child(
                        div()
                            .flex_1()
                            .w_full()
                            .min_h(px(0.0))
                            .overflow_hidden()
                            .child(self.render_players_content(
                                displayed,
                                window,
                                on_action.clone(),
                            )),
                    ),
            )
            // 删除确认模态框
            .when_some(self.confirm_delete_puuid.clone(), |this, puuid| {
                this.child(self.render_delete_modal(puuid, on_action.clone()))
            })
    }

    /// 无外部回调的默认渲染
    pub fn render(&self, window: &mut Window) -> impl IntoElement {
        self.render_panel(window, |_act, _win, _app| {})
    }

    /// P01 顶栏结构：标题 + 人数微标 + 说明 + 导入标记/导出标记按钮
    fn render_header_bar<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(SavedPlayersAction, &mut Window, &mut App) + 'static + Clone,
    {
        let on_action_import = on_action.clone();
        let on_action_export = on_action.clone();

        div()
            .flex()
            .items_center()
            .justify_between()
            .mb_3()
            .gap_3()
            // 第一行左侧：标题「路人集」+ 数量微标 + 分隔点「·」+ 说明文案
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::EXTRA_BOLD)
                            .text_color(Theme::TEXT_PRIMARY())
                            .child("路人集"),
                    )
                    .when(self.total_count > 0, |this| {
                        this.child(
                            div()
                                .px_2()
                                .py_0p5()
                                .rounded_full()
                                .bg(Theme::AURORA_CYAN().opacity(0.15))
                                .border_1()
                                .border_color(Theme::AURORA_CYAN().opacity(0.35))
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(Theme::ACCENT_PRIMARY())
                                .child(self.total_count.to_string()),
                        )
                    })
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::TEXT_MUTED())
                            .child("·"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::TEXT_MUTED())
                            .child("记录并标记曾与你同局竞技的召唤师"),
                    ),
            )
            // 第一行右侧：导入标记 / 导出标记按钮组
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .px_3()
                            .py_1p5()
                            .rounded_md()
                            .border_1()
                            .border_color(Theme::CARD_BORDER())
                            .bg(Theme::CARD_BG())
                            .hover(|s| {
                                s.bg(Theme::CARD_BG_HOVER())
                                    .border_color(Theme::AURORA_CYAN())
                            })
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(Theme::TEXT_PRIMARY())
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                on_action_import(SavedPlayersAction::ImportTagged, win, app);
                            })
                            .child("📥")
                            .child("导入标记"),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_1p5()
                            .rounded_md()
                            .border_1()
                            .border_color(Theme::CARD_BORDER())
                            .bg(Theme::CARD_BG())
                            .hover(|s| {
                                s.bg(Theme::CARD_BG_HOVER())
                                    .border_color(Theme::AURORA_CYAN())
                            })
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(Theme::TEXT_PRIMARY())
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                on_action_export(SavedPlayersAction::ExportTagged, win, app);
                            })
                            .child("📤")
                            .child("导出标记"),
                    ),
            )
    }

    /// P02 第二行工具条：搜索输入框 + 三段分段器
    fn render_toolbar_row<F>(&self, window: &mut Window, on_action: F) -> impl IntoElement
    where
        F: Fn(SavedPlayersAction, &mut Window, &mut App) + 'static + Clone,
    {
        let is_search_focused = self.search_focus_handle.is_focused(window);
        let on_action_search = on_action.clone();
        let on_action_clear = on_action.clone();
        let on_action_submit = on_action.clone();

        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .mb_4()
            // 左侧搜索输入框：无放大镜，支持快捷键与清空按钮，回车即刻触发
            .child(text_field(
                "saved-players-search-box",
                &self.search_query,
                "搜索召唤师名称、Tag 或标记...",
                px(320.0),
                &self.search_focus_handle,
                is_search_focused,
                TextFieldOptions::default(),
                move |s, win, app| on_action_search(SavedPlayersAction::SearchInput(s), win, app),
                move |win, app| on_action_clear(SavedPlayersAction::SearchClear, win, app),
                move |_, win, app| on_action_submit(SavedPlayersAction::SearchSubmit, win, app),
            ))
            // 右侧三段分段器：「全部」「标记玩家」「多次相遇」
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .p_1()
                    .rounded_lg()
                    .bg(Theme::HOVER_BG())
                    .border_1()
                    .border_color(Theme::CARD_BORDER())
                    .children(FILTER_OPTIONS.iter().map(|&opt| {
                        let is_active = self.active_tag_filter == opt;
                        let on_action_opt = on_action.clone();
                        div()
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .cursor_pointer()
                            .text_xs()
                            .font_weight(if is_active {
                                FontWeight::BOLD
                            } else {
                                FontWeight::NORMAL
                            })
                            .bg(if is_active {
                                Theme::ACCENT_PRIMARY()
                            } else {
                                hsla(0.0, 0.0, 0.0, 0.0)
                            })
                            .text_color(if is_active {
                                Theme::TEXT_ON_PRIMARY()
                            } else {
                                Theme::TEXT_MUTED()
                            })
                            .hover(|s| {
                                if !is_active {
                                    s.text_color(Theme::TEXT_PRIMARY())
                                } else {
                                    s
                                }
                            })
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                on_action_opt(
                                    SavedPlayersAction::FilterTag(opt.to_string()),
                                    win,
                                    app,
                                );
                            })
                            .child(opt)
                    })),
            )
    }

    /// P05 空态渲染 / 列表渲染
    fn render_players_content<'a, F>(
        &'a self,
        displayed: Vec<&'a SavedPlayer>,
        window: &mut Window,
        on_action: F,
    ) -> AnyElement
    where
        F: Fn(SavedPlayersAction, &mut Window, &mut App) + 'static + Clone,
    {
        // 搜索无结果空态
        if displayed.is_empty() && !self.search_query.trim().is_empty() {
            let on_clear = on_action.clone();
            let kw = self.search_query.trim().to_string();
            return div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_3()
                .bg(Theme::CARD_BG())
                .border_1()
                .border_color(Theme::CARD_BORDER())
                .rounded_xl()
                .p_8()
                .child(div().text_3xl().child("🔍"))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(Theme::TEXT_PRIMARY())
                        .child(format!("未找到包含 \"{}\" 的玩家", kw)),
                )
                .child(
                    div()
                        .px_4()
                        .py_1p5()
                        .rounded_md()
                        .bg(Theme::CARD_BG())
                        .border_1()
                        .border_color(Theme::CARD_BORDER())
                        .text_xs()
                        .text_color(Theme::TEXT_PRIMARY())
                        .hover(|s| {
                            s.bg(Theme::CARD_BG_HOVER())
                                .border_color(Theme::AURORA_CYAN())
                                .text_color(Theme::ACCENT_PRIMARY())
                        })
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            on_clear(SavedPlayersAction::SearchClear, win, app);
                        })
                        .child("清空搜索"),
                )
                .into_any_element();
        }

        // 初始空态（无任何记录）
        if displayed.is_empty() {
            return div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_2()
                .bg(Theme::CARD_BG())
                .border_1()
                .border_color(Theme::CARD_BORDER())
                .rounded_xl()
                .p_8()
                .child(div().text_3xl().child("📑"))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(Theme::TEXT_PRIMARY())
                        .child("暂无记录的同局玩家"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(Theme::TEXT_MUTED())
                        .child("在对局结束后，曾同局的玩家将会自动记录在此"),
                )
                .into_any_element();
        }

        let has_more = self.players.len() < self.total_count as usize;
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
                // 滚轮向下滚动且靠近底部时，自动加载下一页
                if has_more && ev.delta.pixel_delta(px(20.0)).y < px(0.0) {
                    let offset_y = -scroll_handle.offset().y;
                    let max_h = scroll_handle.max_offset().height;
                    if offset_y + px(300.0) >= max_h {
                        on_more_wheel(SavedPlayersAction::LoadMore, win, app);
                    }
                }
            })
            .children(
                displayed
                    .into_iter()
                    .map(|p| self.render_player_card(p, window, on_action.clone())),
            )
            // 列表底部加载更多 / 已加载全部
            .when(has_more, |this| {
                let on_more = on_more_click.clone();
                this.child(
                    div()
                        .id("saved-players-load-more")
                        .flex()
                        .items_center()
                        .justify_center()
                        .py_3()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            on_more(SavedPlayersAction::LoadMore, win, app);
                        })
                        .child(div().text_xs().text_color(Theme::TEXT_MUTED()).child(
                            if is_loading {
                                "正在加载更多..."
                            } else {
                                "点击或滚动加载更多"
                            },
                        )),
                )
            })
            .when(!has_more && self.total_count > 0, |this| {
                this.child(
                    div().flex().items_center().justify_center().py_3().child(
                        div()
                            .text_xs()
                            .text_color(Theme::TEXT_MUTED())
                            .child("已加载全部玩家"),
                    ),
                )
            })
            .into_any_element()
    }

    /// P03 玩家卡片结构与行内编辑
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

        // 召唤师头像
        let avatar_url = p.profile_icon_url();
        let avatar_img = loader.get_cached_render_image(&avatar_url);

        // 英雄头像图标
        let champ_url = p.champion_icon_url();
        let champ_img = champ_url
            .as_ref()
            .and_then(|u| loader.get_cached_render_image(u));

        // 标签颜色
        let tag_label = p.tag.clone().unwrap_or_default();
        let (tag_bg, tag_text, tag_border) = tag_badge_colors(&tag_label);

        let p_puuid_expand = p.puuid.clone();
        let p_puuid_pin = p.puuid.clone();
        let p_puuid_edit = p.puuid.clone();
        let p_puuid_delete = p.puuid.clone();
        let p_puuid_save = p.puuid.clone();

        let on_action_expand = on_action.clone();
        let on_action_pin = on_action.clone();
        let on_action_edit = on_action.clone();
        let on_action_delete = on_action.clone();
        let on_action_save = on_action.clone();
        let on_action_cancel = on_action.clone();

        div()
            .px(px(14.0))
            .py(px(10.0))
            .rounded_lg()
            // Vue .player-card 的 --card-bg 预合成到页面底色，Mica 开关前后保持一致；
            // 展开后的相遇记录浅灰底继续叠在这层之上
            .bg(Theme::over_page(Theme::CARD_BG()))
            .border_1()
            .border_color(Theme::CARD_BORDER())
            .hover(|s| s.border_color(Theme::AURORA_CYAN().opacity(0.35)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    // 玩家卡片主区域（点击整行展开/收起相遇记录）
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                on_action_expand(
                                    SavedPlayersAction::ToggleExpand(p_puuid_expand.clone()),
                                    win,
                                    app,
                                );
                            })
                            // 左侧：头像 + 英雄小图标 + 玩家信息行
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    // 44x44 圆形头像
                                    .child(
                                        div()
                                            .size(px(44.0))
                                            .rounded_full()
                                            .border_2()
                                            .border_color(Theme::CARD_BORDER())
                                            .bg(Theme::ACCENT_LIGHT())
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
                                            }),
                                    )
                                    // 若 champion_id > 0，显示英雄小图标
                                    .when(p.champion_id > 0, |this| {
                                        this.child(
                                            div()
                                                .size(px(34.0))
                                                .rounded(px(8.0))
                                                .border_1()
                                                .border_color(Theme::CARD_BORDER())
                                                .bg(Theme::ITEM_SLOT_BG())
                                                .overflow_hidden()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .when_some(champ_img, |sub, c_img| {
                                                    sub.child(
                                                        img(c_img)
                                                            .size_full()
                                                            .object_fit(ObjectFit::Cover),
                                                    )
                                                }),
                                        )
                                    })
                                    // 玩家信息行
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            // 昵称 + #TagLine
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_baseline()
                                                    .gap_1p5()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(Theme::TEXT_PRIMARY())
                                                            .child(p.summoner_name.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(Theme::TEXT_MUTED())
                                                            .child(if p.tag_line.is_empty() {
                                                                "#—".to_string()
                                                            } else {
                                                                format!("#{}", p.tag_line)
                                                            }),
                                                    ),
                                            )
                                            // 徽标集合：tag徽标 + 队列徽标 + 相遇次数徽标
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_1p5()
                                                    // 置顶徽标
                                                    .when(p.is_pinned, |this| {
                                                        this.child(
                                                            div()
                                                                .px_2()
                                                                .py_0p5()
                                                                .rounded_full()
                                                                .bg(Theme::GOLD().opacity(0.15))
                                                                .border_1()
                                                                .border_color(
                                                                    Theme::GOLD().opacity(0.4),
                                                                )
                                                                .text_xs()
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(Theme::GOLD())
                                                                .child("置顶"),
                                                        )
                                                    })
                                                    // tag 徽标
                                                    .when(p.tag.is_some(), {
                                                        let tag_label = tag_label.clone();
                                                        |this| {
                                                            this.child(
                                                                div()
                                                                    .px_2()
                                                                    .py_0p5()
                                                                    .rounded_full()
                                                                    .bg(tag_bg)
                                                                    .border_1()
                                                                    .border_color(tag_border)
                                                                    .text_xs()
                                                                    .font_weight(FontWeight::MEDIUM)
                                                                    .text_color(tag_text)
                                                                    .child(tag_label),
                                                            )
                                                        }
                                                    })
                                                    // 队列徽标
                                                    .when_some(
                                                        p.last_queue_type.as_ref(),
                                                        |this, q| {
                                                            this.child(
                                                                div()
                                                                    .px_2()
                                                                    .py_0p5()
                                                                    .rounded_sm()
                                                                    .bg(Theme::CARD_BORDER())
                                                                    .text_xs()
                                                                    .text_color(
                                                                        Theme::TEXT_SECONDARY(),
                                                                    )
                                                                    .child(
                                                                        queue_type_display_name(q),
                                                                    ),
                                                            )
                                                        },
                                                    )
                                                    // 相遇次数徽标
                                                    .when(p.encountered_count >= 2, |this| {
                                                        this.child(
                                                            div()
                                                                .px_2()
                                                                .py_0p5()
                                                                .rounded_sm()
                                                                .bg(Theme::CARD_BORDER())
                                                                .text_xs()
                                                                .font_weight(FontWeight::MEDIUM)
                                                                .text_color(Theme::TEXT_MUTED())
                                                                .child(format!(
                                                                    "相遇 {} 次",
                                                                    p.encountered_count
                                                                )),
                                                        )
                                                    }),
                                            ),
                                    ),
                            )
                            // 右侧：最后相遇时间 + 三个操作按钮 + 折叠箭头
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(Theme::TEXT_MUTED())
                                            .child(format_time(p.last_met_timestamp)),
                                    )
                                    // 三个操作按钮：置顶 + 编辑标记 + 删除（与主区域同一行）
                                    .child({
                                        let p_puuid_pin = p_puuid_pin.clone();
                                        let p_puuid_edit = p_puuid_edit.clone();
                                        let p_puuid_delete = p_puuid_delete.clone();
                                        let on_action_pin = on_action_pin.clone();
                                        let on_action_edit = on_action_edit.clone();
                                        let on_action_delete = on_action_delete.clone();
                                        let is_pinned = p.is_pinned;

                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_1p5()
                                            .child(
                                                div()
                                                    .px_2p5()
                                                    .py_0p5()
                                                    .rounded_sm()
                                                    .border_1()
                                                    .border_color(if is_pinned {
                                                        Theme::GOLD().opacity(0.6)
                                                    } else {
                                                        Theme::CARD_BORDER()
                                                    })
                                                    .bg(if is_pinned {
                                                        Theme::GOLD().opacity(0.12)
                                                    } else {
                                                        Theme::CARD_BG()
                                                    })
                                                    .text_color(if is_pinned {
                                                        Theme::GOLD()
                                                    } else {
                                                        Theme::TEXT_MUTED()
                                                    })
                                                    .text_xs()
                                                    .cursor_pointer()
                                                    .hover(|s| {
                                                        s.bg(Theme::CARD_BG_HOVER())
                                                            .border_color(Theme::GOLD())
                                                            .text_color(Theme::GOLD())
                                                    })
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        move |_ev, win, app| {
                                                            app.stop_propagation();
                                                            on_action_pin(
                                                                SavedPlayersAction::TogglePin(
                                                                    p_puuid_pin.clone(),
                                                                ),
                                                                win,
                                                                app,
                                                            );
                                                        },
                                                    )
                                                    .child(if is_pinned {
                                                        "★ 已置顶"
                                                    } else {
                                                        "☆ 置顶"
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .px_2p5()
                                                    .py_0p5()
                                                    .rounded_sm()
                                                    .border_1()
                                                    .border_color(Theme::CARD_BORDER())
                                                    .bg(Theme::CARD_BG())
                                                    .text_color(Theme::TEXT_MUTED())
                                                    .text_xs()
                                                    .cursor_pointer()
                                                    .hover(|s| {
                                                        s.bg(Theme::CARD_BG_HOVER())
                                                            .border_color(Theme::AURORA_CYAN())
                                                            .text_color(Theme::ACCENT_PRIMARY())
                                                    })
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        move |_ev, win, app| {
                                                            app.stop_propagation();
                                                            on_action_edit(
                                                                SavedPlayersAction::StartEdit(
                                                                    p_puuid_edit.clone(),
                                                                ),
                                                                win,
                                                                app,
                                                            );
                                                        },
                                                    )
                                                    .child("编辑标记"),
                                            )
                                            .child(
                                                div()
                                                    .px_2p5()
                                                    .py_0p5()
                                                    .rounded_sm()
                                                    .border_1()
                                                    .border_color(Theme::LOSS_BORDER().opacity(0.5))
                                                    .bg(Theme::CARD_BG())
                                                    .text_color(Theme::LOSS_TEXT())
                                                    .text_xs()
                                                    .cursor_pointer()
                                                    .hover(|s| {
                                                        s.bg(Theme::LOSS_BG())
                                                            .border_color(Theme::LOSS_BORDER())
                                                    })
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        move |_ev, win, app| {
                                                            app.stop_propagation();
                                                            on_action_delete(
                                                                SavedPlayersAction::AskDelete(
                                                                    p_puuid_delete.clone(),
                                                                ),
                                                                win,
                                                                app,
                                                            );
                                                        },
                                                    )
                                                    .child("删除"),
                                            )
                                    })
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(Theme::TEXT_MUTED())
                                            .child(if is_expanded { "▾" } else { "▸" }),
                                    ),
                            ),
                    )
                    // 卡片下方编辑区域（仅在编辑状态下展示，非编辑态不占额外行空间）
                    .when(is_editing, |this| {
                        this.child(div().flex().justify_end().pt_1().child({
                            let is_tag_focused = self.edit_tag_focus_handle.is_focused(window);
                            let edit_tag_h = self.edit_tag_focus_handle.clone();
                            let on_action_tag_key = on_action.clone();
                            let on_action_tag_clear = on_action.clone();
                            let p_puuid_save_clone = p_puuid_save.clone();
                            let on_save = on_action_save.clone();
                            let on_cancel = on_action_cancel.clone();

                            // 编辑态：单行输入框 + 保存 + 取消
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(text_field(
                                    "saved-player-tag-input",
                                    &self.editing_tag_buffer,
                                    "添加个性化标记...",
                                    px(220.0),
                                    &edit_tag_h,
                                    is_tag_focused,
                                    TextFieldOptions::default(),
                                    move |s, win, app| {
                                        on_action_tag_key(
                                            SavedPlayersAction::EditTagInput(s),
                                            win,
                                            app,
                                        )
                                    },
                                    move |win, app| {
                                        on_action_tag_clear(
                                            SavedPlayersAction::EditTagInput(String::new()),
                                            win,
                                            app,
                                        )
                                    },
                                    move |_, win, app| {
                                        on_save(
                                            SavedPlayersAction::SaveEdit(
                                                p_puuid_save_clone.clone(),
                                            ),
                                            win,
                                            app,
                                        )
                                    },
                                ))
                                .child(
                                    div()
                                        .px_3()
                                        .py_1()
                                        .rounded_sm()
                                        .bg(Theme::ACCENT_PRIMARY())
                                        .text_color(Theme::TEXT_ON_PRIMARY())
                                        .text_xs()
                                        .cursor_pointer()
                                        .hover(|s| s.opacity(0.9))
                                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                            on_action_save(
                                                SavedPlayersAction::SaveEdit(p_puuid_save.clone()),
                                                win,
                                                app,
                                            );
                                        })
                                        .child("保存"),
                                )
                                .child(
                                    div()
                                        .px_3()
                                        .py_1()
                                        .rounded_sm()
                                        .border_1()
                                        .border_color(Theme::CARD_BORDER())
                                        .bg(Theme::CARD_BG())
                                        .text_color(Theme::TEXT_MUTED())
                                        .text_xs()
                                        .cursor_pointer()
                                        .hover(|s| s.bg(Theme::CARD_BG_HOVER()))
                                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                            on_cancel(SavedPlayersAction::CancelEdit, win, app);
                                        })
                                        .child("取消"),
                                )
                                .into_any_element()
                        }))
                    })
                    // P04: 相遇记录展开层
                    .when(is_expanded, |this| {
                        this.child(self.render_encounter_section(on_action.clone()))
                    }),
            )
    }

    /// P04 相遇记录展开层
    fn render_encounter_section<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(SavedPlayersAction, &mut Window, &mut App) + 'static + Clone,
    {
        div()
            .pt(px(8.0))
            .border_t_1()
            .border_dashed()
            .border_color(Theme::CARD_BORDER())
            .child(if self.is_games_loading {
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .py_2()
                    .text_xs()
                    .text_color(Theme::TEXT_MUTED())
                    .child("正在加载相遇记录...")
                    .into_any_element()
            } else if self.expanded_games.is_empty() {
                div()
                    .py_2()
                    .text_xs()
                    .text_color(Theme::TEXT_MUTED())
                    .child("暂无详细相遇记录")
                    .into_any_element()
            } else {
                let on_more = on_action.clone();
                let has_more_games = self.expanded_games_total as usize > self.expanded_games.len();

                div()
                    .flex()
                    .flex_col()
                    .gap(px(3.0))
                    .children(self.expanded_games.iter().map(|game| {
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_2p5()
                            .py(px(2.5))
                            .rounded_sm()
                            .bg(Theme::HOVER_BG())
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::TEXT_PRIMARY())
                                    .child(queue_type_display_name(&game.queue_type)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::TEXT_MUTED())
                                    .child(format_time(game.timestamp)),
                            )
                    }))
                    .when(has_more_games, |this| {
                        this.child(
                            div().flex().justify_center().pt(px(4.0)).child(
                                div()
                                    .px_3()
                                    .py_0p5()
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(Theme::CARD_BORDER())
                                    .bg(Theme::CARD_BG())
                                    .text_xs()
                                    .text_color(Theme::TEXT_MUTED())
                                    .hover(|s| {
                                        s.bg(Theme::CARD_BG_HOVER())
                                            .border_color(Theme::AURORA_CYAN())
                                            .text_color(Theme::ACCENT_PRIMARY())
                                    })
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                        on_more(SavedPlayersAction::LoadMoreGames, win, app);
                                    })
                                    .child("加载更多"),
                            ),
                        )
                    })
                    .into_any_element()
            })
    }

    /// 删除确认弹窗
    fn render_delete_modal<F>(&self, puuid: String, on_action: F) -> impl IntoElement
    where
        F: Fn(SavedPlayersAction, &mut Window, &mut App) + 'static + Clone,
    {
        let on_confirm = on_action.clone();
        let on_cancel = on_action.clone();

        div()
            .absolute()
            .inset_0()
            .bg(hsla(0.0, 0.0, 0.0, 0.45))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(320.0))
                    .p_5()
                    .rounded_xl()
                    .bg(Theme::CARD_BG_HOVER())
                    .border_1()
                    .border_color(Theme::CARD_BORDER())
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::TEXT_PRIMARY())
                            .child("删除确认"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(Theme::TEXT_SECONDARY())
                            .child("确定要删除该玩家吗？"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap_2()
                            .child(
                                div()
                                    .px_3()
                                    .py_1p5()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(Theme::CARD_BORDER())
                                    .bg(Theme::CARD_BG())
                                    .text_xs()
                                    .text_color(Theme::TEXT_MUTED())
                                    .cursor_pointer()
                                    .hover(|s| s.bg(Theme::CARD_BG_HOVER()))
                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                        on_cancel(SavedPlayersAction::CancelDelete, win, app);
                                    })
                                    .child("取消"),
                            )
                            .child(
                                div()
                                    .px_3()
                                    .py_1p5()
                                    .rounded_md()
                                    .bg(Theme::LOSS_BG())
                                    .border_1()
                                    .border_color(Theme::LOSS_BORDER())
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::LOSS_TEXT())
                                    .cursor_pointer()
                                    .hover(|s| s.opacity(0.9))
                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                        on_confirm(
                                            SavedPlayersAction::ConfirmDelete(puuid.clone()),
                                            win,
                                            app,
                                        );
                                    })
                                    .child("确认删除"),
                            ),
                    ),
            )
    }
}
