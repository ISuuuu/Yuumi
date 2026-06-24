use tauri::{AppHandle, Manager};
use tokio::sync::mpsc;

use crate::config::FunctionsConfig;

// ─── 游戏流程事件 ───

#[derive(Debug, Clone)]
pub enum GameflowEvent {
    PhaseChanged(String),
    ReadyCheck(ReadyCheckData),
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadyCheckData {
    pub state: Option<String>,
    pub player_response: Option<String>,
}

/// 启动游戏流程自动化后台任务。
/// 处理：自动接受匹配、自动重连。
pub fn start(app_handle: AppHandle, mut rx: mpsc::Receiver<GameflowEvent>) {
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let cfg = {
                let state = app_handle.state::<crate::AppState>();
                let lock = state.config.read().await;
                lock.functions.clone()
            };

            match event {
                GameflowEvent::PhaseChanged(phase) => {
                    handle_phase_change(&app_handle, &phase, &cfg).await;
                }
                GameflowEvent::ReadyCheck(data) => {
                    handle_ready_check(&app_handle, &data, &cfg).await;
                }
            }
        }
    });
}

/// 游戏阶段变化处理
async fn handle_phase_change(app_handle: &AppHandle, phase: &str, cfg: &FunctionsConfig) {
    log::info!("游戏阶段: {}", phase);

    // 自动重连
    if phase == "InProgress" && cfg.enable_auto_reconnect {
        log::info!("检测到游戏进行中，尝试自动重连...");
        lcu_post(app_handle, "/lol-gameflow/v1/reconnect").await;
    }
}

/// 匹配准备检查处理
async fn handle_ready_check(
    app_handle: &AppHandle,
    data: &ReadyCheckData,
    cfg: &FunctionsConfig,
) {
    if !cfg.enable_auto_accept_matching {
        return;
    }

    // 只在玩家还未响应时自动接受
    if let Some(ref response) = data.player_response {
        if response != "None" && response != "Pending" {
            return;
        }
    }

    let delay = cfg.auto_accept_matching_delay;
    if delay > 0 {
        tokio::time::sleep(std::time::Duration::from_millis(delay as u64)).await;
    }

    log::info!("自动接受匹配");
    lcu_post(app_handle, "/lol-matchmaking/v1/ready-check/accept").await;
}

/// 通用 LCU POST 请求
async fn lcu_post(app_handle: &AppHandle, path: &str) -> bool {
    let state = app_handle.state::<crate::AppState>();
    let lock = state.lcu_client.read().await;
    let lcu = match lock.as_ref() {
        Some(c) => c,
        None => return false,
    };

    let url = format!("https://127.0.0.1:{}{}", lcu.port, path);
    let auth = crate::build_auth_header(&lcu.token);

    match lcu
        .http_client
        .post(&url)
        .header("Authorization", auth)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => true,
        Ok(resp) => {
            log::warn!("LCU POST {} 失败: HTTP {}", path, resp.status());
            false
        }
        Err(e) => {
            log::error!("LCU POST {} 请求失败: {}", path, e);
            false
        }
    }
}
