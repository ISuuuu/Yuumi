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
/// 处理：自动接受匹配、自动重连、自动创建大厅、对局结束上传。
pub fn start(
    app_handle: AppHandle,
    mut rx: mpsc::Receiver<GameflowEvent>,
    upload_trigger: crate::upload::UploadTrigger,
) {
    tokio::spawn(async move {
        let mut lobby_created = false;
        let mut upload_trigger = upload_trigger;

        while let Some(event) = rx.recv().await {
            let cfg = {
                let state = app_handle.state::<crate::AppState>();
                let lock = state.config.read().await;
                lock.functions.clone()
            };

            match event {
                GameflowEvent::PhaseChanged(phase) => {
                    handle_phase_change(
                        &app_handle,
                        &phase,
                        &cfg,
                        &mut lobby_created,
                        &mut upload_trigger,
                    )
                    .await;
                }
                GameflowEvent::ReadyCheck(data) => {
                    handle_ready_check(&app_handle, &data, &cfg).await;
                }
            }
        }
    });
}

/// 游戏阶段变化处理
async fn handle_phase_change(
    app_handle: &AppHandle,
    phase: &str,
    cfg: &FunctionsConfig,
    lobby_created: &mut bool,
    upload_trigger: &mut crate::upload::UploadTrigger,
) {
    log::info!("游戏阶段: {}", phase);

    // 空闲状态 → 自动创建预设大厅
    if phase == "None" {
        if *lobby_created {
            *lobby_created = false;
        }
        if cfg.enable_auto_create_lobby {
            try_create_default_lobby(app_handle, cfg, lobby_created).await;
        }
    }

    // 游戏进行中 → 自动重连
    if phase == "InProgress" && cfg.enable_auto_reconnect {
        log::info!("检测到游戏进行中，尝试自动重连...");
        lcu_post(app_handle, "/lol-gameflow/v1/reconnect").await;
    }

    // 状态转换检测 → 上传队列（包含延迟 2 秒 + 去重）
    upload_trigger.on_phase_change(phase, app_handle).await;
}

/// 自动创建预设大厅（对应 Python `_tryCreateDefaultLobby`）
async fn try_create_default_lobby(
    app_handle: &AppHandle,
    cfg: &FunctionsConfig,
    lobby_created: &mut bool,
) {
    if *lobby_created {
        return;
    }

    let queue_id = cfg.default_game_mode;
    log::info!("自动创建预设大厅: queueId={}", queue_id);

    for attempt in 0..30 {
        // 检查 LCU 是否仍然连接
        {
            let state = app_handle.state::<crate::AppState>();
            let lock = state.lcu_client.read().await;
            if lock.is_none() {
                log::info!("LCU 已断开，停止创建大厅");
                return;
            }
        }

        // 检查当前阶段是否仍为 None
        {
            let state = app_handle.state::<crate::AppState>();
            let lock = state.lcu_client.read().await;
            if let Some(lcu) = lock.as_ref() {
                let url = format!(
                    "https://127.0.0.1:{}/lol-gameflow/v1/gameflow-phase",
                    lcu.port
                );
                let auth = crate::build_auth_header(&lcu.token);
                if let Ok(resp) = lcu
                    .http_client
                    .get(&url)
                    .header("Authorization", auth)
                    .send()
                    .await
                {
                    if let Ok(phase) = resp.text().await {
                        let phase = phase.trim_matches('"');
                        if !matches!(phase, "None" | "" | "WaitingForStats" | "PreEndOfGame") {
                            log::info!("当前阶段为 {}，跳过创建大厅", phase);
                            *lobby_created = true;
                            return;
                        }
                    }
                }
            }
        }

        // 尝试创建大厅
        let body = serde_json::json!({ "queueId": queue_id });
        let state = app_handle.state::<crate::AppState>();
        let lock = state.lcu_client.read().await;
        if let Some(lcu) = lock.as_ref() {
            let url = format!("https://127.0.0.1:{}/lol-lobby/v2/lobby", lcu.port);
            let auth = crate::build_auth_header(&lcu.token);

            match lcu
                .http_client
                .post(&url)
                .header("Authorization", auth)
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    if resp.status().is_success() {
                        log::info!("预设大厅创建成功 (尝试 {})", attempt + 1);
                        *lobby_created = true;
                        return;
                    }
                    let status = resp.status().as_u16();
                    if status < 500 {
                        log::warn!("创建大厅失败 (HTTP {})，停止重试", status);
                        *lobby_created = true;
                        return;
                    }
                    log::warn!("创建大厅失败 (HTTP {})，重试中...", status);
                }
                Err(e) => {
                    log::warn!("创建大厅请求失败: {}，重试中...", e);
                }
            }
        }

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }

    log::warn!("创建预设大厅：30 次重试均失败");
    *lobby_created = true;
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
pub async fn lcu_post(app_handle: &AppHandle, path: &str) -> bool {
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
