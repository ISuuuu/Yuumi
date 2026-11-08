use gpui::prelude::*;
use gpui::{
    div, px, App, Application, Bounds, Context, InteractiveElement, IntoElement, ParentElement,
    Point, Render, Size, StatefulInteractiveElement, Styled, Window, WindowBounds, WindowHandle,
    WindowOptions,
};
use std::path::Path;

mod agent_settings_view;
mod asset_loader;
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

use bench_overlay::BenchOverlayView;
use components::input::SearchInput;
use components::sidebar::Sidebar;
use components::titlebar::TitleBar;
use game_info_view::{load_mock_game_info, GameInfoData, GameInfoView};
use match_model::{load_fixture_game_details, load_real_fixtures, MatchItem};
use services::{ConnectionStatus, LiveLcuEvent, LiveLcuService};
use theme::Theme;
use tray::TrayManager;
use views::career::{CareerHeaderStats, CareerView};
use views::home::HomeView;
use views::saved_players::SavedPlayersView;
use views::search::SearchView;
use views::settings::SettingsView;
use views::tools::ToolsView;
use views::Route;

pub const MAIN_WINDOW_TITLE: &str = "Yuumi · 英雄联盟对局助手 (GPUI 高性能原生内核)";
pub const BENCH_WINDOW_TITLE: &str = "Yuumi - 大乱斗板凳席悬浮窗";

pub struct MainWindow {
    titlebar: TitleBar,
    search_input: SearchInput,
    matches: Vec<MatchItem>,
    stats: CareerHeaderStats,
    career_view: CareerView,
    game_info: GameInfoData,
    tools_view: ToolsView,
    search_view: SearchView,
    saved_players_view: SavedPlayersView,
    settings_view: SettingsView,
    current_route: Route,
    _tray: Option<TrayManager>,
    overlay_window: Option<WindowHandle<BenchOverlayView>>,
    live_service: LiveLcuService,
    connection_status: ConnectionStatus,
}

impl MainWindow {
    pub fn new(cx: &mut Context<Self>) -> Self {
        // 尝试从项目 testdata 目录加载 8 份真实 LCU 对局抓包快照
        let fixture_path = Path::new("../../src-tauri/testdata/match");
        let alt_fixture_path = Path::new("src-tauri/testdata/match");
        let matches = if fixture_path.exists() {
            load_real_fixtures(fixture_path)
        } else if alt_fixture_path.exists() {
            load_real_fixtures(alt_fixture_path)
        } else {
            Vec::new()
        };

        let stats = CareerHeaderStats::compute(&matches);
        let career_view = CareerView::new();
        let game_info = load_mock_game_info();
        let tools_view = ToolsView::new();
        let mut search_view = SearchView::new(cx).with_matches(matches.clone());
        search_view.details_cache = load_fixture_game_details();
        let saved_players_view = SavedPlayersView::new();
        let settings_view = SettingsView::new();
        let tray = TrayManager::new().ok();
        let search_input = SearchInput::new("输入召唤师名 / 英雄过滤...");

        // 初始化真实 LCU 实时监控与调度服务
        let asset_cache = std::sync::Arc::new(yuumi_core::lcu::asset_cache::AssetCache::default());
        let live_service = LiveLcuService::new(asset_cache);
        live_service.start();

        // 订阅 LCU 领域事件与连接状态并在 GPUI 主调度器中消费
        let mut event_rx = live_service.subscribe_events();
        cx.spawn(async move |entity, cx| {
            while let Ok(event) = event_rx.recv().await {
                let event_clone = event.clone();
                let _ = entity.update(cx, |view, cx| {
                    view.handle_live_event(event_clone, cx);
                });
            }
        })
        .detach();

        // 异步预热战绩卡片中的核心资源（英雄头像等），拉取后触发视图重绘
        let asset_paths: Vec<String> = matches
            .iter()
            .map(|m| m.display.champion_icon_url.clone())
            .filter(|u| !u.is_empty())
            .collect();

        cx.background_executor()
            .spawn(async move {
                let loader = asset_loader::global_asset_loader();
                for path in asset_paths {
                    let _ = loader.load_render_image(&path).await;
                }
            })
            .detach();

        Self {
            titlebar: TitleBar::new(MAIN_WINDOW_TITLE),
            search_input,
            matches,
            stats,
            career_view,
            game_info,
            tools_view,
            search_view,
            saved_players_view,
            settings_view,
            current_route: Route::Home,
            _tray: tray,
            overlay_window: None,
            live_service,
            connection_status: ConnectionStatus::Disconnected,
        }
    }

