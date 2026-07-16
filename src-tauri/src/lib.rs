pub mod agents;
pub mod commands;
pub mod config;
pub mod lcu;
pub mod logging;
pub mod loot;
pub mod parsers;
pub mod signalr;
pub mod tools;
pub mod updater;
pub mod upload;

use crate::updater::{PendingUpdate, UpdateInfo};
use base64::Engine;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::Arc;
use std::sync::Mutex;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::window::Effect;
use tauri::Emitter;
use tauri::Manager;
use tokio::sync::{mpsc, watch, RwLock, Semaphore};

/// 包装 tauri::async_runtime::spawn，捕获并记录后台任务异常终止。
/// 不会丢失 JoinHandle 的错误信息，避免任务静默崩溃。
pub(crate) fn spawn_log_panic<F>(future: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let handle = tauri::async_runtime::spawn(future);
    tauri::async_runtime::spawn(async move {
        if let Err(e) = handle.await {
            log::error!("后台任务异常终止: {:?}", e);
        }
    });
}

/// LCU 连接凭证及预配置的 HTTP Client
pub struct LcuClient {
    pub pid: u32,
    pub port: u16,
    pub token: String,
    /// 登录大区标识（来自 LeagueClientUx 命令行 --rso_platform_id=），
    /// 用于 SGP 观战等需要大区信息的场景。非腾讯大区时为 None。
    pub server: Option<String>,
    pub http_client: reqwest::Client,
}

/// 供 Tauri 管理的全局状态
pub struct AppState {
    pub lcu_client: Arc<RwLock<Option<LcuClient>>>,
    pub config: Arc<RwLock<config::AppConfig>>,
    /// LCU 连接后加载的游戏资源路径映射（物品/技能/符文 iconPath）
    pub game_data: Arc<RwLock<lcu::game_data::GameDataAssets>>,
    /// BP agent 的选人会话发送端
    pub bp_session_tx: mpsc::Sender<agents::auto_bp::ChampSelectSession>,
    /// 游戏流程 agent 的事件发送端
    pub gameflow_tx: mpsc::Sender<agents::auto_match::GameflowEvent>,
    /// 上传队列（可用于外部手动触发上传）
    pub upload_queue: Arc<upload::UploadQueue>,
    /// WebSocket 连接取消信号发送端（新连接时发送取消旧循环）
    pub ws_cancel_tx: Mutex<Option<watch::Sender<bool>>>,
    /// LCU API 并发信号量（由 config.ApiConcurrencyNumber 控制）
    pub api_semaphore: RwLock<Arc<Semaphore>>,
    /// BP 状态重置标志（gameflow 阶段变化时置为 true，BP agent 检查后置 false）
    pub bp_reset_flag: AtomicBool,
    /// BP 锁定后台任务版本号（用于标记和防止残留协程竞态）
    pub bp_task_id: AtomicU64,
    /// 后台下载进行中标志，防止重复启动多个下载
    pub is_downloading: AtomicBool,
    /// 正在后台下载的更新信息
    pub downloading_update: Mutex<Option<UpdateInfo>>,
    /// 后台已下载完成的待安装更新
    pub pending_update: Mutex<Option<PendingUpdate>>,
}

