//! 自动化 Agent 后台 Worker 运行时
//!
//! 职责：把 LCU WebSocket 事件流泵入两条独立的 `mpsc` 通道，分别由 `auto_match` 与
//! `auto_bp` 的常驻 `tokio::spawn` 任务消费，做到：
//!
//! - **不阻塞 WS 读取循环**：投递使用 `try_send`，通道积压时丢弃最新事件（LCU 事件是
//!   状态快照，新事件总会覆盖旧事件，无需可靠队列语义）；
//! - **超时保护**：单个事件的处理被 `tokio::time::timeout` 包裹，Agent 内部即使出现
//!   HTTP 挂起也不会拖死 Worker，更不会 panic 传播到 WS 循环；
//! - **事件级幂等**：`auto_bp` Worker 用 `ChampSelectSession::action_signature()` 过滤
//!   仅倒计时变化而可执行状态未变的重复推送，配合 `AgentSupervisor` 的任务去重，
//!   避免同一动作被重复提交；
//! - **无泄漏退出**：所有发送端释放后 Worker 自然退出，`shutdown()` 带超时兜底。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use yuumi_core::agents::{
    auto_bp::{handle_champ_select_session, ChampSelectSession},
    auto_match::{
        handle_honor_ballot, handle_invitations, handle_phase_change, handle_ready_check,
        handle_reset_lobby_state, handle_tag_reminders, AutoMatchState, HonorBallot, ReadyCheckData,
        ReceivedInvitation,
    },
    config::{AutoBpConfig, AutoMatchConfig},
    lifecycle::LifecycleManager,
    supervisor::AgentSupervisor,
};
use yuumi_core::lcu::client::LcuClient;

/// 事件通道容量：允许 WS 洪峰下的短暂积压
const CHANNEL_CAPACITY: usize = 64;

/// 单个事件的处理超时上限（Agent 只做「派生任务 + 非阻塞投递」，正常耗时远低于此值）
const HANDLER_TIMEOUT: Duration = Duration::from_secs(5);

/// Worker 退出等待上限
const WORKER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);

/// 投递给 `auto_match` Worker 的事件
#[derive(Debug, Clone)]
pub enum MatchAgentEvent {
    /// 游戏流转阶段变化（原始阶段字符串）
    PhaseChanged(String),
    /// 匹配确认状态推送
    ReadyCheck(ReadyCheckData),
    /// 结算荣誉投票推送（`/lol-honor-v2/v1/ballot`）
    HonorBallot(Box<HonorBallot>),
    /// 收到的组队邀请列表（`/lol-lobby/v2/received-invitations`）
    ReceivedInvitations(Vec<ReceivedInvitation>),
    /// [D04-e] 选人阶段带标记玩家映射（puuid → 标签），由 WS 分派点从路人集只读构造
    TagReminders(HashMap<String, String>),
    /// 重置大厅创建状态并尝试建房（配置变更或用户手动触发时）
    ResetLobbyState,
}

/// 投递给 `auto_bp` Worker 的事件
#[derive(Debug, Clone)]
pub enum BpAgentEvent {
    /// 选人会话全量快照
    ChampSelect(Box<ChampSelectSession>),
}

/// 可克隆的事件投递端
#[derive(Clone)]
pub struct AgentDispatcher {
    match_tx: mpsc::Sender<MatchAgentEvent>,
    bp_tx: mpsc::Sender<BpAgentEvent>,
}

impl AgentDispatcher {
    /// 投递对局流程事件；返回 `false` 表示事件被丢弃（积压已满或 Worker 已退出）
    pub fn dispatch_match(&self, event: MatchAgentEvent) -> bool {
        match self.match_tx.try_send(event) {
            Ok(()) => true,
            Err(mpsc::error::TrySendError::Full(_)) => {
                log::warn!("[AgentRuntime] auto_match 通道积压已满，丢弃本次事件");
                false
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                log::warn!("[AgentRuntime] auto_match Worker 已退出，丢弃本次事件");
                false
            }
        }
    }

    /// 投递选人会话事件；返回 `false` 表示事件被丢弃
    pub fn dispatch_bp(&self, event: BpAgentEvent) -> bool {
        match self.bp_tx.try_send(event) {
            Ok(()) => true,
            Err(mpsc::error::TrySendError::Full(_)) => {
                log::warn!("[AgentRuntime] auto_bp 通道积压已满，丢弃本次事件");
                false
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                log::warn!("[AgentRuntime] auto_bp Worker 已退出，丢弃本次事件");
                false
            }
        }
    }

    /// 触发重置大厅创建状态并尝试建房
    pub fn dispatch_reset_lobby(&self) -> bool {
        self.dispatch_match(MatchAgentEvent::ResetLobbyState)
    }
}

/// 两个后台 Worker 的句柄集合
struct AgentWorkers {
    match_worker: JoinHandle<()>,
    bp_worker: JoinHandle<()>,
}

/// 一次 LCU 连接生命周期内的 Agent 运行时（连接建立时创建，断开时销毁）
pub struct AgentRuntime {
    dispatcher: AgentDispatcher,
    workers: AgentWorkers,
}

