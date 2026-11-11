//! 自动选人/禁人/召唤师技能 Agent，以及大乱斗板凳席（Bench）数据模型与换人接口

use crate::agents::config::{AutoBpConfig, ARAM_BENCH_QUEUE_IDS};
use crate::agents::lifecycle::LifecycleManager;
use crate::agents::supervisor::{AgentSupervisor, DuplicateStrategy, TaskKind};
use crate::lcu::client::LcuClient;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
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
    pub bans: ChampSelectBans,
    #[serde(default)]
    pub pick_order_swaps: Vec<ChampSelectSwap>,
    #[serde(default)]
    pub trades: Vec<ChampSelectTrade>,
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

/// 双方禁用列表
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectBans {
    #[serde(default)]
    pub my_team_bans: Vec<i32>,
    #[serde(default)]
    pub their_team_bans: Vec<i32>,
}

impl ChampSelectBans {
    /// 双方全部禁用英雄（预选与禁用候选需同时排除）
    pub fn all(&self) -> Vec<i32> {
        self.my_team_bans
            .iter()
            .chain(self.their_team_bans.iter())
            .copied()
            .collect()
    }
}

/// 楼层交换请求
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectSwap {
    pub id: i32,
    #[serde(default)]
    pub state: String,
}

/// 英雄交换请求
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectTrade {
    pub id: i32,
    #[serde(default)]
    pub state: String,
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
    #[serde(default)]
    pub spell1_id: i32,
    #[serde(default)]
    pub spell2_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectTimer {
    pub adjusted_time_left_in_phase: Option<u64>,
    pub phase: Option<String>,
    /// LCU 生成该 timer 快照时的 epoch 毫秒，用于校正推送延迟
    #[serde(default)]
    pub internal_now_in_epoch_ms: Option<u64>,
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
///
/// 执行顺序对照旧版 Agent：楼层交换 → 英雄交换 → 亮英雄 → 禁用 → 召唤师技能 → 锁定。
pub async fn handle_champ_select_session(
    client: &LcuClient,
    session: &ChampSelectSession,
    config: &AutoBpConfig,
    supervisor: &AgentSupervisor,
    lifecycle: &Arc<LifecycleManager>,
) {
    let any_enabled = config.auto_pick
        || config.confirm_on_timeout
        || config.auto_pick_intent
        || config.auto_ban
        || config.auto_spell
        || config.auto_pick_order_swap
        || config.auto_champ_trade;
    if !any_enabled {
        return;
    }

    // 旧版用 -1 表示会话重置信号，此时不执行任何动作
    let cell_id = session.local_player_cell_id;
    if cell_id < 0 {
        return;
    }

    // 1. 楼层交换 / 英雄交换
    if config.auto_pick_order_swap {
        auto_accept_pick_order_swaps(client, session, supervisor, lifecycle).await;
    }
    if config.auto_champ_trade {
        auto_accept_trades(client, session, supervisor, lifecycle).await;
    }

    let position = session
        .my_team
        .iter()
        .find(|p| p.cell_id == cell_id)
        .map(|p| p.assigned_position.clone())
        .unwrap_or_default();

    // 2. 亮英雄（不锁定）
    if config.auto_pick_intent {
        preset_champion(client, session, config, &position).await;
    }

    // 3. 自动禁用
    if config.auto_ban {
        schedule_ban(client, session, config, &position, supervisor, lifecycle).await;
    }

    // 4. 自动配置召唤师技能
    if config.auto_spell {
        schedule_spell(client, session, config, &position, supervisor).await;
    }

    // 5. 超时前自动锁定（支持 auto_pick 或 confirm_on_timeout）
    if config.auto_pick || config.confirm_on_timeout {
        schedule_pick(client, session, config, &position, supervisor, lifecycle).await;
    }
}

/// 自动同意楼层交换请求（仅处理 RECEIVED 状态）
async fn auto_accept_pick_order_swaps(
    client: &LcuClient,
    session: &ChampSelectSession,
    supervisor: &AgentSupervisor,
    lifecycle: &Arc<LifecycleManager>,
) {
    let Some(swap) = session
        .pick_order_swaps
        .iter()
        .find(|s| s.state == "RECEIVED")
    else {
        return;
    };
    let swap_id = swap.id;
    let url = format!(
        "/lol-champ-select/v1/session/swaps/pick-order/{}/accept",
        swap_id
    );

    let client = client.clone();
    let guard = lifecycle.guard(false).await;
    let lifecycle_clone = lifecycle.clone();
    log::info!("[Agent] 自动接受楼层交换: swapId={}", swap_id);

    let _ = supervisor
        .spawn(
            TaskKind::AutoSwap,
            DuplicateStrategy::ReplaceOld,
            move |ctx| async move {
                sleep(Duration::from_millis(500)).await;
                if ctx.is_cancelled() {
                    return;
                }
                let snapshot = lifecycle_clone.snapshot().await;
                if let Err(e) = guard.ensure_valid(&snapshot) {
                    log::info!("[Agent] 楼层交换任务已失效: {}", e);
                    return;
                }
                if let Err(e) = client.post(&url, None).await {
                    log::warn!("[Agent] 接受楼层交换失败: {}", e);
                } else {
                    log::info!("[Agent] 已接受楼层交换 swapId={}", swap_id);
                }
            },
        )
        .await;
}

/// 自动同意英雄交换请求（仅处理 RECEIVED 状态）
async fn auto_accept_trades(
    client: &LcuClient,
    session: &ChampSelectSession,
    supervisor: &AgentSupervisor,
    lifecycle: &Arc<LifecycleManager>,
) {
    let Some(trade) = session.trades.iter().find(|t| t.state == "RECEIVED") else {
        return;
    };
    let trade_id = trade.id;
    let url = format!("/lol-champ-select/v1/session/trades/{}/accept", trade_id);

    let client = client.clone();
    let guard = lifecycle.guard(false).await;
    let lifecycle_clone = lifecycle.clone();
    log::info!("[Agent] 自动接受英雄交换: tradeId={}", trade_id);

    let _ = supervisor
        .spawn(
            TaskKind::AutoTrade,
            DuplicateStrategy::ReplaceOld,
            move |ctx| async move {
                sleep(Duration::from_millis(500)).await;
                if ctx.is_cancelled() {
                    return;
                }
                let snapshot = lifecycle_clone.snapshot().await;
                if let Err(e) = guard.ensure_valid(&snapshot) {
                    log::info!("[Agent] 英雄交换任务已失效: {}", e);
                    return;
                }
                if let Err(e) = client.post(&url, None).await {
                    log::warn!("[Agent] 接受英雄交换失败: {}", e);
                } else {
                    log::info!("[Agent] 已接受英雄交换 tradeId={}", trade_id);
                }
            },
        )
        .await;
}

/// 亮英雄：向自己的 pick action 提交 championId 但 completed=false（不锁定）
async fn preset_champion(
    client: &LcuClient,
    session: &ChampSelectSession,
    config: &AutoBpConfig,
    position: &str,
) {
    // 大乱斗家族共享板凳席，亮英雄会与换人冲突
    if session.bench_enabled {
        return;
    }

    let cell_id = session.local_player_cell_id;
    let Some(player) = session.my_team.iter().find(|p| p.cell_id == cell_id) else {
        return;
    };
    // 已经亮过或已锁定：不重复发送
    if player.champion_id != 0 || player.champion_pick_intent != 0 {
        return;
    }

    let all_bans = session.bans.all();
    let mut candidates = config.pick_candidates(position);
    candidates.retain(|c| *c > 0 && !all_bans.contains(c));
    // 开启自动禁用时排除自己计划禁用的英雄，避免「刚亮出就被自己禁掉」
    if config.auto_ban {
        let ban_pool = config.ban_candidates(position);
        candidates.retain(|c| !ban_pool.contains(c));
    }
    let Some(champion_id) = candidates.first().copied() else {
        log::debug!("[Agent] 亮英雄跳过: 无可用候选（position={}）", position);
        return;
    };

    let Some(action) = session
        .actions
        .iter()
        .flatten()
        .rev()
        .find(|a| a.actor_cell_id == cell_id && a.action_type == "pick")
    else {
        return;
    };

    let url = format!("/lol-champ-select/v1/session/actions/{}", action.id);
    let payload = serde_json::json!({
        "championId": champion_id,
        "completed": false,
    });
    if let Err(e) = client.patch(&url, Some(payload)).await {
        log::warn!("[Agent] 亮英雄失败 (champ={}): {}", champion_id, e);
    } else {
        log::debug!("[Agent] 亮英雄成功 (champ={})", champion_id);
    }
}

/// 超时前自动锁定：按 session timer 计算等待时间，阶段结束前 5 秒提交
async fn schedule_pick(
    client: &LcuClient,
    session: &ChampSelectSession,
    config: &AutoBpConfig,
    position: &str,
    supervisor: &AgentSupervisor,
    lifecycle: &Arc<LifecycleManager>,
) {
    let cell_id = session.local_player_cell_id;
    let Some(action) = session.actions.iter().flatten().find(|a| {
        a.actor_cell_id == cell_id && a.action_type == "pick" && a.is_in_progress && !a.completed
    }) else {
        return;
    };

    let player = session.my_team.iter().find(|p| p.cell_id == cell_id);
    let intent = player.map(|p| p.champion_pick_intent).unwrap_or(0);
    let candidates = config.pick_candidates(position);
    let target = if action.champion_id > 0 {
        action.champion_id
    } else if intent > 0 {
        intent
    } else {
        candidates.first().copied().unwrap_or(0)
    };
    if target <= 0 {
        log::debug!("[Agent] 自动锁定跳过: 无目标英雄（position={}）", position);
        return;
    }

    let sleep_ms = if config.confirm_on_timeout {
        compute_lock_delay_ms(session.timer.as_ref(), current_epoch_ms())
    } else {
        500
    };

    let action_id = action.id;
    let url = format!("/lol-champ-select/v1/session/actions/{}", action_id);
    let client = client.clone();
    let guard = lifecycle.guard(false).await;
    let supervisor_clone = supervisor.clone();
    let lifecycle_clone = lifecycle.clone();

    let _ = supervisor
        .spawn(
            TaskKind::AutoPickLock,
            DuplicateStrategy::ReplaceOld,
            move |ctx| async move {
                tokio::select! {
                    _ = ctx.cancel_token.cancelled() => return,
                    _ = sleep(Duration::from_millis(sleep_ms)) => {}
                }
                if ctx.is_cancelled() {
                    return;
                }
                let snapshot = lifecycle_clone.snapshot().await;
                if let Err(e) = guard.ensure_valid(&snapshot) {
                    log::info!("[Agent] 自动锁定放弃: {}", e);
                    return;
                }

                // 倒计时结束前重新拉取最新会话二次复核，对齐旧版 do_auto_complete
                let fresh = match fetch_champ_select_session(&client).await {
                    Ok(s) => s,
                    Err(e) => {
                        log::warn!("[Agent] 自动锁定放弃: 获取最新会话失败: {}", e);
                        return;
                    }
                };

                let fresh_action = fresh.actions.iter().flatten().find(|a| {
                    a.id == action_id && a.actor_cell_id == cell_id && a.action_type == "pick"
                });
                let Some(fresh_action) = fresh_action else {
                    log::info!("[Agent] 自动锁定放弃: 未找到对应 pick 动作");
                    return;
                };
                if fresh_action.completed {
                    log::info!("[Agent] 自动锁定放弃: 动作已完成（玩家已手动锁定或阶段已过）");
                    return;
                }
                if !fresh_action.is_in_progress {
                    log::info!("[Agent] 自动锁定放弃: 动作已不在进行中");
                    return;
                }

                // 目标英雄确定：手选 > 预选/候选
                let final_target = if fresh_action.champion_id > 0 {
                    fresh_action.champion_id
                } else {
                    target
                };
                if final_target <= 0 {
                    log::warn!("[Agent] 自动锁定放弃: 无有效目标英雄");
                    return;
                }

                // 检查不可选英雄（已被他人锁定或已被禁用）
                let mut cant_select: Vec<i32> = Vec::new();
                for a in fresh.actions.iter().flatten() {
                    if a.action_type == "pick" && a.completed && a.actor_cell_id != cell_id {
                        cant_select.push(a.champion_id);
                    }
                }
                cant_select.extend(fresh.bans.all());
                if cant_select.contains(&final_target) {
                    log::warn!(
                        "[Agent] 自动锁定放弃: 目标英雄 {} 已被禁用或被他人选走",
                        final_target
                    );
                    return;
                }

                let _permit = supervisor_clone.acquire_exclusive_permit().await;
                let payload = serde_json::json!({
                    "championId": final_target,
                    "completed": true,
                });
                if let Err(e) = client.patch(&url, Some(payload)).await {
                    log::warn!("[Agent] 自动锁定失败 (champ={}): {}", final_target, e);
                } else {
                    log::info!("[Agent] 自动锁定成功 (champ={})", final_target);
                }
            },
        )
        .await;
}

/// 自动禁用：分路候选 + 假禁 + 延迟，延迟后按最新 session 复核候选可用性
async fn schedule_ban(
    client: &LcuClient,
    session: &ChampSelectSession,
    config: &AutoBpConfig,
    position: &str,
    supervisor: &AgentSupervisor,
    lifecycle: &Arc<LifecycleManager>,
) {
    let cell_id = session.local_player_cell_id;
    let Some(action) = session
        .actions
        .iter()
        .flatten()
        .find(|a| a.actor_cell_id == cell_id && a.action_type == "ban" && a.is_in_progress)
    else {
        return;
    };

    let all_bans = session.bans.all();
    let mut candidates = config.ban_candidates(position);
    candidates.retain(|c| *c > 0 && !all_bans.contains(c));
    if config.pretend_ban {
        let intents: Vec<i32> = session
            .my_team
            .iter()
            .map(|p| p.champion_pick_intent)
            .filter(|id| *id != 0)
            .collect();
        candidates.retain(|c| !intents.contains(c));
    }
    if candidates.is_empty() {
        log::debug!("[Agent] 自动禁人跳过: 无可用候选（position={}）", position);
        return;
    }

    let action_id = action.id;
    let delay = Duration::from_millis(config.ban_delay_ms as u64);
    let url = format!("/lol-champ-select/v1/session/actions/{}", action_id);
    let client = client.clone();
    let guard = lifecycle.guard(false).await;
    let supervisor_clone = supervisor.clone();
    let lifecycle_clone = lifecycle.clone();

    let _ = supervisor
        .spawn(
            TaskKind::AutoBanLock,
            DuplicateStrategy::RejectNew,
            move |ctx| async move {
                if !delay.is_zero() {
                    tokio::select! {
                        _ = ctx.cancel_token.cancelled() => return,
                        _ = sleep(delay) => {}
                    }
                }
                if ctx.is_cancelled() {
                    return;
                }
                let snapshot = lifecycle_clone.snapshot().await;
                if let Err(e) = guard.ensure_valid(&snapshot) {
                    log::info!("[Agent] 自动禁人放弃: {}", e);
                    return;
                }

                // 延迟期间可能出现新的禁用，按最新会话重新挑选候选
                let champion_id = match fetch_champ_select_session(&client).await {
                    Ok(fresh) => {
                        let new_bans = fresh.bans.all();
                        candidates.iter().find(|c| !new_bans.contains(c)).copied()
                    }
                    Err(_) => candidates.first().copied(),
                };
                let Some(champion_id) = champion_id else {
                    log::warn!("[Agent] 自动禁人放弃: 候选英雄已被全部禁用");
                    return;
                };

                let _permit = supervisor_clone.acquire_exclusive_permit().await;
                let payload = serde_json::json!({
                    "championId": champion_id,
                    "completed": true,
                });
                if let Err(e) = client.patch(&url, Some(payload)).await {
                    log::warn!("[Agent] 自动禁人锁定失败 (champ={}): {}", champion_id, e);
                } else {
                    log::info!("[Agent] 自动禁人锁定成功 (champ={})", champion_id);
                }
            },
        )
        .await;
}

/// 自动设置召唤师技能：分路技能优先，已与目标一致时不重复写入
async fn schedule_spell(
    client: &LcuClient,
    session: &ChampSelectSession,
    config: &AutoBpConfig,
    position: &str,
    supervisor: &AgentSupervisor,
) {
    let spells = config.spell_pair(position);
    if spells.len() < 2 || spells[0] <= 0 || spells[1] <= 0 {
        return;
    }
    let (s1, s2) = (spells[0], spells[1]);

    if let Some(player) = session
        .my_team
        .iter()
        .find(|p| p.cell_id == session.local_player_cell_id)
    {
        if player.spell1_id == s1 && player.spell2_id == s2 {
            return;
        }
    }

    let client = client.clone();
    let supervisor_clone = supervisor.clone();

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

/// 自动锁定的提前量：在阶段结束前预留的毫秒数
const LOCK_BUFFER_MS: u64 = 5_000;
/// 缺失 timer 数据时的兜底等待时长
const DEFAULT_PHASE_LEFT_MS: u64 = 10_000;
/// timer 异常值上限，防御异常剩余时间导致长时间空等
const MAX_PHASE_WAIT_MS: u64 = 180_000;

fn current_epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 计算距「应当发出锁定请求」还需等待的毫秒数。
///
/// `adjusted_time_left_in_phase` 是 LCU 推送该 session 那一刻的剩余时间，
/// 用同一快照里的 `internal_now_in_epoch_ms` 反推已过时间即可校正推送延迟。
pub fn compute_lock_delay_ms(timer: Option<&ChampSelectTimer>, now_ms: u64) -> u64 {
    let Some(timer) = timer else {
        return DEFAULT_PHASE_LEFT_MS.saturating_sub(LOCK_BUFFER_MS);
    };

    let elapsed_ms = timer
        .internal_now_in_epoch_ms
        .map(|lcu_now| now_ms.saturating_sub(lcu_now))
        .unwrap_or(0);

    let remaining_ms = timer
        .adjusted_time_left_in_phase
        .unwrap_or(DEFAULT_PHASE_LEFT_MS)
        .min(MAX_PHASE_WAIT_MS)
        .saturating_sub(elapsed_ms);

    remaining_ms.saturating_sub(LOCK_BUFFER_MS)
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
            bans: ChampSelectBans::default(),
            pick_order_swaps: Vec::new(),
            trades: Vec::new(),
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

    #[test]
    fn test_swap_and_trade_payload() {
        let json = r#"{
            "actions": [],
            "myTeam": [{"cellId": 1, "spell1Id": 4, "spell2Id": 14}],
            "bans": {"myTeamBans": [1, 2], "theirTeamBans": [3]},
            "pickOrderSwaps": [{"id": 8, "state": "RECEIVED"}],
            "trades": [{"id": 12, "state": "AVAILABLE"}]
        }"#;
        let session: ChampSelectSession = serde_json::from_str(json).unwrap();
        assert_eq!(session.bans.all(), vec![1, 2, 3]);
        assert_eq!(session.pick_order_swaps[0].id, 8);
        assert_eq!(session.pick_order_swaps[0].state, "RECEIVED");
        assert_eq!(session.trades[0].state, "AVAILABLE");
        assert_eq!(session.my_team[0].spell1_id, 4);
        assert_eq!(session.my_team[0].spell2_id, 14);
    }

    #[test]
    fn test_candidates_prefer_lane_then_default() {
        let mut config = AutoBpConfig {
            pick_champion_ids: vec![1, 2],
            ..Default::default()
        };
        assert_eq!(config.pick_candidates("middle"), vec![1, 2]);

        config.pick_champion_ids_mid = vec![7];
        assert_eq!(config.pick_candidates("middle"), vec![7, 1, 2]);
        // 未配置的分路保留默认候选
        assert_eq!(config.pick_candidates("top"), vec![1, 2]);
    }

    #[test]
    fn test_ban_candidates_include_lane_and_default() {
        let config = AutoBpConfig {
            ban_champion_ids: vec![9],
            ban_champion_ids_sup: vec![16],
            ..Default::default()
        };
        assert_eq!(config.ban_candidates("utility"), vec![16, 9]);
        assert_eq!(config.ban_candidates("top"), vec![9]);
    }

    #[test]
    fn test_spell_pair_falls_back_to_default() {
        let mut config = AutoBpConfig::default();
        assert_eq!(config.spell_pair("middle"), vec![4, 14]);
        config.spell_ids_mid = vec![4];
        assert_eq!(config.spell_pair("middle"), vec![4, 14]);
        config.spell_ids_mid = vec![12, 11];
        assert_eq!(config.spell_pair("middle"), vec![12, 11]);
    }

    #[test]
    fn test_lock_delay_compensation() {
        // 快照时刻即当前时刻：剩余 30s → 等待 25s
        let timer = ChampSelectTimer {
            adjusted_time_left_in_phase: Some(30_000),
            phase: Some("BAN_PICK".into()),
            internal_now_in_epoch_ms: Some(1_000),
        };
        assert_eq!(compute_lock_delay_ms(Some(&timer), 1_000), 25_000);
        // 快照已过去 3s：真实剩余 27s → 等待 22s
        assert_eq!(compute_lock_delay_ms(Some(&timer), 4_000), 22_000);
        // 剩余不足缓冲 → 立即锁定
        let short = ChampSelectTimer {
            adjusted_time_left_in_phase: Some(3_000),
            phase: Some("BAN_PICK".into()),
            internal_now_in_epoch_ms: Some(1_000),
        };
        assert_eq!(compute_lock_delay_ms(Some(&short), 1_000), 0);
        // timer 缺失 → 兜底等待
        assert_eq!(
            compute_lock_delay_ms(None, 1_000),
            DEFAULT_PHASE_LEFT_MS - LOCK_BUFFER_MS
        );
    }

    #[test]
    fn test_auto_pick_conditions_and_target_priority() {
        let config = AutoBpConfig {
            auto_pick: false,
            confirm_on_timeout: true,
            auto_pick_intent: false,
            ..AutoBpConfig::default()
        };

        // 仅开启 confirm_on_timeout，any_enabled 应判定为真
        let any_enabled = config.auto_pick
            || config.confirm_on_timeout
            || config.auto_pick_intent
            || config.auto_ban
            || config.auto_spell
            || config.auto_pick_order_swap
            || config.auto_champ_trade;
        assert!(any_enabled);

        // 两者都关闭时，不应调度锁定
        let disabled = AutoBpConfig {
            confirm_on_timeout: false,
            ..config
        };
        assert!(!(disabled.auto_pick || disabled.confirm_on_timeout));

        // 目标选择优先级：手选(action.champion_id) > 预选(intent) > 候选(candidates)
        let candidates = [238, 91];
        let pick_target = |action_champion_id: i32, intent: i32| {
            if action_champion_id > 0 {
                action_champion_id
            } else if intent > 0 {
                intent
            } else {
                candidates.first().copied().unwrap_or(0)
            }
        };

        // 1. 有手选英雄
        assert_eq!(pick_target(157, 84), 157);
        // 2. 无手选，有预选
        assert_eq!(pick_target(0, 84), 84);
        // 3. 无手选，无预选，有候选
        assert_eq!(pick_target(0, 0), 238);
        // 4. 无有效目标
        let empty: [i32; 0] = [];
        assert_eq!(empty.first().copied().unwrap_or(0), 0);
    }
}
