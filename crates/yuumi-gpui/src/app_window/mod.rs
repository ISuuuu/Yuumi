// 主窗口模块：MainWindow 状态、构造、资源预热与窗口尺寸。
//
// 各子模块按职责承载主窗的动作与路由渲染，共用同一个 MainWindow 实体与全局配置：
// - shell:          Render 入口、标题栏、侧栏、页面分派、回到顶部
// - lifecycle:      LiveLcuEvent 分派、配置同步、独立窗口（板凳席 / OP.GG）生命周期
// - career / search / game_info / saved_players / tools / settings:
//                   各领域的路由渲染、动作分发与跨页接线
use gpui::prelude::*;
use gpui::{
    div, px, App, Bounds, ClipboardItem, Context, Div, Entity, FocusHandle, FontWeight,
    InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, Point, Render,
    ScrollHandle, SharedString, Size, Stateful, StatefulInteractiveElement, Styled,
    TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind,
    WindowOptions,
};
use std::sync::Arc;
use std::time::Duration;

use crate::asset_loader;
use crate::bench_overlay::{load_saved_origin, BenchOverlayView};
use crate::components;
use crate::components::notice_popup::notice_popup;
use crate::components::sidebar::Sidebar;
use crate::components::titlebar::{phase_label, TitleBar};
use crate::components::toast::{ToastKind, ToastStack};
use crate::match_model::MatchItem;
use crate::opgg::{OpggWindow, OPGG_WINDOW_TITLE};
use crate::platform;
use crate::services;
use crate::services::{ConnectionStatus, LiveLcuEvent, LiveLcuService};
use crate::theme::Theme;
use crate::tray::{TrayAction, TrayManager};
use crate::views;
use crate::views::career::{
    CareerBarAction, CareerHeaderStats, CareerView, LootAction, LootModal, LootProgress,
    MasteryFilterAction,
};
use crate::views::game_info::{GameInfoTeamTab, GameInfoView, GameInfoViewMode};
use crate::views::home::HomeView;
use crate::views::saved_players::SavedPlayersView;
use crate::views::search::SearchView;
use crate::views::settings::{SettingsAction, SettingsView};
use crate::views::tft::TftView;
use crate::views::tools::{ToolsAction, ToolsView};
use crate::views::Route;
use yuumi_core::agents::{AutomationSettings, ChampSelectSession, GamePhase};
use yuumi_core::config::AppConfig;
use yuumi_core::lcu::game_data::GameDataAssets;
use yuumi_core::parsers::game_info::{LiveGameInfo, RankedStats};
use yuumi_core::parsers::loot_actions::{LootActionItem, LootActionKind};
use yuumi_core::parsers::summoner::SummonerInfo;

mod career;
mod f_updates;
mod game_info;
mod lifecycle;
mod saved_players;
mod search;
mod settings;
mod shell;
mod tft;
mod tools;

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
    /// 云顶海克斯搜索框的焦点句柄
    tft_augment_focus: FocusHandle,
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
    tft_view: TftView,
    current_route: Route,
    /// 侧栏是否展开（64 ↔ 210，对照 Vue NavigationSidebar）
    sidebar_expanded: bool,
    /// 路由历史，供标题栏返回键使用（对照 Vue pageHistory）
    route_history: Vec<Route>,
    tray: Option<TrayManager>,
    overlay_window: Option<WindowHandle<BenchOverlayView>>,
    live_service: LiveLcuService,
    cloud_tasks: services::cloud_tasks::CloudTasks,
    cloud_game_id: Option<u64>,
    cloud_match_played: bool,
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
    f_update: f_updates::UpdateFlow,
    has_update: bool,
    /// 公告弹窗是否打开
    notice_popup_open: bool,
    /// 公告内容
    notice_content: Option<String>,
    /// 公告是否正在加载
    notice_loading: bool,
    /// 公告是否加载出错
    notice_error: bool,
    /// 公告弹窗自动关闭的时钟纪元（用于防止旧计时器误关新打开的公告）
    notice_dismiss_epoch: u64,
    /// OP.GG 独立窗口句柄（存在即复用并聚焦）
    opgg_window: Option<WindowHandle<OpggWindow>>,
    mica_active: bool,
    /// 窗口级 Toast 队列（对照 Vue NMessageProvider，全部业务反馈的统一出口）
    toast_stack: Entity<ToastStack>,
    /// 最近一次已成功记录相遇的对局 ID（防止结算多事件重复写入）
    last_recorded_encounter_game_id: Option<u64>,
    /// 主窗最近一次渲染时的窗口区域（OP.GG 独立窗据此跟随主窗所在屏幕）
    main_window_bounds: Option<Bounds<Pixels>>,
}

impl MainWindow {
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
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
        let tft_augment_focus = cx.focus_handle();

        // 加载持久化的全局配置与自动化配置（异常信息留给首帧 Toast）
        let (app_config, config_error) = AppConfig::load_with_error();
        // 尽早初始化全局语言：首帧 Toast（如配置异常提示）与后续渲染都要用配置里的语言
        crate::i18n::set_language(crate::i18n::Lang::from_config(
            &app_config.personalization.language,
        ));
        let automation = app_config.to_automation_settings();
        let tools_view = ToolsView::new();

        let mut search_view = SearchView::new(cx).with_matches(matches.clone());
        search_view.has_upload_api_url = !app_config.general.upload_api_url.trim().is_empty();
        search_view.upload_enabled = app_config.functions.upload_enabled;
        let saved_players_view = SavedPlayersView::new(cx);
        let mut settings_view = SettingsView::new();
        settings_view.bind_f_inputs(&app_config, cx);
        let tft_view = TftView::new();
        let home_view = HomeView::new();