impl AppState {
    /// 获取 LCU 连接读锁，未连接时返回错误
    pub async fn lcu(&self) -> Result<tokio::sync::RwLockReadGuard<'_, Option<LcuClient>>, String> {
        let lock = self.lcu_client.read().await;
        if lock.is_some() {
            Ok(lock)
        } else {
            Err("LCU 未连接，请先启动英雄联盟客户端".to_string())
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            let _ = app.emit("single-instance", (argv, cwd));
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // 加载配置并做 clamp 限制，防止因 api_concurrency_number 为 0 导致请求挂起
            let mut app_config = config::AppConfig::load();
            if !(1..=32).contains(&app_config.functions.api_concurrency_number) {
                let clamped = app_config.functions.api_concurrency_number.clamp(1, 32);
                log::warn!(
                    "配置的 API 并发数 {} 不在 1..=32 范围内，已自动调整为 {}",
                    app_config.functions.api_concurrency_number,
                    clamped
                );
                app_config.functions.api_concurrency_number = clamped;
                app_config.save();
            }
            let api_concurrency = app_config.functions.api_concurrency_number as usize;

            let app_config_arc = Arc::new(RwLock::new(app_config));
            log::info!("配置已加载");

            // 创建 agent 通信 channels
            let (bp_tx, bp_rx) = mpsc::channel::<agents::auto_bp::ChampSelectSession>(32);
            let (gameflow_tx, gameflow_rx) = mpsc::channel::<agents::auto_match::GameflowEvent>(32);

            // 创建上传队列
            let upload_queue = Arc::new(upload::UploadQueue::new(app.handle().clone()));
            let upload_trigger = upload::UploadTrigger::new(upload_queue.clone());

            // 初始化全局状态
            let lcu_state: Arc<RwLock<Option<LcuClient>>> = Arc::new(RwLock::new(None));
            let game_data: Arc<RwLock<lcu::game_data::GameDataAssets>> =
                Arc::new(RwLock::new(lcu::game_data::GameDataAssets::default()));
            let state = AppState {
                lcu_client: lcu_state.clone(),
                config: app_config_arc.clone(),
                game_data: game_data.clone(),
                bp_session_tx: bp_tx,
                gameflow_tx,
                upload_queue,
                ws_cancel_tx: Mutex::new(None),
                api_semaphore: RwLock::new(Arc::new(Semaphore::new(api_concurrency))),
                bp_reset_flag: AtomicBool::new(false),
                bp_task_id: AtomicU64::new(0),
                is_downloading: AtomicBool::new(false),
                downloading_update: Mutex::new(None),
                pending_update: Mutex::new(None),
            };
            app.manage(state);

            // 启动 Agents
            agents::auto_bp::start(app.handle().clone(), bp_rx);
            agents::auto_match::start(app.handle().clone(), gameflow_rx, upload_trigger);
            agents::auto_screenshot::start(app.handle().clone());

            // 启动 LCU 进程监测
            let app_handle = app.handle().clone();
            lcu::monitor::start(app_handle, lcu_state, game_data);

            // 条件启动 SignalR Hub 远程反代
            {
                let cfg_snapshot = app_config_arc.blocking_read();
                let general = &cfg_snapshot.general;
                let functions = &cfg_snapshot.functions;
                if functions.lcu_realtime_enabled && !general.upload_api_url.is_empty() {
                    let server_url = general.upload_api_url.clone();
                    let user_id = if !general.signalr_user_id.is_empty() {
                        general.signalr_user_id.clone()
                    } else if !functions.lcu_user_id.is_empty() {
                        functions.lcu_user_id.clone()
                    } else {
                        "lcu_user_001".to_string()
                    };
                    log::info!("启动 SignalR Hub 远程反代");
                    signalr::start(app.handle().clone(), server_url, user_id);
                }
            }

            // 开发模式下自动打开 DevTools
            #[cfg(debug_assertions)]
            {
                if let Some(window) = app.get_webview_window("main") {
                    window.open_devtools();
                }
            }

            // ─── 云母效果 (Win11 Mica) ───
            {
                let cfg_snapshot = app_config_arc.blocking_read();
                if cfg_snapshot.personalization.mica_enabled {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.set_effects(tauri::utils::config::WindowEffectsConfig {
                            effects: vec![Effect::Mica],
                            state: None,
                            radius: None,
                            color: None,
                        });
                        log::info!("已启用云母效果 (Mica)");
                    }
                }
            }

            // ─── 系统托盘 ───
            let hide_tft = app_config_arc.blocking_read().functions.hide_tft;
            let tray_menu = crate::commands::config::build_tray_menu(app.handle(), hide_tft)?;

