use std::sync::Arc;
use sysinfo::{ProcessesToUpdate, System};
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};

use crate::LcuClient;

const POLL_INTERVAL: Duration = Duration::from_secs(2);
const PROCESS_NAME: &str = "LeagueClientUx.exe";

/// 启动 LCU 进程轮询监测器。
/// 每 2 秒检测一次 LeagueClientUx.exe 是否存在，
/// 若存在则从命令行参数中提取 --app-port 和 --remoting-auth-token，
/// 更新全局 LcuClient 状态并广播事件。
pub fn start(app_handle: AppHandle, lcu_state: Arc<RwLock<Option<LcuClient>>>) {
    tokio::spawn(async move {
        let mut sys = System::new_all();
        let mut was_connected = false;

        loop {
            sleep(POLL_INTERVAL).await;
            sys.refresh_processes(ProcessesToUpdate::All, true);

            let lcu_info = find_lcu_process(&sys);

            let mut lock = lcu_state.write().await;

            match lcu_info {
                Some((port, token)) => {
                    // LCU 已启动
                    let needs_reconnect = match lock.as_ref() {
                        Some(client) => client.port != port || client.token != token,
                        None => true,
                    };

                    if needs_reconnect {
                        log::info!("检测到 LCU: port={}, 建立新连接", port);

                        // 创建忽略 SSL 的 HTTP 客户端
                        match reqwest::Client::builder()
                            .danger_accept_invalid_certs(true)
                            .build()
                        {
                            Ok(http_client) => {
                                let client = LcuClient {
                                    port,
                                    token: token.clone(),
                                    http_client,
                                };
                                *lock = Some(client);
                                was_connected = true;

                                // 广播前端
                                let _ = app_handle.emit(
                                    "lcu-client-started",
                                    serde_json::json!({ "port": port }),
                                );

                                // 启动 WebSocket 监听
                                super::ws::connect(app_handle.clone(), port, token.clone());
                            }
                            Err(e) => {
                                log::error!("创建 HTTP 客户端失败: {}", e);
                            }
                        }
                    }
                }
                None => {
                    // LCU 已关闭
                    if was_connected {
                        log::info!("LCU 已断开");
                        *lock = None;
                        was_connected = false;
                        let _ = app_handle.emit("lcu-client-ended", ());
                    } else if lock.is_some() {
                        *lock = None;
                    }
                }
            }
        }
    });
}

/// 从 sysinfo 中查找 LeagueClientUx.exe 并提取 port 和 token
fn find_lcu_process(sys: &System) -> Option<(u16, String)> {
    let mut port: Option<u16> = None;
    let mut token: Option<String> = None;

    for (_, process) in sys.processes() {
        if process.name().to_string_lossy() == PROCESS_NAME {
            for arg in process.cmd() {
                let arg = arg.to_string_lossy();
                if let Some(p) = arg.strip_prefix("--app-port=") {
                    port = p.parse::<u16>().ok();
                } else if let Some(t) = arg.strip_prefix("--remoting-auth-token=") {
                    token = Some(t.to_string());
                }
            }
            break;
        }
    }

    match (port, token) {
        (Some(p), Some(t)) => Some((p, t)),
        _ => None,
    }
}