impl AgentRuntime {
    /// 启动两个后台 Worker；`client` 为本次连接对应的 LCU 客户端快照
    pub fn start(
        client: LcuClient,
        supervisor: Arc<AgentSupervisor>,
        lifecycle: Arc<LifecycleManager>,
        auto_match_cfg: Arc<watch::Sender<AutoMatchConfig>>,
        auto_bp_cfg: Arc<watch::Sender<AutoBpConfig>>,
    ) -> Self {
        let (match_tx, match_rx) = mpsc::channel::<MatchAgentEvent>(CHANNEL_CAPACITY);
        let (bp_tx, bp_rx) = mpsc::channel::<BpAgentEvent>(CHANNEL_CAPACITY);

        let match_worker = tokio::spawn(run_match_worker(
            client.clone(),
            supervisor.clone(),
            lifecycle.clone(),
            auto_match_cfg,
            match_rx,
        ));
        let bp_worker = tokio::spawn(run_bp_worker(
            client,
            supervisor,
            lifecycle,
            auto_bp_cfg,
            bp_rx,
        ));

        log::info!("[AgentRuntime] auto_match / auto_bp 后台 Worker 已启动");

        Self {
            dispatcher: AgentDispatcher { match_tx, bp_tx },
            workers: AgentWorkers {
                match_worker,
                bp_worker,
            },
        }
    }

    /// 获取投递端
    pub fn dispatcher(&self) -> AgentDispatcher {
        self.dispatcher.clone()
    }

    /// 关闭运行时：先释放发送端（触发 Worker 自然退出），再带超时等待 Worker 结束
    pub async fn shutdown(self) {
        let Self {
            dispatcher,
            workers,
        } = self;
        drop(dispatcher);

        for (label, handle) in [
            ("auto_match", workers.match_worker),
            ("auto_bp", workers.bp_worker),
        ] {
            match tokio::time::timeout(WORKER_SHUTDOWN_TIMEOUT, handle).await {
                Ok(Ok(())) => log::info!("[AgentRuntime] {} Worker 已正常退出", label),
                Ok(Err(e)) => log::warn!("[AgentRuntime] {} Worker 异常结束: {}", label, e),
                Err(_) => log::warn!("[AgentRuntime] {} Worker 退出超时，已放弃等待", label),
            }
        }
    }
}

/// `auto_match` Worker：消费阶段变迁与匹配确认事件
async fn run_match_worker(
    client: LcuClient,
    supervisor: Arc<AgentSupervisor>,
    lifecycle: Arc<LifecycleManager>,
    cfg_tx: Arc<watch::Sender<AutoMatchConfig>>,
    mut rx: mpsc::Receiver<MatchAgentEvent>,
) {
    let mut state = AutoMatchState::new();

    while let Some(event) = rx.recv().await {
        let cfg = { cfg_tx.borrow().clone() };
        match event {
            MatchAgentEvent::PhaseChanged(phase) => {
                run_guarded(
                    "gameflow-phase",
                    handle_phase_change(
                        &client,
                        &phase,
                        &cfg,
                        &supervisor,
                        &lifecycle,
                        &mut state,
                    ),
                )
                .await;
            }
            MatchAgentEvent::ReadyCheck(ready) => {
                run_guarded(
                    "ready-check",
                    handle_ready_check(
                        &client,
                        &ready,
                        &cfg,
                        &supervisor,
                        &lifecycle,
                        &mut state,
                    ),
                )
                .await;
            }
            MatchAgentEvent::HonorBallot(ballot) => {
                run_guarded(
                    "honor-ballot",
                    handle_honor_ballot(&client, &ballot, &cfg, &supervisor, &mut state),
                )
                .await;
            }
            MatchAgentEvent::ReceivedInvitations(invitations) => {
                run_guarded(
                    "received-invitations",
                    handle_invitations(&client, &invitations, &cfg),
                )
                .await;
            }
            MatchAgentEvent::TagReminders(tagged) => {
                run_guarded(
                    "tag-reminders",
                    handle_tag_reminders(
                        &client,
                        tagged,
                        &cfg,
                        &supervisor,
                        &lifecycle,
                        &mut state,
                    ),
                )
                .await;
            }
            MatchAgentEvent::ResetLobbyState => {
                run_guarded(
                    "reset-lobby-state",
                    handle_reset_lobby_state(&client, &cfg, &lifecycle, &mut state),
                )
                .await;
            }
        }
    }
    log::info!("[AgentRuntime] auto_match Worker 退出（事件通道已关闭）");
}

/// `auto_bp` Worker：消费选人会话事件，并对只含倒计时变化的重复快照做去重
async fn run_bp_worker(
    client: LcuClient,
    supervisor: Arc<AgentSupervisor>,
    lifecycle: Arc<LifecycleManager>,
    cfg_tx: Arc<watch::Sender<AutoBpConfig>>,
    mut rx: mpsc::Receiver<BpAgentEvent>,
) {
    let mut last_signature: Option<u64> = None;

    while let Some(event) = rx.recv().await {
        let BpAgentEvent::ChampSelect(session) = event;

        // 事件级幂等：可执行状态未变化（仅倒计时在跳）时跳过重复处理
        let signature = session.action_signature();
        if last_signature == Some(signature) {
            continue;
        }
        last_signature = Some(signature);

        let cfg = { cfg_tx.borrow().clone() };
        run_guarded(
            "champ-select-session",
            handle_champ_select_session(&client, session.as_ref(), &cfg, &supervisor, &lifecycle),
        )
        .await;
    }
    log::info!("[AgentRuntime] auto_bp Worker 退出（事件通道已关闭）");
}

/// 带超时保护地执行一次 Agent 处理
async fn run_guarded<F>(label: &str, fut: F)
where
    F: std::future::Future<Output = ()>,
{
    match tokio::time::timeout(HANDLER_TIMEOUT, fut).await {
        Ok(()) => {}
        Err(_) => log::warn!(
            "[AgentRuntime] {} 事件处理超时（{:?}），已放弃本次处理",
            label,
            HANDLER_TIMEOUT
        ),
    }
}