            let _tray = TrayIconBuilder::with_id("main_tray")
                .icon(app.default_window_icon().cloned().unwrap_or_else(|| {
                    log::warn!("default_window_icon 为 None，使用 1x1 透明像素占位");
                    tauri::image::Image::new(&[0, 0, 0, 0], 1, 1)
                }))
                .menu(&tray_menu)
                .tooltip("Yuumi")
                .on_menu_event(
                    move |app: &tauri::AppHandle, event: tauri::menu::MenuEvent| {
                        let id = event.id().as_ref().to_string();
                        if id == "exit" {
                            app.exit(0);
                        } else {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.unminimize();
                                let _ = window.show();
                                let _ = window.set_focus();
                                // 重新应用云母效果以防失效
                                let state = app.state::<AppState>();
                                let is_mica_enabled = {
                                    if let Ok(cfg) = state.config.try_read() {
                                        cfg.personalization.mica_enabled
                                    } else {
                                        false
                                    }
                                };
                                if is_mica_enabled {
                                    let _ =
                                        crate::commands::tools::set_mica_effect(app.clone(), true);
                                }
                            }
                            let _ = app.emit("tray-navigate", &id);
                        }
                    },
                )
                .on_tray_icon_event(|tray: &tauri::tray::TrayIcon, event: TrayIconEvent| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.unminimize();
                            let _ = window.show();
                            let _ = window.set_focus();
                            // 重新应用云母效果以防失效
                            let state = app.state::<AppState>();
                            let is_mica_enabled = {
                                if let Ok(cfg) = state.config.try_read() {
                                    cfg.personalization.mica_enabled
                                } else {
                                    false
                                }
                            };
                            if is_mica_enabled {
                                let _ = crate::commands::tools::set_mica_effect(app.clone(), true);
                            }
                        }
                    }
                })
                .build(app)?;

            // ─── 启动时静默检查并后台下载更新 ───
            {
                let cfg_snapshot = app_config_arc.blocking_read();
                let enable_check = cfg_snapshot.general.enable_check_update;
                drop(cfg_snapshot);
                if enable_check {
                    let app_handle = app.handle().clone();
                    crate::spawn_log_panic(async move {
                        // 延迟 3 秒，等待主窗口完全加载后再检查
                        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                        // start_background_download 内部自动完成：检查→后台下载→存储→事件通知
                        updater::start_background_download(app_handle).await;
                    });
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            lcu::client::call_lcu_api,
            lcu::client::get_lcu_asset,
            parsers::summoner::get_current_summoner,
            parsers::match_parser::get_match_history,
            parsers::match_parser::get_match_history_sgp,
            parsers::match_parser::get_recent_teammates,
            parsers::game_info::get_game_player_summaries,
            parsers::game_info::get_player_fate_info,
            parsers::tft::get_tft_data,
            tools::create_5v5_practice_lobby,
            tools::aram_reroll_and_swap_back,
            tools::apply_rune_page,
            tools::get_lcu_zoom,
            tools::fix_lcu_window,
            tools::clear_game_cache,
            tools::open_log_folder,
            tools::fetch_opgg_data,
            tools::get_champion_skins,
            tools::get_game_settings_readonly,
            tools::set_game_settings_readonly,
            tools::spectate_directly,
            loot::get_openable_loots,
            loot::batch_open_loots,
            loot::smart_open_all_loots,
            loot::get_loot_inventory,
            loot::disenchant_loot,
            loot::reroll_loot,
            loot::upgrade_loot,
            loot::get_essence_balances,
            commands::config::get_config,
            commands::config::update_config,
            commands::config::get_config_load_error,
            commands::config::get_close_to_tray,
            commands::lcu::get_lcu_connection_info,
            commands::lcu::get_map_side,
            commands::tools::detect_lol_path,
            commands::tools::select_lol_folder,
            commands::tools::select_folder,
            commands::tools::open_screenshot_folder,
            commands::tools::set_mica_effect,
            commands::tools::launch_lol_client,
            commands::lcu::get_game_data_assets,
            upload::upload_single_match,
            upload::batch_upload_matches,
            signalr::get_signalr_status,
            updater::check_update,
            updater::install_update,
            updater::install_pending_update,
            commands::tools::show_bench_overlay_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("你好, {}! 欢迎使用 Yuumi!", name)
}

/// 构建 LCU Basic Auth header 值
pub fn build_auth_header(token: &str) -> String {
    let credentials = format!("riot:{}", token);
    let encoded = base64::engine::general_purpose::STANDARD.encode(&credentials);
    format!("Basic {}", encoded)
}
