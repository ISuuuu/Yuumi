use gpui::prelude::*;
use gpui::{
    div, px, App, Application, Bounds, ClipboardItem, Context, FocusHandle, FontWeight,
    InteractiveElement, IntoElement, MouseButton, ParentElement, Point, Render, ScrollHandle, Size,
    StatefulInteractiveElement, Styled, TitlebarOptions, Window, WindowBounds, WindowHandle,
    WindowKind, WindowOptions,
};
use std::sync::Arc;
use std::time::Duration;

mod agent_settings_view;
mod asset_loader;
mod assets;
mod bench_overlay;
mod career_view;
mod components;
mod game_info_view;
mod match_model;
pub mod platform;
pub mod services;
mod theme;
mod tray;
pub mod views;

use bench_overlay::{load_saved_origin, BenchOverlayView};
use components::sidebar::Sidebar;
use components::titlebar::{phase_label, TitleBar};
use match_model::MatchItem;
use services::{ConnectionStatus, LiveLcuEvent, LiveLcuService};
use theme::Theme;
use tray::TrayManager;
use views::career::{CareerBarAction, CareerHeaderStats, CareerView, MasteryFilterAction};
use views::game_info::{GameInfoTeamTab, GameInfoView, GameInfoViewMode};
use views::home::HomeView;
use views::saved_players::SavedPlayersView;
use views::search::SearchView;
use views::settings::SettingsView;
use views::tools::ToolsView;
use views::Route;
use yuumi_core::agents::{AutomationSettings, ChampSelectSession, GamePhase};
use yuumi_core::lcu::game_data::GameDataAssets;
use yuumi_core::parsers::game_info::LiveGameInfo;

/// 主窗口 Win32 标题（与标题栏显示的应用名保持一致，对照 Vue `tauri.conf.json` 的 `title`）
pub const MAIN_WINDOW_TITLE: &str = components::titlebar::APP_NAME;
pub const BENCH_WINDOW_TITLE: &str = "Yuumi - 大乱斗板凳席悬浮窗";

/// 预热图片资源：在 Tokio 运行时高并发异步拉取并解码为 RenderImage，完成后回到主线程重绘。
///
/// 彻底禁止在 UI 主线程读磁盘和解码图片，所有 I/O 与图像算法全部在后台 Tokio 线程池完成。
fn prewarm_assets(paths: Vec<String>, cx: &mut Context<MainWindow>) {
    if paths.is_empty() {
        return;
    }

    let handle = yuumi_core::runtime::tokio_handle();
    cx.spawn(async move |this, cx| {
        let Some(handle) = handle else {
            log::error!("[MainWindow] Tokio 运行时未注册，跳过资源预热");
            return;
        };
        let _ = handle
            .spawn(async move {
                let loader = asset_loader::global_asset_loader();
                let mut tasks = Vec::with_capacity(paths.len());
                for path in paths {
                    let l = loader.clone();
                    tasks.push(tokio::spawn(async move {
                        let _ = l.load_render_image(&path).await;
                    }));
                }
                for t in tasks {
                    let _ = t.await;
                }
            })
            .await;
        // 解码落地后重绘，避免图片资源加载完成却不刷新
        let _ = this.update(cx, |_this, cx| cx.notify());
    })
    .detach();
}

pub struct MainWindow {
    titlebar: TitleBar,
    /// 生涯页英雄搜索框的焦点句柄（gpui 0.2 无内置输入元素）
    career_mastery_focus: FocusHandle,
    matches: Vec<MatchItem>,
    stats: CareerHeaderStats,
    career_view: CareerView,
    /// 当前 10 人对局信息快照（由 LiveLcuService 实时推送，退回大厅后保留最后一次）
    game_info: Option<Arc<LiveGameInfo>>,
    /// 是否正在并发拉取 10 人数据
    game_info_loading: bool,
    /// 10 人对局信息视图模式：按队伍(5列) 或 全部战绩(10列)
    game_info_view_mode: GameInfoViewMode,
    /// 10 人对局信息按队伍模式下的选中队伍 Tab (友方 / 敌方)
    game_info_active_tab: GameInfoTeamTab,
    /// 当前 gameflow 阶段
    current_phase: String,
    tools_view: ToolsView,
    search_view: SearchView,
    saved_players_view: SavedPlayersView,
    settings_view: SettingsView,
    current_route: Route,
    /// 侧栏是否展开（64 ↔ 210，对照 Vue NavigationSidebar）
    sidebar_expanded: bool,
    /// 路由历史，供标题栏返回键使用（对照 Vue pageHistory）
    route_history: Vec<Route>,
    _tray: Option<TrayManager>,
    overlay_window: Option<WindowHandle<BenchOverlayView>>,
    live_service: LiveLcuService,
    connection_status: ConnectionStatus,
    /// 工具箱自动化开关的权威配置（启动时从磁盘加载，变更即回写）
    automation: AutomationSettings,
    /// 当前选人会话快照（大乱斗悬浮窗的数据来源）
    bench_session: Option<ChampSelectSession>,
    /// LCU 预加载的游戏资源字典（用于悬浮窗英雄名）
    game_assets: Option<Arc<GameDataAssets>>,
    /// 主视口全局滚动跟踪句柄（用于监听滚动到底部与回到顶部）
    main_scroll_handle: ScrollHandle,
}

impl MainWindow {
    pub fn new(cx: &mut Context<Self>) -> Self {
        // 战绩与 10 人对局信息全部来自真实 LCU 数据，启动时保持平滑空态
        let matches: Vec<MatchItem> = Vec::new();

        let stats = CareerHeaderStats::compute(&matches);
        let career_view = CareerView::new();
        let career_mastery_focus = cx.focus_handle();

        // 加载持久化的自动化开关，并用它初始化工具箱视图
        let automation = AutomationSettings::load();
        let tools_view = ToolsView::from_settings(&automation);

        let search_view = SearchView::new(cx).with_matches(matches.clone());
        let saved_players_view = SavedPlayersView::new(cx);
        let settings_view = SettingsView::new();
        let tray = match TrayManager::new() {
            Ok(tray) => Some(tray),
            Err(e) => {
                log::warn!("[Tray] 托盘创建失败: {}，托盘功能不可用", e);
                None
            }
        };

        // 初始化真实 LCU 实时监控与调度服务
        let asset_cache = std::sync::Arc::new(yuumi_core::lcu::asset_cache::AssetCache::default());
        asset_loader::init_global_asset_loader(asset_cache.clone());
        let live_service = LiveLcuService::new(asset_cache);

        // 启动即把持久化配置灌入 Agent 运行时，避免默认值覆盖用户选择
        let _ = live_service
            .auto_match_cfg
            .send(automation.auto_match.clone());
        let _ = live_service.auto_bp_cfg.send(automation.auto_bp.clone());

        // 优先订阅 LCU 领域事件与连接状态，避免漏掉启动瞬间的广播消息
        let mut event_rx = live_service.subscribe_events();
        live_service.start();

        // 订阅 LCU 领域事件与连接状态并在 GPUI 主调度器中消费
        cx.spawn(async move |entity, cx| {
            while let Ok(event) = event_rx.recv().await {
                let event_clone = event.clone();
                let _ = entity.update(cx, |view, cx| {
                    view.handle_live_event(event_clone, cx);
                });
            }
        })
        .detach();

        // 启动即保持空态，真实战绩到达后由事件统一预热资源并重绘
        Self {
            titlebar: TitleBar::new(components::titlebar::APP_NAME),
            matches,
            stats,
            career_view,
            game_info: None,
            game_info_loading: false,
            game_info_view_mode: GameInfoViewMode::ByTeam,
            game_info_active_tab: GameInfoTeamTab::Ally,
            current_phase: "None".to_string(),
            tools_view,
            search_view,
            saved_players_view,
            settings_view,
            career_mastery_focus,
            current_route: Route::Home,
            sidebar_expanded: false,
            route_history: Vec::new(),
            _tray: tray,
            overlay_window: None,
            live_service,
            connection_status: ConnectionStatus::Disconnected,
            automation,
            bench_session: None,
            game_assets: None,
            main_scroll_handle: ScrollHandle::new(),
        }
    }