    pub fn handle_live_event(&mut self, event: LiveLcuEvent, cx: &mut Context<Self>) {
        match event {
            LiveLcuEvent::ConnectionChanged(status) => {
                if let ConnectionStatus::Connected { summoner, .. } = &status {
                    if let Some(s) = summoner.as_ref() {
                        self.career_view.summoner = Some(s.clone());
                    }
                }
                self.connection_status = status;
                cx.notify();
            }
            LiveLcuEvent::MatchHistoryLoaded(matches) => {
                let items: Vec<MatchItem> = matches.into_iter().map(MatchItem::new).collect();
                self.stats = CareerHeaderStats::compute(&items);
                self.search_view.matches = items.clone();
                self.matches = items;
                cx.notify();
            }
            LiveLcuEvent::GamePhaseChanged(phase) => {
                log::info!("[MainWindow] 接收到游戏阶段更新: {:?}", phase);
                cx.notify();
            }
            _ => {}
        }
    }

    pub fn toggle_card_expanded(&mut self, idx: usize, cx: &mut Context<Self>) {
        if let Some(m) = self.matches.get_mut(idx) {
            m.expanded = !m.expanded;
            cx.notify();
        }
    }

    pub fn clear_search(&mut self, cx: &mut Context<Self>) {
        self.search_input.clear();
        cx.notify();
    }

    pub fn switch_route(&mut self, route: Route, cx: &mut Context<Self>) {
        self.current_route = route;
        cx.notify();
    }

    pub fn toggle_auto_accept(&mut self, cx: &mut Context<Self>) {
        self.tools_view.match_config.auto_accept = !self.tools_view.match_config.auto_accept;
        let _ = self
            .live_service
            .auto_match_cfg
            .send(self.tools_view.match_config.clone());
        cx.notify();
    }

    pub fn toggle_auto_reconnect(&mut self, cx: &mut Context<Self>) {
        self.tools_view.match_config.auto_reconnect = !self.tools_view.match_config.auto_reconnect;
        let _ = self
            .live_service
            .auto_match_cfg
            .send(self.tools_view.match_config.clone());
        cx.notify();
    }

    pub fn toggle_auto_play_again(&mut self, cx: &mut Context<Self>) {
        self.tools_view.match_config.auto_play_again =
            !self.tools_view.match_config.auto_play_again;
        let _ = self
            .live_service
            .auto_match_cfg
            .send(self.tools_view.match_config.clone());
        cx.notify();
    }

    pub fn toggle_auto_pick(&mut self, cx: &mut Context<Self>) {
        self.tools_view.bp_config.auto_pick = !self.tools_view.bp_config.auto_pick;
        let _ = self
            .live_service
            .auto_bp_cfg
            .send(self.tools_view.bp_config.clone());
        cx.notify();
    }

    pub fn toggle_auto_ban(&mut self, cx: &mut Context<Self>) {
        self.tools_view.bp_config.auto_ban = !self.tools_view.bp_config.auto_ban;
        let _ = self
            .live_service
            .auto_bp_cfg
            .send(self.tools_view.bp_config.clone());
        cx.notify();
    }

    pub fn toggle_auto_spell(&mut self, cx: &mut Context<Self>) {
        self.tools_view.bp_config.auto_spell = !self.tools_view.bp_config.auto_spell;
        let _ = self
            .live_service
            .auto_bp_cfg
            .send(self.tools_view.bp_config.clone());
        cx.notify();
    }

