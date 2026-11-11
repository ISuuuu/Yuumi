//! 自动对局流程 Agent（接受匹配、重连、再来一局、荣誉点赞、邀请、自动建房、报边）

use crate::agents::config::{AutoMatchConfig, ARAM_BENCH_QUEUE_IDS};
use crate::agents::lifecycle::{GamePhase, LifecycleManager, PhaseTransition};
use crate::agents::supervisor::{AgentSupervisor, DuplicateStrategy, TaskKind};
use crate::lcu::client::LcuClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time::sleep;

/// 结算阶段「再来一局」的固定缓冲（对照旧版 spawn_auto_play_again 的三档延迟）
const PLAY_AGAIN_STATS_DELAY: Duration = Duration::from_millis(10_000);
const PLAY_AGAIN_PRE_END_DELAY: Duration = Duration::from_millis(3_250);
const PLAY_AGAIN_END_DELAY: Duration = Duration::from_millis(1_575);

/// 自动重连最大尝试次数与首次退避
const RECONNECT_MAX_ATTEMPTS: u32 = 5;
const RECONNECT_BASE_BACKOFF: Duration = Duration::from_millis(500);

/// 自动建房重试轮数与间隔
const LOBBY_CREATE_MAX_ATTEMPTS: u32 = 12;
const LOBBY_CREATE_RETRY_INTERVAL: Duration = Duration::from_secs(2);
/// 建房成功后 LCU 可能瞬时闪回 None，此窗口内的 None 不重复建房
const LOBBY_FLICKER_WINDOW: Duration = Duration::from_millis(2000);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadyCheckData {
    pub state: Option<String>,
    pub player_response: Option<String>,
}

/// `/lol-matchmaking/v1/ready-check` 的完整状态（延迟后二次校验用）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadyCheckStatus {
    pub player_response: Option<String>,
    pub error_code: Option<String>,
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
pub struct HonorVotePool {
    #[serde(default)]
    pub votes: i32,
}

/// 结算荣誉投票信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HonorBallot {
    #[serde(default)]
    pub eligible_allies: Vec<HonorEligiblePlayer>,
    #[serde(default)]
    pub eligible_opponents: Vec<HonorEligiblePlayer>,
    #[serde(default)]
    pub vote_pool: Option<HonorVotePool>,
}

/// 收到的组队邀请
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceivedInvitation {
    #[serde(default)]
    pub invitation_id: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub can_accept_invitation: Option<bool>,
}

/// 自动建房状态（后台任务与主循环共享，仅做短临界区读写，不跨 await 持锁）
#[derive(Default)]
struct LobbyFlags {
    created: bool,
    running: bool,
    last_create: Option<Instant>,
}

/// auto_match Worker 的事件级状态：跨事件去重与阶段复位
#[derive(Default)]
pub struct AutoMatchState {
    /// 本轮 ReadyCheck 是否已发起过接受
    ready_check_accepted: bool,
    /// 本局是否已自动点赞
    honored: bool,
    /// [D04-e] 已发送过标记提醒的对局代际（match_epoch）；新对局自动失效
    tag_reminder_epoch: Option<u64>,
    /// 自动建房标志
    lobby: Arc<Mutex<LobbyFlags>>,
}

impl AutoMatchState {
    pub fn new() -> Self {
        Self::default()
    }

    /// [D04-e] 同局去重：同一 match_epoch 只放行一次标记提醒，
    /// 下一局代际变化时自动重置，无需手动维护「离开阶段」复位。
    fn claim_tag_reminder(&mut self, epoch: u64) -> bool {
        if self.tag_reminder_epoch == Some(epoch) {
            return false;
        }
        self.tag_reminder_epoch = Some(epoch);
        true
    }
}

