//! 自动选人/禁人/召唤师技能 Agent，以及大乱斗板凳席（Bench）数据模型与换人接口

use crate::agents::config::{AutoBpConfig, ARAM_BENCH_QUEUE_IDS};
use crate::agents::lifecycle::LifecycleManager;
use crate::agents::supervisor::{AgentSupervisor, DuplicateStrategy, TaskKind};
use crate::lcu::client::LcuClient;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
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
    /// 板凳席可交换英雄（大乱斗 / 海克斯大乱斗）
    #[serde(default)]
    pub bench_champions: Vec<BenchChampion>,
    /// 当前选人会话所属队列 ID
    #[serde(default)]
    pub queue_id: i32,
    /// 是否为自定义房间（自定义大乱斗的队列 ID 为 0，需借助此标记判定）
    #[serde(default)]
    pub is_custom_game: bool,
    /// 剩余摇号（reroll）次数
    #[serde(default)]
    pub rerolls_remaining: i32,
    pub timer: Option<ChampSelectTimer>,
}

/// 板凳席上的单个英雄
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchChampion {
    #[serde(default)]
    pub champion_id: i32,
    /// 该英雄是否已归属本方（自己或队友已换取）
    #[serde(default)]
    pub is_mine: bool,
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

impl ChampSelectSession {
    /// 是否是拥有共享板凳席的大乱斗家族队列（主判定为极地大乱斗 450）
    ///
    /// 自定义房间的队列 ID 为 0，无法用队列号判定，改用 `benchEnabled`：
    /// 共享板凳席只存在于大乱斗家族地图，经典图自定义局该标记恒为 false。
    pub fn is_aram(&self) -> bool {
        if ARAM_BENCH_QUEUE_IDS.contains(&self.queue_id) {
            return true;
        }
        self.is_custom_game && self.bench_enabled
    }

    /// 当前登录玩家已选定的英雄 ID（未选择为 0）
    pub fn local_champion_id(&self) -> i32 {
        self.my_team
            .iter()
            .find(|p| p.cell_id == self.local_player_cell_id)
            .map(|p| p.champion_id)
            .unwrap_or(0)
    }

    /// 板凳席英雄 ID 列表（已剔除非法 ID）
    pub fn bench_champion_ids(&self) -> Vec<i32> {
        self.bench_champions
            .iter()
            .map(|c| c.champion_id)
            .filter(|id| *id > 0)
            .collect()
    }

    /// 与「自身可执行状态」相关的稳定指纹。
    ///
    /// 事件流里 timer 倒计时会持续变化，同一份可执行状态会被反复推送。
    /// 该指纹刻意排除时间字段，供 Agent Worker 做事件级去重，避免重复提交。
    pub fn action_signature(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.queue_id.hash(&mut hasher);
        self.local_player_cell_id.hash(&mut hasher);
        self.local_champion_id().hash(&mut hasher);
        for group in &self.actions {
            for action in group {
                if action.actor_cell_id == self.local_player_cell_id {
                    action.id.hash(&mut hasher);
                    action.action_type.hash(&mut hasher);
                    action.completed.hash(&mut hasher);
                    action.is_in_progress.hash(&mut hasher);
                }
            }
        }
        hasher.finish()
    }
}

/// 一键换取板凳席英雄（`POST /lol-champ-select/v1/session/bench/swap/{championId}`）
pub async fn request_bench_swap(client: &LcuClient, champion_id: i32) -> Result<(), String> {
    if champion_id <= 0 {
        return Err(format!("无效的板凳席英雄 ID: {}", champion_id));
    }
    client
        .post(
            &format!("/lol-champ-select/v1/session/bench/swap/{}", champion_id),
            None,
        )
        .await
        .map(|_| ())
        .map_err(|e| format!("换取板凳席英雄失败 (champ={}): {}", champion_id, e))
}

/// 摇号（重随）当前英雄
pub async fn request_reroll(client: &LcuClient) -> Result<(), String> {
    client
        .post(
            "/lol-champ-select/v1/session/my-selection/reroll",
            Some(serde_json::json!({})),
        )
        .await
        .map(|_| ())
        .map_err(|e| format!("摇号失败: {}", e))
}

/// 拉取当前选人会话（无活跃会话时 LCU 返回 404）
pub async fn fetch_champ_select_session(client: &LcuClient) -> Result<ChampSelectSession, String> {
    let value = client
        .get("/lol-champ-select/v1/session")
        .await
        .map_err(|e| format!("获取选人会话失败: {}", e))?;
    serde_json::from_value(value).map_err(|e| format!("选人会话解析失败: {}", e))
}

/// 摇号并瞬时换回原英雄
///
/// reroll 后原英雄会进入板凳席，需要等待服务端状态落地再从板凳席换回；
/// 轮询间隔 150ms × 12 次，覆盖实测的 reroll 生效延迟。
pub async fn request_reroll_and_restore(client: &LcuClient) -> Result<i32, String> {
    const RETRIES: usize = 12;
    const INTERVAL: Duration = Duration::from_millis(150);

    let original = fetch_champ_select_session(client)
        .await?
        .local_champion_id();
    if original <= 0 {
        return Err("尚未选择英雄，无法摇号换回".to_string());
    }

    request_reroll(client).await?;

    for _ in 0..RETRIES {
        sleep(INTERVAL).await;
        let Ok(session) = fetch_champ_select_session(client).await else {
            continue;
        };
        if session.bench_champion_ids().contains(&original) {
            request_bench_swap(client, original).await?;
            return Ok(original);
        }
    }

    Err("摇号后原英雄未进入板凳席，换回失败".to_string())
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
        // 经典模式载荷不含板凳席与队列字段，必须安全退化为默认值
        assert_eq!(session.queue_id, 0);
        assert!(session.bench_champions.is_empty());
        assert!(!session.is_aram());
    }

    #[test]
    fn test_aram_bench_session_deserialization() {
        let json = r#"{
            "localPlayerCellId": 1,
            "queueId": 450,
            "benchEnabled": true,
            "actions": [],
            "myTeam": [{
                "cellId": 1,
                "championId": 103,
                "championPickIntent": 0,
                "assignedPosition": ""
            }],
            "benchChampions": [
                { "championId": 22, "isMine": false },
                { "championId": 0, "isMine": false },
                { "championId": 99, "isMine": true }
            ]
        }"#;

        let session: ChampSelectSession = serde_json::from_str(json).unwrap();
        assert!(session.is_aram());
        assert!(session.bench_enabled);
        assert_eq!(session.local_champion_id(), 103);
        // 非法英雄 ID 0 必须被过滤
        assert_eq!(session.bench_champion_ids(), vec![22, 99]);
        assert!(session.bench_champions[2].is_mine);
    }

    #[test]
    fn test_bench_queue_family_detection() {
        let mut session: ChampSelectSession =
            serde_json::from_str(r#"{"actions":[],"queueId":450}"#).unwrap();
        assert!(session.is_aram());

        session.queue_id = 2400;
        assert!(session.is_aram());

        session.queue_id = 420;
        assert!(!session.is_aram());
    }

    #[test]
    fn test_action_signature_ignores_timer_but_tracks_actions() {
        let base = r#"{
            "localPlayerCellId": 0,
            "queueId": 420,
            "actions": [[{
                "id": 7,
                "actorCellId": 0,
                "championId": 0,
                "completed": false,
                "isInProgress": true,
                "type": "pick"
            }]],
            "myTeam": [{"cellId": 0, "championId": 0}]
        }"#;
        let a: ChampSelectSession = serde_json::from_str(base).unwrap();

        // 仅倒计时变化 → 指纹不变（事件去重生效）
        let tick: ChampSelectSession = serde_json::from_str(
            r#"{
                "localPlayerCellId": 0,
                "queueId": 420,
                "actions": [[{
                    "id": 7,
                    "actorCellId": 0,
                    "championId": 0,
                    "completed": false,
                    "isInProgress": true,
                    "type": "pick"
                }]],
                "myTeam": [{"cellId": 0, "championId": 0}],
                "timer": {"adjustedTimeLeftInPhase": 12000, "phase": "BAN_PICK"}
            }"#,
        )
        .unwrap();
        assert_eq!(a.action_signature(), tick.action_signature());

        // 动作完成 → 指纹变化（允许继续推进）
        let done: ChampSelectSession = serde_json::from_str(
            r#"{
                "localPlayerCellId": 0,
                "queueId": 420,
                "actions": [[{
                    "id": 7,
                    "actorCellId": 0,
                    "championId": 157,
                    "completed": true,
                    "isInProgress": false,
                    "type": "pick"
                }]],
                "myTeam": [{"cellId": 0, "championId": 157}]
            }"#,
        )
        .unwrap();
        assert_ne!(a.action_signature(), done.action_signature());
    }

    #[tokio::test]
    async fn test_bench_swap_rejects_invalid_champion_id() {
        let client = LcuClient::new(1234, 2999, "mock".to_string(), None).unwrap();
        assert!(request_bench_swap(&client, 0).await.is_err());
        assert!(request_bench_swap(&client, -3).await.is_err());
    }

    #[test]
    fn test_custom_aram_session_detected_by_bench_enabled() {
        let custom = ChampSelectSession {
            actions: Vec::new(),
            local_player_cell_id: 0,
            my_team: Vec::new(),
            bench_enabled: true,
            bench_champions: vec![BenchChampion {
                champion_id: 22,
                is_mine: false,
            }],
            queue_id: 0,
            is_custom_game: true,
            rerolls_remaining: 2,
            timer: None,
        };
        // 自定义大乱斗：队列 ID 为 0，靠 isCustomGame + benchEnabled 识别
        assert!(custom.is_aram());

        // 经典图自定义局：板凳席未启用，不应误判为大乱斗
        let classic = ChampSelectSession {
            bench_enabled: false,
            ..custom
        };
        assert!(!classic.is_aram());
    }

    #[test]
    fn test_aram_queue_and_rerolls_remaining_payload() {
        let json = r#"{
            "localPlayerCellId": 0,
            "actions": [],
            "myTeam": [],
            "benchEnabled": true,
            "queueId": 450,
            "rerollsRemaining": 2,
            "benchChampions": [{"championId": 22, "isMine": false}]
        }"#;

        let session: ChampSelectSession = serde_json::from_str(json).unwrap();
        assert!(session.is_aram());
        assert_eq!(session.rerolls_remaining, 2);
        assert_eq!(session.bench_champion_ids(), vec![22]);
    }
}