        // 窗口级 Toast 队列：内容变更时通知主窗重绘浮层
        let toast_stack = cx.new(|_| ToastStack::new());
        cx.observe(&toast_stack, |_, _, cx| cx.notify()).detach();

        // 对照 Vue A01：配置文件异常时首帧以警告 Toast 呈现（Vue 弹窗为主、Toast 降级）
        if let Some(config_error) = config_error {
            toast_stack.update(cx, |stack, cx| {
                stack.push(
                    ToastKind::Warning,
                    format!("{}:\n{}", crate::i18n::t("settings.configLoadError"), config_error),
                    cx,
                );
            });
        }
        let tray = match TrayManager::new(app_config.functions.hide_tft) {
            Ok(tray) => Some(tray),
            Err(e) => {
                log::warn!("[Tray] 托盘创建失败: {}，托盘功能不可用", e);
                None
            }
        };

        // 检查「启动时自动运行 LOL」配置
        if app_config.general.enable_start_lol_with_app {
            let paths = app_config.general.lol_path.clone();
            let wegame = app_config.general.wegame_path.clone();
            std::thread::spawn(move || {
                let selected = paths
                    .iter()
                    .find(|p| *p == yuumi_core::config::WEGAME_MARKER)
                    .or_else(|| paths.first());
                let _ =
                    services::launch_lol_client(selected.map(|s| s.as_str()), wegame.as_deref());
            });
        }

        // 初始化真实 LCU 实时监控与调度服务
        let asset_cache = std::sync::Arc::new(yuumi_core::lcu::asset_cache::AssetCache::default());
        asset_loader::init_global_asset_loader(asset_cache.clone());
        let live_service = LiveLcuService::new(asset_cache);
        let mut cloud_tasks = services::cloud_tasks::CloudTasks::new(live_service.clone());
        let mut cloud_status = cloud_tasks.subscribe_status();
        cloud_tasks.apply(&app_config);
        cx.spawn(async move |entity, cx| loop {
            let status = *cloud_status.borrow_and_update();
            if entity
                .update(cx, |view, cx| {
                    view.settings_view.cloud_status = status;
                    cx.notify();
                })
                .is_err()
                || cloud_status.changed().await.is_err()
            {
                break;
            }
        })
        .detach();

        // 批量上传结果回传（对照 Vue D15-D18）：仅消费最新一条，避免快速连发时堆积
        let mut upload_outcomes = cloud_tasks.subscribe_upload_outcomes();
        cx.spawn(async move |entity, cx| loop {
            let outcome = upload_outcomes.borrow_and_update().clone();
            if let Some(outcome) = outcome {
                if entity
                    .update(cx, |view, cx| {
                        view.show_upload_outcome(outcome, cx);
                    })
                    .is_err()
                {
                    break;
                }
            }
            if upload_outcomes.changed().await.is_err() {
                break;
            }
        })
        .detach();

        // 启动即把持久化配置灌入 Agent 运行时，避免默认值覆盖用户选择
        live_service.update_auto_match_cfg(automation.auto_match.clone());
        live_service.update_auto_bp_cfg(automation.auto_bp.clone());

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

        // 启动系统托盘事件轮询任务（每 100ms 检查一次菜单或图标点击事件）
        cx.spawn(async move |entity, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let action = entity.update(cx, |this, _cx| {
                this.tray.as_ref().and_then(|t| t.poll_event())
            });
            if let Ok(Some(action)) = action {
                let _ = entity.update(cx, |this, cx| {
                    this.handle_tray_action(action, cx);
                });
            }
        })
        .detach();

        // 启动即静默检查版本更新（手动检查走设置页）
        cx.spawn(async move |entity, cx| {
            let _ = entity.update(cx, |view, cx| view.check_updates(false, cx));
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
            tft_view,
            career_mastery_focus,
            loot_value_focus,
            tft_augment_focus,
            current_route: Route::Home,
            sidebar_expanded: false,
            route_history: Vec::new(),
            tray,
            overlay_window: None,
            live_service,
            cloud_tasks,
            cloud_game_id: None,
            cloud_match_played: false,
            connection_status: ConnectionStatus::Disconnected,
            app_config,
            automation,
            bench_session: None,
            game_assets: None,
            main_scroll_handle: ScrollHandle::new(),
            f_update: f_updates::UpdateFlow::default(),
            has_update: false,
            notice_popup_open: false,
            notice_content: None,
            notice_loading: false,
            notice_error: false,
            notice_dismiss_epoch: 0,
            opgg_window: None,
            mica_active: false,
            toast_stack,
            last_recorded_encounter_game_id: None,
            main_window_bounds: None,
        }
    }

    /// 主窗 Toast 统一入口：所有业务反馈（成功/错误/警告/信息）都经这里入队
    pub(crate) fn toast(
        &mut self,
        kind: ToastKind,
        message: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.toast_stack.update(cx, |stack, cx| {
            stack.push(kind, message, cx);
        });
    }
}

/// 主窗口默认内容区 1200 × 910（对照 Vue `src-tauri/tauri.conf.json`），居中并按可用工作区收敛
pub(crate) fn main_window_bounds(cx: &App) -> WindowBounds {
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

/// 设置页 DPI 缩放选项（对照 Vue B12）→ GPUI scale factor 覆盖。
/// "Auto"（或未知值）返回 None 表示跟随系统 DPI，其余按百分比固定缩放。
pub(crate) fn scale_factor_override_for(dpi_scale: &str) -> Option<f32> {
    match dpi_scale {
        "100" => Some(1.0),
        "125" => Some(1.25),
        "150" => Some(1.5),
        _ => None,
    }
}