    pub fn toggle_bench_overlay_setting(&mut self, cx: &mut Context<Self>) {
        self.tools_view.bench_overlay = !self.tools_view.bench_overlay;
        cx.notify();
    }

    pub fn toggle_auto_trade(&mut self, cx: &mut Context<Self>) {
        self.tools_view.auto_trade = !self.tools_view.auto_trade;
        cx.notify();
    }

    pub fn toggle_auto_honor(&mut self, cx: &mut Context<Self>) {
        self.tools_view.auto_honor = !self.tools_view.auto_honor;
        self.tools_view.match_config.auto_honor = self.tools_view.auto_honor;
        let _ = self
            .live_service
            .auto_match_cfg
            .send(self.tools_view.match_config.clone());
        cx.notify();
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

    pub fn select_theme_index(&mut self, idx: usize, cx: &mut Context<Self>) {
        self.settings_view.selected_theme_index = idx;
        cx.notify();
    }

    pub fn toggle_bench_overlay(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(handle) = self.overlay_window.take() {
            let _ = handle.update(cx, |_view, _window, _cx| {
                // 关闭悬浮窗
            });
            cx.notify();
        } else {
            // 打开置顶半透明大乱斗悬浮窗
            let new_handle = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Point {
                            x: px(400.0),
                            y: px(120.0),
                        },
                        size: Size {
                            width: px(380.0),
                            height: px(180.0),
                        },
                    })),
                    titlebar: None,
                    focus: true,
                    is_movable: true,
                    ..Default::default()
                },
                |_window, cx| cx.new(|_cx| BenchOverlayView::new()),
            );

            if let Ok(h) = new_handle {
                self.overlay_window = Some(h);
                // 异步将板凳席小窗设置为 Windows 原生置顶与工具窗口（隐藏任务栏图标）
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    if let Some(hwnd) = platform::find_window_by_title(BENCH_WINDOW_TITLE) {
                        platform::set_window_topmost(hwnd, true);
                        platform::set_window_toolwindow(hwnd);
                    }
                });
            }
            cx.notify();
        }
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
                // 如果没有网络或真实拉取失败，尝试使用现有的 mock 详情作为保底
                if let Some(first_detail) = this.search_view.details_cache.values().next().cloned()
                {
                    let mut fallback = first_detail;
                    fallback.game_id = game_id;
                    this.search_view.insert_game_detail(game_id, fallback);
                }
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

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(Theme::BG_APP)
            .font_family(Theme::font_family())
            // 1. 无边框自定义标题栏（接入 Win32 最小化与安全退出）
            .child(self.titlebar.render(
                window,
                |_w, _app| {
                    if let Some(hwnd) = platform::find_window_by_title(MAIN_WINDOW_TITLE) {
                        platform::minimize_window(hwnd);
                    }
                },
                |_w, app| {
                    app.quit();
                },
            ))
            // 2. 主体区域：48px 极光侧边栏 + 主路由视口
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .overflow_hidden()
                    // 左侧 48px 导航侧边栏
                    .child(Sidebar::render(self.current_route, {
                        let entity = view_entity.clone();
                        move |route, _window, app| {
                            entity.update(app, |this, cx| {
                                this.switch_route(route, cx);
                            });
                        }
                    }))
                    // 右侧主视口
                    .child(
                        div()
                            .id("main-scroll-view")
                            .flex_1()
                            .h_full()
                            .overflow_y_scroll()
                            .when(self.current_route == Route::Home, |el| {
                                let entity = view_entity.clone();
                                el.p_4().child(HomeView::render(
                                    &self.connection_status,
                                    move |target_route, _window, app| {
                                        entity.update(app, |this, cx| {
                                            this.switch_route(target_route, cx);
                                        });
                                    },
                                ))
                            })
                            .when(self.current_route == Route::Career, |el| {
                                let view = view_entity.clone();
                                let view_matches = view_entity.clone();
                                el.child(self.career_view.render(
                                    &self.matches,
                                    stats,
                                    move |target_tab, _window, app| {
                                        view.update(app, |this, cx| {
                                            this.career_view.active_tab = target_tab;
                                            cx.notify();
                                        });
                                    },
                                    move |clicked_idx, _window, app| {
                                        view_matches.update(app, |this, cx| {
                                            this.toggle_card_expanded(clicked_idx, cx);
                                        });
                                    },
                                ))
                            })
                            .when(self.current_route == Route::GameInfo, |el| {
                                el.p_4().child(GameInfoView::render(&self.game_info))
                            })
                            .when(self.current_route == Route::Search, |el| {
                                let view = view_entity.clone();
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
                                ))
                            })
                            .when(self.current_route == Route::SavedPlayers, |el| {
                                let view = view_entity.clone();
                                let on_filter_tag = {
                                    let v = view.clone();
                                    move |tag: &str, _win: &mut Window, app: &mut App| {
                                        let tag_str = tag.to_string();
                                        v.update(app, |this, cx| {
                                            this.saved_players_view.set_filter(tag_str);
                                            cx.notify();
                                        });
                                    }
                                };
                                let on_toggle_pin = {
                                    let v = view.clone();
                                    move |puuid: &str, _win: &mut Window, app: &mut App| {
                                        let p = puuid.to_string();
                                        v.update(app, |this, cx| {
                                            this.saved_players_view.toggle_pin(&p);
                                            cx.notify();
                                        });
                                    }
                                };
                                let on_toggle_expand = {
                                    let v = view.clone();
                                    move |puuid: &str, _win: &mut Window, app: &mut App| {
                                        let p = puuid.to_string();
                                        v.update(app, |this, cx| {
                                            this.saved_players_view.toggle_expand(&p);
                                            cx.notify();
                                        });
                                    }
                                };
                                let on_remove_player = {
                                    let v = view.clone();
                                    move |puuid: &str, _win: &mut Window, app: &mut App| {
                                        let p = puuid.to_string();
                                        v.update(app, |this, cx| {
                                            this.saved_players_view.remove_player(&p);
                                            cx.notify();
                                        });
                                    }
                                };

                                el.child(self.saved_players_view.render_panel(
                                    on_filter_tag,
                                    on_toggle_pin,
                                    on_toggle_expand,
                                    on_remove_player,
                                ))
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
                    ),
            )
    }
}

