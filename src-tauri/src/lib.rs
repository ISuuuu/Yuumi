pub mod agents;
pub mod config;
pub mod lcu;
pub mod parsers;
pub mod tools;

use base64::Engine;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::{mpsc, RwLock};

/// LCU 连接凭证及预配置的 HTTP Client
pub struct LcuClient {
    pub port: u16,
    pub token: String,
    pub http_client: reqwest::Client,
}

/// 供 Tauri 管理的全局状态
pub struct AppState {
    pub lcu_client: Arc<RwLock<Option<LcuClient>>>,
    pub config: Arc<RwLock<config::AppConfig>>,
    /// BP agent 的选人会话发送端
    pub bp_session_tx: mpsc::Sender<agents::auto_bp::ChampSelectSession>,
    /// 游戏流程 agent 的事件发送端
    pub gameflow_tx: mpsc::Sender<agents::auto_match::GameflowEvent>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // 加载配置
            let app_config = config::AppConfig::load();
            log::info!("配置已加载");

            // 创建 agent 通信 channels
            let (bp_tx, bp_rx) = mpsc::channel::<agents::auto_bp::ChampSelectSession>(32);
            let (gameflow_tx, gameflow_rx) = mpsc::channel::<agents::auto_match::GameflowEvent>(32);

            // 初始化全局状态
            let lcu_state: Arc<RwLock<Option<LcuClient>>> = Arc::new(RwLock::new(None));
            let state = AppState {
                lcu_client: lcu_state.clone(),
                config: Arc::new(RwLock::new(app_config)),
                bp_session_tx: bp_tx,
                gameflow_tx,
            };
            app.manage(state);

            // 启动 Agents
            agents::auto_bp::start(app.handle().clone(), bp_rx);
            agents::auto_match::start(app.handle().clone(), gameflow_rx);

            // 启动 LCU 进程监测
            let app_handle = app.handle().clone();
            lcu::monitor::start(app_handle, lcu_state);

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
            parsers::summoner::get_current_summoner,
            parsers::match_parser::get_match_history,
            parsers::game_info::get_game_player_summaries,
            parsers::tft::get_tft_data,
            tools::create_5v5_practice_lobby,
            tools::aram_reroll_and_swap_back,
            tools::apply_rune_page,
            tools::get_lcu_zoom,
            tools::fix_lcu_window,
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