/// 游戏流转阶段变迁处理
pub async fn handle_phase_change(
    client: &LcuClient,
    phase: &str,
    config: &AutoMatchConfig,
    supervisor: &AgentSupervisor,
    lifecycle: &Arc<LifecycleManager>,
    state: &mut AutoMatchState,
) {
    let transition = lifecycle.on_phase_changed(phase).await;
    log::info!(
        "[Agent] 游戏流转阶段推进: phase={}, transition={:?}",
        phase,
        transition
    );

    let previous = match &transition {
        PhaseTransition::PhaseChanged { previous, .. }
        | PhaseTransition::NewMatchStarted { previous, .. }
        | PhaseTransition::MatchTerminated { previous, .. } => Some(previous.clone()),
        PhaseTransition::NoChange => None,
    };

    let current = GamePhase::from(phase);

    // 离开 ReadyCheck：立即取消仍在延迟等待中的接受任务
    if current != GamePhase::ReadyCheck {
        state.ready_check_accepted = false;
        supervisor.cancel(&TaskKind::AutoAccept).await;
    }
    // 离开 Reconnect：取消退避等待中的重连任务
    if current != GamePhase::Reconnect {
        supervisor.cancel(&TaskKind::AutoReconnect).await;
    }
    // 离开结算阶段：取消等待中的再来一局
    if !matches!(
        current,
        GamePhase::WaitingForStats | GamePhase::PreEndOfGame | GamePhase::EndOfGame
    ) {
        supervisor.cancel(&TaskKind::AutoPlayAgain).await;
    }
    // 离开 ChampSelect：取消选人期间派生的锁定、禁用与报边任务
    if current != GamePhase::ChampSelect {
        supervisor.cancel(&TaskKind::AutoPickLock).await;
        supervisor.cancel(&TaskKind::AutoBanLock).await;
        supervisor
            .cancel(&TaskKind::Other("AramTeamSide".to_string()))
            .await;
    }
    // 进入新对局：复位本局点赞标记
    if current == GamePhase::InProgress {
        state.honored = false;
    }

    match current {
        GamePhase::ReadyCheck => {
            if config.auto_accept && !state.ready_check_accepted {
                state.ready_check_accepted = true;
                spawn_auto_accept(client, config.auto_accept_delay, supervisor, lifecycle).await;
            }
        }
        GamePhase::Reconnect => {
            // 从结算/大厅/空闲进入的 Reconnect 视为结算残留，不重连
            let already_finished = matches!(
                previous,
                Some(GamePhase::WaitingForStats)
                    | Some(GamePhase::PreEndOfGame)
                    | Some(GamePhase::EndOfGame)
                    | Some(GamePhase::Lobby)
                    | Some(GamePhase::None)
            );
            if config.auto_reconnect && !already_finished {
                spawn_auto_reconnect(client, supervisor, lifecycle).await;
            }
        }
        GamePhase::WaitingForStats => {
            if config.auto_play_again {
                spawn_auto_play_again(client, PLAY_AGAIN_STATS_DELAY, supervisor, lifecycle).await;
            }
        }
        GamePhase::PreEndOfGame => {
            if config.auto_play_again {
                spawn_auto_play_again(client, PLAY_AGAIN_PRE_END_DELAY, supervisor, lifecycle)
                    .await;
            }
        }
        GamePhase::EndOfGame => {
            if config.auto_play_again {
                spawn_auto_play_again(client, PLAY_AGAIN_END_DELAY, supervisor, lifecycle).await;
            }
        }
        GamePhase::None => {
            let within_flicker = match state.lobby.lock() {
                Ok(flags) => flags
                    .last_create
                    .map(|t| t.elapsed() < LOBBY_FLICKER_WINDOW)
                    .unwrap_or(false),
                Err(_) => false,
            };
            if !within_flicker {
                if let Ok(mut flags) = state.lobby.lock() {
                    flags.created = false;
                }
            }
            if config.auto_create_lobby {
                try_create_lobby(client, config.default_game_mode, state.lobby.clone());
            }
        }
        GamePhase::Lobby => {
            // 已在大厅：标记为已建厅，避免回到 None 前重复建房
            if let Ok(mut flags) = state.lobby.lock() {
                flags.created = true;
            }
        }
        GamePhase::ChampSelect if config.aram_team_side => {
            spawn_aram_team_side(client, config.aram_team_side_visible, supervisor).await;
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
    lifecycle: &Arc<LifecycleManager>,
    state: &mut AutoMatchState,
) {
    if !config.auto_accept {
        return;
    }
    // 只在本轮 ReadyCheck 未处理过时发起一次
    if state.ready_check_accepted {
        return;
    }

    let is_in_progress = ready.state.as_deref() == Some("InProgress");
    let responded = matches!(
        ready.player_response.as_deref(),
        Some("Accepted") | Some("Declined")
    );

    if responded {
        state.ready_check_accepted = true;
        return;
    }
    if !is_in_progress {
        state.ready_check_accepted = false;
        return;
    }

    state.ready_check_accepted = true;
    spawn_auto_accept(client, config.auto_accept_delay, supervisor, lifecycle).await;
}

/// 重置大厅创建状态并尝试建房（配置变更或用户手动触发时调用）
pub async fn handle_reset_lobby_state(
    client: &LcuClient,
    config: &AutoMatchConfig,
    lifecycle: &Arc<LifecycleManager>,
    state: &mut AutoMatchState,
) {
    log::info!("[Agent] 收到重置大厅创建状态请求");
    if let Ok(mut flags) = state.lobby.lock() {
        flags.created = false;
        flags.running = false;
    }
    let phase = lifecycle.snapshot().await.phase;
    let is_idle = phase == GamePhase::None
        || matches!(
            client.get("/lol-gameflow/v1/gameflow-phase").await,
            Ok(serde_json::Value::String(ref p)) if p == "None" || p.is_empty()
        );
    if is_idle && config.auto_create_lobby {
        try_create_lobby(client, config.default_game_mode, state.lobby.clone());
    }
}

/// 结算荣誉投票：为可点赞队友投出有效票后提交投票
pub async fn handle_honor_ballot(
    client: &LcuClient,
    ballot: &HonorBallot,
    config: &AutoMatchConfig,
    supervisor: &AgentSupervisor,
    state: &mut AutoMatchState,
) {
    if !config.auto_honor || state.honored {
        return;
    }

    let allies: Vec<String> = ballot
        .eligible_allies
        .iter()
        .filter(|p| !p.bot_player)
        .filter_map(|p| p.puuid.clone())
        .collect();
    if allies.is_empty() {
        log::debug!("[Agent] 荣誉投票：无可用队友，跳过");
        return;
    }

    state.honored = true;

    let votes = ballot
        .vote_pool
        .as_ref()
        .map(|p| p.votes)
        .unwrap_or(1)
        .max(1) as usize;
    let count = allies.len().min(votes);
    let targets: Vec<String> = allies.into_iter().take(count).collect();
    let client = client.clone();
    let supervisor_clone = supervisor.clone();

    let _ = supervisor
        .spawn(
            TaskKind::AutoHonor,
            DuplicateStrategy::RejectNew,
            move |ctx| async move {
                if ctx.is_cancelled() {
                    return;
                }
                let _permit = supervisor_clone.acquire_exclusive_permit().await;
                for puuid in &targets {
                    let body = serde_json::json!({
                        "honorType": "HEART",
                        "recipientPuuid": puuid,
                    });
                    if let Err(e) = client
                        .post("/lol-honor-v2/v1/honor-player", Some(body))
                        .await
                    {
                        log::warn!("[Agent] 自动点赞失败: {}", e);
                    }
                }
                if let Err(e) = client
                    .post("/lol-honor-v2/v1/ballot", Some(serde_json::json!({})))
                    .await
                {
                    log::warn!("[Agent] 提交荣誉投票失败: {}", e);
                } else {
                    log::info!("[Agent] 自动荣誉点赞完成，共 {} 票", targets.len());
                }
            },
        )
        .await;
}

/// 收到的组队邀请：仅接受处于 Pending 且允许接受的邀请
pub async fn handle_invitations(
    client: &LcuClient,
    invitations: &[ReceivedInvitation],
    config: &AutoMatchConfig,
) {
    if !config.auto_handle_invite {
        return;
    }

    for inv in invitations {
        let pending = inv.state.as_deref() == Some("Pending");
        let can_accept = inv.can_accept_invitation.unwrap_or(false);
        let Some(invitation_id) = inv.invitation_id.as_deref() else {
            continue;
        };
        if !pending || !can_accept {
            continue;
        }

        let path = format!(
            "/lol-lobby/v2/received-invitations/{}/accept",
            invitation_id
        );
        match client.post(&path, None).await {
            Ok(_) => log::info!("[Agent] 已自动接受邀请 {}", invitation_id),
            Err(e) => log::warn!("[Agent] 自动接受邀请 {} 失败: {}", invitation_id, e),
        }
    }
}

// ─── 内部动作 ───

/// 延迟接受匹配：延迟后重新校验阶段与 ReadyCheck 状态，避免误接受下一局
async fn spawn_auto_accept(
    client: &LcuClient,
    delay_secs: u64,
    supervisor: &AgentSupervisor,
    lifecycle: &Arc<LifecycleManager>,
) {
    let client = client.clone();
    let guard = lifecycle.guard(false).await;
    let supervisor_clone = supervisor.clone();
    let lifecycle_clone = lifecycle.clone();

    let _ = supervisor
        .spawn(
            TaskKind::AutoAccept,
            DuplicateStrategy::RejectNew,
            move |ctx| async move {
                if delay_secs > 0 {
                    tokio::select! {
                        _ = ctx.cancel_token.cancelled() => {
                            log::info!("[Agent] 自动接受任务已取消");
                            return;
                        }
                        _ = sleep(Duration::from_secs(delay_secs)) => {}
                    }
                }
                if ctx.is_cancelled() {
                    return;
                }

                let snapshot = lifecycle_clone.snapshot().await;
                if guard.ensure_valid(&snapshot).is_err() || snapshot.phase != GamePhase::ReadyCheck
                {
                    log::info!("[Agent] 自动接受放弃: 阶段已变为 {:?}", snapshot.phase);
                    return;
                }

                match fetch_ready_check_status(&client).await {
                    Some(status) => {
                        if matches!(
                            status.player_response.as_deref(),
                            Some("Accepted") | Some("Declined")
                        ) {
                            log::debug!(
                                "[Agent] 玩家已响应匹配 ({:?})，跳过自动接受",
                                status.player_response
                            );
                            return;
                        }
                        if status.error_code.is_some() {
                            log::warn!(
                                "[Agent] ReadyCheck 异常 ({:?})，跳过自动接受",
                                status.error_code
                            );
                            return;
                        }
                    }
                    None => {
                        log::debug!("[Agent] 读取 ReadyCheck 状态失败，跳过自动接受");
                        return;
                    }
                }

                let _permit = supervisor_clone.acquire_exclusive_permit().await;
                match client
                    .post(
                        "/lol-matchmaking/v1/ready-check/accept",
                        Some(serde_json::json!({})),
                    )
                    .await
                {
                    Ok(_) => log::info!("[Agent] 自动接受匹配成功"),
                    Err(e) => log::warn!("[Agent] 自动接受匹配失败: {}", e),
                }
            },
        )
        .await;
}

/// 自动重连：仅在 Reconnect 阶段有效，并在发起重连前校验进程存活与阶段权威性，避免对局结束误拉起
async fn spawn_auto_reconnect(
    client: &LcuClient,
    supervisor: &AgentSupervisor,
    lifecycle: &Arc<LifecycleManager>,
) {
    let client = client.clone();
    let guard = lifecycle.guard(false).await;
    let supervisor_clone = supervisor.clone();
    let lifecycle_clone = lifecycle.clone();

    let _ = supervisor
        .spawn(
            TaskKind::AutoReconnect,
            DuplicateStrategy::RejectNew,
            move |ctx| async move {
                for attempt in 1..=RECONNECT_MAX_ATTEMPTS {
                    if ctx.is_cancelled() {
                        log::info!("[Agent] 自动重连任务已取消");
                        return;
                    }
                    let snapshot = lifecycle_clone.snapshot().await;
                    if guard.ensure_valid(&snapshot).is_err()
                        || snapshot.phase != GamePhase::Reconnect
                    {
                        log::info!("[Agent] 自动重连放弃: 阶段为 {:?}", snapshot.phase);
                        return;
                    }

                    // 发起请求前向 LCU 主动二次校验当前阶段并检查游戏进程存活，防止对局结束结算闪现误拉起
                    if !is_reconnect_still_valid(&client).await {
                        return;
                    }

                    let _permit = supervisor_clone.acquire_exclusive_permit().await;
                    match client
                        .post("/lol-gameflow/v1/reconnect", Some(serde_json::json!({})))
                        .await
                    {
                        Ok(_) => {
                            log::info!("[Agent] 自动重连成功");
                            return;
                        }
                        Err(e) if e.contains("[403]") => {
                            log::debug!(
                                "[Agent] 重连接口尚未开放（第 {}/{} 次），等待重试",
                                attempt,
                                RECONNECT_MAX_ATTEMPTS
                            );
                        }
                        Err(e) => {
                            log::warn!(
                                "[Agent] 重连失败（第 {}/{} 次）: {}",
                                attempt,
                                RECONNECT_MAX_ATTEMPTS,
                                e
                            );
                        }
                    }
                    drop(_permit);

                    if attempt < RECONNECT_MAX_ATTEMPTS {
                        let backoff = RECONNECT_BASE_BACKOFF * (1u32 << (attempt - 1));
                        tokio::select! {
                            _ = ctx.cancel_token.cancelled() => {
                                log::info!("[Agent] 自动重连在退避期被取消");
                                return;
                            }
                            _ = sleep(backoff) => {}
                        }
                    }
                }
                log::warn!("[Agent] 自动重连 {} 次尝试均失败", RECONNECT_MAX_ATTEMPTS);
            },
        )
        .await;
}

/// 在发起重连请求前，主动查询 LCU 当前权威状态与游戏进程存活情况，确认仍处于断线重连态。
/// 规避对局结束 Alt+F4 退出或自然结算退出时 WebSocket 事件积压或服务端结算延迟导致的误拉起。
async fn is_reconnect_still_valid(client: &LcuClient) -> bool {
    // 1. 游戏进程已退出 → 对局必然已结束（或用户主动退出），不重连。
    //    Alt+F4 主动退出或对局正常结束退出后游戏进程立即消失，LCU 却仍会短暂停留在 Reconnect 阶段等待
    //    服务端结算写入（实测 2~4s 后才推进到 WaitingForStats），此时发起重连
    //    会把游戏进程重新拉起并卡在加载页或报错。真实网络掉线时游戏进程仍在运行，
    //    因此「进程缺失」是区分主动退出/对局结束与真实掉线的可靠判据。
    if !is_game_process_alive().await {
        log::info!("[Agent] 游戏进程已退出，判定对局已结束，取消自动重连");
        return false;
    }

    // 2. 主动拉取 gameflow-phase
    if let Ok(value) = client.get("/lol-gameflow/v1/gameflow-phase").await {
        if let Some(phase) = value.as_str() {
            if phase != "Reconnect" {
                log::info!(
                    "[Agent] 主动查询 LCU 阶段为 {}，已脱离断线重连状态，取消自动重连",
                    phase
                );
                return false;
            }
        }
    }

    // 3. 主动拉取 gameflow session 中的 phase 字段
    if let Ok(session) = client.get("/lol-gameflow/v1/session").await {
        if let Some(session_phase) = session.get("phase").and_then(|v| v.as_str()) {
            if session_phase != "Reconnect" {
                log::info!(
                    "[Agent] Gameflow session 权威阶段为 {}，非重连状态，取消自动重连",
                    session_phase
                );
                return false;
            }
        }
    }

    true
}

/// 判断对局游戏进程（League of Legends.exe）是否仍在运行。
/// 网络掉线时游戏进程存活，重连才有意义；对局结束或主动退出后进程消失，
/// 此时客户端的 Reconnect 只是等待结算写入的残留态。
async fn is_game_process_alive() -> bool {
    if !detect_game_process().await {
        return false;
    }
    // 进程仍在时复核一次：对局结束/退出后 Windows 回收进程可能滞后数百毫秒，
    // 单次采样有可能在退出瞬间读到残留进程，把「已退出」误判为「仍在游戏中」。
    sleep(Duration::from_millis(300)).await;
    detect_game_process().await
}

/// 单次枚举系统进程，判断对局客户端是否在运行。
async fn detect_game_process() -> bool {
    let detected = tokio::task::spawn_blocking(|| {
        use sysinfo::System;
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        sys.processes().values().any(|p| {
            p.name()
                .to_string_lossy()
                .to_lowercase()
                .starts_with("league of legends")
        })
    })
    .await;

    // 进程枚举任务异常时保守放行，避免误禁真实掉线的自动重连
    detected.unwrap_or(true)
}

/// 延迟再来一局：延迟后校验代际与阶段，仍在结算流程才发出请求
async fn spawn_auto_play_again(
    client: &LcuClient,
    delay: Duration,
    supervisor: &AgentSupervisor,
    lifecycle: &Arc<LifecycleManager>,
) {
    let client = client.clone();
    let guard = lifecycle.guard(false).await;
    let supervisor_clone = supervisor.clone();
    let lifecycle_clone = lifecycle.clone();

    let _ = supervisor
        .spawn(
            TaskKind::AutoPlayAgain,
            DuplicateStrategy::RejectNew,
            move |ctx| async move {
                tokio::select! {
                    _ = ctx.cancel_token.cancelled() => {
                        log::info!("[Agent] 再来一局任务已取消");
                        return;
                    }
                    _ = sleep(delay) => {}
                }
                if ctx.is_cancelled() {
                    return;
                }

                let snapshot = lifecycle_clone.snapshot().await;
                if guard.ensure_valid(&snapshot).is_err()
                    || !matches!(
                        snapshot.phase,
                        GamePhase::WaitingForStats | GamePhase::PreEndOfGame | GamePhase::EndOfGame
                    )
                {
                    log::info!("[Agent] 再来一局放弃: 阶段为 {:?}", snapshot.phase);
                    return;
                }

                let _permit = supervisor_clone.acquire_exclusive_permit().await;
                match client
                    .post("/lol-lobby/v2/play-again", Some(serde_json::json!({})))
                    .await
                {
                    Ok(_) => log::info!("[Agent] 延时再来一局已触发"),
                    Err(e) => log::warn!("[Agent] 再来一局失败: {}", e),
                }
            },
        )
        .await;
}

/// 空闲阶段自动创建预设房间：已在房间内或刚建过则跳过
fn try_create_lobby(client: &LcuClient, queue_id: u32, flags: Arc<Mutex<LobbyFlags>>) {
    {
        let Ok(mut f) = flags.lock() else {
            return;
        };
        if f.created || f.running {
            return;
        }
        if f.last_create
            .map(|t| t.elapsed() < LOBBY_FLICKER_WINDOW)
            .unwrap_or(false)
        {
            log::debug!("[Agent] 距上次建房不足防抖窗口，跳过本次自动建房");
            return;
        }
        f.running = true;
    }

    let client = client.clone();
    let flags_clone = flags.clone();
    tokio::spawn(async move {
        for attempt in 1..=LOBBY_CREATE_MAX_ATTEMPTS {
            // 已在有效房间内：不再创建，避免把玩家踢出小队
            if client.get("/lol-lobby/v2/lobby").await.is_ok() {
                log::info!("[Agent] 当前已在房间内，跳过自动建房");
                if let Ok(mut f) = flags_clone.lock() {
                    f.created = true;
                    f.running = false;
                    f.last_create = Some(Instant::now());
                }
                return;
            }

            let phase = client.get("/lol-gameflow/v1/gameflow-phase").await;
            let still_idle = matches!(
                phase,
                Ok(serde_json::Value::String(ref p))
                    if p == "None" || p.is_empty() || p == "WaitingForStats" || p == "PreEndOfGame"
            );
            if !still_idle {
                log::info!("[Agent] 阶段已变化，放弃自动建房");
                if let Ok(mut f) = flags_clone.lock() {
                    f.created = true;
                    f.running = false;
                }
                return;
            }

            let body = serde_json::json!({ "queueId": queue_id });
            match client.post("/lol-lobby/v2/lobby", Some(body)).await {
                Ok(_) => {
                    log::info!(
                        "[Agent] 自动创建房间请求已发送 (queueId={})，第 {} 次尝试",
                        queue_id,
                        attempt
                    );
                    if let Ok(mut f) = flags_clone.lock() {
                        f.last_create = Some(Instant::now());
                    }

                    // 等待 1 秒确认大厅稳定，未被客户端启动时初始路由冲掉
                    sleep(Duration::from_millis(1000)).await;

                    let phase = client.get("/lol-gameflow/v1/gameflow-phase").await;
                    let is_in_lobby = match phase {
                        Ok(serde_json::Value::String(ref p)) => p == "Lobby",
                        _ => false,
                    };
                    let lobby_exists = client.get("/lol-lobby/v2/lobby").await.is_ok();

                    if is_in_lobby || lobby_exists {
                        log::info!(
                            "[Agent] 自动创建房间成功并已确认处于大厅 (queueId={})",
                            queue_id
                        );
                        if let Ok(mut f) = flags_clone.lock() {
                            f.created = true;
                            f.running = false;
                            f.last_create = Some(Instant::now());
                        }
                        return;
                    } else {
                        log::warn!(
                            "[Agent] 自动创建房间后未稳定在大厅（可能被客户端重置），准备重试"
                        );
                        if let Ok(mut f) = flags_clone.lock() {
                            f.last_create = None;
                        }
                    }
                }
                Err(e) => {
                    if e.contains("[409]") || e.contains("409") {
                        log::info!("[Agent] 建房返回 409，视为已在房间内");
                        if let Ok(mut f) = flags_clone.lock() {
                            f.created = true;
                            f.running = false;
                            f.last_create = Some(Instant::now());
                        }
                        return;
                    }
                    log::warn!(
                        "[Agent] 自动建房失败（第 {}/{} 次）: {}",
                        attempt,
                        LOBBY_CREATE_MAX_ATTEMPTS,
                        e
                    );
                }
            }

            if attempt < LOBBY_CREATE_MAX_ATTEMPTS {
                sleep(LOBBY_CREATE_RETRY_INTERVAL).await;
            }
        }

        if let Ok(mut f) = flags_clone.lock() {
            f.created = false;
            f.running = false;
        }
        log::warn!(
            "[Agent] 自动建房 {} 次尝试均失败",
            LOBBY_CREATE_MAX_ATTEMPTS
        );
    });
}

/// 选人阶段播报我方阵营（大乱斗 / 匹配 / 排位通用）
async fn spawn_aram_team_side(
    client: &LcuClient,
    visible_to_team: bool,
    supervisor: &AgentSupervisor,
) {
    let client = client.clone();
    let _ = supervisor
        .spawn(
            TaskKind::Other("AramTeamSide".to_string()),
            DuplicateStrategy::RejectNew,
            move |ctx| async move {
                // 轮询等待选人会话就绪（最多 8 次 × 500ms）
                let mut session = None;
                for _ in 0..8 {
                    if ctx.is_cancelled() {
                        return;
                    }
                    sleep(Duration::from_millis(500)).await;
                    if let Ok(value) = client.get("/lol-champ-select/v1/session").await {
                        if value.is_object() {
                            session = Some(value);
                            break;
                        }
                    }
                }
                let Some(session) = session else {
                    log::warn!("[Agent] 报边：等待选人会话超时");
                    return;
                };

                // 优先 pin-drop 通知中的地图阵营，失败则按 cellId 判定
                let mut side = None;
                if let Ok(data) = client
                    .get("/lol-champ-select/v1/pin-drop-notification")
                    .await
                {
                    if let Some(map_side) = data.get("mapSide").and_then(|v| v.as_str()) {
                        if !map_side.is_empty() {
                            side = Some(map_side.to_lowercase());
                        }
                    }
                }
                let side = side.unwrap_or_else(|| {
                    let cell_id = session
                        .get("localPlayerCellId")
                        .and_then(|c| c.as_i64())
                        .unwrap_or(0);
                    if cell_id < 5 { "blue" } else { "red" }.to_string()
                });
                let side_name = if side == "blue" {
                    "蓝色方"
                } else {
                    "红色方"
                };

                let room_hint = session
                    .pointer("/chatDetails/chatRoomName")
                    .and_then(|v| v.as_str())
                    .or_else(|| {
                        session
                            .pointer("/chatDetails/multiUserChatId")
                            .and_then(|v| v.as_str())
                    })
                    .map(|s| s.to_string());

                let Some(conv_id) =
                    wait_for_champ_select_conv_id(&client, room_hint.as_deref(), 10).await
                else {
                    log::warn!("[Agent] 报边：未找到选人聊天会话");
                    return;
                };

                // 等待本地与队友完成进入聊天室，避免消息被入房初始化冲刷
                sleep(Duration::from_millis(2000)).await;

                let message_type = if visible_to_team {
                    "chat"
                } else {
                    "celebration"
                };
                let message = serde_json::json!({
                    "body": format!("[Yuumi] 本局我方在{}", side_name),
                    "type": message_type,
                });
                let path = format!("/lol-chat/v1/conversations/{}/messages", conv_id);
                for attempt in 0..3 {
                    if attempt > 0 {
                        sleep(Duration::from_millis(600)).await;
                    }
                    match client.post(&path, Some(message.clone())).await {
                        Ok(_) => {
                            log::info!(
                                "[Agent] 报边成功：我方在{}（队友可见={}）",
                                side_name,
                                visible_to_team
                            );
                            return;
                        }
                        Err(e) => {
                            log::warn!("[Agent] 报边发送失败（第 {} 次）: {}", attempt + 1, e)
                        }
                    }
                }
                log::warn!("[Agent] 报边最终发送失败");
            },
        )
        .await;
}

/// [D04-e] 选人阶段标记提醒入口：消费 `tag_reminder` 配置、按 match_epoch 做同局去重，
/// 再把实际发送投递到后台任务（`send_tag_reminders` 内含轮询与缓冲 sleep，不阻塞 Worker）。
///
/// `puuid → 标签` 映射由 WS 分派点从路人集存储只读构造后随事件传入，本函数不访问存储。
pub async fn handle_tag_reminders(
    client: &LcuClient,
    tagged: HashMap<String, String>,
    config: &AutoMatchConfig,
    supervisor: &AgentSupervisor,
    lifecycle: &Arc<LifecycleManager>,
    state: &mut AutoMatchState,
) {
    if !config.tag_reminder || tagged.is_empty() {
        return;
    }

    let epoch = lifecycle.snapshot().await.match_epoch;
    if !state.claim_tag_reminder(epoch) {
        log::debug!(
            "[Agent] 标记提醒：本局(epoch={})已提醒，跳过重复事件",
            epoch
        );
        return;
    }

    let client = client.clone();
    let _ = supervisor
        .spawn(
            TaskKind::Other("TagReminders".to_string()),
            DuplicateStrategy::RejectNew,
            move |ctx| async move {
                if ctx.is_cancelled() {
                    return;
                }
                let reminded = send_tag_reminders(&client, &tagged).await;
                log::info!("[Agent] 标记提醒本局完成，命中 {} 人", reminded);
            },
        )
        .await;
}

/// 选人阶段对带标记玩家发送聊天提醒。
///
/// 标记数据来自路人集存储（C 工单契约）：调用方负责把 `puuid → 标签` 映射传入，
/// 本函数只做选人会话读取与聊天消息发送，不直接访问存储。
pub async fn send_tag_reminders(client: &LcuClient, tagged: &HashMap<String, String>) -> usize {
    if tagged.is_empty() {
        return 0;
    }

    let mut session = None;
    for _ in 0..6 {
        sleep(Duration::from_millis(500)).await;
        if let Ok(value) = client.get("/lol-champ-select/v1/session").await {
            if value.is_object() {
                session = Some(value);
                break;
            }
        }
    }
    let Some(session) = session else {
        return 0;
    };

    let room_hint = session
        .pointer("/chatDetails/chatRoomName")
        .and_then(|v| v.as_str())
        .or_else(|| {
            session
                .pointer("/chatDetails/multiUserChatId")
                .and_then(|v| v.as_str())
        })
        .map(|s| s.to_string());

    let Some(conv_id) = wait_for_champ_select_conv_id(client, room_hint.as_deref(), 10).await
    else {
        log::debug!("[Agent] 标记提醒：未找到选人聊天会话");
        return 0;
    };

    sleep(Duration::from_millis(2000)).await;

    let my_team = session
        .get("myTeam")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut reminded = 0usize;
    for player in my_team {
        let puuid = player
            .get("puuid")
            .and_then(|p| p.as_str())
            .unwrap_or("")
            .to_string();
        let Some(tag) = tagged.get(&puuid) else {
            continue;
        };
        let name = player
            .get("summonerName")
            .and_then(|n| n.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(puuid.as_str())
            .to_string();
        let message = serde_json::json!({
            "body": format!("[Yuumi] 玩家 {} 已被标记：{}", name, tag),
            "type": "celebration",
        });
        let path = format!("/lol-chat/v1/conversations/{}/messages", conv_id);
        match client.post(&path, Some(message)).await {
            Ok(_) => {
                log::info!("[Agent] 已提醒标记玩家: {} ({})", name, tag);
                reminded += 1;
            }
            Err(e) => log::warn!("[Agent] 标记提醒发送失败: {}", e),
        }
    }

    if reminded == 0 {
        log::debug!("[Agent] 标记提醒：本局没有命中已标记玩家");
    }
    reminded
}

/// 轮询等待选人聊天会话 ID
async fn wait_for_champ_select_conv_id(
    client: &LcuClient,
    room_hint: Option<&str>,
    max_attempts: usize,
) -> Option<String> {
    for attempt in 0..max_attempts {
        if attempt > 0 {
            sleep(Duration::from_millis(500)).await;
        }
        let Ok(value) = client.get("/lol-chat/v1/conversations").await else {
            continue;
        };
        let Some(list) = value.as_array() else {
            continue;
        };

        if let Some(hint) = room_hint.filter(|s| !s.is_empty()) {
            if let Some(conv) = list.iter().find(|c| {
                let id = c.get("id").and_then(|i| i.as_str()).unwrap_or("");
                let name = c.get("name").and_then(|n| n.as_str()).unwrap_or("");
                id.contains(hint) || name == hint
            }) {
                if let Some(id) = conv.get("id").and_then(|i| i.as_str()) {
                    return Some(id.to_string());
                }
            }
        }

        if let Some(conv) = list.iter().find(|c| {
            let conv_type = c.get("type").and_then(|t| t.as_str()).unwrap_or("");
            let conv_id = c.get("id").and_then(|i| i.as_str()).unwrap_or("");
            conv_type == "championSelect"
                || conv_type == "customGame"
                || conv_id.contains("champ-select")
                || conv_id.contains("champSelect")
        }) {
            if let Some(id) = conv.get("id").and_then(|i| i.as_str()) {
                return Some(id.to_string());
            }
        }
    }
    None
}

/// 读取当前 ReadyCheck 状态
async fn fetch_ready_check_status(client: &LcuClient) -> Option<ReadyCheckStatus> {
    let value = client.get("/lol-matchmaking/v1/ready-check").await.ok()?;
    serde_json::from_value(value).ok()
}

/// 大乱斗家族队列判定（供报边与外部调用复用）
pub fn is_aram_queue(queue_id: i32) -> bool {
    ARAM_BENCH_QUEUE_IDS.contains(&queue_id)
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

    #[test]
    fn test_honor_ballot_deserialization() {
        let json = r#"{
            "eligibleAllies": [
                {"botPlayer": false, "puuid": "p1"},
                {"botPlayer": true, "puuid": "p2"},
                {"botPlayer": false}
            ],
            "eligibleOpponents": [],
            "votePool": {"votes": 2}
        }"#;
        let ballot: HonorBallot = serde_json::from_str(json).unwrap();
        assert_eq!(ballot.eligible_allies.len(), 3);
        assert_eq!(ballot.vote_pool.as_ref().map(|p| p.votes), Some(2));
        assert!(ballot.eligible_allies[1].bot_player);
    }

    #[test]
    fn test_invitation_deserialization() {
        let json = r#"{"invitationId":"abc","state":"Pending","canAcceptInvitation":true}"#;
        let inv: ReceivedInvitation = serde_json::from_str(json).unwrap();
        assert_eq!(inv.invitation_id.as_deref(), Some("abc"));
        assert_eq!(inv.can_accept_invitation, Some(true));
    }

    #[test]
    fn test_is_aram_queue_family() {
        assert!(is_aram_queue(450));
        assert!(is_aram_queue(2400));
        assert!(!is_aram_queue(420));
    }

    /// [D04-e] 同局去重：同一 match_epoch 只提醒一次，新对局（epoch 变化）自动重置
    #[test]
    fn test_tag_reminder_dedup_per_match_epoch() {
        let mut state = AutoMatchState::new();
        assert!(state.claim_tag_reminder(2));
        assert!(!state.claim_tag_reminder(2));
        assert!(state.claim_tag_reminder(3));
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
            auto_handle_invite: false,
            auto_create_lobby: false,
            default_game_mode: 2400,
            aram_team_side: false,
            aram_team_side_visible: false,
            tag_reminder: false,
        };
        let supervisor = AgentSupervisor::new();
        let lifecycle = Arc::new(LifecycleManager::new());
        let mut state = AutoMatchState::new();

        handle_phase_change(
            &client,
            "ReadyCheck",
            &config,
            &supervisor,
            &lifecycle,
            &mut state,
        )
        .await;
        let snap = lifecycle.snapshot().await;
        assert_eq!(snap.phase, GamePhase::ReadyCheck);
        assert_eq!(snap.match_epoch, 2);

        // 离开 ReadyCheck 时必须取消等待中的接受任务（外部无实际请求，仅验证不 panic）
        handle_phase_change(
            &client,
            "ChampSelect",
            &config,
            &supervisor,
            &lifecycle,
            &mut state,
        )
        .await;
        assert_eq!(lifecycle.snapshot().await.phase, GamePhase::ChampSelect);
    }

    #[tokio::test]
    async fn test_ready_check_only_accepts_once() {
        let client = LcuClient::new(1234, 2999, "mock".to_string(), None).unwrap();
        let config = AutoMatchConfig::default();
        let supervisor = AgentSupervisor::new();
        let lifecycle = Arc::new(LifecycleManager::new());
        let mut state = AutoMatchState::new();

        handle_phase_change(
            &client,
            "ReadyCheck",
            &config,
            &supervisor,
            &lifecycle,
            &mut state,
        )
        .await;
        let first = supervisor.running_task_count().await;

        // 同一轮重复 ReadyCheck 事件不得再次派发任务
        handle_ready_check(
            &client,
            &ReadyCheckData {
                state: Some("InProgress".to_string()),
                player_response: Some("None".to_string()),
            },
            &config,
            &supervisor,
            &lifecycle,
            &mut state,
        )
        .await;
        assert_eq!(supervisor.running_task_count().await, first);

        supervisor.cancel_all().await;
    }

    #[tokio::test]
    async fn test_lobby_flags_lifecycle_and_reset() {
        let client = LcuClient::new(1234, 2999, "mock".to_string(), None).unwrap();
        let config = AutoMatchConfig::default();
        let supervisor = AgentSupervisor::new();
        let lifecycle = Arc::new(LifecycleManager::new());
        let mut state = AutoMatchState::new();

        // 1. 进入 Lobby 阶段，标记 created = true
        handle_phase_change(
            &client,
            "Lobby",
            &config,
            &supervisor,
            &lifecycle,
            &mut state,
        )
        .await;
        assert!(state.lobby.lock().unwrap().created);

        // 2. 超出防抖时间进入 None，created 重置为 false
        handle_phase_change(
            &client,
            "None",
            &config,
            &supervisor,
            &lifecycle,
            &mut state,
        )
        .await;
        assert!(!state.lobby.lock().unwrap().created);

        // 3. 模拟建房标记为已创建后，调用 handle_reset_lobby_state 重置
        state.lobby.lock().unwrap().created = true;
        handle_reset_lobby_state(&client, &config, &lifecycle, &mut state).await;
        assert!(!state.lobby.lock().unwrap().created);
    }
}
