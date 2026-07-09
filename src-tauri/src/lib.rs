pub mod agents;
pub mod config;
pub mod lcu;
pub mod logging;
pub mod loot;
pub mod parsers;
pub mod signalr;
pub mod tools;
pub mod updater;
pub mod upload;

use crate::updater::PendingUpdate;
use base64::Engine;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::Arc;
use std::sync::Mutex;
use tauri::menu::{MenuBuilder, MenuItem, PredefinedMenuItem};
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
    pub api_semaphore: Arc<Semaphore>,
    /// BP 状态重置标志（gameflow 阶段变化时置为 true，BP agent 检查后置 false）
    pub bp_reset_flag: AtomicBool,
    /// BP 锁定后台任务版本号（用于标记和防止残留协程竞态）
    pub bp_task_id: AtomicU64,
    /// 后台下载进行中标志，防止重复启动多个下载
    pub is_downloading: AtomicBool,
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
            // 加载配置
            let app_config = config::AppConfig::load();
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
                api_semaphore: Arc::new(Semaphore::new(api_concurrency)),
                bp_reset_flag: AtomicBool::new(false),
                bp_task_id: AtomicU64::new(0),
                is_downloading: AtomicBool::new(false),
                pending_update: Mutex::new(None),
            };
            app.manage(state);

            // 启动 Agents
            agents::auto_bp::start(app.handle().clone(), bp_rx);
            agents::auto_match::start(app.handle().clone(), gameflow_rx, upload_trigger);

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
            let tray_menu = build_tray_menu(app.handle(), hide_tft)?;

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
                                    let _ = set_mica_effect(app.clone(), true);
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
                                let _ = set_mica_effect(app.clone(), true);
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
            parsers::game_info::get_game_player_summaries,
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
            get_config,
            update_config,
            get_config_load_error,
            get_close_to_tray,
            get_lcu_connection_info,
            get_map_side,
            detect_lol_path,
            select_lol_folder,
            set_mica_effect,
            launch_lol_client,
            get_game_data_assets,
            upload::upload_single_match,
            upload::batch_upload_matches,
            signalr::get_signalr_status,
            updater::check_update,
            updater::install_update,
            updater::install_pending_update,
            show_bench_overlay_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDataAssetsDisplay {
    pub items: std::collections::HashMap<i32, String>,
    pub spells: std::collections::HashMap<i32, String>,
    pub runes: std::collections::HashMap<i32, String>,
}

/// 获取 LCU 预加载的静态资源映射 (ID -> iconPath)
#[tauri::command]
async fn get_game_data_assets(
    app_state: tauri::State<'_, AppState>,
) -> Result<GameDataAssetsDisplay, String> {
    let gd = app_state.game_data.read().await;
    Ok(GameDataAssetsDisplay {
        items: gd.items.clone(),
        spells: gd.spells.clone(),
        runes: gd.runes.clone(),
    })
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LcuConnectionDetails {
    pub pid: u32,
    pub port: u16,
    pub token: String,
    pub server: Option<String>,
}

/// 获取当前 LCU 连接信息（PID、端口、Token、大区）
#[tauri::command]
async fn get_lcu_connection_info(
    app_state: tauri::State<'_, AppState>,
) -> Result<Option<LcuConnectionDetails>, String> {
    let lock = app_state.lcu_client.read().await;
    match lock.as_ref() {
        Some(client) => Ok(Some(LcuConnectionDetails {
            pid: client.pid,
            port: client.port,
            token: client.token.clone(),
            server: client.server.clone(),
        })),
        None => Ok(None),
    }
}

/// 获取选人阶段所在队伍（蓝色方/红色方）
#[tauri::command]
async fn get_map_side(app_state: tauri::State<'_, AppState>) -> Result<Option<String>, String> {
    let lock = app_state.lcu_client.read().await;
    let lcu = lock.as_ref().ok_or("LCU 未连接")?;

    let auth = build_auth_header(&lcu.token);
    let base = format!("https://127.0.0.1:{}", lcu.port);

    // 方法1: 从 pin-drop-notification 获取 mapSide
    // 重试最多 5 次因为选人会话初始化可能稍有延迟
    let map_side_url = format!("{}/lol-champ-select/v1/pin-drop-notification", base);
    for i in 0..5 {
        if i > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(600)).await;
        }
        match lcu
            .http_client
            .get(&map_side_url)
            .header("Authorization", &auth)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(data) = resp.json::<serde_json::Value>().await {
                    if let Some(side) = data.get("mapSide").and_then(|v| v.as_str()) {
                        if !side.is_empty() {
                            log::info!("获取队伍信息成功 (pin-drop): {}", side);
                            return Ok(Some(side.to_string()));
                        }
                    }
                }
            }
            Ok(resp) => log::warn!("pin-drop-notification 返回 HTTP {}", resp.status()),
            Err(e) => log::warn!("pin-drop-notification 请求失败: {}", e),
        }
    }

    // 方法2: 读取选人会话来推断队伍
    // 如果 myTeam 的 `cellId` 小的一方为蓝色方
    let session_url = format!("{}/lol-champ-select/v1/session", base);
    match lcu
        .http_client
        .get(&session_url)
        .header("Authorization", &auth)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(data) = resp.json::<serde_json::Value>().await {
                let _cell_id = data
                    .get("localPlayerCellId")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                if let Some(my_team) = data.get("myTeam").and_then(|v| v.as_array()) {
                    // 检查 myTeam 中最小 cellId 来判断哪一侧
                    let min_cell = my_team
                        .iter()
                        .filter_map(|p| p.get("cellId").and_then(|c| c.as_i64()))
                        .min()
                        .unwrap_or(0);
                    let max_cell = my_team
                        .iter()
                        .filter_map(|p| p.get("cellId").and_then(|c| c.as_i64()))
                        .max()
                        .unwrap_or(0);
                    // 在 5v5 中，cellId 范围 0-4 = 蓝色方，5-9 = 红色方
                    let side = if min_cell < 5 && max_cell < 5 {
                        "blue"
                    } else if min_cell >= 5 {
                        "red"
                    } else {
                        // 无法从 cellId 确定，尝试从已用的英雄 ID 推断
                        return Ok(None);
                    };
                    log::info!(
                        "获取队伍信息成功 (session cellId): {}, min={}, max={}",
                        side,
                        min_cell,
                        max_cell
                    );
                    return Ok(Some(side.to_string()));
                }
            }
        }
        _ => {}
    }

    log::warn!("无法确定队伍信息");
    Ok(None)
}