    pub fn handle_live_event(&mut self, event: LiveLcuEvent, cx: &mut Context<Self>) {
        match event {
            LiveLcuEvent::ConnectionChanged(status) => {
                if let ConnectionStatus::Connected { summoner, .. } = &status {
                    if let Some(s) = summoner.as_ref() {
                        self.career_view.summoner = Some(s.clone());
                        self.saved_players_view
                            .set_self_puuid(Some(s.puuid.clone()));
                        // 资料卡头像也要预热，否则 LcuImage 只显示占位文字
                        prewarm_assets(vec![s.icon_path()], cx);
                    }
                }
                if matches!(status, ConnectionStatus::Disconnected) {
                    // 客户端断开：立即收起悬浮窗，避免残留空窗
                    self.bench_session = None;
                    self.game_info_loading = false;
                    self.close_bench_overlay(cx);
                }
                self.connection_status = status;
                cx.notify();
            }
            LiveLcuEvent::MatchHistoryLoaded(matches) => {
                log::info!("[MainWindow] 接收到真实战绩数据: count={}", matches.len());
                let items: Vec<MatchItem> = matches.into_iter().map(MatchItem::new).collect();
                self.stats = CareerHeaderStats::compute(&items);
                self.search_view.matches = items.clone();

                // 异步预热战绩资源：路径必须与渲染同源
                // （装备格渲染用 item_icon_urls，旧逻辑按 item_id 拼 /items/{id}.png 导致缓存永远不命中）
                let mut asset_paths = Vec::new();
                for m in &items {
                    let d = &m.display;
                    for url in [
                        &d.champion_icon_url,
                        &d.spell1_icon_url,
                        &d.spell2_icon_url,
                        &d.rune_icon_url,
                    ] {
                        if !url.is_empty() {
                            asset_paths.push(url.clone());
                        }
                    }
                    asset_paths.extend(d.item_icon_urls.iter().filter(|u| !u.is_empty()).cloned());
                    asset_paths.extend(
                        d.augment_icon_urls
                            .iter()
                            .filter(|u| !u.is_empty())
                            .cloned(),
                    );
                }
                // 20 场战绩里装备/英雄高度重复，去重以免打爆 LCU 请求
                asset_paths.sort();
                asset_paths.dedup();
                prewarm_assets(asset_paths, cx);

                self.matches = items;
                cx.notify();
            }
            LiveLcuEvent::RankedStatsLoaded(stats) => {
                log::info!(
                    "[MainWindow] 接收到真实排位数据: 队列数={}",
                    stats.queue_map.len()
                );
                self.career_view.ranked = Some(stats);
                cx.notify();
            }
            LiveLcuEvent::ChampionMasteryLoaded(mut masteries) => {
                log::info!(
                    "[MainWindow] 接收到真实熟练度数据: count={}",
                    masteries.len()
                );
                // 解析层不依赖资产字典，英雄名在此用已预加载的资源补全
                if let Some(assets) = self.game_assets.as_ref() {
                    for item in masteries.iter_mut() {
                        if let Some(name) = assets.champions.get(&(item.champion_id as i32)) {
                            item.champion_name = name.clone();
                        }
                    }
                }
                let asset_paths: Vec<String> = masteries
                    .iter()
                    .map(|m| m.champion_icon_url.clone())
                    .filter(|u| !u.is_empty())
                    .collect();
                prewarm_assets(asset_paths, cx);

                self.career_view.mastery_list = masteries;
                cx.notify();
            }
            LiveLcuEvent::LootSummaryLoaded(balances, items) => {
                log::info!(
                    "[MainWindow] 接收到真实战利品数据: 蓝精粹={}, 橙精粹={}, 物品数={}",
                    balances.blue_essence,
                    balances.orange_essence,
                    items.len()
                );
                self.career_view.essences = balances;
                self.career_view.loot_items = items;
                cx.notify();
            }
            LiveLcuEvent::GameDataReady(assets) => {
                self.game_assets = Some(assets);
                cx.notify();
            }
            LiveLcuEvent::GamePhaseChanged(phase) => {
                log::info!("[MainWindow] 接收到游戏阶段更新: {:?}", phase);
                self.current_phase = phase.as_str().to_string();
                if !matches!(
                    self.current_phase.as_str(),
                    "ChampSelect" | "GameStart" | "InProgress" | "Reconnect"
                ) {
                    self.game_info_loading = false;
                }
                if phase != GamePhase::ChampSelect {
                    // 选人结束（或进入其它阶段）即收起板凳席悬浮窗
                    self.bench_session = None;
                    self.close_bench_overlay(cx);
                }

                // 对局结束（结算阶段）：若存在 10 人快照，自动持久化记录同局相遇玩家
                if matches!(
                    phase,
                    GamePhase::PreEndOfGame | GamePhase::EndOfGame | GamePhase::WaitingForStats
                ) {
                    if let Some(ref info) = self.game_info {
                        if let Some(ref s) = self.saved_players_view.storage {
                            let self_puuid = self
                                .career_view
                                .summoner
                                .as_ref()
                                .map(|sum| sum.puuid.as_str())
                                .unwrap_or("");
                            let game_id = info.game_id.unwrap_or(0);
                            let queue_type = info.queue_id.to_string();
                            let players_to_record: Vec<yuumi_core::storage::GamePlayerRecord> =
                                info.players()
                                    .filter(|p| !p.is_local_player && !p.puuid.is_empty())
                                    .map(|p| yuumi_core::storage::GamePlayerRecord {
                                        puuid: p.puuid.clone(),
                                        summoner_name: p.summoner_name.clone(),
                                        tag_line: if p.tag_line.is_empty() {
                                            None
                                        } else {
                                            Some(p.tag_line.clone())
                                        },
                                        profile_icon_id: 0,
                                        champion_id: p.champion_id,
                                    })
                                    .collect();

                            if !players_to_record.is_empty()
                                && !self_puuid.is_empty()
                                && game_id > 0
                            {
                                match s.record_encounters(
                                    &players_to_record,
                                    self_puuid,
                                    game_id,
                                    &queue_type,
                                ) {
                                    Ok(rec) => {
                                        log::info!(
                                            "[MainWindow] 对局结束自动记录相遇玩家: 共 {} 人",
                                            rec
                                        );
                                        self.saved_players_view.reload(Some(self_puuid));
                                    }
                                    Err(e) => {
                                        log::warn!("[MainWindow] 自动记录相遇玩家失败: {}", e);
                                    }
                                }
                            }
                        }
                    }
                }
                cx.notify();
            }
            LiveLcuEvent::GameInfoLoading => {
                self.game_info_loading = true;
                cx.notify();
            }
            LiveLcuEvent::GameInfoUpdated(info) => {
                log::info!(
                    "[MainWindow] 对局信息更新: {} 蓝{}人 vs 红{}人",
                    info.phase_label,
                    info.blue_team.len(),
                    info.red_team.len()
                );
                // 预热 10 人英雄头像，避免看板首帧只有占位块
                let asset_paths: Vec<String> = info
                    .players()
                    .filter(|p| p.champion_id > 0)
                    .map(|p| {
                        format!(
                            "/lol-game-data/assets/v1/champion-icons/{}.png",
                            p.champion_id
                        )
                    })
                    .collect();
                prewarm_assets(asset_paths, cx);

                self.game_info = Some(info);
                self.game_info_loading = false;
                cx.notify();
            }
            LiveLcuEvent::ChampSelectUpdated(session) => {
                if session.is_aram() {
                    self.bench_session = Some(session);
                } else {
                    self.bench_session = None;
                }
                self.sync_bench_overlay(cx);
                cx.notify();
            }
            // 占位：战利品写动作进度等新事件；穷尽匹配兼容分支，整合时由主进程
            // 在此写入 career_view 进度面板并 cx.notify()（非动作接线，启动/自动流程均不触发写操作）
            _ => {}
        }
    }

