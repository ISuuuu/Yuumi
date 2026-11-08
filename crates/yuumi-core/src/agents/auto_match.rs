//! 自动对局流程 Agent（接受匹配、重连、再来一局、荣誉点赞）

use crate::agents::config::AutoMatchConfig;
use crate::agents::lifecycle::{GamePhase, LifecycleManager};
use crate::agents::supervisor::{AgentSupervisor, DuplicateStrategy, TaskKind};
use crate::lcu::client::LcuClient;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadyCheckData {
    pub state: Option<String>,
    pub player_response: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HonorEligiblePlayer {
    #[serde(default)]
    pub bot_player: bool,
    #[serde(default)]
    pub puuid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HonorBallot {
    #[serde(default)]
    pub eligible_allies: Vec<HonorEligiblePlayer>,
}

/// 游戏流转阶段变迁处理
pub async fn handle_phase_change(
    client: &LcuClient,
    phase: &str,
    config: &AutoMatchConfig,
    supervisor: &AgentSupervisor,
    lifecycle: &LifecycleManager,
) {
    let transition = lifecycle.on_phase_changed(phase).await;
    log::info!("[Agent] 游戏流转阶段推进: phase={}, transition={:?}", phase, transition);

    match GamePhase::from(phase) {
        GamePhase::Reconnect => {
            if config.auto_reconnect {
                let client = client.clone();
                let guard = lifecycle.guard(false).await;
                let supervisor_clone = supervisor.clone();

                let _ = supervisor
                    .spawn(TaskKind::AutoReconnect, DuplicateStrategy::RejectNew, move |ctx| async move {
                        sleep(Duration::from_millis(1500)).await;
                        if ctx.is_cancelled() {
                            return;
                        }
                        let _permit = supervisor_clone.acquire_exclusive_permit().await;
                        if client.post("/lol-gameflow/v1/reconnect", Some(serde_json::json!({}))).await.is_ok() {
                            log::info!("[Agent] 自动重连已请求成功 (guard={:?})", guard);
                        }
                    })
                    .await;
            }
        }
        GamePhase::EndOfGame => {
            if config.auto_play_again {
                let delay = config.auto_play_again_delay;
                let client = client.clone();
                let guard = lifecycle.guard(false).await;
                let supervisor_clone = supervisor.clone();

                let _ = supervisor
                    .spawn(TaskKind::AutoPlayAgain, DuplicateStrategy::RejectNew, move |ctx| async move {
                        sleep(Duration::from_secs(delay)).await;
                        if ctx.is_cancelled() {
                            return;
                        }
                        let _permit = supervisor_clone.acquire_exclusive_permit().await;
                        if client.post("/lol-lobby/v2/play-again", Some(serde_json::json!({}))).await.is_ok() {
                            log::info!("[Agent] 延时再来一局已触发 (guard={:?})", guard);
                        }
                    })
                    .await;
            }
        }
        GamePhase::None | GamePhase::Lobby => {
            // 退出对局或回到大厅，取消无关正在运行的任务
            supervisor.cancel(&TaskKind::AutoAccept).await;
            supervisor.cancel(&TaskKind::AutoReconnect).await;
        }
        _ => {}
    }
}

/// 匹配确认（ReadyCheck）事件响应
pub async fn handle_ready_check(
    client: &LcuClient,
    ready: &ReadyCheckData,
    config: &AutoMatchConfig,
    supervisor: &AgentSupervisor,
    lifecycle: &LifecycleManager,
) {
    if !config.auto_accept {
        return;
    }

    let is_in_progress = ready.state.as_deref() == Some("InProgress");
    let not_yet_accepted = ready.player_response.as_deref() != Some("Accepted");

    if is_in_progress && not_yet_accepted {
        let delay = config.auto_accept_delay;
        let client = client.clone();
        let guard = lifecycle.guard(false).await;
        let supervisor_clone = supervisor.clone();

        let _ = supervisor
            .spawn(TaskKind::AutoAccept, DuplicateStrategy::RejectNew, move |ctx| async move {
                if delay > 0 {
                    sleep(Duration::from_secs(delay)).await;
                }
                if ctx.is_cancelled() {
                    return;
                }
                let _permit = supervisor_clone.acquire_exclusive_permit().await;
                match client.post("/lol-matchmaking/v1/ready-check/accept", Some(serde_json::json!({}))).await {
                    Ok(_) => {
                        log::info!("[Agent] 自动接受匹配成功 (guard={:?})", guard);
                    }
                    Err(e) => {
                        log::warn!("[Agent] 自动接受匹配失败: {}", e);
                    }
                }
            })
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ready_check_deserialization() {
        let json = r#"{"state":"InProgress","playerResponse":"None"}"#;
        let data: ReadyCheckData = serde_json::from_str(json).unwrap();
        assert_eq!(data.state.as_deref(), Some("InProgress"));
        assert_eq!(data.player_response.as_deref(), Some("None"));
    }

    #[tokio::test]
    async fn test_phase_change_lifecycle_trigger() {
        let client = LcuClient::new(1234, 2999, "mock".to_string(), None).unwrap();
        let config = AutoMatchConfig {
            auto_accept: true,
            auto_accept_delay: 0,
            auto_reconnect: false,
            auto_play_again: false,
            auto_play_again_delay: 0,
            auto_honor: false,
        };
        let supervisor = AgentSupervisor::new();
        let lifecycle = LifecycleManager::new();

        handle_phase_change(&client, "ReadyCheck", &config, &supervisor, &lifecycle).await;
        let snap = lifecycle.snapshot().await;
        assert_eq!(snap.phase, GamePhase::ReadyCheck);
        assert_eq!(snap.match_epoch, 2);
    }
}
