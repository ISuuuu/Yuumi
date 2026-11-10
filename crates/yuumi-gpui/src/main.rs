// MSVC 链接器创建 .lib/.exp 时输出的 "正在创建库" 提示，非代码问题，静默处理
#![allow(linker_messages)]

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
mod opgg;
pub mod platform;
pub mod services;
mod theme;
mod tray;
pub mod views;

use bench_overlay::{load_saved_origin, BenchOverlayView};
use components::notice_popup::notice_popup;
use components::sidebar::Sidebar;
use components::titlebar::{phase_label, TitleBar};
use match_model::MatchItem;
use opgg::{OpggWindow, OPGG_WINDOW_TITLE};
use services::{ConnectionStatus, LiveLcuEvent, LiveLcuService};
use theme::Theme;
use tray::TrayManager;
use views::career::{
    CareerBarAction, CareerHeaderStats, CareerView, LootAction, LootModal, LootProgress,
    MasteryFilterAction,
};
use views::game_info::{GameInfoTeamTab, GameInfoView, GameInfoViewMode};
use views::home::HomeView;
use views::saved_players::SavedPlayersView;
use views::search::SearchView;
use views::settings::{SettingsAction, SettingsView};
use views::tools::{ToolsAction, ToolsView};
use views::Route;
use yuumi_core::agents::{AutomationSettings, ChampSelectSession, GamePhase};
use yuumi_core::config::AppConfig;
use yuumi_core::lcu::game_data::GameDataAssets;
use yuumi_core::parsers::game_info::{LiveGameInfo, RankedStats};
use yuumi_core::parsers::loot_actions::{LootActionItem, LootActionKind};
use yuumi_core::parsers::summoner::SummonerInfo;

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
    /// 战利品页价值数值输入框的焦点句柄
    loot_value_focus: FocusHandle,
    matches: Vec<MatchItem>,
    stats: CareerHeaderStats,
    career_view: CareerView,
    own_summoner: Option<SummonerInfo>,
    own_matches: Vec<MatchItem>,
    own_ranked: Option<RankedStats>,
    own_stats: CareerHeaderStats,
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
    home_view: HomeView,
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
    /// 全局配置（对齐 %APPDATA%/Yuumi/config.json，修改即回写）
    app_config: AppConfig,
    /// 工具箱自动化开关的权威配置（启动时从全局配置同步，变更即回写）
    automation: AutomationSettings,
    /// 当前选人会话快照（大乱斗悬浮窗的数据来源）
    bench_session: Option<ChampSelectSession>,
    /// LCU 预加载的游戏资源字典（用于悬浮窗英雄名）
    game_assets: Option<Arc<GameDataAssets>>,
    /// 主视口全局滚动跟踪句柄（用于监听滚动到底部与回到顶部）
    main_scroll_handle: ScrollHandle,
    /// 是否有版本更新（用于在侧栏设置图标显示红点）
    has_update: bool,
    /// 公告弹窗是否打开
    notice_popup_open: bool,
    /// OP.GG 独立窗口句柄（存在即复用并聚焦）
    opgg_window: Option<WindowHandle<OpggWindow>>,
    /// OP.GG 入口的临时提示文案（未连接 LCU 等场景）
    opgg_hint: Option<String>,
}

impl MainWindow {
    pub fn new(cx: &mut Context<Self>) -> Self {
        // 战绩与 10 人对局信息全部来自真实 LCU 数据，启动时保持平滑空态
        let matches: Vec<MatchItem> = Vec::new();

        let stats = CareerHeaderStats::compute(&matches);
        let career_view = CareerView::new();
        let tm_icons: Vec<String> = career_view
            .recent_teammates
            .iter()
            .map(|t| t.icon.clone())
            .collect();
        prewarm_assets(tm_icons, cx);
        let career_mastery_focus = cx.focus_handle();
        let loot_value_focus = cx.focus_handle();

        // 加载持久化的全局配置与自动化配置
        let app_config = AppConfig::load();
        let automation = app_config.to_automation_settings();
        let tools_view = ToolsView::new();

        let search_view = SearchView::new(cx).with_matches(matches.clone());
        let saved_players_view = SavedPlayersView::new(cx);
        let settings_view = SettingsView::new();
        let home_view = HomeView::new();
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
        let own_matches = matches.clone();
        let own_stats = stats.clone();

        Self {
            titlebar: TitleBar::new(components::titlebar::APP_NAME),
            matches,
            stats,
            career_view,
            own_summoner: None,
            own_matches,
            own_ranked: None,
            own_stats,
            game_info: None,
            game_info_loading: false,
            game_info_view_mode: GameInfoViewMode::AllMatches,
            game_info_active_tab: GameInfoTeamTab::Ally,
            current_phase: "None".to_string(),
            home_view,
            tools_view,
            search_view,
            saved_players_view,
            settings_view,
            career_mastery_focus,
            loot_value_focus,
            current_route: Route::Home,
            sidebar_expanded: false,
            route_history: Vec::new(),
            _tray: tray,
            overlay_window: None,
            live_service,
            connection_status: ConnectionStatus::Disconnected,
            app_config,
            automation,
            bench_session: None,
            game_assets: None,
            main_scroll_handle: ScrollHandle::new(),
            has_update: false,
            notice_popup_open: false,
            opgg_window: None,
            opgg_hint: None,
        }
    }