    /// 依据开关状态与当前选人会话同步大乱斗板凳席悬浮窗
    pub fn sync_bench_overlay(&mut self, cx: &mut Context<Self>) {
        let should_show = self.automation.aram_bench_overlay
            && self
                .bench_session
                .as_ref()
                .map(|session| session.is_aram())
                .unwrap_or(false);

        if !should_show {
            self.close_bench_overlay(cx);
            return;
        }

        let Some(session) = self.bench_session.clone() else {
            return;
        };

        // 已开窗：仅推送最新数据
        if let Some(handle) = &self.overlay_window {
            let assets = self.game_assets.clone();
            let result = handle.update(cx, |view, _window, cx| {
                view.apply_session(&session, assets.as_deref(), cx);
            });
            if result.is_err() {
                log::warn!("[BenchOverlay] 悬浮窗已失效，重新创建");
                self.overlay_window = None;
            } else {
                return;
            }
        }

        self.open_bench_overlay(cx);
    }

    /// 创建置顶工具窗口并灌入当前板凳席数据
    fn open_bench_overlay(&mut self, cx: &mut Context<Self>) {
        let service = self.live_service.clone();
        let session = self.bench_session.clone();
        let assets = self.game_assets.clone();

        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                // 恢复上次拖拽后的位置，首次运行落在默认坐标
                origin: load_saved_origin().unwrap_or(Point {
                    x: px(420.0),
                    y: px(140.0),
                }),
                size: Size {
                    width: px(452.0),
                    height: px(268.0),
                },
            })),
            // 无系统装饰，仅保留标题供 Win32 定位到 HWND（置顶 / 工具窗样式）
            titlebar: Some(TitlebarOptions {
                title: Some(BENCH_WINDOW_TITLE.into()),
                appears_transparent: true,
                traffic_light_position: None,
            }),
            kind: WindowKind::Normal,
            focus: false,
            is_movable: true,
            is_resizable: false,
            ..Default::default()
        };

        let new_handle = cx.open_window(window_options, move |_window, cx| {
            cx.new(move |cx| {
                let mut view = BenchOverlayView::new(service);
                if let Some(session) = &session {
                    view.apply_session(session, assets.as_deref(), cx);
                }
                view
            })
        });

        if let Ok(handle) = new_handle {
            self.overlay_window = Some(handle);
            // 异步将板凳席小窗设置为 Windows 原生置顶与工具窗口（隐藏任务栏图标）
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(100));
                if let Some(hwnd) = platform::find_window_by_title(BENCH_WINDOW_TITLE) {
                    platform::set_window_topmost(hwnd, true);
                    platform::set_window_toolwindow(hwnd);
                } else {
                    log::warn!("[BenchOverlay] 未找到悬浮窗 HWND，置顶样式未应用");
                }
            });
        }
    }

    /// 关闭并销毁悬浮窗
    pub fn close_bench_overlay(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.overlay_window.take() {
            let _ = handle.update(cx, |view, window, cx| {
                view.clear(cx);
                window.remove_window();
            });
        }
    }

    /// 将工具箱开关状态同步进全局配置、Agent 运行时与磁盘
    fn persist_automation(&mut self, cx: &mut Context<Self>) {
        self.automation = self.tools_view.to_settings();

        let _ = self
            .live_service
            .auto_match_cfg
            .send(self.automation.auto_match.clone());
        let _ = self
            .live_service
            .auto_bp_cfg
            .send(self.automation.auto_bp.clone());

        if let Err(e) = self.automation.save() {
            log::warn!("[Tools] 自动化配置持久化失败: {}", e);
        }

        // 悬浮窗开关变更需立即生效
        self.sync_bench_overlay(cx);
        cx.notify();
    }

    pub fn toggle_card_expanded(&mut self, idx: usize, cx: &mut Context<Self>) {
        if let Some(m) = self.matches.get_mut(idx) {
            m.expanded = !m.expanded;
            cx.notify();
        }
    }

    pub fn refresh_career(&mut self, _cx: &mut Context<Self>) {
        let live = self.live_service.clone();
        yuumi_core::runtime::spawn_background(async move {
            live.refresh_career_data().await;
        });
    }

    /// 控制战绩查询页队列下拉的开合
    pub fn set_search_queue_menu(&mut self, open: bool, cx: &mut Context<Self>) {
        self.search_view.queue_menu_open = open;
        cx.notify();
    }

    /// 处理近期统计横条的队列筛选与浮层开合
    pub fn apply_career_bar_action(&mut self, action: CareerBarAction, cx: &mut Context<Self>) {
        match action {
            CareerBarAction::TogglePopup(kind) => {
                self.career_view.popup = if self.career_view.popup == Some(kind) {
                    None
                } else {
                    Some(kind)
                };
            }
            CareerBarAction::ClosePopup => self.career_view.popup = None,
            CareerBarAction::SelectQueue(id) => {
                self.career_view.selected_queue = id;
                self.career_view.popup = None;
            }
        }
        cx.notify();
    }

    /// 更新熟练度页的英雄搜索词
    pub fn set_mastery_query(&mut self, text: String, cx: &mut Context<Self>) {
        self.career_view.mastery_query = text;
        cx.notify();
    }

    /// 应用熟练度页的筛选 / 排序动作
    pub fn apply_mastery_filter(&mut self, action: MasteryFilterAction, cx: &mut Context<Self>) {
        match action {
            MasteryFilterAction::Chest(value) => self.career_view.mastery_chest = value,
            MasteryFilterAction::Level(value) => self.career_view.mastery_level = value,
            MasteryFilterAction::Sort(value) => self.career_view.mastery_sort = value,
            MasteryFilterAction::ToggleSortDir => {
                self.career_view.mastery_sort_desc = !self.career_view.mastery_sort_desc
            }
            MasteryFilterAction::ToggleDropdown(kind) => {
                let slot = &mut self.career_view.mastery_dropdown;
                *slot = if *slot == Some(kind) {
                    None
                } else {
                    Some(kind)
                };
            }
            MasteryFilterAction::CloseDropdown => self.career_view.mastery_dropdown = None,
        }
        cx.notify();
    }

    /// 复制当前召唤师的 Riot ID，并在资料卡上短暂回显反馈
    pub fn copy_riot_id(&mut self, cx: &mut Context<Self>) {
        let text = match self.career_view.summoner.as_ref() {
            Some(summoner) => summoner.full_name(),
            None => return,
        };

        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.career_view.copied = true;
        cx.notify();

        cx.spawn(async move |entity, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1500))
                .await;
            let _ = entity.update(cx, |this, cx| {
                this.career_view.copied = false;
                cx.notify();
            });
        })
        .detach();
    }

    /// 跳转到当前召唤师的战绩查询（对照 Vue `Career.vue` 的 `goToHistory`）
    pub fn open_own_history(&mut self, cx: &mut Context<Self>) {
        let name = match self.career_view.summoner.as_ref() {
            Some(summoner) => summoner.full_name(),
            None => return,
        };

        self.perform_search(&name, cx);
        self.switch_route(Route::Search, cx);
    }

    pub fn switch_route(&mut self, route: Route, cx: &mut Context<Self>) {
        if route != self.current_route {
            self.route_history.push(self.current_route);
            if self.route_history.len() > 20 {
                self.route_history.remove(0);
            }
            self.current_route = route;
            self.main_scroll_handle.set_offset(Point::default());
            if self.current_route == Route::SavedPlayers {
                self.reload_saved_players(cx);
            }
        }
        cx.notify();
    }

    /// 异步拉取路人集首屏（完全剥离到 Tokio 阻塞池，0 阻塞 UI 主线程）
    pub fn reload_saved_players(&mut self, cx: &mut Context<Self>) {
        let storage = self.saved_players_view.storage.clone();
        let Some(storage) = storage else {
            return;
        };

        let filter = if self.saved_players_view.active_tag_filter == "全部" {
            None
        } else {
            Some(self.saved_players_view.active_tag_filter.clone())
        };
        let search = if self.saved_players_view.search_query.trim().is_empty() {
            None
        } else {
            Some(self.saved_players_view.search_query.trim().to_string())
        };
        let self_p = self.saved_players_view.self_puuid.clone();

        let handle = yuumi_core::runtime::tokio_handle();
        cx.spawn(async move |entity, cx| {
            let Some(handle) = handle else {
                return;
            };
            let res = handle
                .spawn(async move {
                    tokio::task::spawn_blocking(move || {
                        let total = storage
                            .count_players(self_p.as_deref(), filter.as_deref(), search.as_deref())
                            .unwrap_or(0);
                        let list = storage
                            .query_players(
                                self_p.as_deref(),
                                filter.as_deref(),
                                search.as_deref(),
                                views::saved_players::SAVED_PLAYERS_PAGE_SIZE,
                                0,
                            )
                            .unwrap_or_default();
                        (total, list)
                    })
                    .await
                    .unwrap_or((0, Vec::new()))
                })
                .await;

            if let Ok((total, list)) = res {
                let _ = entity.update(cx, |this, cx| {
                    let mut asset_paths = Vec::new();
                    for p in &list {
                        asset_paths.push(p.profile_icon_url());
                        if let Some(champ) = p.champion_icon_url() {
                            asset_paths.push(champ);
                        }
                    }
                    prewarm_assets(asset_paths, cx);

                    this.saved_players_view.current_page = 1;
                    this.saved_players_view.total_count = total;
                    this.saved_players_view.players = list;
                    this.saved_players_view.is_loading_more = false;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// 异步拉取路人集下一页（完全剥离到 Tokio 阻塞池，0 阻塞 UI 主线程）
    pub fn load_more_saved_players(&mut self, cx: &mut Context<Self>) {
        if self.saved_players_view.is_loading_more
            || self.saved_players_view.players.len() >= self.saved_players_view.total_count as usize
        {
            return;
        }

        self.saved_players_view.is_loading_more = true;
        let storage = self.saved_players_view.storage.clone();
        let Some(storage) = storage else {
            self.saved_players_view.is_loading_more = false;
            return;
        };

        let next_page = self.saved_players_view.current_page + 1;
        let offset = (next_page - 1) * views::saved_players::SAVED_PLAYERS_PAGE_SIZE;
        let filter = if self.saved_players_view.active_tag_filter == "全部" {
            None
        } else {
            Some(self.saved_players_view.active_tag_filter.clone())
        };
        let search = if self.saved_players_view.search_query.trim().is_empty() {
            None
        } else {
            Some(self.saved_players_view.search_query.trim().to_string())
        };
        let self_p = self.saved_players_view.self_puuid.clone();

        let handle = yuumi_core::runtime::tokio_handle();
        cx.spawn(async move |entity, cx| {
            let Some(handle) = handle else {
                return;
            };
            let res = handle
                .spawn(async move {
                    tokio::task::spawn_blocking(move || {
                        storage.query_players(
                            self_p.as_deref(),
                            filter.as_deref(),
                            search.as_deref(),
                            views::saved_players::SAVED_PLAYERS_PAGE_SIZE,
                            offset,
                        )
                    })
                    .await
                    .unwrap_or_else(|e| Err(e.to_string()))
                })
                .await;

            if let Ok(Ok(more)) = res {
                let _ = entity.update(cx, |this, cx| {
                    let mut asset_paths = Vec::new();
                    for p in &more {
                        asset_paths.push(p.profile_icon_url());
                        if let Some(champ) = p.champion_icon_url() {
                            asset_paths.push(champ);
                        }
                    }
                    prewarm_assets(asset_paths, cx);

                    this.saved_players_view.players.extend(more);
                    this.saved_players_view.current_page = next_page;
                    this.saved_players_view.is_loading_more = false;
                    cx.notify();
                });
            } else {
                let _ = entity.update(cx, |this, _cx| {
                    this.saved_players_view.is_loading_more = false;
                });
            }
        })
        .detach();
    }

    /// 返回上一路由（标题栏返回键）
    pub fn go_back(&mut self, cx: &mut Context<Self>) {
        if let Some(previous) = self.route_history.pop() {
            self.current_route = previous;
            cx.notify();
        }
    }

    /// 折叠 / 展开侧栏
    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_expanded = !self.sidebar_expanded;
        cx.notify();
    }

    pub fn toggle_auto_accept(&mut self, cx: &mut Context<Self>) {
        self.tools_view.match_config.auto_accept = !self.tools_view.match_config.auto_accept;
        self.persist_automation(cx);
    }

    pub fn toggle_auto_reconnect(&mut self, cx: &mut Context<Self>) {
        self.tools_view.match_config.auto_reconnect = !self.tools_view.match_config.auto_reconnect;
        self.persist_automation(cx);
    }

    pub fn toggle_auto_play_again(&mut self, cx: &mut Context<Self>) {
        self.tools_view.match_config.auto_play_again =
            !self.tools_view.match_config.auto_play_again;
        self.persist_automation(cx);
    }

    pub fn toggle_auto_pick(&mut self, cx: &mut Context<Self>) {
        self.tools_view.bp_config.auto_pick = !self.tools_view.bp_config.auto_pick;
        self.persist_automation(cx);
    }

    pub fn toggle_auto_ban(&mut self, cx: &mut Context<Self>) {
        self.tools_view.bp_config.auto_ban = !self.tools_view.bp_config.auto_ban;
        self.persist_automation(cx);
    }

    pub fn toggle_auto_spell(&mut self, cx: &mut Context<Self>) {
        self.tools_view.bp_config.auto_spell = !self.tools_view.bp_config.auto_spell;
        self.persist_automation(cx);
    }

    pub fn toggle_bench_overlay_setting(&mut self, cx: &mut Context<Self>) {
        self.tools_view.bench_overlay = !self.tools_view.bench_overlay;
        self.persist_automation(cx);
    }

    pub fn toggle_auto_trade(&mut self, cx: &mut Context<Self>) {
        self.tools_view.auto_trade = !self.tools_view.auto_trade;
        self.persist_automation(cx);
    }

    pub fn toggle_auto_honor(&mut self, cx: &mut Context<Self>) {
        self.tools_view.match_config.auto_honor = !self.tools_view.match_config.auto_honor;
        self.persist_automation(cx);
    }

    pub fn toggle_close_to_tray(&mut self, cx: &mut Context<Self>) {
        self.settings_view.close_to_tray = !self.settings_view.close_to_tray;
        cx.notify();
    }

    pub fn toggle_start_with_windows(&mut self, cx: &mut Context<Self>) {
        self.settings_view.start_with_windows = !self.settings_view.start_with_windows;
        cx.notify();
    }

    pub fn toggle_minimize_on_game(&mut self, cx: &mut Context<Self>) {
        self.settings_view.minimize_on_game_start = !self.settings_view.minimize_on_game_start;
        cx.notify();
    }

    pub fn toggle_mica(&mut self, cx: &mut Context<Self>) {
        self.settings_view.mica_enabled = !self.settings_view.mica_enabled;
        cx.notify();
    }

    pub fn toggle_hide_tft(&mut self, cx: &mut Context<Self>) {
        self.settings_view.hide_tft = !self.settings_view.hide_tft;
        cx.notify();
    }

    pub fn toggle_hide_saved_players(&mut self, cx: &mut Context<Self>) {
        self.settings_view.hide_saved_players = !self.settings_view.hide_saved_players;
        cx.notify();
    }

    /// 加载高保真对局演示预览数据（优先真实快照，兜底演示）
    pub fn load_game_info_demo(&mut self, cx: &mut Context<Self>) {
        self.game_info = Some(std::sync::Arc::new(
            yuumi_core::parsers::game_info::LiveGameInfo::load_snapshot_or_demo(),
        ));
        cx.notify();
    }

    pub fn switch_game_info_view_mode(&mut self, mode: GameInfoViewMode, cx: &mut Context<Self>) {
        self.game_info_view_mode = mode;
        cx.notify();
    }

    pub fn switch_game_info_team_tab(&mut self, tab: GameInfoTeamTab, cx: &mut Context<Self>) {
        self.game_info_active_tab = tab;
        cx.notify();
    }

    /// 退出演示预览模式
    pub fn exit_game_info_demo(&mut self, cx: &mut Context<Self>) {
        if let Some(info) = &self.game_info {
            if info.is_demo {
                self.game_info = None;
                cx.notify();
            }
        }
    }

    pub fn select_theme_index(&mut self, idx: usize, cx: &mut Context<Self>) {
        self.settings_view.selected_theme_index = idx;
        cx.notify();
    }

    pub fn perform_search(&mut self, query: &str, cx: &mut Context<Self>) {
        let trimmed = query.trim().to_string();
        if trimmed.is_empty() {
            return;
        }

        self.search_view.set_search_query(&trimmed);
        self.search_view.set_searching(true);
        cx.notify();

        let live_service = self.live_service.clone();
        let query_name = trimmed.clone();

        cx.spawn(async move |entity, cx| {
            let client_opt = live_service.get_client().await;
            let assets_opt = live_service.get_game_data_assets().await;

            let (client, assets) = match (client_opt, assets_opt) {
                (Some(c), Some(a)) => (c, a),
                _ => {
                    let _ = entity.update(cx, |this, cx| {
                        this.search_view
                            .set_search_error("客户端未连接或离线中，请先启动英雄联盟");
                        cx.notify();
                    });
                    return;
                }
            };

            // 1. 查询召唤师信息
            match yuumi_core::parsers::summoner::fetch_summoner_by_name(&client, &query_name).await
            {
                Ok(summoner) => {
                    let full_name = summoner.full_name();
                    // 2. 查询近期战绩 (0..20)
                    match yuumi_core::parsers::match_parser::fetch_match_history(
                        &client,
                        &summoner.puuid,
                        Some(0),
                        Some(19),
                        &assets,
                    )
                    .await
                    {
                        Ok(matches) => {
                            let items: Vec<MatchItem> =
                                matches.into_iter().map(MatchItem::new).collect();
                            let first_game_id = items.first().map(|m| m.display.game_id);

                            let _ = entity.update(cx, |this, cx| {
                                this.search_view.set_search_results(full_name, items);
                                cx.notify();
                            });

                            // 3. 异步拉取首场详情填入 details_cache
                            if let Some(gid) = first_game_id {
                                if let Ok(detail) =
                                    yuumi_core::parsers::match_detail::fetch_game_detail(
                                        &client, gid, &assets,
                                    )
                                    .await
                                {
                                    let _ = entity.update(cx, |this, cx| {
                                        this.search_view.insert_game_detail(gid, detail);
                                        cx.notify();
                                    });
                                }
                            }
                        }
                        Err(e) => {
                            let _ = entity.update(cx, |this, cx| {
                                this.search_view
                                    .set_search_error(format!("获取战绩失败: {}", e));
                                cx.notify();
                            });
                        }
                    }
                }
                Err(e) => {
                    let _ = entity.update(cx, |this, cx| {
                        this.search_view
                            .set_search_error(format!("未找到召唤师: {}", e));
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    pub fn load_match_detail(&mut self, game_id: u64, cx: &mut Context<Self>) {
        if self.search_view.details_cache.contains_key(&game_id) {
            self.search_view.selected_game_id = Some(game_id);
            cx.notify();
            return;
        }

        self.search_view.selected_game_id = Some(game_id);
        self.search_view.is_loading_detail = true;
        cx.notify();

        let live_service = self.live_service.clone();
        cx.spawn(async move |entity, cx| {
            let client_opt = live_service.get_client().await;
            let assets_opt = live_service.get_game_data_assets().await;

            if let (Some(client), Some(assets)) = (client_opt, assets_opt) {
                if let Ok(detail) =
                    yuumi_core::parsers::match_detail::fetch_game_detail(&client, game_id, &assets)
                        .await
                {
                    let _ = entity.update(cx, |this, cx| {
                        this.search_view.insert_game_detail(game_id, detail);
                        this.search_view.is_loading_detail = false;
                        cx.notify();
                    });
                    return;
                }
            }

            let _ = entity.update(cx, |this, cx| {
                // 拉取失败时保持空态，不再用其它对局详情冒充当前对局
                log::warn!("[MainWindow] 对局详情拉取失败: game_id={}", game_id);
                this.search_view.is_loading_detail = false;
                cx.notify();
            });
        })
        .detach();
    }
}

impl Render for MainWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let stats = &self.stats;
        let _is_overlay_open = self.overlay_window.is_some();
        let view_entity = cx.entity().clone();

        // 标题栏依据真实连接状态与游戏阶段展示上下文徽章
        let connected = matches!(self.connection_status, ConnectionStatus::Connected { .. });
        let phase = if connected {
            phase_label(self.current_phase.as_str())
        } else {
            None
        };
        let show_back = !self.route_history.is_empty();

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(Theme::BG_APP)
            .font_family(Theme::font_family())
            // 1. 无边框自定义标题栏（Logo + Yummi + 阶段徽章 + 窗口按钮）
            .child(self.titlebar.render(
                show_back,
                phase,
                window,
                {
                    let entity = view_entity.clone();
                    move |_w, app| {
                        entity.update(app, |this, cx| this.go_back(cx));
                    }
                },
                |w, _app| {
                    w.minimize_window();
                },
                |_w, app| {
                    app.quit();
                },
            ))
            // 2. 主体区域：64/210 导航侧栏 + 主路由视口
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .overflow_hidden()
                    .child(Sidebar::render(
                        self.current_route,
                        self.sidebar_expanded,
                        self.career_view.summoner.as_ref(),
                        {
                            let entity = view_entity.clone();
                            move |route, _window, app| {
                                entity.update(app, |this, cx| {
                                    this.switch_route(route, cx);
                                });
                            }
                        },
                        {
                            let entity = view_entity.clone();
                            move |_window, app| {
                                entity.update(app, |this, cx| {
                                    this.toggle_sidebar(cx);
                                });
                            }
                        },
                    ))
                    // 右侧主视口：首页与路人集自行撑满（路人集采用吸顶Header+内部独立平滑滚动），其余路由沿用统一外层滚动
                    .child(
                        div()
                            .id("main-scroll-view")
                            .flex_1()
                            .h_full()
                            .min_w(px(0.0))
                            .track_scroll(&self.main_scroll_handle)
                            .when(
                                self.current_route != Route::Home
                                    && self.current_route != Route::SavedPlayers,
                                |el| el.overflow_y_scroll(),
                            )
                            .when(self.current_route == Route::Home, |el| {
                                el.child(HomeView::render(&self.connection_status))
                            })
                            .when(self.current_route == Route::Career, |el| {
                                let view = view_entity.clone();
                                let view_matches = view_entity.clone();
                                let view_refresh = view_entity.clone();
                                let view_copy = view_entity.clone();
                                let view_history = view_entity.clone();
                                let view_query = view_entity.clone();
                                let view_filter = view_entity.clone();
                                let view_bar = view_entity.clone();
                                el.child(self.career_view.render(
                                    &self.matches,
                                    stats,
                                    move |target_tab, _window, app| {
                                        view.update(app, |this, cx| {
                                            this.career_view.active_tab = target_tab;
                                            this.career_view.popup = None;
                                            cx.notify();
                                        });
                                    },
                                    move |clicked_idx, _window, app| {
                                        view_matches.update(app, |this, cx| {
                                            this.toggle_card_expanded(clicked_idx, cx);
                                        });
                                    },
                                    move |_window, app| {
                                        view_refresh.update(app, |this, cx| {
                                            this.refresh_career(cx);
                                        });
                                    },
                                    move |_window, app| {
                                        view_copy.update(app, |this, cx| this.copy_riot_id(cx));
                                    },
                                    move |_window, app| {
                                        view_history
                                            .update(app, |this, cx| this.open_own_history(cx));
                                    },
                                    &self.career_mastery_focus,
                                    self.career_mastery_focus.is_focused(window),
                                    move |text: String, _window, app| {
                                        view_query.update(app, |this, cx| {
                                            this.set_mastery_query(text, cx)
                                        });
                                    },
                                    move |action, _window, app| {
                                        view_filter.update(app, |this, cx| {
                                            this.apply_mastery_filter(action, cx)
                                        });
                                    },
                                    move |action: CareerBarAction, _window, app| {
                                        view_bar.update(app, |this, cx| {
                                            this.apply_career_bar_action(action, cx)
                                        });
                                    },
                                ))
                            })
                            .when(self.current_route == Route::GameInfo, |el| {
                                let connected = matches!(
                                    self.connection_status,
                                    ConnectionStatus::Connected { .. }
                                );
                                let view_mode = self.game_info_view_mode;
                                let active_tab = self.game_info_active_tab;
                                let view_load_demo = view_entity.clone();
                                let view_exit_demo = view_entity.clone();
                                let view_mode_cb = view_entity.clone();
                                let view_tab_cb = view_entity.clone();
                                el.px_3().py_2().child(GameInfoView::render(
                                    self.game_info.as_deref(),
                                    self.current_phase.as_str(),
                                    self.game_info_loading,
                                    connected,
                                    view_mode,
                                    active_tab,
                                    move |_event, _window, app| {
                                        view_load_demo.update(app, |this, cx| {
                                            this.load_game_info_demo(cx);
                                        });
                                    },
                                    move |_event, _window, app| {
                                        view_exit_demo.update(app, |this, cx| {
                                            this.exit_game_info_demo(cx);
                                        });
                                    },
                                    move |mode, _window, app| {
                                        view_mode_cb.update(app, |this, cx| {
                                            this.switch_game_info_view_mode(mode, cx);
                                        });
                                    },
                                    move |tab, _window, app| {
                                        view_tab_cb.update(app, |this, cx| {
                                            this.switch_game_info_team_tab(tab, cx);
                                        });
                                    },
                                ))
                            })
                            .when(self.current_route == Route::Search, |el| {
                                let view = view_entity.clone();
                                let view_sq = view.clone();
                                let on_select_match = {
                                    let v = view.clone();
                                    move |game_id: u64, _win: &mut Window, app: &mut App| {
                                        v.update(app, |this, cx| {
                                            this.load_match_detail(game_id, cx);
                                        });
                                    }
                                };
                                let on_filter_queue = {
                                    let v = view.clone();
                                    move |qid: Option<i32>, _win: &mut Window, app: &mut App| {
                                        v.update(app, |this, cx| {
                                            this.search_view.active_queue_filter = qid;
                                            cx.notify();
                                        });
                                    }
                                };
                                let on_search = {
                                    let v = view.clone();
                                    move |query: &str, _win: &mut Window, app: &mut App| {
                                        let q = query.to_string();
                                        v.update(app, |this, cx| {
                                            this.perform_search(&q, cx);
                                        });
                                    }
                                };
                                let on_input_change = {
                                    let v = view.clone();
                                    move |new_text: String, _win: &mut Window, app: &mut App| {
                                        v.update(app, |this, cx| {
                                            this.search_view.set_search_query(new_text);
                                            cx.notify();
                                        });
                                    }
                                };
                                el.child(self.search_view.render(
                                    on_select_match,
                                    on_filter_queue,
                                    on_search,
                                    on_input_change,
                                    self.search_view.focus_handle.is_focused(window),
                                    move |open, _window, app| {
                                        view_sq.update(app, |this, cx| {
                                            this.set_search_queue_menu(open, cx)
                                        });
                                    },
                                ))
                            })
                            .when(self.current_route == Route::SavedPlayers, |el| {
                                let view = view_entity.clone();
                                let on_action =
                                    move |action: views::saved_players::SavedPlayersAction,
                                          _win: &mut Window,
                                          app: &mut App| {
                                        view.update(app, |this, cx| {
                                            use views::saved_players::SavedPlayersAction;
                                            match action {
                                                SavedPlayersAction::FilterTag(tag) => {
                                                    this.saved_players_view.active_tag_filter = tag;
                                                    this.reload_saved_players(cx);
                                                }
                                                SavedPlayersAction::TogglePin(puuid) => {
                                                    this.saved_players_view.toggle_pin(&puuid);
                                                }
                                                SavedPlayersAction::ToggleExpand(puuid) => {
                                                    this.saved_players_view.toggle_expand(&puuid);
                                                }
                                                SavedPlayersAction::RemovePlayer(puuid) => {
                                                    this.saved_players_view.remove_player(&puuid);
                                                }
                                                SavedPlayersAction::StartEdit(puuid) => {
                                                    this.saved_players_view.start_edit(&puuid);
                                                }
                                                SavedPlayersAction::SaveEdit(puuid) => {
                                                    this.saved_players_view.save_edit(&puuid);
                                                }
                                                SavedPlayersAction::CancelEdit => {
                                                    this.saved_players_view.cancel_edit();
                                                }
                                                SavedPlayersAction::SetEditTag(tag) => {
                                                    this.saved_players_view.set_editing_tag(&tag);
                                                }
                                                SavedPlayersAction::LoadMore => {
                                                    this.load_more_saved_players(cx);
                                                }
                                                SavedPlayersAction::SearchInput(q) => {
                                                    this.saved_players_view.search_query = q;
                                                    this.reload_saved_players(cx);
                                                }
                                                SavedPlayersAction::SearchSubmit => {
                                                    this.reload_saved_players(cx);
                                                }
                                                SavedPlayersAction::SearchClear => {
                                                    this.saved_players_view.search_query.clear();
                                                    this.reload_saved_players(cx);
                                                }
                                                SavedPlayersAction::EditTagInput(t) => {
                                                    this.saved_players_view.editing_tag_buffer = t;
                                                }
                                                SavedPlayersAction::EditNotesInput(n) => {
                                                    this.saved_players_view.editing_notes_buffer =
                                                        n;
                                                }
                                            }
                                            cx.notify();
                                        });
                                    };

                                el.child(self.saved_players_view.render_panel(window, on_action))
                            })
                            .when(self.current_route == Route::Tools, |el| {
                                let view = view_entity.clone();
                                let p_accept = {
                                    let v = view.clone();
                                    move |_ev: &_, _win: &mut _, app: &mut _| {
                                        v.update(app, |this, cx| this.toggle_auto_accept(cx));
                                    }
                                };
                                let p_reconn = {
                                    let v = view.clone();
                                    move |_ev: &_, _win: &mut _, app: &mut _| {
                                        v.update(app, |this, cx| this.toggle_auto_reconnect(cx));
                                    }
                                };
                                let p_again = {
                                    let v = view.clone();
                                    move |_ev: &_, _win: &mut _, app: &mut _| {
                                        v.update(app, |this, cx| this.toggle_auto_play_again(cx));
                                    }
                                };
                                let p_pick = {
                                    let v = view.clone();
                                    move |_ev: &_, _win: &mut _, app: &mut _| {
                                        v.update(app, |this, cx| this.toggle_auto_pick(cx));
                                    }
                                };
                                let p_ban = {
                                    let v = view.clone();
                                    move |_ev: &_, _win: &mut _, app: &mut _| {
                                        v.update(app, |this, cx| this.toggle_auto_ban(cx));
                                    }
                                };
                                let p_spell = {
                                    let v = view.clone();
                                    move |_ev: &_, _win: &mut _, app: &mut _| {
                                        v.update(app, |this, cx| this.toggle_auto_spell(cx));
                                    }
                                };
                                let p_bench = {
                                    let v = view.clone();
                                    move |_ev: &_, _win: &mut _, app: &mut _| {
                                        v.update(app, |this, cx| {
                                            this.toggle_bench_overlay_setting(cx)
                                        });
                                    }
                                };
                                let p_trade = {
                                    let v = view.clone();
                                    move |_ev: &_, _win: &mut _, app: &mut _| {
                                        v.update(app, |this, cx| this.toggle_auto_trade(cx));
                                    }
                                };
                                let p_honor = {
                                    let v = view.clone();
                                    move |_ev: &_, _win: &mut _, app: &mut _| {
                                        v.update(app, |this, cx| this.toggle_auto_honor(cx));
                                    }
                                };

                                let p_fix_win = {
                                    let _v = view.clone();
                                    move |_ev: &_, _win: &mut _, _app: &mut _| {
                                        std::thread::spawn(|| {
                                            match services::fix_client_window() {
                                                Ok(msg) => log::info!("[Tools] {}", msg),
                                                Err(e) => log::warn!("[Tools] 修复窗口失败: {}", e),
                                            }
                                        });
                                    }
                                };
                                let p_clear_cache = {
                                    let v = view.clone();
                                    move |_ev: &_, _win: &mut _, app: &mut _| {
                                        v.update(app, |this, _cx| {
                                            let client_path =
                                                this.settings_view.client_path.clone();
                                            std::thread::spawn(move || {
                                                match services::clear_game_cache(Some(&client_path))
                                                {
                                                    Ok(msg) => log::info!("[Tools] {}", msg),
                                                    Err(e) => {
                                                        log::warn!("[Tools] 清理缓存失败: {}", e)
                                                    }
                                                }
                                            });
                                        });
                                    }
                                };

                                el.child(self.tools_view.render_panel(
                                    p_accept,
                                    p_reconn,
                                    p_again,
                                    p_pick,
                                    p_ban,
                                    p_spell,
                                    p_bench,
                                    p_trade,
                                    p_honor,
                                    p_fix_win,
                                    p_clear_cache,
                                ))
                            })
                            .when(self.current_route == Route::Settings, |el| {
                                let view = view_entity.clone();
                                let p_close_tray = {
                                    let v = view.clone();
                                    move |_ev: &_, _win: &mut _, app: &mut _| {
                                        v.update(app, |this, cx| this.toggle_close_to_tray(cx));
                                    }
                                };
                                let p_start_win = {
                                    let v = view.clone();
                                    move |_ev: &_, _win: &mut _, app: &mut _| {
                                        v.update(app, |this, cx| {
                                            this.toggle_start_with_windows(cx)
                                        });
                                    }
                                };
                                let p_min_game = {
                                    let v = view.clone();
                                    move |_ev: &_, _win: &mut _, app: &mut _| {
                                        v.update(app, |this, cx| this.toggle_minimize_on_game(cx));
                                    }
                                };
                                let p_mica = {
                                    let v = view.clone();
                                    move |_ev: &_, _win: &mut _, app: &mut _| {
                                        v.update(app, |this, cx| this.toggle_mica(cx));
                                    }
                                };
                                let p_hide_tft = {
                                    let v = view.clone();
                                    move |_ev: &_, _win: &mut _, app: &mut _| {
                                        v.update(app, |this, cx| this.toggle_hide_tft(cx));
                                    }
                                };
                                let p_hide_saved = {
                                    let v = view.clone();
                                    move |_ev: &_, _win: &mut _, app: &mut _| {
                                        v.update(app, |this, cx| {
                                            this.toggle_hide_saved_players(cx)
                                        });
                                    }
                                };
                                let p_theme = {
                                    let v = view.clone();
                                    move |idx: usize, _win: &mut _, app: &mut _| {
                                        v.update(app, |this, cx| this.select_theme_index(idx, cx));
                                    }
                                };
                                let p_detect_path = {
                                    let v = view.clone();
                                    move |_ev: &_, _win: &mut _, app: &mut _| {
                                        v.update(app, |this, cx| {
                                            if let Some(path) = services::detect_lol_path() {
                                                log::info!(
                                                    "[Settings] 检测到 LOL 客户端路径: {}",
                                                    path
                                                );
                                                this.settings_view.client_path = path;
                                                cx.notify();
                                            } else {
                                                log::warn!(
                                                    "[Settings] 未能自动检测到 LOL 客户端路径"
                                                );
                                            }
                                        });
                                    }
                                };
                                let p_select_folder = {
                                    let v = view.clone();
                                    move |_ev: &_, _win: &mut _, app: &mut _| {
                                        v.update(app, |this, cx| {
                                            if let Some(path) = services::select_folder() {
                                                log::info!(
                                                    "[Settings] 手动选择 LOL 客户端路径: {}",
                                                    path
                                                );
                                                this.settings_view.client_path = path;
                                                cx.notify();
                                            }
                                        });
                                    }
                                };
                                let p_open_log = {
                                    let _v = view.clone();
                                    move |_ev: &_, _win: &mut _, _app: &mut _| {
                                        std::thread::spawn(|| match services::open_log_folder() {
                                            Ok(path) => log::info!(
                                                "[Settings] 已打开日志目录: {}",
                                                path.display()
                                            ),
                                            Err(e) => {
                                                log::warn!("[Settings] 打开日志目录失败: {}", e)
                                            }
                                        });
                                    }
                                };
                                let p_settings_clear_cache = {
                                    let v = view.clone();
                                    move |_ev: &_, _win: &mut _, app: &mut _| {
                                        v.update(app, |this, _cx| {
                                            let client_path =
                                                this.settings_view.client_path.clone();
                                            std::thread::spawn(move || {
                                                match services::clear_game_cache(Some(&client_path))
                                                {
                                                    Ok(msg) => log::info!("[Settings] {}", msg),
                                                    Err(e) => {
                                                        log::warn!("[Settings] 清理缓存失败: {}", e)
                                                    }
                                                }
                                            });
                                        });
                                    }
                                };

                                el.child(self.settings_view.render_panel(
                                    p_close_tray,
                                    p_start_win,
                                    p_min_game,
                                    p_mica,
                                    p_hide_tft,
                                    p_hide_saved,
                                    p_theme,
                                    p_detect_path,
                                    p_select_folder,
                                    p_open_log,
                                    p_settings_clear_cache,
                                ))
                            }),
                    )
                    // 右下角回到顶部悬浮按钮
                    .when(self.current_route != Route::Home, |this| {
                        let v = view_entity.clone();
                        let is_saved_page = self.current_route == Route::SavedPlayers;
                        let scroll_y = if is_saved_page {
                            -self.saved_players_view.scroll_handle.offset().y
                        } else {
                            -self.main_scroll_handle.offset().y
                        };
                        let show_btn = scroll_y > px(180.0);
                        this.when(show_btn, |panel| {
                            panel.child(
                                div()
                                    .absolute()
                                    .bottom(px(32.0))
                                    .right(px(32.0))
                                    .size(px(40.0))
                                    .rounded_full()
                                    .bg(Theme::CARD_BG)
                                    .border_1()
                                    .border_color(Theme::AURORA_CYAN)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .hover(|s| {
                                        s.bg(Theme::CARD_BG_HOVER)
                                            .border_color(Theme::ACCENT_PRIMARY)
                                    })
                                    .on_mouse_down(MouseButton::Left, move |_ev, _win, app| {
                                        v.update(app, |this, cx| {
                                            if this.current_route == Route::SavedPlayers {
                                                this.saved_players_view
                                                    .scroll_handle
                                                    .set_offset(Point::default());
                                            } else {
                                                this.main_scroll_handle
                                                    .set_offset(Point::default());
                                            }
                                            cx.notify();
                                        });
                                    })
                                    .child(
                                        div()
                                            .text_base()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(Theme::ACCENT_PRIMARY)
                                            .child("↑"),
                                    ),
                            )
                        })
                    }),
            )
    }
}

/// 主窗口默认内容区 1200 × 910（对照 Vue `src-tauri/tauri.conf.json`），居中并按可用工作区收敛
fn main_window_bounds(cx: &App) -> WindowBounds {
    let mut width = px(1200.0);
    let mut height = px(910.0);
    if let Some(display) = cx.primary_display() {
        let bounds = display.bounds();
        // 预留任务栏与阴影边距，保证窗口按钮始终可见
        let max_width = (bounds.size.width - px(80.0)).max(px(720.0));
        let max_height = (bounds.size.height - px(120.0)).max(px(480.0));
        width = width.min(max_width);
        height = height.min(max_height);
    }
    WindowBounds::centered(Size { width, height }, cx)
}

fn main() {
    // 显式安装全局默认 CryptoProvider，防止 rustls 0.23+ 多 provider 特性冲突
    let _ = rustls::crypto::ring::default_provider().install_default();

    // 0. 初始化全局 Tokio 异步运行时，供后台网络 I/O、WebSocket 与 LCU 轮询使用
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("初始化 Tokio 运行时失败");
    // 注册全局句柄：GPUI 的 background_executor 运行在 Windows 线程池上，
    // 线程池任务需要靠该句柄把网络 I/O 投递回运行时，否则会 panic（no reactor running）
    yuumi_core::runtime::set_tokio_handle(runtime.handle().clone());
    let _runtime_guard = runtime.enter();

    // 1. Windows 单实例互斥量检测（Named Mutex）
    let mutex_name = "Global\\Yuumi_GPUI_SingleInstance_Mutex";
    let _instance_guard = match platform::acquire_single_instance(mutex_name) {
        Ok(guard) => Some(guard),
        Err(platform::SingleInstanceError::AlreadyRunning) => {
            eprintln!("检测到 Yuumi 已在运行中，激活现有窗口...");
            if let Some(hwnd) = platform::find_main_window_hwnd() {
                platform::restore_and_foreground(hwnd);
            }
            return;
        }
        Err(e) => {
            eprintln!("创建单实例互斥量警告: {:?}", e);
            None
        }
    };

    // 2. 初始化平台日志系统（输出到标准错误与调试输出）
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    log::info!("Yuumi GPUI 原生 Direct3D 11/DirectWrite 渲染宿主正在启动...");

    // 3. 创建 GPUI 原生应用程序宿主（挂载内嵌图标与 Logo 资源）
    let app = Application::new().with_assets(assets::Assets);

    app.run(move |cx| {
        // 创建主应用窗口（无边框，1200×910 居中，可调整大小）
        let window_options = WindowOptions {
            window_bounds: Some(main_window_bounds(cx)),
            window_min_size: Some(Size {
                width: px(880.0),
                height: px(600.0),
            }),
            // 彻底禁用操作系统传统标题栏（appears_transparent=true 仍隐藏系统标题栏）；
            // title 用于 Win32 侧精确定位主窗口：单实例二次启动时激活已有窗口依赖它
            titlebar: Some(TitlebarOptions {
                title: Some(MAIN_WINDOW_TITLE.into()),
                appears_transparent: true,
                traffic_light_position: None,
            }),
            focus: true,
            is_movable: true,
            ..Default::default()
        };

        let window_handle = cx
            .open_window(window_options, |_window, cx| cx.new(MainWindow::new))
            .expect("创建 Yuumi GPUI 主窗口失败");

        // 异步应用 Windows 11 Mica / DwmExtendFrameIntoClientArea 亚克力极光特效与圆角
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(60));
            if let Some(hwnd) = platform::find_main_window_hwnd() {
                platform::apply_mica_or_acrylic(hwnd);
                platform::set_window_rounded_corners(hwnd);
            } else {
                log::warn!("[Platform] 未找到主窗口 HWND，跳过 Mica/圆角设置");
            }
        });

        let _ = window_handle;
    });
}
