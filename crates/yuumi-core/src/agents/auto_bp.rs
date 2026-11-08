//! 自动选人/禁人/召唤师技能 Agent

use crate::agents::config::AutoBpConfig;
use crate::agents::lifecycle::LifecycleManager;
use crate::agents::supervisor::{AgentSupervisor, DuplicateStrategy, TaskKind};
use crate::lcu::client::LcuClient;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectSession {
    pub actions: Vec<Vec<ChampSelectAction>>,
    #[serde(default)]
    pub local_player_cell_id: i32,
    #[serde(default)]
    pub my_team: Vec<ChampSelectPlayer>,
    #[serde(default)]
    pub bench_enabled: bool,
    pub timer: Option<ChampSelectTimer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectAction {
    pub id: i32,
    pub actor_cell_id: i32,
    #[serde(default)]
    pub champion_id: i32,
    #[serde(default)]
    pub completed: bool,
    #[serde(default)]
    pub is_in_progress: bool,
    #[serde(rename = "type")]
    pub action_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectPlayer {
    pub cell_id: i32,
    #[serde(default)]
    pub champion_id: i32,
    #[serde(default)]
    pub champion_pick_intent: i32,
    #[serde(default)]
    pub assigned_position: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectTimer {
    pub adjusted_time_left_in_phase: Option<u64>,
    pub phase: Option<String>,
}

/// 处理选人会话更新事件
pub async fn handle_champ_select_session(
    client: &LcuClient,
    session: &ChampSelectSession,
    config: &AutoBpConfig,
    supervisor: &AgentSupervisor,
    lifecycle: &LifecycleManager,
) {
    if !config.auto_pick && !config.auto_ban && !config.auto_spell {
        return;
    }

    let cell_id = session.local_player_cell_id;
    let guard = lifecycle.guard(false).await;

    // 1. 查找当前属于自己的活跃 Action
    for action_group in &session.actions {
        for action in action_group {
            if action.actor_cell_id == cell_id && action.is_in_progress && !action.completed {
                if action.action_type == "pick" && config.auto_pick {
                    schedule_pick(client, action.id, session, config, supervisor, &guard).await;
                } else if action.action_type == "ban" && config.auto_ban {
                    schedule_ban(client, action.id, config, supervisor, &guard).await;
                }
            }
        }
    }

    // 2. 自动配置召唤师技能（若未锁定）
    if config.auto_spell {
        schedule_spell(client, config, supervisor).await;
    }
}

async fn schedule_pick(
    client: &LcuClient,
    action_id: i32,
    session: &ChampSelectSession,
    config: &AutoBpConfig,
    supervisor: &AgentSupervisor,
    _guard: &crate::agents::lifecycle::GenerationGuard,
) {
    // 计算首选英雄
    let intent_champ = session
        .my_team
        .iter()
        .find(|p| p.cell_id == session.local_player_cell_id)
        .map(|p| p.champion_pick_intent)
        .unwrap_or(0);

    let pick_champ = if config.auto_pick_intent && intent_champ > 0 {
        intent_champ
    } else {
        *config.pick_champion_ids.first().unwrap_or(&0)
    };

    if pick_champ <= 0 {
        return;
    }

    let client = client.clone();
    let supervisor_clone = supervisor.clone();

    let _ = supervisor
        .spawn(
            TaskKind::AutoPickLock,
            DuplicateStrategy::ReplaceOld,
            move |ctx| async move {
                // 选人缓冲 500ms
                sleep(Duration::from_millis(500)).await;
                if ctx.is_cancelled() {
                    return;
                }

                let _permit = supervisor_clone.acquire_exclusive_permit().await;
                let url = format!("/lol-champ-select/v1/session/actions/{}", action_id);
                let payload = serde_json::json!({
                    "championId": pick_champ,
                    "completed": true
                });

                if let Err(e) = client.patch(&url, Some(payload)).await {
                    log::warn!("[Agent] 自动选人锁定失败 (champ={}): {}", pick_champ, e);
                } else {
                    log::info!("[Agent] 自动选人锁定成功 (champ={})", pick_champ);
                }
            },
        )
        .await;
}

async fn schedule_ban(
    client: &LcuClient,
    action_id: i32,
    config: &AutoBpConfig,
    supervisor: &AgentSupervisor,
    _guard: &crate::agents::lifecycle::GenerationGuard,
) {
    let ban_champ = match config.ban_champion_ids.first() {
        Some(&id) if id > 0 => id,
        _ => return,
    };

    let client = client.clone();
    let supervisor_clone = supervisor.clone();

    let _ = supervisor
        .spawn(
            TaskKind::AutoBanLock,
            DuplicateStrategy::RejectNew,
            move |ctx| async move {
                sleep(Duration::from_millis(800)).await;
                if ctx.is_cancelled() {
                    return;
                }

                let _permit = supervisor_clone.acquire_exclusive_permit().await;
                let url = format!("/lol-champ-select/v1/session/actions/{}", action_id);
                let payload = serde_json::json!({
                    "championId": ban_champ,
                    "completed": true
                });

                if let Err(e) = client.patch(&url, Some(payload)).await {
                    log::warn!("[Agent] 自动禁人锁定失败 (champ={}): {}", ban_champ, e);
                } else {
                    log::info!("[Agent] 自动禁人锁定成功 (champ={})", ban_champ);
                }
            },
        )
        .await;
}

async fn schedule_spell(client: &LcuClient, config: &AutoBpConfig, supervisor: &AgentSupervisor) {
    if config.spell1_id <= 0 || config.spell2_id <= 0 {
        return;
    }

    let client = client.clone();
    let supervisor_clone = supervisor.clone();
    let s1 = config.spell1_id;
    let s2 = config.spell2_id;

    let _ = supervisor
        .spawn(
            TaskKind::AutoSpell,
            DuplicateStrategy::ReplaceOld,
            move |ctx| async move {
                sleep(Duration::from_millis(200)).await;
                if ctx.is_cancelled() {
                    return;
                }

                let _permit = supervisor_clone.acquire_exclusive_permit().await;
                let payload = serde_json::json!({
                    "spell1Id": s1,
                    "spell2Id": s2,
                });

                if let Err(e) = client
                    .patch("/lol-champ-select/v1/session/my-selection", Some(payload))
                    .await
                {
                    log::warn!("[Agent] 自动设置召唤师技能失败: {}", e);
                } else {
                    log::info!("[Agent] 自动配置召唤师技能成功: s1={}, s2={}", s1, s2);
                }
            },
        )
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_champ_select_session_deserialization() {
        let json = r#"{
            "localPlayerCellId": 2,
            "actions": [[{
                "id": 1,
                "actorCellId": 2,
                "championId": 0,
                "completed": false,
                "isInProgress": true,
                "type": "pick"
            }]],
            "myTeam": [{
                "cellId": 2,
                "championId": 0,
                "championPickIntent": 157,
                "assignedPosition": "MIDDLE"
            }]
        }"#;

        let session: ChampSelectSession = serde_json::from_str(json).unwrap();
        assert_eq!(session.local_player_cell_id, 2);
        assert_eq!(session.actions[0][0].action_type, "pick");
        assert_eq!(session.my_team[0].champion_pick_intent, 157);
    }
}