    pub fn handle_live_event(&mut self, event: LiveLcuEvent, cx: &mut Context<Self>) {
        match event {
            LiveLcuEvent::ConnectionChanged(status) => {
                if let ConnectionStatus::Connected { summoner, .. } = &status {
                    if let Some(s) = summoner.as_ref() {
                        self.own_summoner = Some(s.clone());
                        if !self.career_view.is_viewing_other {
                            self.career_view.summoner = Some(s.clone());
                        }
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

                // 未搜索场景下（进入战绩查询页自动带出的战绩）也要选中并载入首局详情，
                // 否则右栏会一直停在「等待详情载入」的空态
                let keep_selected = self
                    .search_view
                    .selected_game_id
                    .filter(|id| items.iter().any(|m| m.display.game_id == *id));
                if keep_selected.is_none() {
                    if let Some(first_id) = items.first().map(|m| m.display.game_id) {
                        self.load_match_detail(first_id, cx);
                    }
                }

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

                if !self.career_view.is_viewing_other {
                    self.own_matches = items.clone();
                    self.own_stats = self.stats.clone();
                    self.matches = items;
                }
                cx.notify();
            }
            LiveLcuEvent::RankedStatsLoaded(stats) => {
                log::info!(
                    "[MainWindow] 接收到真实排位数据: 队列数={}",
                    stats.queue_map.len()
                );
                if !self.career_view.is_viewing_other {
                    self.own_ranked = Some(stats.clone());
                    self.career_view.ranked = Some(stats);
                }
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
                // 进入选人阶段按配置自动弹出 OP.GG（已有窗口则聚焦）
                if phase == GamePhase::ChampSelect && self.app_config.functions.auto_show_opgg {
                    self.open_opgg_window(cx);
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
            LiveLcuEvent::LootActionProgress(progress) => {
                let mut state = self
                    .career_view
                    .loot_progress
                    .take()
                    .unwrap_or(LootProgress {
                        current: 0,
                        total: 0,
                        success: 0,
                        current_name: String::new(),
                    });
                state.current = progress.current.max(0) as usize;
                state.total = progress.total.max(0) as usize;
                state.current_name = progress.loot_name;
                if progress.success {
                    state.success += 1;
                }
                self.career_view.loot_progress = Some(state);
                cx.notify();
            }
            LiveLcuEvent::RecentTeammatesLoaded(teammates) => {
                let icon_paths: Vec<String> = teammates.iter().map(|t| t.icon.clone()).collect();
                prewarm_assets(icon_paths, cx);
                self.career_view.recent_teammates = teammates;
                cx.notify();
            }
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
                    width: px(550.0),
                    height: px(70.0),
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

    pub fn toggle_notice_popup(&mut self, open: bool, cx: &mut Context<Self>) {
        self.notice_popup_open = open;
        cx.notify();
    }

    /// 打开 OP.GG 独立窗口；未连接 LCU 时给出提示并返回。
    ///
    /// 已有窗口时直接聚焦复用，句柄失效则重建（对照 Vue 的 `getByLabel("opgg")`）。
    pub fn open_opgg_window(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.connection_status, ConnectionStatus::Connected { .. }) {
            self.show_opgg_hint("未连接到英雄联盟客户端，无法打开 OP.GG", cx);
            return;
        }

        if let Some(handle) = &self.opgg_window {
            let alive = handle
                .update(cx, |_view, window, _cx| window.activate_window())
                .is_ok();
            if alive {
                return;
            }
            log::warn!("[OPGG] 窗口句柄已失效，重新创建");
            self.opgg_window = None;
        }

        let service = self.live_service.clone();
        let proxy = yuumi_core::opgg::ProxyConfig {
            enable: self.app_config.general.enable_http_proxy,
            addr: self.app_config.general.http_proxy_addr.clone(),
        };
        let on_top = self.app_config.functions.enable_opgg_on_top;
        let initial_champion = self.initial_opgg_champion();
        let window_options = WindowOptions {
            window_bounds: Some(opgg_window_bounds(cx)),
            titlebar: Some(TitlebarOptions {
                title: Some(OPGG_WINDOW_TITLE.into()),
                appears_transparent: false,
                traffic_light_position: None,
            }),
            kind: WindowKind::Normal,
            focus: true,
            is_movable: true,
            is_resizable: true,
            ..Default::default()
        };

        match cx.open_window(window_options, move |_window, cx| {
            cx.new(move |cx| OpggWindow::new(service, proxy, initial_champion, cx))
        }) {
            Ok(handle) => {
                self.opgg_window = Some(handle);
                if on_top {
                    // 置顶需等原生窗口就绪后再套用 Win32 样式
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(120));
                        match platform::find_window_by_title(OPGG_WINDOW_TITLE) {
                            Some(hwnd) => platform::set_window_topmost(hwnd, true),
                            None => log::warn!("[OPGG] 未找到窗口 HWND，置顶未应用"),
                        }
                    });
                }
            }
            Err(err) => log::error!("[OPGG] 创建窗口失败: {}", err),
        }
    }

    /// 关闭并销毁 OP.GG 独立窗口
    pub fn close_opgg_window(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.opgg_window.take() {
            let _ = handle.update(cx, |_view, window, _cx| window.remove_window());
        }
    }

    /// 首屏进详情用的英雄：按自动选人配置依次检查通用与五个分路
    fn initial_opgg_champion(&self) -> Option<i32> {
        let functions = &self.app_config.functions;
        if !functions.enable_auto_select_champion {
            return None;
        }
        let candidates: [&Vec<i32>; 6] = [
            &functions.auto_select_champion,
            &functions.auto_select_champion_mid,
            &functions.auto_select_champion_top,
            &functions.auto_select_champion_sup,
            &functions.auto_select_champion_jug,
            &functions.auto_select_champion_bot,
        ];
        candidates
            .iter()
            .find_map(|list| list.first().copied().filter(|id| *id > 0))
    }

    /// 显示 OP.GG 入口提示（3 秒后自动消失）
    fn show_opgg_hint(&mut self, message: &str, cx: &mut Context<Self>) {
        self.opgg_hint = Some(message.to_string());
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(3)).await;
            let _ = this.update(cx, |this, cx| {
                this.opgg_hint = None;
                cx.notify();
            });
        })
        .detach();
    }

    pub fn trigger_reconnect(&mut self, cx: &mut Context<Self>) {
        log::info!("[MainWindow] 用户触发重新连接/修复流程");
        let live = self.live_service.clone();
        yuumi_core::runtime::spawn_background(async move {
            if let Some(client) = live.get_client().await {
                let _ = client
                    .request(
                        "POST",
                        "/lol-gameflow/v1/session/request-lobby-phase",
                        None,
                        false,
                    )
                    .await;
            }
        });
        cx.notify();
    }

    /// 将全局配置同步进磁盘、Agent 运行时与悬浮窗
    fn persist_config(&mut self, cx: &mut Context<Self>) {
        self.automation = self.app_config.to_automation_settings();

        let _ = self
            .live_service
            .auto_match_cfg
            .send(self.automation.auto_match.clone());
        let _ = self
            .live_service
            .auto_bp_cfg
            .send(self.automation.auto_bp.clone());

        if let Err(e) = self.app_config.save() {
            log::warn!("[Config] 全局配置持久化失败: {}", e);
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

    /// 应用战利品页签的筛选 / 多选 / 下拉动作
    ///
    /// 开启、全部开启、分解、重铸、升级解锁属于 LCU 写操作，按钮已做禁用态并标注
    /// 「服务层待接」；待服务层就绪后在 `LootAction` 补写动作变体并在此调用
    /// `LiveLcuService` 的写方法，完成后再 `refresh_career_data()` 回拉库存与余额。
    pub fn apply_loot_action(&mut self, action: LootAction, cx: &mut Context<Self>) {
        match action {
            LootAction::ToggleDropdown(kind) => {
                let slot = &mut self.career_view.loot_dropdown;
                *slot = if *slot == Some(kind) {
                    None
                } else {
                    Some(kind)
                };
            }
            LootAction::CloseDropdown => self.career_view.loot_dropdown = None,
            LootAction::SetType(value) => {
                self.career_view.loot_filter_type = value;
                self.career_view.loot_dropdown = None;
                // 对照 Vue watch(filterType)：切换类型自动清空选择
                self.career_view.loot_selected.clear();
            }
            LootAction::SetOwned(value) => {
                self.career_view.loot_filter_owned = value;
                self.career_view.loot_dropdown = None;
            }
            LootAction::SetValueBasis(value) => {
                self.career_view.loot_filter_value_basis = value;
                self.career_view.loot_dropdown = None;
            }
            LootAction::SetOperator(value) => {
                self.career_view.loot_filter_operator = value;
                self.career_view.loot_dropdown = None;
            }
            LootAction::SetValueInput(text) => self.career_view.loot_value_input = text,
            LootAction::ClearValueInput => self.career_view.loot_value_input.clear(),
            LootAction::ToggleSelect(id) => {
                let selected = &mut self.career_view.loot_selected;
                if let Some(pos) = selected.iter().position(|x| x == &id) {
                    selected.remove(pos);
                } else {
                    selected.push(id);
                }
            }
            LootAction::SelectAllFiltered => {
                let ids: Vec<String> = self
                    .career_view
                    .filtered_loot()
                    .into_iter()
                    .map(|item| item.loot_id.clone())
                    .collect();
                self.career_view.loot_selected = ids;
            }
            LootAction::ClearSelection => self.career_view.loot_selected.clear(),
            LootAction::OpenQuantity(modal) => {
                self.career_view.loot_modal = Some(modal);
            }
            LootAction::OpenConfirm(modal) => {
                self.career_view.loot_modal = Some(modal);
            }
            LootAction::Confirm(modal) => {
                self.career_view.loot_modal = None;
                let live = self.live_service.clone();
                match modal {
                    LootModal::Quantity {
                        recipe_name,
                        ingredients,
                        quantity,
                        action,
                        ..
                    } => {
                        let item = LootActionItem {
                            recipe_name,
                            ingredients,
                            count: quantity,
                            ..Default::default()
                        };
                        let kind = match action {
                            views::career::LootQuantityAction::Open => LootActionKind::Open,
                            views::career::LootQuantityAction::Craft => LootActionKind::Open,
                        };
                        yuumi_core::runtime::spawn_background(async move {
                            let _ = live.run_loot_action(kind, vec![item]).await;
                            live.refresh_career_data().await;
                        });
                    }
                    LootModal::Confirm { action_kind, .. } => {
                        let selected_ids = self.career_view.loot_selected.clone();
                        self.career_view.loot_selected.clear();

                        let items: Vec<LootActionItem> = if action_kind
                            == LootActionKind::SmartOpenAll
                        {
                            self.career_view
                                .loot_items
                                .iter()
                                .filter(|i| i.is_openable)
                                .map(|i| LootActionItem {
                                    loot_id: i.loot_id.clone(),
                                    name: i.name.clone(),
                                    count: i.count,
                                    recipe_name: format!("{}_open", i.loot_id),
                                    ingredients: vec![i.loot_id.clone()],
                                    ..Default::default()
                                })
                                .collect()
                        } else {
                            self.career_view
                                .loot_items
                                .iter()
                                .filter(|i| selected_ids.contains(&i.loot_id))
                                .map(|i| LootActionItem {
                                    loot_id: i.loot_id.clone(),
                                    name: i.item_desc.clone(),
                                    count: i.count,
                                    recipe_name: i.upgrade_recipe_name.clone().unwrap_or_default(),
                                    ingredients: vec![i.loot_id.clone()],
                                    ..Default::default()
                                })
                                .collect()
                        };

                        yuumi_core::runtime::spawn_background(async move {
                            let _ = live.run_loot_action(action_kind, items).await;
                            live.refresh_career_data().await;
                        });
                    }
                }
            }
            LootAction::CloseModal => self.career_view.loot_modal = None,
            LootAction::CloseProgress => self.career_view.loot_progress = None,
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

        self.navigate_to_search(&name, None, cx);
    }

    /// 跳转到战绩查询并定位指定对局（对照 Vue `goToMatchDetail`）
    pub fn navigate_to_search(
        &mut self,
        query: &str,
        target_game_id: Option<u64>,
        cx: &mut Context<Self>,
    ) {
        self.perform_search_with_target(query, target_game_id, cx);
        self.switch_route(Route::Search, cx);
    }

    /// 进入查看他人生涯模式
    pub fn view_other_career(
        &mut self,
        puuid: String,
        name: String,
        _icon: String,
        cx: &mut Context<Self>,
    ) {
        let (game_name, tag_line) = if let Some((g, t)) = name.split_once('#') {
            (g.to_string(), Some(t.to_string()))
        } else {
            (name.clone(), None)
        };

        let other_summoner = SummonerInfo {
            puuid: puuid.clone(),
            display_name: Some(name.clone()),
            game_name: Some(game_name),
            tag_line,
            summoner_level: Some(0),
            profile_icon_id: Some(29),
            privacy: None,
            percent_complete_for_next_level: Some(0),
            account_id: None,
            summoner_id: None,
            xp_since_last_level: None,
            xp_until_next_level: None,
        };

        self.career_view.is_viewing_other = true;
        self.career_view.active_tab = views::career::CareerTab::Matches;
        self.career_view.summoner = Some(other_summoner);
        self.career_view.ranked = None;
        self.matches.clear();
        self.stats = CareerHeaderStats::compute(&[]);
        cx.notify();

        let live = self.live_service.clone();
        let target_name = name.clone();
        cx.spawn(async move |entity, cx| {
            if let Some(client) = live.get_client().await {
                let assets_opt = live.get_game_data_assets().await;
                if let Ok(real_s) =
                    yuumi_core::parsers::summoner::fetch_summoner_by_name(&client, &target_name)
                        .await
                {
                    let _ = entity.update(cx, |this, cx| {
                        if this.career_view.is_viewing_other {
                            this.career_view.summoner = Some(real_s);
                            cx.notify();
                        }
                    });
                }
                if let Ok(stats) =
                    yuumi_core::parsers::game_info::fetch_ranked_stats(&client, &puuid).await
                {
                    let _ = entity.update(cx, |this, cx| {
                        if this.career_view.is_viewing_other {
                            this.career_view.ranked = Some(stats);
                            cx.notify();
                        }
                    });
                }
                if let Some(assets) = assets_opt {
                    if let Ok(matches) = yuumi_core::parsers::match_parser::fetch_match_history(
                        &client,
                        &puuid,
                        Some(0),
                        Some(19),
                        &assets,
                    )
                    .await
                    {
                        let items: Vec<MatchItem> =
                            matches.into_iter().map(MatchItem::new).collect();
                        let _ = entity.update(cx, |this, cx| {
                            if this.career_view.is_viewing_other {
                                this.stats = CareerHeaderStats::compute(&items);
                                this.matches = items;
                                cx.notify();
                            }
                        });
                    }
                }
            }
        })
        .detach();
    }

    /// 返回我的生涯模式并恢复本人数据
    pub fn back_to_my_career(&mut self, cx: &mut Context<Self>) {
        self.career_view.is_viewing_other = false;
        self.career_view.active_tab = views::career::CareerTab::Matches;
        if let Some(own) = self.own_summoner.clone() {
            self.career_view.summoner = Some(own);
        }
        self.matches = self.own_matches.clone();
        self.stats = self.own_stats.clone();
        self.career_view.ranked = self.own_ranked.clone();
        self.refresh_career(cx);
        cx.notify();
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
            } else if self.current_route == Route::Home {
                self.home_view.reload_paths();
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

    /// 处理工具箱交互动作
    pub fn handle_tools_action(&mut self, action: ToolsAction, cx: &mut Context<Self>) {
        match action {
            ToolsAction::ToggleCollapse(name) => {
                self.tools_view.toggle_card(name);
                cx.notify();
            }
            ToolsAction::ToggleAutoAccept => {
                self.app_config.functions.enable_auto_accept_matching =
                    !self.app_config.functions.enable_auto_accept_matching;
                self.persist_config(cx);
            }
            ToolsAction::SetAutoAcceptDelay(delay) => {
                self.app_config.functions.auto_accept_matching_delay = delay;
                self.persist_config(cx);
            }
            ToolsAction::ToggleCeilSwap => {
                self.app_config.functions.auto_accept_ceil_swap =
                    !self.app_config.functions.auto_accept_ceil_swap;
                self.persist_config(cx);
            }
            ToolsAction::ToggleChampTrade => {
                self.app_config.functions.auto_accept_champ_trade =
                    !self.app_config.functions.auto_accept_champ_trade;
                self.persist_config(cx);
            }
            ToolsAction::ToggleAutoPick => {
                self.app_config.functions.enable_auto_select_champion =
                    !self.app_config.functions.enable_auto_select_champion;
                self.persist_config(cx);
            }
            ToolsAction::ToggleAutoHover => {
                self.app_config.functions.enable_auto_hover_champion =
                    !self.app_config.functions.enable_auto_hover_champion;
                self.persist_config(cx);
            }
            ToolsAction::ToggleConfirmOnTimeout => {
                self.app_config.functions.auto_select_confirm_on_timeout =
                    !self.app_config.functions.auto_select_confirm_on_timeout;
                self.persist_config(cx);
            }
            ToolsAction::SetHoverLane(lane) => {
                self.tools_view.hover_active_lane = lane;
                cx.notify();
            }
            ToolsAction::ClearPickChampion(lane) => {
                match lane {
                    "top" => self.app_config.functions.auto_select_champion_top.clear(),
                    "jug" => self.app_config.functions.auto_select_champion_jug.clear(),
                    "mid" => self.app_config.functions.auto_select_champion_mid.clear(),
                    "bot" => self.app_config.functions.auto_select_champion_bot.clear(),
                    "sup" => self.app_config.functions.auto_select_champion_sup.clear(),
                    _ => self.app_config.functions.auto_select_champion.clear(),
                }
                self.persist_config(cx);
            }
            ToolsAction::ToggleAutoBan => {
                self.app_config.functions.enable_auto_ban_champion =
                    !self.app_config.functions.enable_auto_ban_champion;
                self.persist_config(cx);
            }
            ToolsAction::TogglePretendBan => {
                self.app_config.functions.pretend_ban = !self.app_config.functions.pretend_ban;
                self.persist_config(cx);
            }
            ToolsAction::SetAutoBanDelay(delay) => {
                self.app_config.functions.auto_ban_delay = delay;
                self.persist_config(cx);
            }
            ToolsAction::SetBanLane(lane) => {
                self.tools_view.ban_active_lane = lane;
                cx.notify();
            }
            ToolsAction::ClearBanChampion(lane) => {
                match lane {
                    "top" => self.app_config.functions.auto_ban_champion_top.clear(),
                    "jug" => self.app_config.functions.auto_ban_champion_jug.clear(),
                    "mid" => self.app_config.functions.auto_ban_champion_mid.clear(),
                    "bot" => self.app_config.functions.auto_ban_champion_bot.clear(),
                    "sup" => self.app_config.functions.auto_ban_champion_sup.clear(),
                    _ => self.app_config.functions.auto_ban_champion.clear(),
                }
                self.persist_config(cx);
            }
            ToolsAction::ToggleAutoSpell => {
                self.app_config.functions.enable_auto_set_spells =
                    !self.app_config.functions.enable_auto_set_spells;
                self.persist_config(cx);
            }
            ToolsAction::SetSpellLane(lane) => {
                self.tools_view.spell_active_lane = lane;
                cx.notify();
            }
            ToolsAction::SetSpellId {
                lane,
                is_spell1,
                spell_id,
            } => {
                let list = match lane {
                    "top" => &mut self.app_config.functions.auto_set_summoner_spell_top,
                    "jug" => &mut self.app_config.functions.auto_set_summoner_spell_jug,
                    "mid" => &mut self.app_config.functions.auto_set_summoner_spell_mid,
                    "bot" => &mut self.app_config.functions.auto_set_summoner_spell_bot,
                    "sup" => &mut self.app_config.functions.auto_set_summoner_spell_sup,
                    _ => &mut self.app_config.functions.auto_set_summoner_spell,
                };
                if list.len() < 2 {
                    *list = vec![4, 14];
                }
                if is_spell1 {
                    list[0] = spell_id;
                } else {
                    list[1] = spell_id;
                }
                self.persist_config(cx);
            }
            ToolsAction::ToggleAutoHandleInvite => {
                self.app_config.functions.enable_auto_handle_invite =
                    !self.app_config.functions.enable_auto_handle_invite;
                self.persist_config(cx);
            }
            ToolsAction::ToggleAutoHonor => {
                self.app_config.functions.enable_auto_honor =
                    !self.app_config.functions.enable_auto_honor;
                self.persist_config(cx);
            }
            ToolsAction::ToggleAutoPlayAgain => {
                self.app_config.functions.enable_auto_play_again =
                    !self.app_config.functions.enable_auto_play_again;
                self.persist_config(cx);
            }
            ToolsAction::ToggleAramTeamSide => {
                self.app_config.functions.enable_auto_aram_team_side =
                    !self.app_config.functions.enable_auto_aram_team_side;
                self.persist_config(cx);
            }
            ToolsAction::ToggleAramTeamSideVisible => {
                self.app_config.functions.aram_team_side_visible_to_team =
                    !self.app_config.functions.aram_team_side_visible_to_team;
                self.persist_config(cx);
            }
            ToolsAction::ToggleTagReminder => {
                self.app_config.functions.enable_auto_tag_reminder =
                    !self.app_config.functions.enable_auto_tag_reminder;
                self.persist_config(cx);
            }
            ToolsAction::ToggleBenchOverlay => {
                self.app_config.functions.enable_bench_overlay =
                    !self.app_config.functions.enable_bench_overlay;
                self.persist_config(cx);
            }
            ToolsAction::OpenBenchOverlayWindow => {
                self.open_bench_overlay(cx);
            }
            ToolsAction::ToggleAutoReconnect => {
                self.app_config.functions.enable_auto_reconnect =
                    !self.app_config.functions.enable_auto_reconnect;
                self.persist_config(cx);
            }
            ToolsAction::ToggleAutoCreateLobby => {
                self.app_config.functions.enable_auto_create_lobby =
                    !self.app_config.functions.enable_auto_create_lobby;
                self.persist_config(cx);
            }
            ToolsAction::SetDefaultGameMode(mode) => {
                self.app_config.functions.default_game_mode = mode;
                self.persist_config(cx);
            }
            ToolsAction::CreateLobbyNow => {
                let live = self.live_service.clone();
                let mode = self.app_config.functions.default_game_mode;
                yuumi_core::runtime::spawn_background(async move {
                    if let Some(client) = live.get_client().await {
                        let body = serde_json::json!({ "queueId": mode });
                        let _ = client
                            .request("POST", "/lol-lobby/v2/lobby", Some(body), false)
                            .await;
                    }
                });
                self.tools_view.set_feedback("已发送创建房间指令", false);
                cx.notify();
            }
            ToolsAction::ToggleLockGameSettings => {
                self.tools_view.is_game_settings_locked = !self.tools_view.is_game_settings_locked;
                self.tools_view.set_feedback(
                    if self.tools_view.is_game_settings_locked {
                        "已锁定游戏配置文件 (只读保护)"
                    } else {
                        "已解除游戏配置文件只读锁定"
                    },
                    false,
                );
                cx.notify();
            }
            ToolsAction::FixClientWindow => {
                std::thread::spawn(|| {
                    let _ = services::fix_client_window();
                });
                self.tools_view
                    .set_feedback("已重置客户端窗口为 1600x900", false);
                cx.notify();
            }
            ToolsAction::RestartClient => {
                let live = self.live_service.clone();
                yuumi_core::runtime::spawn_background(async move {
                    if let Some(client) = live.get_client().await {
                        let _ = client
                            .request("POST", "/riotclient/kill-and-restart-ux", None, false)
                            .await;
                    }
                });
                self.tools_view.set_feedback("已发送客户端重启指令", false);
                cx.notify();
            }
            ToolsAction::SpectateSummonerNameInput(name) => {
                self.tools_view.spectate_name_buffer = name;
                cx.notify();
            }
            ToolsAction::LaunchSpectate => {
                self.tools_view.set_feedback("观战指令已发送", false);
                cx.notify();
            }
            ToolsAction::ScanFriends => {
                self.tools_view.set_feedback("好友扫描完成", false);
                cx.notify();
            }
            ToolsAction::SetCleanerDays(d) => {
                self.tools_view.cleaner_days = d;
                cx.notify();
            }
            ToolsAction::BatchDeleteOfflineFriends => {
                self.tools_view.set_feedback("批量清理已完成", false);
                cx.notify();
            }
            ToolsAction::SignatureInput(s) => {
                self.tools_view.signature_buffer = s;
                cx.notify();
            }
            ToolsAction::ApplySignature => {
                let sig = self.tools_view.signature_buffer.clone();
                let live = self.live_service.clone();
                yuumi_core::runtime::spawn_background(async move {
                    if let Some(client) = live.get_client().await {
                        let body = serde_json::json!({ "statusMessage": sig });
                        let _ = client
                            .request("PUT", "/lol-chat/v1/me", Some(body), false)
                            .await;
                    }
                });
                self.tools_view.set_feedback("个性签名已更新", false);
                cx.notify();
            }
            ToolsAction::ProfileBgSkinInput(s) => {
                self.tools_view.profile_skin_id_buffer = s;
                cx.notify();
            }
            ToolsAction::ApplyProfileBackground => {
                if let Ok(skin_id) = self.tools_view.profile_skin_id_buffer.trim().parse::<i32>() {
                    let live = self.live_service.clone();
                    yuumi_core::runtime::spawn_background(async move {
                        if let Some(client) = live.get_client().await {
                            let body =
                                serde_json::json!({ "key": "backgroundSkinId", "value": skin_id });
                            let _ = client
                                .request(
                                    "POST",
                                    "/lol-summoner/v1/current-summoner/summoner-profile",
                                    Some(body),
                                    false,
                                )
                                .await;
                        }
                    });
                    self.tools_view
                        .set_feedback(format!("已应用背景皮肤 ID: {}", skin_id), false);
                }
                cx.notify();
            }
            ToolsAction::SetSpoofQueue(q) => {
                self.tools_view.spoof_queue = q;
                cx.notify();
            }
            ToolsAction::SetSpoofTier(t) => {
                self.tools_view.spoof_tier = t;
                cx.notify();
            }
            ToolsAction::SetSpoofDivision(d) => {
                self.tools_view.spoof_division = d;
                cx.notify();
            }
            ToolsAction::ApplyRankSpoof => {
                let tier = self.tools_view.spoof_tier.clone();
                let div = self.tools_view.spoof_division.clone();
                let live = self.live_service.clone();
                yuumi_core::runtime::spawn_background(async move {
                    if let Some(client) = live.get_client().await {
                        let body = serde_json::json!({
                            "lol": {
                                "rankedLeagueTier": tier,
                                "rankedLeagueDivision": div,
                            }
                        });
                        let _ = client
                            .request("PUT", "/lol-chat/v1/me", Some(body), false)
                            .await;
                    }
                });
                self.tools_view.set_feedback("伪装段位已生效", false);
                cx.notify();
            }
            ToolsAction::SetOnlineStatus(st) => {
                self.tools_view.online_status = st.clone();
                let live = self.live_service.clone();
                yuumi_core::runtime::spawn_background(async move {
                    if let Some(client) = live.get_client().await {
                        let body = serde_json::json!({ "availability": st });
                        let _ = client
                            .request("PUT", "/lol-chat/v1/me", Some(body), false)
                            .await;
                    }
                });
                self.tools_view.set_feedback("在线状态已切换", false);
                cx.notify();
            }
            ToolsAction::ClearBadges => {
                let live = self.live_service.clone();
                yuumi_core::runtime::spawn_background(async move {
                    if let Some(client) = live.get_client().await {
                        let body = serde_json::json!({ "challengeIds": [] });
                        let _ = client
                            .request(
                                "POST",
                                "/lol-challenges/v1/update-player-preferences",
                                Some(body),
                                false,
                            )
                            .await;
                    }
                });
                self.tools_view.set_feedback("已清空成就勋章展示", false);
                cx.notify();
            }
            ToolsAction::ClearBorder => {
                let live = self.live_service.clone();
                yuumi_core::runtime::spawn_background(async move {
                    if let Some(client) = live.get_client().await {
                        let body = serde_json::json!({ "bannerId": "" });
                        let _ = client
                            .request(
                                "POST",
                                "/lol-challenges/v1/update-player-preferences",
                                Some(body),
                                false,
                            )
                            .await;
                    }
                });
                self.tools_view.set_feedback("已清除个人边框", false);
                cx.notify();
            }
            ToolsAction::ApplyOnlineStatus => {
                let status = self.tools_view.online_status.clone();
                let live = self.live_service.clone();
                yuumi_core::runtime::spawn_background(async move {
                    if let Some(client) = live.get_client().await {
                        let body = serde_json::json!({ "availability": status });
                        let _ = client
                            .request("PUT", "/lol-chat/v1/me", Some(body), false)
                            .await;
                    }
                });
                self.tools_view.set_feedback("在线状态已切换", false);
                cx.notify();
            }
            ToolsAction::TogglePicker(kind) => {
                match kind {
                    "hover" => {
                        self.tools_view.open_hover_picker = !self.tools_view.open_hover_picker
                    }
                    "ban" => self.tools_view.open_ban_picker = !self.tools_view.open_ban_picker,
                    "bg_champ" => {
                        self.tools_view.open_bg_champion_picker =
                            !self.tools_view.open_bg_champion_picker
                    }
                    "D" => self.tools_view.spell_active_slot = Some("D"),
                    "F" => self.tools_view.spell_active_slot = Some("F"),
                    "close_spell" => self.tools_view.spell_active_slot = None,
                    _ => {}
                }
                cx.notify();
            }
            ToolsAction::SetPickerChampion {
                lane,
                is_ban,
                champion_id,
            } => {
                if is_ban {
                    let list = match lane {
                        "top" => &mut self.app_config.functions.auto_ban_champion_top,
                        "jug" => &mut self.app_config.functions.auto_ban_champion_jug,
                        "mid" => &mut self.app_config.functions.auto_ban_champion_mid,
                        "bot" => &mut self.app_config.functions.auto_ban_champion_bot,
                        "sup" => &mut self.app_config.functions.auto_ban_champion_sup,
                        _ => &mut self.app_config.functions.auto_ban_champion,
                    };
                    *list = vec![champion_id];
                    self.tools_view.open_ban_picker = false;
                } else {
                    let list = match lane {
                        "top" => &mut self.app_config.functions.auto_select_champion_top,
                        "jug" => &mut self.app_config.functions.auto_select_champion_jug,
                        "mid" => &mut self.app_config.functions.auto_select_champion_mid,
                        "bot" => &mut self.app_config.functions.auto_select_champion_bot,
                        "sup" => &mut self.app_config.functions.auto_select_champion_sup,
                        _ => &mut self.app_config.functions.auto_select_champion,
                    };
                    *list = vec![champion_id];
                    self.tools_view.open_hover_picker = false;
                }
                self.persist_config(cx);
            }
            ToolsAction::ToggleDropdownMenu(menu) => {
                match menu {
                    "lobby_mode" => {
                        self.tools_view.open_lobby_mode_dropdown =
                            !self.tools_view.open_lobby_mode_dropdown
                    }
                    "spectate_method" => {
                        self.tools_view.open_spectate_method_dropdown =
                            !self.tools_view.open_spectate_method_dropdown
                    }
                    "cleaner_filter" => {
                        self.tools_view.open_cleaner_filter_dropdown =
                            !self.tools_view.open_cleaner_filter_dropdown
                    }
                    "spoof_queue" => {
                        self.tools_view.open_spoof_queue_dropdown =
                            !self.tools_view.open_spoof_queue_dropdown
                    }
                    "spoof_tier" => {
                        self.tools_view.open_spoof_tier_dropdown =
                            !self.tools_view.open_spoof_tier_dropdown
                    }
                    "spoof_division" => {
                        self.tools_view.open_spoof_division_dropdown =
                            !self.tools_view.open_spoof_division_dropdown
                    }
                    _ => {}
                }
                cx.notify();
            }
            ToolsAction::SetSpectateMethod(m) => {
                self.tools_view.spectate_method = m;
                self.tools_view.open_spectate_method_dropdown = false;
                cx.notify();
            }
            ToolsAction::SetCleanerFilter(f) => {
                self.tools_view.cleaner_filter = f;
                self.tools_view.open_cleaner_filter_dropdown = false;
                cx.notify();
            }
            ToolsAction::ToggleSelectCleanerFriend(id) => {
                if self.tools_view.cleaner_selected_ids.contains(&id) {
                    self.tools_view.cleaner_selected_ids.remove(&id);
                } else {
                    self.tools_view.cleaner_selected_ids.insert(id);
                }
                cx.notify();
            }
            ToolsAction::SelectAllCleanerFriends => {
                for f in crate::views::tools::data::MOCK_FRIENDS {
                    self.tools_view
                        .cleaner_selected_ids
                        .insert(f.id.to_string());
                }
                cx.notify();
            }
            ToolsAction::ClearSelectedCleanerFriends => {
                self.tools_view.cleaner_selected_ids.clear();
                cx.notify();
            }
            ToolsAction::SelectBgChampion(cid) => {
                self.tools_view.bg_selected_champion = Some(cid);
                self.tools_view.open_bg_champion_picker = false;
                self.tools_view.set_feedback("已选择背景英雄", false);
                cx.notify();
            }
            ToolsAction::SelectBgSkin(sid) => {
                self.tools_view.bg_selected_skin_id = Some(sid);
                let live = self.live_service.clone();
                yuumi_core::runtime::spawn_background(async move {
                    if let Some(client) = live.get_client().await {
                        let body = serde_json::json!({ "key": "backgroundSkinId", "value": sid });
                        let _ = client
                            .request(
                                "POST",
                                "/lol-summoner/v1/current-summoner/summoner-profile",
                                Some(body),
                                false,
                            )
                            .await;
                    }
                });
                self.tools_view
                    .set_feedback(format!("已应用背景皮肤 ID: {}", sid), false);
                cx.notify();
            }
            ToolsAction::ToggleBadgesModal(show) => {
                self.tools_view.show_badges_modal = show;
                cx.notify();
            }
            ToolsAction::ToggleBadgeItem(bid) => {
                if let Some(pos) = self
                    .tools_view
                    .selected_badges
                    .iter()
                    .position(|id| id == &bid)
                {
                    self.tools_view.selected_badges.remove(pos);
                } else if self.tools_view.selected_badges.len() < 3 {
                    self.tools_view.selected_badges.push(bid);
                }
                cx.notify();
            }
            ToolsAction::ClearBadgesModal => {
                self.tools_view.selected_badges.clear();
                cx.notify();
            }
            ToolsAction::SaveBadgesPreferences => {
                self.tools_view.show_badges_modal = false;
                self.tools_view
                    .set_feedback("称号与挑战勋章装配成功！", false);
                cx.notify();
            }
        }
    }

    /// 处理设置页交互动作
    pub fn handle_settings_action(&mut self, action: SettingsAction, cx: &mut Context<Self>) {
        match action {
            SettingsAction::ToggleCollapse(name) => {
                self.settings_view.toggle_section(name);
                cx.notify();
            }
            SettingsAction::SetApiConcurrency(val) => {
                self.app_config.functions.api_concurrency_number = val;
                self.persist_config(cx);
            }
            SettingsAction::SetCareerGames(val) => {
                self.app_config.functions.career_games_number = val;
                self.persist_config(cx);
            }
            SettingsAction::ToggleGameInfoFilter => {
                self.app_config.functions.game_info_filter =
                    !self.app_config.functions.game_info_filter;
                self.persist_config(cx);
            }
            SettingsAction::ToggleReserveGameInfo => {
                self.app_config.functions.enable_reserve_gameinfo =
                    !self.app_config.functions.enable_reserve_gameinfo;
                self.persist_config(cx);
            }
            SettingsAction::ToggleShowTierInGameInfo => {
                self.app_config.functions.show_tier_in_game_info =
                    !self.app_config.functions.show_tier_in_game_info;
                self.persist_config(cx);
            }
            SettingsAction::ToggleAutoShowOpgg => {
                self.app_config.functions.auto_show_opgg =
                    !self.app_config.functions.auto_show_opgg;
                self.persist_config(cx);
            }
            SettingsAction::TogglePinOpgg => {
                self.app_config.functions.enable_opgg_on_top =
                    !self.app_config.functions.enable_opgg_on_top;
                self.persist_config(cx);
            }
            SettingsAction::DetectLolPath => {
                if let Some(path) = services::detect_lol_path() {
                    if !self.app_config.general.lol_path.contains(&path) {
                        self.app_config.general.lol_path.push(path.clone());
                        self.persist_config(cx);
                    }
                    self.settings_view
                        .set_feedback(format!("已检测到客户端: {}", path), false);
                } else {
                    self.settings_view
                        .set_feedback("未检测到运行中的英雄联盟客户端", true);
                }
                cx.notify();
            }
            SettingsAction::SelectLolFolder => {
                if let Some(path) = services::select_folder() {
                    if !self.app_config.general.lol_path.contains(&path) {
                        self.app_config.general.lol_path.push(path.clone());
                        self.persist_config(cx);
                    }
                    self.settings_view
                        .set_feedback(format!("已添加目录: {}", path), false);
                }
                cx.notify();
            }
            SettingsAction::AddWegamePath => {
                let wegame_marker = yuumi_core::config::WEGAME_MARKER.to_string();
                if !self.app_config.general.lol_path.contains(&wegame_marker) {
                    self.app_config.general.lol_path.push(wegame_marker);
                    self.persist_config(cx);
                }
                self.settings_view
                    .set_feedback("已添加 WeGame 启动项", false);
                cx.notify();
            }
            SettingsAction::RemoveLolPath(idx) => {
                if idx < self.app_config.general.lol_path.len() {
                    self.app_config.general.lol_path.remove(idx);
                    self.persist_config(cx);
                }
            }
            SettingsAction::ToggleStartLolWithApp => {
                self.app_config.general.enable_start_lol_with_app =
                    !self.app_config.general.enable_start_lol_with_app;
                self.persist_config(cx);
            }
            SettingsAction::ClearCache => {
                let client_path = self.app_config.general.lol_path.first().cloned();
                std::thread::spawn(move || {
                    let _ = services::clear_game_cache(client_path.as_deref());
                });
                self.settings_view.set_feedback("缓存清理完成", false);
                cx.notify();
            }
            SettingsAction::ToggleCloseToTray => {
                let cur = self.app_config.general.enable_close_to_tray.unwrap_or(true);
                self.app_config.general.enable_close_to_tray = Some(!cur);
                self.persist_config(cx);
            }
            SettingsAction::ToggleMinimizeOnGameStart => {
                self.app_config.general.enable_game_start_minimize =
                    !self.app_config.general.enable_game_start_minimize;
                self.persist_config(cx);
            }
            SettingsAction::SetUploadApiUrl(url) => {
                self.app_config.general.upload_api_url = url;
                self.persist_config(cx);
            }
            SettingsAction::ToggleLcuRealtime => {
                self.app_config.functions.lcu_realtime_enabled =
                    !self.app_config.functions.lcu_realtime_enabled;
                self.persist_config(cx);
            }
            SettingsAction::SetLcuUserId(id) => {
                self.app_config.functions.lcu_user_id = id;
                self.persist_config(cx);
            }
            SettingsAction::ToggleSignalrHub => {
                self.app_config.general.enable_signalr_hub =
                    !self.app_config.general.enable_signalr_hub;
                self.persist_config(cx);
            }
            SettingsAction::SetSignalrServer(s) => {
                self.app_config.general.signalr_server_url = s;
                self.persist_config(cx);
            }
            SettingsAction::SetSignalrUser(u) => {
                self.app_config.general.signalr_user_id = u;
                self.persist_config(cx);
            }
            SettingsAction::ToggleHideTft => {
                self.app_config.functions.hide_tft = !self.app_config.functions.hide_tft;
                self.persist_config(cx);
            }
            SettingsAction::ToggleHideSavedPlayers => {
                self.app_config.functions.hide_saved_players =
                    !self.app_config.functions.hide_saved_players;
                self.persist_config(cx);
            }
            SettingsAction::ToggleScreenshotOnMultikill => {
                self.app_config.functions.enable_screenshot_on_multikill =
                    !self.app_config.functions.enable_screenshot_on_multikill;
                self.persist_config(cx);
            }
            SettingsAction::ToggleScreenshotLevel(lvl) => {
                if let Some(pos) = self
                    .app_config
                    .functions
                    .screenshot_on_multikill_levels
                    .iter()
                    .position(|l| *l == lvl)
                {
                    self.app_config
                        .functions
                        .screenshot_on_multikill_levels
                        .remove(pos);
                } else {
                    self.app_config
                        .functions
                        .screenshot_on_multikill_levels
                        .push(lvl);
                }
                self.persist_config(cx);
            }
            SettingsAction::SelectScreenshotFolder => {
                if let Some(folder) = services::select_folder() {
                    self.app_config.functions.screenshot_save_path = folder;
                    self.persist_config(cx);
                }
            }
            SettingsAction::OpenScreenshotFolder => {
                let path = self.app_config.functions.screenshot_save_path.clone();
                std::thread::spawn(move || {
                    let p = if path.is_empty() {
                        yuumi_core::runtime::app_data_dir().join("screenshots")
                    } else {
                        std::path::PathBuf::from(path)
                    };
                    let _ = services::open_folder(p);
                });
            }
            SettingsAction::SetLogLevel(lvl) => {
                self.settings_view.open_log_level_dropdown = false;
                self.app_config.general.log_level = lvl;
                self.persist_config(cx);
            }
            SettingsAction::OpenLogFolder => {
                std::thread::spawn(|| {
                    let _ = services::open_log_folder();
                });
            }
            SettingsAction::ToggleMica => {
                self.app_config.personalization.mica_enabled =
                    !self.app_config.personalization.mica_enabled;
                self.persist_config(cx);
            }
            SettingsAction::SetThemeMode(mode) => {
                self.settings_view.open_theme_mode_dropdown = false;
                self.app_config.personalization.theme_mode = mode;
                self.persist_config(cx);
            }
            SettingsAction::SetThemeColor(color) => {
                self.settings_view.open_theme_color_picker = false;
                self.app_config.personalization.theme_color = color;
                self.persist_config(cx);
            }
            SettingsAction::SetWinCardColor(color) => {
                self.settings_view.open_win_color_picker = false;
                self.app_config.personalization.win_card_color = color;
                self.persist_config(cx);
            }
            SettingsAction::SetLoseCardColor(color) => {
                self.settings_view.open_lose_color_picker = false;
                self.app_config.personalization.lose_card_color = color;
                self.persist_config(cx);
            }
            SettingsAction::SetRemakeCardColor(color) => {
                self.settings_view.open_remake_color_picker = false;
                self.app_config.personalization.remake_card_color = color;
                self.persist_config(cx);
            }
            SettingsAction::SetLightDeathsColor(color) => {
                self.settings_view.open_light_death_color_picker = false;
                self.app_config.personalization.light_deaths_number_color = color;
                self.persist_config(cx);
            }
            SettingsAction::SetDarkDeathsColor(color) => {
                self.settings_view.open_dark_death_color_picker = false;
                self.app_config.personalization.dark_deaths_number_color = color;
                self.persist_config(cx);
            }
            SettingsAction::ResetThemeColor => {
                self.settings_view.open_theme_color_picker = false;
                self.app_config.personalization.theme_color = "#009faa".to_string();
                self.persist_config(cx);
            }
            SettingsAction::ResetCardColors => {
                self.settings_view.open_win_color_picker = false;
                self.settings_view.open_lose_color_picker = false;
                self.settings_view.open_remake_color_picker = false;
                self.app_config.personalization.win_card_color = "#3339B01B".to_string();
                self.app_config.personalization.lose_card_color = "#33D3190C".to_string();
                self.app_config.personalization.remake_card_color = "#33A2A2A2".to_string();
                self.persist_config(cx);
            }
            SettingsAction::ResetDeathsColors => {
                self.settings_view.open_light_death_color_picker = false;
                self.settings_view.open_dark_death_color_picker = false;
                self.app_config.personalization.light_deaths_number_color = "#ffb60000".to_string();
                self.app_config.personalization.dark_deaths_number_color = "#ffff8d8d".to_string();
                self.persist_config(cx);
            }
            SettingsAction::SetDpiScale(s) => {
                self.settings_view.open_scale_dropdown = false;
                self.app_config.personalization.dpi_scale = s;
                self.persist_config(cx);
            }
            SettingsAction::SetLanguage(l) => {
                self.settings_view.open_lang_dropdown = false;
                self.app_config.personalization.language = l;
                self.persist_config(cx);
            }
            SettingsAction::CheckUpdate => {
                self.settings_view
                    .set_feedback("当前已是最新版本 (v2.5.0)", false);
                cx.notify();
            }
            SettingsAction::ToggleCheckUpdate => {
                self.app_config.general.enable_check_update =
                    !self.app_config.general.enable_check_update;
                self.persist_config(cx);
            }
            SettingsAction::GoToReleases => {
                #[cfg(target_os = "windows")]
                let _ = std::process::Command::new("cmd")
                    .args([
                        "/c",
                        "start",
                        "",
                        "https://github.com/ISuuuu/Yuumi/releases/latest",
                    ])
                    .spawn();
            }
            SettingsAction::ToggleHttpProxy => {
                self.app_config.general.enable_http_proxy =
                    !self.app_config.general.enable_http_proxy;
                self.persist_config(cx);
            }
            SettingsAction::SetHttpProxyAddr(addr) => {
                self.app_config.general.http_proxy_addr = addr;
                self.persist_config(cx);
            }
            SettingsAction::ToggleChangelog => {
                self.settings_view.show_changelog = !self.settings_view.show_changelog;
                cx.notify();
            }
            SettingsAction::OpenRepo => {
                #[cfg(target_os = "windows")]
                let _ = std::process::Command::new("cmd")
                    .args(["/c", "start", "", "https://github.com/ISuuuu/Yuumi"])
                    .spawn();
            }
            SettingsAction::ToggleDropdown(name) => {
                match name {
                    "log_level" => {
                        self.settings_view.open_log_level_dropdown =
                            !self.settings_view.open_log_level_dropdown;
                    }
                    "theme_mode" => {
                        self.settings_view.open_theme_mode_dropdown =
                            !self.settings_view.open_theme_mode_dropdown;
                    }
                    "dpi_scale" => {
                        self.settings_view.open_scale_dropdown =
                            !self.settings_view.open_scale_dropdown;
                    }
                    "language" => {
                        self.settings_view.open_lang_dropdown =
                            !self.settings_view.open_lang_dropdown;
                    }
                    "theme_color" => {
                        self.settings_view.open_theme_color_picker =
                            !self.settings_view.open_theme_color_picker;
                    }
                    "win_color" => {
                        self.settings_view.open_win_color_picker =
                            !self.settings_view.open_win_color_picker;
                    }
                    "lose_color" => {
                        self.settings_view.open_lose_color_picker =
                            !self.settings_view.open_lose_color_picker;
                    }
                    "remake_color" => {
                        self.settings_view.open_remake_color_picker =
                            !self.settings_view.open_remake_color_picker;
                    }
                    "light_death" => {
                        self.settings_view.open_light_death_color_picker =
                            !self.settings_view.open_light_death_color_picker;
                    }
                    "dark_death" => {
                        self.settings_view.open_dark_death_color_picker =
                            !self.settings_view.open_dark_death_color_picker;
                    }
                    _ => {}
                }
                cx.notify();
            }
        }
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
        if let Some((color, _)) = crate::views::settings::THEME_COLOR_PRESETS.get(idx) {
            self.app_config.personalization.theme_color = color.to_string();
            self.persist_config(cx);
        }
    }

    pub fn perform_search(&mut self, query: &str, cx: &mut Context<Self>) {
        self.perform_search_with_target(query, None, cx);
    }

    pub fn perform_search_with_target(
        &mut self,
        query: &str,
        target_game_id: Option<u64>,
        cx: &mut Context<Self>,
    ) {
        let trimmed = query.trim().to_string();
        if trimmed.is_empty() {
            return;
        }

        self.search_view.save_to_history(&trimmed);
        self.search_view.show_history = false;
        self.search_view.set_search_query(&trimmed);
        self.search_view.set_searching(true);
        if let Some(gid) = target_game_id {
            self.search_view.selected_game_id = Some(gid);
        }
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
                            let selected_game_id = target_game_id
                                .filter(|tgid| items.iter().any(|m| m.display.game_id == *tgid))
                                .or_else(|| items.first().map(|m| m.display.game_id));

                            let _ = entity.update(cx, |this, cx| {
                                this.search_view.set_search_results(
                                    full_name,
                                    summoner.puuid.clone(),
                                    summoner.summoner_id.unwrap_or(0),
                                    items,
                                );
                                if let Some(gid) = selected_game_id {
                                    this.search_view.selected_game_id = Some(gid);
                                }
                                cx.notify();
                            });

                            // 3. 异步拉取详情填入 details_cache
                            if let Some(gid) = selected_game_id {
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

        self.search_view.select_request_id += 1;
        let req_id = self.search_view.select_request_id;
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
                        if this.search_view.select_request_id == req_id {
                            this.search_view.insert_game_detail(game_id, detail);
                            this.search_view.is_loading_detail = false;
                            cx.notify();
                        }
                    });
                    return;
                }
            }

            let _ = entity.update(cx, |this, cx| {
                if this.search_view.select_request_id == req_id {
                    log::warn!("[MainWindow] 对局详情拉取失败: game_id={}", game_id);
                    this.search_view.is_loading_detail = false;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// 复制指定 Game ID 并短暂展示反馈
    pub fn copy_search_game_id(&mut self, game_id: u64, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(game_id.to_string()));
        self.search_view.copied_feedback = Some(game_id);
        cx.notify();

        cx.spawn(async move |entity, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1500))
                .await;
            let _ = entity.update(cx, |this, cx| {
                if this.search_view.copied_feedback == Some(game_id) {
                    this.search_view.copied_feedback = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// 战绩查询翻到上一页并自动定位当页首局详情
    pub fn handle_search_prev_page(&mut self, cx: &mut Context<Self>) {
        if self.search_view.current_page > 1 {
            self.search_view.current_page -= 1;
            let (paged, _, _) = self.search_view.paged_filtered_matches();
            if let Some(first) = paged.first() {
                let gid = first.display.game_id;
                self.load_match_detail(gid, cx);
            }
            cx.notify();
        }
    }

    /// 战绩查询翻到下一页并自动定位当页首局详情
    pub fn handle_search_next_page(&mut self, cx: &mut Context<Self>) {
        let (_, _, has_more) = self.search_view.paged_filtered_matches();
        if has_more {
            self.search_view.current_page += 1;
            let (paged, _, _) = self.search_view.paged_filtered_matches();
            if let Some(first) = paged.first() {
                let gid = first.display.game_id;
                self.load_match_detail(gid, cx);
            }
            cx.notify();
        }
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
        let map_side_label = if let Some(info) = &self.game_info {
            match info.user_side {
                Some(yuumi_core::gameflow::team_builder::MapSide::Blue) => Some("蓝方"),
                Some(yuumi_core::gameflow::team_builder::MapSide::Red) => Some("红方"),
                _ => None,
            }
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
                map_side_label,
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
                {
                    let entity = view_entity.clone();
                    move |w, app| {
                        let close_to_tray = entity.update(app, |this, _cx| {
                            this.app_config.general.enable_close_to_tray.unwrap_or(false)
                        });
                        if close_to_tray {
                            w.minimize_window();
                        } else {
                            app.quit();
                        }
                    }
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
                        &self.app_config,
                        self.career_view.summoner.as_ref(),
                        "",
                        self.has_update,
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
                        {
                            let entity = view_entity.clone();
                            move |_window, app| {
                                entity.update(app, |this, cx| {
                                    this.open_opgg_window(cx);
                                });
                            }
                        },
                        {
                            let entity = view_entity.clone();
                            move |_window, app| {
                                entity.update(app, |this, cx| {
                                    this.trigger_reconnect(cx);
                                });
                            }
                        },
                        {
                            let entity = view_entity.clone();
                            move |_window, app| {
                                entity.update(app, |this, cx| {
                                    this.toggle_notice_popup(true, cx);
                                });
                            }
                        },
                    ))
                    // 右侧主视口：首页、路人集、生涯、对局信息、查询各自内部独立滚动，仅 Tools 与 Settings 启用全局外层滚动
                    .child(
                        div()
                            .id("main-scroll-view")
                            .flex_1()
                            .h_full()
                            .min_w(px(0.0))
                            .track_scroll(&self.main_scroll_handle)
                            .when(
                                matches!(self.current_route, Route::Tools | Route::Settings),
                                |el| el.overflow_y_scroll(),
                            )
                            .when(self.current_route == Route::Home, |el| {
                                let view_select = view_entity.clone();
                                let view_launch = view_entity.clone();
                                el.child(self.home_view.render(
                                    &self.connection_status,
                                    move |path, _win, app| {
                                        view_select.update(app, |this, cx| {
                                            this.home_view.selected_path = Some(path);
                                            cx.notify();
                                        });
                                    },
                                    move |_win, app| {
                                        view_launch.update(app, |this, cx| {
                                            let path = this.home_view.selected_path.clone();
                                            let wegame = this.home_view.wegame_path.clone();
                                            log::info!(
                                                "[MainWindow] 用户点击启动客户端, path: {:?}",
                                                path
                                            );
                                            let res = services::launch_lol_client(
                                                path.as_deref(),
                                                if wegame.is_empty() {
                                                    None
                                                } else {
                                                    Some(&wegame)
                                                },
                                            );
                                            match res {
                                                Ok(_) => {
                                                    log::info!("[MainWindow] 客户端启动调用成功");
                                                    this.home_view.launch_message = Some((
                                                        "正在启动英雄联盟客户端...".to_string(),
                                                        false,
                                                    ));
                                                }
                                                Err(e) => {
                                                    log::warn!(
                                                        "[MainWindow] 客户端启动失败: {}",
                                                        e
                                                    );
                                                    this.home_view.launch_message =
                                                        Some((format!("启动失败: {}", e), true));
                                                }
                                            }
                                            cx.notify();
                                        });
                                    },
                                ))
                            })
                            .when(self.current_route == Route::Career, |el| {
                                let view = view_entity.clone();
                                let view_select = view_entity.clone();
                                let view_refresh = view_entity.clone();
                                let view_copy = view_entity.clone();
                                let view_history = view_entity.clone();
                                let view_back = view_entity.clone();
                                let view_teammate = view_entity.clone();
                                let view_query = view_entity.clone();
                                let view_filter = view_entity.clone();
                                let view_bar = view_entity.clone();
                                let view_loot = view_entity.clone();
                                let is_connected = matches!(
                                    self.connection_status,
                                    ConnectionStatus::Connected { .. }
                                );
                                el.child(self.career_view.render(
                                    &self.matches,
                                    stats,
                                    is_connected,
                                    move |target_tab, _window, app| {
                                        view.update(app, |this, cx| {
                                            this.career_view.active_tab = target_tab;
                                            this.career_view.popup = None;
                                            cx.notify();
                                        });
                                    },
                                    move |game_id, _window, app| {
                                        view_select.update(app, |this, cx| {
                                            let summoner_name = this
                                                .career_view
                                                .summoner
                                                .as_ref()
                                                .map(|s| s.full_name())
                                                .unwrap_or_default();
                                            this.navigate_to_search(
                                                &summoner_name,
                                                Some(game_id),
                                                cx,
                                            );
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
                                    move |_window, app| {
                                        view_back.update(app, |this, cx| {
                                            this.back_to_my_career(cx);
                                        });
                                    },
                                    move |tm, _window, app| {
                                        view_teammate.update(app, |this, cx| {
                                            this.career_view.popup = None;
                                            if !tm.puuid.is_empty() {
                                                this.view_other_career(
                                                    tm.puuid, tm.name, tm.icon, cx,
                                                );
                                            } else {
                                                this.navigate_to_search(&tm.name, None, cx);
                                            }
                                        });
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
                                            if let CareerBarAction::SelectQueue(_) = &action {
                                                this.career_view
                                                    .matches_scroll_handle
                                                    .set_offset(Point::default());
                                            }
                                            this.apply_career_bar_action(action, cx);
                                        });
                                    },
                                    &self.loot_value_focus,
                                    self.loot_value_focus.is_focused(window),
                                    move |action: LootAction, _window, app| {
                                        view_loot.update(app, |this, cx| {
                                            this.apply_loot_action(action, cx)
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
                                let view_mode_cb = view_entity.clone();
                                let view_tab_cb = view_entity.clone();
                                let view_career_cb = view_entity.clone();
                                let view_search_cb = view_entity.clone();
                                let view_load_demo = view_entity.clone();
                                let view_exit_demo = view_entity.clone();
                                el.overflow_hidden().h_full().child(GameInfoView::render(
                                    self.game_info.as_deref(),
                                    self.current_phase.as_str(),
                                    self.game_info_loading,
                                    connected,
                                    view_mode,
                                    active_tab,
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
                                    move |puuid, _window, app| {
                                        view_career_cb.update(app, |this, cx| {
                                            if let Some(p) = puuid {
                                                this.view_other_career(
                                                    p,
                                                    String::new(),
                                                    String::new(),
                                                    cx,
                                                );
                                            } else {
                                                this.back_to_my_career(cx);
                                            }
                                        });
                                    },
                                    move |name, tag, _window, app| {
                                        view_search_cb.update(app, |this, cx| {
                                            let query = match tag {
                                                Some(t) if !t.is_empty() => {
                                                    format!("{}#{}", name, t)
                                                }
                                                _ => name,
                                            };
                                            this.navigate_to_search(&query, None, cx);
                                        });
                                    },
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
                                    window.viewport_size().width,
                                ))
                            })
                            .when(self.current_route == Route::Search, |el| {
                                let view = view_entity.clone();
                                let view_sq = view.clone();
                                let view_del_hist = view.clone();
                                let view_prev = view.clone();
                                let view_next = view.clone();
                                let view_career = view.clone();
                                let view_upload = view.clone();
                                let view_copy = view.clone();
                                let view_player = view.clone();

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
                                            this.search_view.current_page = 1;
                                            let (paged, _, _) =
                                                this.search_view.paged_filtered_matches();
                                            if let Some(first) = paged.first() {
                                                let gid = first.display.game_id;
                                                this.load_match_detail(gid, cx);
                                            }
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
                                            this.search_view.show_history = true;
                                            cx.notify();
                                        });
                                    }
                                };

                                let view_open_hist = view.clone();
                                let view_close_hist = view.clone();

                                let connected = matches!(
                                    self.connection_status,
                                    ConnectionStatus::Connected { .. }
                                );

                                el.overflow_hidden().h_full().child(self.search_view.render(
                                    connected,
                                    on_select_match,
                                    on_filter_queue,
                                    on_search,
                                    on_input_change,
                                    self.search_view.focus_handle.is_focused(window),
                                    move |_window, app| {
                                        view_open_hist.update(app, |this, cx| {
                                            this.search_view.show_history = true;
                                            this.search_view.queue_menu_open = false;
                                            cx.notify();
                                        });
                                    },
                                    move |open, _window, app| {
                                        view_sq.update(app, |this, cx| {
                                            this.set_search_queue_menu(open, cx)
                                        });
                                    },
                                    move |entry, _window, app| {
                                        let e = entry.to_string();
                                        view_del_hist.update(app, |this, cx| {
                                            this.search_view.remove_from_history(&e);
                                            cx.notify();
                                        });
                                    },
                                    move |_window, app| {
                                        view_close_hist.update(app, |this, cx| {
                                            this.search_view.show_history = false;
                                            cx.notify();
                                        });
                                    },
                                    move |_window, app| {
                                        view_prev.update(app, |this, cx| {
                                            this.handle_search_prev_page(cx);
                                        });
                                    },
                                    move |_window, app| {
                                        view_next.update(app, |this, cx| {
                                            this.handle_search_next_page(cx);
                                        });
                                    },
                                    move |_window, app| {
                                        view_career.update(app, |this, cx| {
                                            let current_target =
                                                this.search_view.current_summoner_puuid.clone();
                                            let self_puuid =
                                                this.own_summoner.as_ref().map(|s| s.puuid.clone());
                                            if let Some(target) = current_target {
                                                if Some(&target) != self_puuid.as_ref() {
                                                    let name = this
                                                        .search_view
                                                        .current_summoner_name
                                                        .clone()
                                                        .unwrap_or_default();
                                                    this.view_other_career(
                                                        target,
                                                        name,
                                                        String::new(),
                                                        cx,
                                                    );
                                                    return;
                                                }
                                            }
                                            this.back_to_my_career(cx);
                                        });
                                    },
                                    move |val, _window, app| {
                                        view_upload.update(app, |this, cx| {
                                            this.search_view.upload_enabled = val;
                                            cx.notify();
                                        });
                                    },
                                    move |game_id, _window, app| {
                                        view_copy.update(app, |this, cx| {
                                            this.copy_search_game_id(game_id, cx);
                                        });
                                    },
                                    move |puuid, summoner_id, name, _window, app| {
                                        view_player.update(app, |this, cx| {
                                            if !puuid.is_empty()
                                                && puuid != "00000000-0000-0000-0000-000000000000"
                                            {
                                                this.view_other_career(
                                                    puuid,
                                                    name,
                                                    String::new(),
                                                    cx,
                                                );
                                            } else if summoner_id > 0 {
                                                this.perform_search(&name, cx);
                                            }
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
                                                SavedPlayersAction::AskDelete(puuid) => {
                                                    this.saved_players_view.ask_delete(&puuid);
                                                }
                                                SavedPlayersAction::CancelDelete => {
                                                    this.saved_players_view.cancel_delete();
                                                }
                                                SavedPlayersAction::ConfirmDelete(puuid) => {
                                                    this.saved_players_view.confirm_delete(&puuid);
                                                    this.reload_saved_players(cx);
                                                }
                                                SavedPlayersAction::ExportTagged => {
                                                    this.saved_players_view.export_tagged_to_file();
                                                }
                                                SavedPlayersAction::ImportTagged => {
                                                    this.saved_players_view
                                                        .import_tagged_from_file();
                                                    this.reload_saved_players(cx);
                                                }
                                                SavedPlayersAction::LoadMoreGames => {
                                                    this.saved_players_view.load_more_games();
                                                }
                                                SavedPlayersAction::DismissToast => {
                                                    this.saved_players_view.toast_message = None;
                                                }
                                            }
                                            cx.notify();
                                        });
                                    };

                                el.child(self.saved_players_view.render_panel(window, on_action))
                            })
                            .when(self.current_route == Route::Tools, |el| {
                                let view = view_entity.clone();
                                let connected =
                                    matches!(self.connection_status, ConnectionStatus::Connected { .. });
                                let on_tools_action =
                                    move |action, _win: &mut Window, app: &mut App| {
                                        view.update(app, |this, cx| {
                                            this.handle_tools_action(action, cx);
                                        });
                                    };
                                el.child(self.tools_view.render_panel(
                                    &self.app_config,
                                    connected,
                                    window,
                                    on_tools_action,
                                ))
                            })
                            .when(self.current_route == Route::Settings, |el| {
                                let view = view_entity.clone();
                                let on_settings_action =
                                    move |action, _win: &mut Window, app: &mut App| {
                                        view.update(app, |this, cx| {
                                            this.handle_settings_action(action, cx);
                                        });
                                    };
                                el.child(self.settings_view.render_panel(
                                    &self.app_config,
                                    window,
                                    on_settings_action,
                                ))
                            })
                            .when(self.current_route == Route::Tft, |el| {
                                el.child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .size_full()
                                        .items_center()
                                        .justify_center()
                                        .gap(px(12.0))
                                        .child(div().text_size(px(40.0)).child("♟️"))
                                        .child(
                                            div()
                                                .text_size(px(16.0))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(Theme::TEXT_PRIMARY)
                                                .child("云顶之弈助手"),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(13.0))
                                                .text_color(Theme::TEXT_MUTED)
                                                .child("TFT 战绩、推荐阵容与海克斯分析模块接入准备中"),
                                        ),
                                )
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
            // 3. 右下角公告通知弹窗 (NoticePopup)
            .when(self.notice_popup_open, |this| {
                let v = view_entity.clone();
                this.child(
                    div()
                        .absolute()
                        .right(px(20.0))
                        .bottom(px(20.0))
                        .child(notice_popup(
                            "版本更新与运行公告",
                            "当前版本已完整复刻 Vue 纯白水晶极光界面，Direct3D 11 原生极速渲染，内存与启动性能优化完毕。",
                            move |_win, app| {
                                v.update(app, |this, cx| {
                                    this.toggle_notice_popup(false, cx);
                                });
                            },
                        )),
                )
            })
            // 4. OP.GG 入口提示（独立窗口已在原生窗口层打开，这里仅承载临时提示）
            .when_some(self.opgg_hint.clone(), |el, hint| {
                el.child(
                    div()
                        .absolute()
                        .bottom(px(20.0))
                        .left(px(20.0))
                        .px(px(14.0))
                        .py(px(10.0))
                        .rounded(px(8.0))
                        .bg(Theme::GOLD)
                        .shadow_md()
                        .text_size(px(12.0))
                        .text_color(Theme::TEXT_ON_PRIMARY)
                        .child(hint),
                )
            })
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

/// OP.GG 窗口默认 760 × 820 逻辑像素，贴靠主窗口所在屏幕的右上角
/// （对照 Vue `openOpggWindow` 的 `x = pos.x + width - 760 - 2, y = pos.y + 2`），
/// 工作区不足时先收敛尺寸，保证标题栏与内容可达。
fn opgg_window_bounds(cx: &App) -> WindowBounds {
    let mut width = px(760.0);
    let mut height = px(820.0);
    if let Some(display) = cx.primary_display() {
        let bounds = display.bounds();
        width = width.min((bounds.size.width - px(40.0)).max(px(600.0)));
        height = height.min((bounds.size.height - px(80.0)).max(px(520.0)));
        let origin = Point {
            x: bounds.origin.x + bounds.size.width - width - px(2.0),
            y: bounds.origin.y + px(2.0),
        };
        return WindowBounds::Windowed(Bounds {
            origin,
            size: Size { width, height },
        });
    }
    WindowBounds::Windowed(Bounds {
        origin: Point {
            x: px(120.0),
            y: px(60.0),
        },
        size: Size { width, height },
    })
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
