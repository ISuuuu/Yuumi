pub mod agents;
pub mod config;
pub mod lcu;
pub mod parsers;
pub mod signalr;
pub mod tools;
pub mod upload;

use base64::Engine;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::{mpsc, RwLock};

/// LCU 连接凭证及预配置的 HTTP Client
pub struct LcuClient {
    pub pid: u32,
    pub port: u16,
    pub token: String,
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
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // 加载配置
            let app_config = config::AppConfig::load();
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
                if general.enable_signalr_hub
                    && !general.signalr_server_url.is_empty()
                    && !general.signalr_user_id.is_empty()
                {
                    log::info!("启动 SignalR Hub 远程反代");
                    signalr::start(
                        app.handle().clone(),
                        general.signalr_server_url.clone(),
                        general.signalr_user_id.clone(),
                    );
                }
            }

            // 开发模式下自动打开 DevTools
            #[cfg(debug_assertions)]
            {
                if let Some(window) = app.get_webview_window("main") {
                    window.open_devtools();
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
            parsers::game_info::get_game_player_summaries,
            parsers::tft::get_tft_data,
            tools::create_5v5_practice_lobby,
            tools::aram_reroll_and_swap_back,
            tools::apply_rune_page,
            tools::get_lcu_zoom,
            tools::fix_lcu_window,
            get_config,
            update_config,
            get_lcu_connection_info,
            get_game_data_assets,
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
}

/// 获取当前 LCU 连接信息（PID、端口、Token）
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
        })),
        None => Ok(None),
    }
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

/// 更新配置（接收完整 AppConfig JSON，写入内存并持久化）
#[tauri::command]
async fn update_config(
    new_config: config::AppConfig,
    app_state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut cfg = app_state.config.write().await;
    *cfg = new_config;
    cfg.save();
    Ok(())
}