fn main() {
    // 0. 初始化全局 Tokio 异步运行时，供后台网络 I/O、WebSocket 与 LCU 轮询使用
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("初始化 Tokio 运行时失败");
    let _runtime_guard = runtime.enter();

    // 1. Windows 单实例互斥量检测（Named Mutex）
    let mutex_name = "Global\\Yuumi_GPUI_SingleInstance_Mutex";
    let _instance_guard = match platform::acquire_single_instance(mutex_name) {
        Ok(guard) => Some(guard),
        Err(platform::SingleInstanceError::AlreadyRunning) => {
            eprintln!("检测到 Yuumi 已在运行中，激活现有窗口...");
            if let Some(hwnd) = platform::find_window_by_title(MAIN_WINDOW_TITLE) {
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

    // 3. 创建 GPUI 原生应用程序宿主
    let app = Application::new();

    app.run(move |cx| {
        // 创建主应用窗口（无边框，极光现代无缝阴影，居中显示）
        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: Point {
                    x: px(160.0),
                    y: px(80.0),
                },
                size: Size {
                    width: px(1080.0),
                    height: px(720.0),
                },
            })),
            titlebar: None, // 彻底禁用操作系统传统标题栏，启用我们纯手工打磨的极光水晶透明标题栏
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
            if let Some(hwnd) = platform::find_window_by_title(MAIN_WINDOW_TITLE) {
                platform::apply_mica_or_acrylic(hwnd);
                platform::set_window_rounded_corners(hwnd);
            }
        });

        let _ = window_handle;
    });
}