/// 自动检测 LOL 客户端安装路径（从运行中的 LeagueClientUx.exe 推断，或从 Windows 注册表兜底）
#[tauri::command]
fn detect_lol_path() -> Result<Option<String>, String> {
    use sysinfo::System;
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    // 1. 优先从运行中的客户端进程推断
    for process in sys.processes().values() {
        let name = process.name().to_string_lossy().to_lowercase();
        if name == "leagueclientux.exe" {
            if let Some(exe_path) = process.exe() {
                let mut dir = exe_path.parent();
                while let Some(d) = dir {
                    if d.join("LeagueClient.exe").exists()
                        || d.join("Client.exe").exists()
                        || d.join("client.exe").exists()
                    {
                        return Ok(Some(d.to_string_lossy().to_string()));
                    }
                    dir = d.parent();
                }
                // 兜底：返回 exe 的上两级
                if let Some(parent) = exe_path.parent() {
                    if let Some(root) = parent.parent() {
                        return Ok(Some(root.to_string_lossy().to_string()));
                    }
                }
            }
        }
    }

    // 2. 进程未运行，则按照 Python 逻辑，尝试从 Windows 注册表获取国服 LOL 路径
    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = std::process::Command::new("reg")
            .args(["query", r"HKCU\SOFTWARE\Tencent\LOL", "/v", "InstallPath"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if line.contains("InstallPath") {
                        if let Some(pos) = line.find("REG_SZ") {
                            let raw_path = line[pos + 6..].trim();
                            if !raw_path.is_empty() {
                                // 统一成正斜杠格式，规范盘符大小写
                                let mut path = raw_path.replace("\\", "/");
                                if path.len() >= 2 && path.as_bytes()[1] == b':' {
                                    let drive =
                                        path.chars().next().unwrap().to_uppercase().to_string();
                                    path = format!("{}{}", drive, &path[1..]);
                                }

                                // 如果是国服，注册表读出来的安装目录下有 TCLS 目录
                                let tcls_dir = std::path::Path::new(&path).join("TCLS");
                                if tcls_dir.exists() {
                                    return Ok(Some(tcls_dir.to_string_lossy().replace("\\", "/")));
                                }

                                return Ok(Some(path));
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(None)
}

/// 打开原生文件夹选择对话框，返回用户选择的路径
#[tauri::command]
fn select_lol_folder() -> Result<Option<String>, String> {
    let folder = rfd::FileDialog::new()
        .set_title("选择英雄联盟客户端安装目录")
        .pick_folder();
    Ok(folder.map(|p| p.to_string_lossy().to_string()))
}

/// 运行时切换云母效果
#[tauri::command]
fn set_mica_effect(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        if enabled {
            window
                .set_effects(tauri::utils::config::WindowEffectsConfig {
                    effects: vec![Effect::Mica],
                    state: None,
                    radius: None,
                    color: None,
                })
                .map_err(|e| e.to_string())?;
        } else {
            window
                .set_effects(tauri::utils::config::WindowEffectsConfig {
                    effects: vec![],
                    state: None,
                    radius: None,
                    color: None,
                })
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// 启动 LOL 客户端（指定路径或从配置中的 lol_path 查找）
#[tauri::command]
async fn launch_lol_client(
    app_state: tauri::State<'_, AppState>,
    path: Option<String>,
) -> Result<(), String> {
    // 先检查是否已有客户端在运行
    {
        use sysinfo::System;
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        let already_running = sys.processes().values().any(|p| {
            p.name()
                .to_string_lossy()
                .eq_ignore_ascii_case("leagueclientux.exe")
        });
        if already_running {
            log::info!("客户端已在运行，跳过启动");
            return Ok(());
        }
    }

    // 智能探测客户端执行文件的辅助函数
    let find_executable = |base_path: &str| -> Option<std::path::PathBuf> {
        let p = std::path::Path::new(base_path);
        let check_list = &[
            ("", "LeagueClient.exe"),
            ("", "Client.exe"),
            ("", "client.exe"),
            ("TCLS", "client.exe"),
            ("TCLS", "Client.exe"),
            ("LeagueClient", "LeagueClient.exe"),
            ("../TCLS", "client.exe"),
            ("../TCLS", "Client.exe"),
            ("../LeagueClient", "LeagueClient.exe"),
        ];

        for (sub_dir, exe_name) in check_list {
            let exe = if sub_dir.is_empty() {
                p.join(exe_name)
            } else {
                p.join(sub_dir).join(exe_name)
            };
            if exe.exists() {
                return Some(exe);
            }
        }
        None
    };

    // 启动可执行文件并处理 UAC 提升的辅助函数
    let spawn_executable = |exe: &std::path::Path, args: &[&str]| -> Result<(), String> {
        let mut cmd = std::process::Command::new(exe);
        cmd.args(args);
        // 关键：设置启动工作目录为 exe 所在的父目录，防止 DLL 加载或配置读取报拒绝访问错误 (os error 5)
        if let Some(parent) = exe.parent() {
            cmd.current_dir(parent);
        }
        match cmd.spawn() {
            Ok(_) => Ok(()),
            Err(e) => {
                let os_err = e.raw_os_error();
                // 拦截 740 (需要提升) 与 5 (拒绝访问) 并尝试以 UAC 管理员提权运行
                if os_err == Some(740) || os_err == Some(5) {
                    log::info!(
                        "启动 LOL 客户端遇到权限限制 ({:?})，尝试提升权限启动...",
                        os_err
                    );
                    #[cfg(target_os = "windows")]
                    {
                        use std::os::windows::process::CommandExt;
                        let working_dir = exe
                            .parent()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();

                        // 格式化参数传给 PowerShell
                        let args_str = args
                            .iter()
                            .map(|arg| format!("'{}'", arg))
                            .collect::<Vec<String>>()
                            .join(", ");

                        let command_str = if args_str.is_empty() {
                            format!(
                                "Start-Process -FilePath '{}' -WorkingDirectory '{}' -Verb RunAs",
                                exe.to_string_lossy(),
                                working_dir
                            )
                        } else {
                            format!("Start-Process -FilePath '{}' -ArgumentList {} -WorkingDirectory '{}' -Verb RunAs", exe.to_string_lossy(), args_str, working_dir)
                        };

                        let status = std::process::Command::new("powershell")
                            .creation_flags(0x08000000) // 隐藏 powershell 窗口
                            .args(["-Command", &command_str])
                            .spawn();
                        if status.is_ok() {
                            return Ok(());
                        }
                    }
                }
                Err(format!("启动失败: {}", e))
            }
        }
    };

    // 智能转换：若是 Riot 纳管的外服，改由 RiotClientServices.exe 启动
    let check_and_launch = |exe: std::path::PathBuf| -> Result<(), String> {
        let mut riot_service = None;
        if exe.file_name().map(|n| n.to_string_lossy().to_lowercase())
            == Some("leagueclient.exe".to_string())
        {
            if let Some(parent) = exe.parent() {
                let same_level = parent.join("RiotClientServices.exe");
                if same_level.exists() {
                    riot_service = Some(same_level);
                } else if let Some(grandparent) = parent.parent() {
                    let parent_level = grandparent.join("RiotClientServices.exe");
                    if parent_level.exists() {
                        riot_service = Some(parent_level);
                    }
                }
            }
        }

        if let Some(service) = riot_service {
            log::info!("检测到外服 Riot 纳管客户端，改由 RiotClientServices.exe 启动");
            let is_pbe = exe.to_string_lossy().to_lowercase().contains("pbe");
            let patchline = if is_pbe { "pbe" } else { "live" };
            let args = &[
                "--launch-product=league_of_legends",
                &format!("--launch-patchline={}", patchline),
            ];
            log::info!("启动 Riot 服务: {:?} {:?}", service, args);
            spawn_executable(&service, args)?;
        } else {
            log::info!("常规方式启动客户端: {:?}", exe);
            spawn_executable(&exe, &[])?;
        }
        Ok(())
    };

    // 指定了路径则直接用
    if let Some(p) = path {
        if let Some(exe) = find_executable(&p) {
            log::info!("启动 LOL 客户端: {:?}", exe);
            check_and_launch(exe)?;
            return Ok(());
        }
        return Err(format!(
            "在 {} 中未找到启动程序 (TCLS/client.exe 或 LeagueClient.exe)",
            p
        ));
    }

    // 否则遍历配置路径
    let cfg = app_state.config.read().await;
    for p in &cfg.general.lol_path {
        if let Some(exe) = find_executable(p) {
            log::info!("启动 LOL 客户端: {:?}", exe);
            check_and_launch(exe)?;
            return Ok(());
        }
    }
    Err("未找到 LeagueClient.exe / Client.exe，请先在设置中配置客户端路径".to_string())
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

/// 获取完整配置
#[tauri::command]
async fn get_config(app_state: tauri::State<'_, AppState>) -> Result<config::AppConfig, String> {
    let cfg = app_state.config.read().await;
    Ok(cfg.clone())
}

/// 读取配置加载错误信息（前端启动时调用，读取后自动清除错误文件）
#[tauri::command]
fn get_config_load_error() -> Option<String> {
    config::AppConfig::take_load_error()
}

/// 校验关键配置字段，防止恶意篡改
fn validate_config(cfg: &config::AppConfig) -> Result<(), String> {
    let url = &cfg.general.upload_api_url;
    if !url.is_empty() && !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("upload_api_url 必须以 http:// 或 https:// 开头".to_string());
    }
    let srv = &cfg.general.signalr_server_url;
    if !srv.is_empty() && !srv.starts_with("http://") && !srv.starts_with("https://") {
        return Err("signalr_server_url 必须以 http:// 或 https:// 开头".to_string());
    }
    if !cfg.personalization.theme_color.starts_with('#') {
        return Err("theme_color 必须是以 # 开头的颜色值".to_string());
    }
    Ok(())
}

/// 更新配置（接收完整 AppConfig JSON，写入内存并持久化）
#[tauri::command]
async fn update_config(
    new_config: config::AppConfig,
    app_state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    validate_config(&new_config)?;

    let (old_enable, old_mode, old_realtime, old_api_url, old_user_id) = {
        let lock = app_state.config.read().await;
        (
            lock.functions.enable_auto_create_lobby,
            lock.functions.default_game_mode,
            lock.functions.lcu_realtime_enabled,
            lock.general.upload_api_url.clone(),
            if !lock.general.signalr_user_id.is_empty() {
                lock.general.signalr_user_id.clone()
            } else if !lock.functions.lcu_user_id.is_empty() {
                lock.functions.lcu_user_id.clone()
            } else {
                "lcu_user_001".to_string()
            },
        )
    };

    let enable_changed = new_config.functions.enable_auto_create_lobby != old_enable;
    let mode_changed = new_config.functions.default_game_mode != old_mode;

    let new_user_id = if !new_config.general.signalr_user_id.is_empty() {
        new_config.general.signalr_user_id.clone()
    } else if !new_config.functions.lcu_user_id.is_empty() {
        new_config.functions.lcu_user_id.clone()
    } else {
        "lcu_user_001".to_string()
    };

    let signalr_changed = new_config.functions.lcu_realtime_enabled != old_realtime
        || new_config.general.upload_api_url != old_api_url
        || new_user_id != old_user_id;

    let hide_tft_changed = {
        let lock = app_state.config.read().await;
        lock.functions.hide_tft != new_config.functions.hide_tft
    };

    {
        let mut cfg = app_state.config.write().await;
        *cfg = new_config.clone();
        cfg.save();
    }

    if hide_tft_changed {
        if let Some(tray) = app_handle.tray_by_id("main_tray") {
            if let Ok(new_menu) = build_tray_menu(&app_handle, new_config.functions.hide_tft) {
                let _ = tray.set_menu(Some(new_menu));
            }
        }
    }

    if enable_changed || mode_changed {
        let _ = app_state
            .gameflow_tx
            .try_send(crate::agents::auto_match::GameflowEvent::ResetLobbyState);
    }

    if signalr_changed {
        if new_config.functions.lcu_realtime_enabled
            && !new_config.general.upload_api_url.is_empty()
        {
            log::info!("配置更新，重新启动 SignalR Hub 远程反代");
            let server_url = new_config.general.upload_api_url.clone();
            signalr::start(app_handle, server_url, new_user_id);
        } else {
            log::info!("配置更新，停止 SignalR Hub 远程反代");
            signalr::stop().await;
        }
    }

    Ok(())
}

/// 读取「关闭时最小化到托盘」开关状态
#[tauri::command]
async fn get_close_to_tray(app_state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let cfg = app_state.config.read().await;
    Ok(cfg.general.enable_close_to_tray.unwrap_or(false))
}

/// 大乱斗板凳席置顶悬浮窗控制命令
#[tauri::command]
async fn show_bench_overlay_window(app_handle: tauri::AppHandle, show: bool) -> Result<(), String> {
    let window = app_handle.get_webview_window("bench-overlay");
    if show {
        if let Some(win) = window {
            let _ = win.show();
            let _ = win.set_focus();
            if let Ok(Some(monitor)) = win.current_monitor() {
                let pos = monitor.position().to_logical::<f64>(monitor.scale_factor());
                let size = monitor.size().to_logical::<f64>(monitor.scale_factor());
                let x = pos.x + (size.width - 550.0) / 2.0;
                let y = pos.y; // 动态定位至该显示器的最顶端
                let _ =
                    win.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(x, y)));
            }
        } else {
            // 计算默认的顶部居中位置
            let mut x = 0.0;
            let mut y = 0.0;
            if let Ok(Some(monitor)) = app_handle.primary_monitor() {
                let pos = monitor.position().to_logical::<f64>(monitor.scale_factor());
                let size = monitor.size().to_logical::<f64>(monitor.scale_factor());
                x = pos.x + (size.width - 550.0) / 2.0;
                y = pos.y; // 动态定位至主显示器的最顶端
            }

            let win = tauri::WebviewWindowBuilder::new(
                &app_handle,
                "bench-overlay",
                tauri::WebviewUrl::App("index.html?window=bench-overlay".into()),
            )
            .title("Yuumi - ARAM Bench")
            .inner_size(550.0, 70.0)
            .position(x, y)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .resizable(false)
            .skip_taskbar(true)
            .build()
            .map_err(|e| e.to_string())?;

            let _ = win.show();
        }
    } else {
        if let Some(win) = window {
            let _ = win.close();
        }
    }
    Ok(())
}

/// 构建系统托盘菜单
fn build_tray_menu<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    hide_tft: bool,
) -> Result<tauri::menu::Menu<R>, tauri::Error> {
    let mut builder = MenuBuilder::new(app)
        .item(&MenuItem::with_id(app, "home", "主页", true, None::<&str>)?)
        .item(&MenuItem::with_id(
            app,
            "career",
            "生涯",
            true,
            None::<&str>,
        )?)
        .item(&MenuItem::with_id(
            app,
            "search",
            "战绩查询",
            true,
            None::<&str>,
        )?)
        .item(&MenuItem::with_id(
            app,
            "gameinfo",
            "对局信息",
            true,
            None::<&str>,
        )?);

    if !hide_tft {
        builder = builder.item(&MenuItem::with_id(app, "tft", "TFT", true, None::<&str>)?);
    }

    let menu = builder
        .item(&MenuItem::with_id(
            app,
            "tools",
            "其他功能",
            true,
            None::<&str>,
        )?)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&MenuItem::with_id(
            app,
            "settings",
            "设置",
            true,
            None::<&str>,
        )?)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&MenuItem::with_id(app, "exit", "退出", true, None::<&str>)?)
        .build()?;

    Ok(menu)
}
