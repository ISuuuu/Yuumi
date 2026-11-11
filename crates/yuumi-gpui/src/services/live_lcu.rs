//! 真实 LCU 实时连接与数据编排管理服务

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, watch, RwLock};
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};
use yuumi_core::agents::{
    auto_bp::{request_bench_swap, request_reroll, request_reroll_and_restore, ChampSelectSession},
    auto_match::ReadyCheckData,
    config::{AutoBpConfig, AutoMatchConfig},
    lifecycle::{GamePhase, LifecycleManager},
    supervisor::AgentSupervisor,
};
use yuumi_core::gameflow::team_builder::MapSide;
use yuumi_core::lcu::{
    asset_cache::AssetCache,
    game_data::{fetch_game_data_assets, GameDataAssets},
    monitor::{detect_lcu, LcuConnectionInfo},
    ws::{start_ws, LcuWsEvent, LcuWsHandle},
};
use yuumi_core::parsers::game_info::{
    fetch_live_game_info, fetch_liveclient_snapshot, fetch_ranked_stats, liveclient_map_side,
    liveclient_teams, merge_liveclient_teams, ChampSelectSnapshot, GameInfoSource,
    GameflowSessionSnapshot, LiveGameInfo, RankedStats,
};
use yuumi_core::parsers::loot::{EssenceBalances, LootInventoryItem};
use yuumi_core::parsers::loot_actions::{
    self, LootActionItem, LootActionKind, LootActionProgress, MAX_LOOP_COUNT,
};
use yuumi_core::parsers::mastery::ChampionMasteryItem;
use yuumi_core::parsers::match_parser::{fetch_match_history, MatchDisplay};
use yuumi_core::parsers::summoner::{fetch_current_summoner, SummonerInfo};
use yuumi_core::parsers::teammates::{fetch_recent_teammates, RecentTeammate};

use crate::bench_overlay::{bench_user_hidden, reset_bench_user_hidden}; // [D07-a]
use crate::services::agent_runtime::{AgentRuntime, BpAgentEvent, MatchAgentEvent};
use yuumi_core::storage::SavedPlayersStorage; // [D04-e]

/// 选人会话变化后的对局信息拉取防抖间隔
const CHAMP_SELECT_DEBOUNCE: Duration = Duration::from_millis(500);
/// 进入对局阶段后的拉取延迟（等待 gameflow session 填充 gameData）
const IN_PROGRESS_DELAY: Duration = Duration::from_millis(800);
/// 对局结束到战绩落库的等待时间
const POST_GAME_HISTORY_DELAY: Duration = Duration::from_secs(3);
/// 生涯页展示的近期对局条数（LCU 产品接口实际会多返回一场，统一裁剪）
const CAREER_MATCH_LIMIT: usize = 20;

/// 全局连接状态
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected {
        info: LcuConnectionInfo,
        summoner: Box<Option<SummonerInfo>>,
    },
}

/// 实时分发的 LCU 领域事件
#[derive(Debug, Clone)]
pub enum LiveLcuEvent {
    ConnectionChanged(ConnectionStatus),
    GamePhaseChanged(GamePhase),
    ChampSelectUpdated(ChampSelectSession),
    MatchHistoryLoaded(Vec<MatchDisplay>),
    ChampionMasteryLoaded(Vec<ChampionMasteryItem>),
    /// 英雄熟练度拉取失败（对照 Vue E01，由生涯页以 Toast 呈现）
    MasteryLoadFailed(String),
    LootSummaryLoaded(EssenceBalances, Vec<LootInventoryItem>),
    /// 战利品写动作的单条进度（由 run_loot_action 逐条广播）
    LootActionProgress(LootActionProgress),
    /// 最近队友聚合完成（近期最多 20 场同队队友列表）
    RecentTeammatesLoaded(Vec<RecentTeammate>),
    /// 本人排位数据（单排 / 灵活等队列段位）
    RankedStatsLoaded(RankedStats),
    /// 10 人对局信息开始拉取（前端置加载态）
    GameInfoLoading,
    /// 10 人对局信息快照更新（退回大厅后仍保留最后一次）
    GameInfoUpdated(Arc<LiveGameInfo>),
    GameDataReady(Arc<GameDataAssets>),
}

/// 实时 LCU 管理服务句柄
#[derive(Clone)]
pub struct LiveLcuService {
    pub status: Arc<watch::Sender<ConnectionStatus>>,
    pub event_tx: broadcast::Sender<LiveLcuEvent>,
    cloud_events: broadcast::Sender<LcuWsEvent>,
    pub supervisor: Arc<AgentSupervisor>,
    pub lifecycle: Arc<LifecycleManager>,
    pub asset_cache: Arc<AssetCache>,
    pub auto_match_cfg: Arc<watch::Sender<AutoMatchConfig>>,
    pub auto_bp_cfg: Arc<watch::Sender<AutoBpConfig>>,
    _auto_match_rx: Arc<watch::Receiver<AutoMatchConfig>>,
    _auto_bp_rx: Arc<watch::Receiver<AutoBpConfig>>,
    pub client: Arc<tokio::sync::RwLock<Option<yuumi_core::lcu::client::LcuClient>>>,
    pub game_data_assets: Arc<tokio::sync::RwLock<Option<Arc<GameDataAssets>>>>,
    /// 当前连接生命周期内的 Agent 后台运行时（未连接时为 None）
    agent_runtime: Arc<tokio::sync::RwLock<Option<AgentRuntime>>>,
    /// 本人近期真实战绩（宿命对局判定与生涯页共用）
    recent_matches: Arc<RwLock<Vec<MatchDisplay>>>,
    /// 最近一次对局信息快照（退回大厅后保留）
    game_info: Arc<RwLock<Option<Arc<LiveGameInfo>>>>,
    /// 当前 gameflow 阶段
    current_phase: Arc<RwLock<String>>,
    /// 对局信息拉取任务（防抖用）
    game_info_task: Arc<RwLock<Option<JoinHandle<()>>>>,
    /// 最近一次拉取的会话指纹，避免同内容重复拉取
    last_game_info_key: Arc<RwLock<String>>,
    /// 对局信息拉取串行化
    refresh_lock: Arc<RwLock<()>>,
}

impl LiveLcuService {
    pub fn new(asset_cache: Arc<AssetCache>) -> Self {
        let (status_tx, _) = watch::channel(ConnectionStatus::Disconnected);
        let (event_tx, _) = broadcast::channel(128);
        let (cloud_events, _) = broadcast::channel(64);
        let (match_tx, match_rx) = watch::channel(AutoMatchConfig::default());
        let (bp_tx, bp_rx) = watch::channel(AutoBpConfig::default());

        Self {
            status: Arc::new(status_tx),
            event_tx,
            cloud_events,
            supervisor: Arc::new(AgentSupervisor::new()),
            lifecycle: Arc::new(LifecycleManager::new()),
            asset_cache,
            auto_match_cfg: Arc::new(match_tx),
            auto_bp_cfg: Arc::new(bp_tx),
            _auto_match_rx: Arc::new(match_rx),
            _auto_bp_rx: Arc::new(bp_rx),
            client: Arc::new(tokio::sync::RwLock::new(None)),
            game_data_assets: Arc::new(tokio::sync::RwLock::new(None)),
            agent_runtime: Arc::new(tokio::sync::RwLock::new(None)),
            recent_matches: Arc::new(RwLock::new(Vec::new())),
            game_info: Arc::new(RwLock::new(None)),
            current_phase: Arc::new(RwLock::new("None".to_string())),
            game_info_task: Arc::new(RwLock::new(None)),
            last_game_info_key: Arc::new(RwLock::new(String::new())),
            refresh_lock: Arc::new(RwLock::new(())),
        }
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<LiveLcuEvent> {
        self.event_tx.subscribe()
    }

    pub fn subscribe_cloud_events(&self) -> broadcast::Receiver<LcuWsEvent> {
        self.cloud_events.subscribe()
    }

    pub fn subscribe_status(&self) -> watch::Receiver<ConnectionStatus> {
        self.status.subscribe()
    }

    /// 更新 Match Agent 配置并立即同步至 channel（确保无接收端时值也能被更新）
    pub fn update_auto_match_cfg(&self, cfg: AutoMatchConfig) {
        self.auto_match_cfg.send_replace(cfg);
    }

    /// 更新 BP Agent 配置并立即同步至 channel（确保无接收端时值也能被更新）
    pub fn update_auto_bp_cfg(&self, cfg: AutoBpConfig) {
        self.auto_bp_cfg.send_replace(cfg);
    }

    /// 重置大厅创建状态并尝试建房（配置变更或用户手动触发时）
    pub async fn reset_lobby_state(&self) {
        let guard = self.agent_runtime.read().await;
        if let Some(runtime) = guard.as_ref() {
            runtime.dispatcher().dispatch_reset_lobby();
        }
    }

    /// 获取当前的 LcuClient（若已连接）
    pub async fn get_client(&self) -> Option<yuumi_core::lcu::client::LcuClient> {
        self.client.read().await.clone()
    }

    /// 获取当前预加载的 GameDataAssets
    pub async fn get_game_data_assets(&self) -> Option<Arc<GameDataAssets>> {
        self.game_data_assets.read().await.clone()
    }

    /// 当前 gameflow 阶段
    pub async fn get_current_phase(&self) -> String {
        self.current_phase.read().await.clone()
    }

    /// 获取本人近期真实战绩（宿命判定数据源）
    async fn get_recent_matches(&self) -> Vec<MatchDisplay> {
        self.recent_matches.read().await.clone()
    }

    /// 启动后台常驻监控与 WebSocket 分发循环
    pub fn start(&self) {
        let this = self.clone();
        tokio::spawn(async move {
            this.run_loop().await;
        });
    }

    async fn run_loop(&self) {
        let mut active_ws: Option<LcuWsHandle> = None;
        let mut active_info: Option<LcuConnectionInfo> = None;
        let mut game_data_assets: Option<Arc<GameDataAssets>> = None;

        loop {
            // 1. 探测 LCU 进程
            let detected = detect_lcu();

            match (detected, &active_info) {
                (Some(info), None) => {
                    log::info!(
                        "[LiveLcu] 检测到本地 LCU 进程: pid={}, port={}",
                        info.pid,
                        info.port
                    );
                    let _ = self.status.send(ConnectionStatus::Connecting);

                    // 实例化 LcuClient
                    if let Ok(client) = yuumi_core::lcu::client::LcuClient::new(
                        info.pid,
                        info.port,
                        info.token.clone(),
                        None,
                    ) {
                        // 预加载 GameData 字典
                        if game_data_assets.is_none() {
                            log::info!("[LiveLcu] 正在从 LCU 预加载 GameData 资源字典...");
                            let assets = fetch_game_data_assets(&client).await;
                            log::info!(
                                "[LiveLcu] GameData 字典加载完毕: items={}, spells={}, runes={}, champions={}, augments={}",
                                assets.items.len(),
                                assets.spells.len(),
                                assets.runes.len(),
                                assets.champions.len(),
                                assets.augments.len(),
                            );
                            // 选择器静态资源：召唤师技能图标 + 后台拉取腾讯英雄别称库
                            super::champion_alias::store_spell_icons(&assets);
                            super::champion_alias::spawn_fetch_tencent_keywords();

                            let arc_assets = Arc::new(assets);
                            game_data_assets = Some(arc_assets.clone());
                            *self.game_data_assets.write().await = Some(arc_assets.clone());
                            let _ = self.event_tx.send(LiveLcuEvent::GameDataReady(arc_assets));
                        }

                        // 同步活跃 LCU 客户端至 AssetCache，以便图片优先通过活跃客户端拉取
                        self.asset_cache.set_lcu_client(Some(client.clone()));
                        *self.client.write().await = Some(client.clone());

                        // 获取当前召唤师
                        log::info!("[LiveLcu] 正在获取当前登录召唤师信息...");
                        let summoner = match fetch_current_summoner(&client).await {
                            Ok(s) => {
                                log::info!(
                                    "[LiveLcu] 成功获取召唤师: {} (puuid: {})",
                                    s.full_name(),
                                    s.puuid
                                );
                                Some(s)
                            }
                            Err(e) => {
                                log::warn!("[LiveLcu] 获取当前召唤师失败: {}", e);
                                None
                            }
                        };
                        let _ = self.status.send(ConnectionStatus::Connected {
                            info: info.clone(),
                            summoner: Box::new(summoner.clone()),
                        });
                        let _ = self.event_tx.send(LiveLcuEvent::ConnectionChanged(
                            ConnectionStatus::Connected {
                                info: info.clone(),
                                summoner: Box::new(summoner.clone()),
                            },
                        ));

                        // 异步并发拉取召唤师战绩、英雄熟练度与战利品
                        let this_career = self.clone();
                        tokio::spawn(async move {
                            this_career.refresh_career_data().await;
                        });

                        // 启动 Agent 后台 Worker（与本次连接同生命周期）
                        let runtime = AgentRuntime::start(
                            client.clone(),
                            self.supervisor.clone(),
                            self.lifecycle.clone(),
                            self.auto_match_cfg.clone(),
                            self.auto_bp_cfg.clone(),
                        );
                        let dispatcher = runtime.dispatcher();
                        *self.agent_runtime.write().await = Some(runtime);

                        // 获取初始游戏阶段并分发给 Agent、Lifecycle 与 UI
                        let initial_phase =
                            match client.get("/lol-gameflow/v1/gameflow-phase").await {
                                Ok(serde_json::Value::String(p)) => p,
                                _ => "None".to_string(),
                            };
                        log::info!("[LiveLcu] 获取到初始游戏阶段: {}", initial_phase);
                        *self.current_phase.write().await = initial_phase.clone();
                        let initial_game_phase = GamePhase::from(initial_phase.as_str());
                        let _ = self.lifecycle.on_phase_changed(&initial_phase).await;
                        dispatcher
                            .dispatch_match(MatchAgentEvent::PhaseChanged(initial_phase.clone()));
                        let _ = self
                            .event_tx
                            .send(LiveLcuEvent::GamePhaseChanged(initial_game_phase));

                        // 处于选人阶段时，主动拉取一次会话以激活 BP Agent 与 UI
                        if initial_phase == "ChampSelect" {
                            if let Ok(val) = client.get("/lol-champ-select/v1/session").await {
                                if let Ok(session) = serde_json::from_value::<
                                    yuumi_core::agents::auto_bp::ChampSelectSession,
                                >(val)
                                {
                                    dispatcher.dispatch_bp(
                                        super::agent_runtime::BpAgentEvent::ChampSelect(Box::new(
                                            session.clone(),
                                        )),
                                    );
                                    let _ = self
                                        .event_tx
                                        .send(LiveLcuEvent::ChampSelectUpdated(session));
                                }
                            }
                        } else if initial_phase == "ReadyCheck" {
                            if let Ok(val) = client.get("/lol-matchmaking/v1/ready-check").await {
                                if let Ok(ready) = serde_json::from_value::<
                                    yuumi_core::agents::auto_match::ReadyCheckData,
                                >(val)
                                {
                                    dispatcher.dispatch_match(
                                        super::agent_runtime::MatchAgentEvent::ReadyCheck(ready),
                                    );
                                }
                            }
                        }

                        // 重连后若正处于对局，补拉当前 10 人对局信息
                        self.schedule_game_info_refresh(initial_phase, Duration::ZERO)
                            .await;

                        // 建立 WebSocket 连接
                        let ws_handle = start_ws(info.port, info.token.clone());
                        let mut ws_rx = ws_handle.subscribe_events();
                        let this_clone = self.clone();

                        tokio::spawn(async move {
                            while let Ok(event) = ws_rx.recv().await {
                                this_clone.handle_ws_event(event).await;
                            }
                            log::info!("[LiveLcu] WS 事件读取任务已结束");
                        });

                        active_ws = Some(ws_handle);
                        active_info = Some(info);
                    }
                }
                (None, Some(_)) => {
                    log::info!("[LiveLcu] LCU 进程已退出，重置连接状态");
                    if let Some(ws) = active_ws.take() {
                        ws.cancel();
                    }
                    active_info = None;
                    self.asset_cache.set_lcu_client(None);
                    *self.client.write().await = None;
                    self.supervisor.cancel_all().await;

                    // 取消对局信息拉取任务并重置去重指纹
                    if let Some(task) = self.game_info_task.write().await.take() {
                        task.abort();
                    }
                    *self.last_game_info_key.write().await = String::new();
                    *self.recent_matches.write().await = Vec::new();

                    // 关闭 Agent Worker：先取出（释放锁）再带超时等待，避免跨 await 持锁
                    let runtime = { self.agent_runtime.write().await.take() };
                    if let Some(runtime) = runtime {
                        runtime.shutdown().await;
                    }

                    let _ = self.lifecycle.on_phase_changed("None").await;
                    let _ = self.status.send(ConnectionStatus::Disconnected);
                    let _ = self.event_tx.send(LiveLcuEvent::ConnectionChanged(
                        ConnectionStatus::Disconnected,
                    ));
                }
                _ => {}
            }

            sleep(Duration::from_secs(2)).await;
        }
    }

    /// 将 WS 事件分发给后台 Agent Worker，并向 UI 广播领域事件
    async fn handle_ws_event(&self, event: LcuWsEvent) {
        if self.cloud_events.receiver_count() > 0 && yuumi_core::signalr::forwards_event(&event.uri)
        {
            let _ = self.cloud_events.send(event.clone());
        }
        let dispatcher = {
            let guard = self.agent_runtime.read().await;
            guard.as_ref().map(|runtime| runtime.dispatcher())
        };

        match event.uri.as_str() {
            "/lol-gameflow/v1/gameflow-phase" => {
                let Some(phase_str) = event.data.as_str().map(|s| s.to_string()) else {
                    return;
                };
                if let Some(d) = &dispatcher {
                    d.dispatch_match(MatchAgentEvent::PhaseChanged(phase_str.clone()));
                }
                // [D07-a] 进入新一轮选人：清除「本会话用户已隐藏」标志，允许下轮自动弹出
                let previous_phase = self.get_current_phase().await;
                if phase_str.as_str() == "ChampSelect" && previous_phase != "ChampSelect" {
                    reset_bench_user_hidden();
                }
                let phase = GamePhase::from(phase_str.as_str());
                *self.current_phase.write().await = phase_str.clone();
                let _ = self
                    .event_tx
                    .send(LiveLcuEvent::GamePhaseChanged(phase.clone()));

                match phase {
                    GamePhase::ChampSelect => {
                        self.schedule_game_info_refresh(phase_str, CHAMP_SELECT_DEBOUNCE)
                            .await;
                    }
                    GamePhase::GameStart | GamePhase::InProgress | GamePhase::Reconnect => {
                        self.schedule_game_info_refresh(phase_str, IN_PROGRESS_DELAY)
                            .await;
                    }
                    GamePhase::PreEndOfGame | GamePhase::WaitingForStats | GamePhase::EndOfGame => {
                        // 若对局进行期间未及时拿到带 gameId 的 10 人快照，结算阶段兜底补拉一次以便记录路人集
                        let missing_game_id = self
                            .game_info
                            .read()
                            .await
                            .as_ref()
                            .and_then(|g| g.game_id)
                            .unwrap_or(0)
                            == 0;
                        if missing_game_id {
                            self.schedule_game_info_refresh(phase_str, Duration::ZERO)
                                .await;
                        }
                        // 对局结束：延迟刷新生涯战绩，10 人快照保留
                        let this = self.clone();
                        tokio::spawn(async move {
                            sleep(POST_GAME_HISTORY_DELAY).await;
                            this.refresh_career_data().await;
                        });
                    }
                    _ => {}
                }
            }
            "/lol-matchmaking/v1/ready-check" => {
                if let Ok(ready) = serde_json::from_value::<ReadyCheckData>(event.data) {
                    if let Some(d) = &dispatcher {
                        d.dispatch_match(MatchAgentEvent::ReadyCheck(ready));
                    }
                }
            }
            "/lol-champ-select/v1/session" => {
                if let Ok(session) = serde_json::from_value::<ChampSelectSession>(event.data) {
                    if let Some(d) = &dispatcher {
                        d.dispatch_bp(BpAgentEvent::ChampSelect(Box::new(session.clone())));
                    }

                    // [D04-e] tag_reminder 开启时，从路人集存储只读构造 puuid→标签 映射并
                    // 投递给 auto_match Worker；同局去重由 handle_tag_reminders 按 match_epoch 完成。
                    let tag_reminder_enabled = self.auto_match_cfg.borrow().tag_reminder;
                    if tag_reminder_enabled {
                        let self_puuid = current_self_puuid(&self.status);
                        let tagged = load_tagged_players_map(&self_puuid);
                        if !tagged.is_empty() {
                            if let Some(d) = &dispatcher {
                                d.dispatch_match(MatchAgentEvent::TagReminders(tagged));
                            }
                        }
                    }

                    // [D07-a] 用户本会话已隐藏板凳席：广播「非板凳席」会话，使主窗清空
                    // bench_session，sync_bench_overlay 判定 should_show=false，重复选人事件
                    // 与 persist_config 都不会再自动重开悬浮窗。
                    let ui_session = if bench_user_hidden() {
                        let mut hidden = session.clone();
                        hidden.queue_id = 0;
                        hidden.is_custom_game = false;
                        hidden.bench_enabled = false;
                        hidden
                    } else {
                        session.clone()
                    };
                    let _ = self
                        .event_tx
                        .send(LiveLcuEvent::ChampSelectUpdated(ui_session));
                }
                // 选人内容变化（锁英雄 / 换位）时刷新对局信息看板
                if self.get_current_phase().await == "ChampSelect" {
                    self.schedule_game_info_refresh(
                        "ChampSelect".to_string(),
                        CHAMP_SELECT_DEBOUNCE,
                    )
                    .await;
                }
            }
            "/lol-gameflow/v1/session" => {
                let phase = self.get_current_phase().await;
                if matches!(phase.as_str(), "GameStart" | "InProgress" | "Reconnect") {
                    self.schedule_game_info_refresh(phase, IN_PROGRESS_DELAY)
                        .await;
                }
            }
            // 结算荣誉投票：交给 auto_match 决定是否自动点赞
            "/lol-honor-v2/v1/ballot" => {
                if let Some(d) = &dispatcher {
                    if let Ok(ballot) = serde_json::from_value::<
                        yuumi_core::agents::auto_match::HonorBallot,
                    >(event.data)
                    {
                        d.dispatch_match(MatchAgentEvent::HonorBallot(Box::new(ballot)));
                    }
                }
            }
            // 收到的组队邀请：交给 auto_match 决定是否自动接受
            "/lol-lobby/v2/received-invitations" => {
                if let Some(d) = &dispatcher {
                    if let Ok(invitations) = serde_json::from_value::<
                        Vec<yuumi_core::agents::auto_match::ReceivedInvitation>,
                    >(event.data)
                    {
                        d.dispatch_match(MatchAgentEvent::ReceivedInvitations(invitations));
                    }
                }
            }
            // 当前登录召唤师信息就绪或切换账号：同步状态并触发生涯战绩刷新
            "/lol-summoner/v1/current-summoner" => {
                let this = self.clone();
                tokio::spawn(async move {
                    if let Some(client) = this.get_client().await {
                        if let Ok(s) = fetch_current_summoner(&client).await {
                            log::info!(
                                "[LiveLcu] 收到当前召唤师更新: {} (puuid: {})",
                                s.full_name(),
                                s.puuid
                            );
                            let current_status = this.status.borrow().clone();
                            if let ConnectionStatus::Connected { info, .. } = current_status {
                                let new_status = ConnectionStatus::Connected {
                                    info,
                                    summoner: Box::new(Some(s)),
                                };
                                let _ = this.status.send(new_status.clone());
                                let _ = this
                                    .event_tx
                                    .send(LiveLcuEvent::ConnectionChanged(new_status));
                            }
                            this.refresh_career_data().await;
                        }
                    }
                });
            }
            _ => {}
        }
    }

    /// 刷新当前登录召唤师的生涯数据（默认最近 20 场战绩）
    pub async fn refresh_career_data(&self) {
        self.refresh_career_data_with_limit(CAREER_MATCH_LIMIT)
            .await;
    }

    /// 刷新当前登录召唤师的生涯数据，指定拉取的战绩场数上限（1..=100）
    pub async fn refresh_career_data_with_limit(&self, limit: usize) {
        let client_opt = self.client.read().await.clone();
        let Some(client) = client_opt else {
            log::warn!("[LiveLcu] 无法刷新生涯数据: LCU client 未连接");
            return;
        };

        // 1. 获取当前召唤师
        let summoner = match fetch_current_summoner(&client).await {
            Ok(s) => s,
            Err(e) => {
                log::error!("[LiveLcu] 刷新生涯数据失败，无法获取召唤师: {}", e);
                return;
            }
        };

        // 2. 获取 GameDataAssets
        let assets_opt = self.game_data_assets.read().await.clone();
        let Some(assets) = assets_opt else {
            log::warn!("[LiveLcu] 无法刷新战绩: GameDataAssets 尚未就绪");
            return;
        };

        log::info!(
            "[LiveLcu] 开始并发拉取召唤师 [{}] 的生涯对局历史 (限 {} 场)、熟练度与战利品...",
            summoner.full_name(),
            limit
        );

        let fetch_limit = limit.clamp(1, 100);
        let end_idx = (fetch_limit as u32).saturating_sub(1);

        // 并发拉取战绩 (endIndex 为闭区间故取 fetch_limit - 1)
        let c_matches = client.clone();
        let puuid_clone = summoner.puuid.clone();
        let assets_clone = assets.clone();
        let this_matches = self.clone();
        let matches_task = tokio::spawn(async move {
            match fetch_match_history(
                &c_matches,
                &puuid_clone,
                Some(0),
                Some(end_idx),
                &assets_clone,
            )
            .await
            {
                Ok(mut matches) => {
                    matches.truncate(fetch_limit);
                    log::info!("[LiveLcu] 成功拉取当前召唤师战绩: count={}", matches.len());
                    // 同步缓存本人近期战绩，供宿命对局判定复用
                    *this_matches.recent_matches.write().await = matches.clone();

                    let game_ids: Vec<u64> = matches.iter().map(|m| m.game_id).collect();
                    let _ = this_matches
                        .event_tx
                        .send(LiveLcuEvent::MatchHistoryLoaded(matches));

                    // 异步聚合近 20 场同队队友数据
                    let c_tm = c_matches.clone();
                    let puuid_tm = puuid_clone.clone();
                    let tx_tm = this_matches.event_tx.clone();
                    tokio::spawn(async move {
                        let tagged_map = std::collections::HashMap::new();
                        match fetch_recent_teammates(&c_tm, &game_ids, &puuid_tm, &tagged_map).await
                        {
                            Ok(teammates) => {
                                log::info!("[LiveLcu] 成功聚合最近队友: count={}", teammates.len());
                                let _ = tx_tm.send(LiveLcuEvent::RecentTeammatesLoaded(teammates));
                            }
                            Err(e) => {
                                log::warn!("[LiveLcu] 聚合最近队友失败: {}", e);
                            }
                        }
                    });
                }
                Err(e) => {
                    log::error!("[LiveLcu] 拉取当前召唤师战绩失败: {}", e);
                }
            }
        });

        // 并发拉取英雄熟练度
        let c_mastery = client.clone();
        let event_tx_mastery = self.event_tx.clone();
        let mastery_task = tokio::spawn(async move {
            match yuumi_core::parsers::mastery::fetch_champion_mastery_list(&c_mastery).await {
                Ok(masteries) => {
                    log::info!("[LiveLcu] 成功拉取英雄熟练度: count={}", masteries.len());
                    let _ = event_tx_mastery.send(LiveLcuEvent::ChampionMasteryLoaded(masteries));
                }
                Err(e) => {
                    log::warn!("[LiveLcu] 拉取英雄熟练度失败: {}", e);
                    let _ = event_tx_mastery.send(LiveLcuEvent::MasteryLoadFailed(e));
                }
            }
        });

        // 并发拉取战利品与精粹
        let c_loot = client.clone();
        let event_tx_loot = self.event_tx.clone();
        let loot_task = tokio::spawn(async move {
            match yuumi_core::parsers::loot::fetch_loot_summary(&c_loot).await {
                Ok((balances, items)) => {
                    log::info!(
                        "[LiveLcu] 成功拉取战利品: 蓝精粹={}, 橙精粹={}, 神话精粹={}, 物品数={}",
                        balances.blue_essence,
                        balances.orange_essence,
                        balances.mythic_essence,
                        items.len()
                    );
                    let _ = event_tx_loot.send(LiveLcuEvent::LootSummaryLoaded(balances, items));
                }
                Err(e) => {
                    log::warn!("[LiveLcu] 拉取战利品库存失败: {}", e);
                }
            }
        });

        // 并发拉取排位数据（单排 / 灵活）
        let c_ranked = client.clone();
        let puuid_ranked = summoner.puuid.clone();
        let event_tx_ranked = self.event_tx.clone();
        let ranked_task = tokio::spawn(async move {
            match fetch_ranked_stats(&c_ranked, &puuid_ranked).await {
                Ok(stats) => {
                    log::info!(
                        "[LiveLcu] 成功拉取排位数据: 队列数={}",
                        stats.queue_map.len()
                    );
                    let _ = event_tx_ranked.send(LiveLcuEvent::RankedStatsLoaded(stats));
                }
                Err(e) => {
                    log::warn!("[LiveLcu] 拉取排位数据失败: {}", e);
                }
            }
        });

        let _ = tokio::join!(matches_task, mastery_task, loot_task, ranked_task);

        log::info!("[LiveLcu] 召唤师生涯数据全部拉取完毕并派发完成");
    }

    /// 战利品写动作统一入口（供 main.rs 在用户主动点击时调用）。
    ///
    /// 这里只提供入口：启动流程与任何自动流程都不会调用本方法，也不会在内部
    /// 常驻后台任务。返回聚合后的逐条进度，并逐条广播
    /// `LiveLcuEvent::LootActionProgress`，由 main.rs 消费后刷新进度面板。
    pub async fn run_loot_action(
        &self,
        kind: LootActionKind,
        items: Vec<LootActionItem>,
    ) -> Result<Vec<LootActionProgress>, String> {
        let client = self
            .get_client()
            .await
            .ok_or_else(|| "英雄联盟客户端未连接".to_string())?;

        let progress = match kind {
            LootActionKind::Open => {
                let first = items
                    .first()
                    .ok_or_else(|| "开启动作缺少条目".to_string())?;
                loot_actions::batch_open_loots(
                    &client,
                    &first.recipe_name,
                    &first.ingredients,
                    first.count,
                )
                .await?
            }
            LootActionKind::SmartOpenAll => {
                let batches: Vec<loot_actions::OpenBatchItem> = items
                    .iter()
                    .map(|i| loot_actions::OpenBatchItem {
                        loot_id: i.loot_id.clone(),
                        name: i.name.clone(),
                        count: i.count,
                        recipe_name: i.recipe_name.clone(),
                        ingredients: i.ingredients.clone(),
                    })
                    .collect();
                loot_actions::smart_open_all_loots(&client, &batches).await?
            }
            LootActionKind::Disenchant => {
                let jobs = to_disenchant_items(&items);
                loot_actions::disenchant_loot(&client, &jobs).await?
            }
            LootActionKind::Reroll => {
                // 对照 Vue：每个条目按 count 展开为 loot_id，3 个一组重铸
                let mut ids = Vec::new();
                for item in &items {
                    let n = item.count.clamp(0, MAX_LOOP_COUNT);
                    for _ in 0..n {
                        ids.push(item.loot_id.clone());
                    }
                }
                loot_actions::reroll_loot(&client, &ids).await?
            }
            LootActionKind::Upgrade => {
                let jobs = to_disenchant_items(&items);
                loot_actions::upgrade_loot(&client, &jobs).await?
            }
        };

        // 仅在用户主动调用本入口时才会执行；逐条广播进度事件
        for p in &progress {
            let _ = self
                .event_tx
                .send(LiveLcuEvent::LootActionProgress(p.clone()));
        }
        Ok(progress)
    }

    /// 一键换取大乱斗板凳席英雄
    pub async fn swap_bench_champion(&self, champion_id: i32) -> Result<(), String> {
        let client = self
            .get_client()
            .await
            .ok_or_else(|| "英雄联盟客户端未连接".to_string())?;
        request_bench_swap(&client, champion_id).await
    }

    /// 摇号（重随）当前英雄
    pub async fn reroll_aram_champion(&self) -> Result<(), String> {
        let client = self
            .get_client()
            .await
            .ok_or_else(|| "英雄联盟客户端未连接".to_string())?;
        request_reroll(&client).await
    }

    /// 摇号并瞬时换回原英雄（返回被换回的原英雄 ID）
    pub async fn reroll_and_restore_aram_champion(&self) -> Result<i32, String> {
        let client = self
            .get_client()
            .await
            .ok_or_else(|| "英雄联盟客户端未连接".to_string())?;
        request_reroll_and_restore(&client).await
    }

    /// 防抖调度对局信息刷新：取消上一次未完成的任务，延迟后重新拉取
    async fn schedule_game_info_refresh(&self, phase: String, delay: Duration) {
        let mut slot = self.game_info_task.write().await;
        if let Some(handle) = slot.take() {
            handle.abort();
        }
        let this = self.clone();
        *slot = Some(tokio::spawn(async move {
            if !delay.is_zero() {
                sleep(delay).await;
            }
            this.refresh_game_info(&phase).await;
        }));
    }

    /// 读取当前阶段的 10 人会话并并发拉取段位 / 近期 KDA / 宿命对局
    async fn refresh_game_info(&self, phase: &str) {
        let _guard = self.refresh_lock.write().await;

        let Some(client) = self.get_client().await else {
            log::debug!("[LiveLcu] LCU 未连接，跳过对局信息拉取");
            return;
        };

        let mut source = match phase {
            "ChampSelect" => {
                let mut pin_side = fetch_pin_drop_side(&client).await;
                if pin_side.is_none() {
                    let prev_in_champ_select = self
                        .game_info
                        .read()
                        .await
                        .as_ref()
                        .is_some_and(|g| g.phase == "ChampSelect");
                    if !prev_in_champ_select {
                        sleep(Duration::from_millis(400)).await;
                        pin_side = fetch_pin_drop_side(&client).await;
                    }
                }
                match client.get("/lol-champ-select/v1/session").await {
                    Ok(value) => match serde_json::from_value::<ChampSelectSnapshot>(value) {
                        Ok(snapshot) => GameInfoSource::from_champ_select(&snapshot, pin_side),
                        Err(e) => {
                            log::warn!("[LiveLcu] 解析选人会话失败: {}", e);
                            None
                        }
                    },
                    Err(e) => {
                        log::debug!("[LiveLcu] 读取选人会话失败: {}", e);
                        None
                    }
                }
            }
            "GameStart" | "InProgress" | "Reconnect" => {
                // 对局刚开始时，LCU gameflow session 可能只填充了一侧队伍；若单边为空则短暂重试
                let mut session_val = None;
                for attempt in 0..3 {
                    if attempt > 0 {
                        sleep(Duration::from_millis(800)).await;
                    }
                    match client.get("/lol-gameflow/v1/session").await {
                        Ok(value) => {
                            let t1_len = value
                                .pointer("/gameData/teamOne")
                                .and_then(|v| v.as_array())
                                .map(|a| a.len())
                                .unwrap_or(0);
                            let t2_len = value
                                .pointer("/gameData/teamTwo")
                                .and_then(|v| v.as_array())
                                .map(|a| a.len())
                                .unwrap_or(0);
                            let is_custom = value
                                .pointer("/gameData/queue/isCustom")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            session_val = Some(value);
                            if is_custom || (t1_len > 0 && t2_len > 0) {
                                break;
                            }
                            log::debug!(
                                "[LiveLcu] 对局会话队伍尚不完整 (t1={}, t2={})，第 {}/3 次等待",
                                t1_len,
                                t2_len,
                                attempt + 1
                            );
                        }
                        Err(e) => {
                            log::debug!(
                                "[LiveLcu] 读取对局会话失败 (第 {}/3 次): {}",
                                attempt + 1,
                                e
                            );
                        }
                    }
                }

                match session_val {
                    Some(value) => match serde_json::from_value::<GameflowSessionSnapshot>(value) {
                        Ok(snapshot) => GameInfoSource::from_gameflow(phase, &snapshot),
                        Err(e) => {
                            log::warn!("[LiveLcu] 解析对局会话失败: {}", e);
                            None
                        }
                    },
                    None => None,
                }
            }
            _ => match client.get("/lol-gameflow/v1/session").await {
                Ok(value) => serde_json::from_value::<GameflowSessionSnapshot>(value)
                    .ok()
                    .and_then(|snapshot| GameInfoSource::from_gameflow(phase, &snapshot)),
                Err(e) => {
                    log::debug!("[LiveLcu] 读取对局会话失败: {}", e);
                    None
                }
            },
        };

        let Some(ref mut src) = source else {
            log::debug!("[LiveLcu] 当前阶段（{}）暂无可用对局会话", phase);
            return;
        };

        let assets = self
            .get_game_data_assets()
            .await
            .unwrap_or_else(|| Arc::new(GameDataAssets::default()));
        let summoner = match fetch_current_summoner(&client).await {
            Ok(s) => Some(s),
            Err(_) => match &*self.status.borrow() {
                ConnectionStatus::Connected { summoner, .. } => (**summoner).clone(),
                _ => None,
            },
        };
        let Some(summoner) = summoner else {
            log::debug!("[LiveLcu] 召唤师信息未就绪，跳过对局信息拉取");
            return;
        };

        let self_summoner_id = summoner.summoner_id.and_then(|id| i64::try_from(id).ok());

        // 对局阶段 2999 LiveClientData 兜底：补全缺失玩家与英雄 ID（原因 D），
        // 并复用同一次拉取结果做阵营判定，避免重复请求
        let matched_direct = src.blue.iter().chain(src.red.iter()).any(|p| {
            p.resolved_puuid() == Some(summoner.puuid.as_str())
                || (self_summoner_id.is_some() && p.resolved_summoner_id() == self_summoner_id)
        });
        let mut liveclient_snap = None;
        if src.phase != "ChampSelect" {
            let needs_completion = src.blue.is_empty()
                || src.red.is_empty()
                || src
                    .blue
                    .iter()
                    .chain(src.red.iter())
                    .any(|p| p.resolved_champion_id() <= 0);
            let needs_side = src.detected_side.is_none() && !matched_direct;
            if needs_completion || needs_side {
                liveclient_snap = fetch_liveclient_snapshot().await;
                if let Some(snap) = &liveclient_snap {
                    let (live_blue, live_red) = liveclient_teams(snap, &assets);
                    if merge_liveclient_teams(
                        &mut src.blue,
                        &mut src.red,
                        live_blue,
                        live_red,
                        &assets,
                    ) {
                        log::info!("[LiveLcu] 已通过 2999 LiveClientData 补全对局队伍与英雄数据");
                    }
                }
            }
        }

        // 对局阶段若 teamOne/teamTwo 均未能直接匹配本人（如脱敏排位或极简会话），尝试 2999 端口与历史阵营兜底
        if src.phase != "ChampSelect" && src.detected_side.is_none() && !matched_direct {
            let fallback_side = match liveclient_snap.as_ref().and_then(liveclient_map_side) {
                Some(side) => Some(side),
                None => self
                    .game_info
                    .read()
                    .await
                    .as_ref()
                    .and_then(|g| g.user_side),
            };
            if let Some(side) = fallback_side {
                src.detected_side = Some(side);
            }
        }

        let key = src.refresh_key();
        if self.game_info.read().await.is_some() && *self.last_game_info_key.read().await == key {
            return;
        }

        let source = source.unwrap();
        let recent = self.get_recent_matches().await;

        log::info!(
            "[LiveLcu] 开始拉取对局信息: phase={}, queue={}, 玩家={}",
            source.phase,
            source.queue_id,
            source.puuids().len()
        );
        let _ = self.event_tx.send(LiveLcuEvent::GameInfoLoading);

        let mut info = fetch_live_game_info(
            &client,
            &summoner.puuid,
            self_summoner_id,
            source,
            &recent,
            &assets,
        )
        .await;

        // 对局进行中精简防丢：若当前 session 只剩单边队伍，而同局先前已记录完整敌我数据，保留先前非空队伍
        if let Some(prev) = self.game_info.read().await.as_ref() {
            let prev_is_same_match = prev.phase != "ChampSelect"
                && info.game_id.is_some()
                && info.game_id == prev.game_id;
            if prev_is_same_match {
                if info.blue_team.is_empty() && !prev.blue_team.is_empty() {
                    log::info!("[LiveLcu] 继承先前蓝方队伍 ({} 人)", prev.blue_team.len());
                    info.blue_team = prev.blue_team.clone();
                }
                if info.red_team.is_empty() && !prev.red_team.is_empty() {
                    log::info!("[LiveLcu] 继承先前红方队伍 ({} 人)", prev.red_team.len());
                    info.red_team = prev.red_team.clone();
                }
            }
        }

        log::info!(
            "[LiveLcu] 对局信息就绪: {} vs {} (user_side={:?})",
            info.blue_team.len(),
            info.red_team.len(),
            info.user_side
        );

        let info = Arc::new(info);
        *self.last_game_info_key.write().await = key;
        *self.game_info.write().await = Some(info.clone());
        let _ = self.event_tx.send(LiveLcuEvent::GameInfoUpdated(info));
    }
}

/// 从选人 pin-drop-notification 读取实时红蓝阵营
async fn fetch_pin_drop_side(client: &yuumi_core::lcu::client::LcuClient) -> Option<MapSide> {
    let data = client
        .get("/lol-champ-select/v1/pin-drop-notification")
        .await
        .ok()?;
    let side = data.get("mapSide").and_then(|v| v.as_str())?;
    match side.trim().to_ascii_lowercase().as_str() {
        "blue" => Some(MapSide::Blue),
        "red" => Some(MapSide::Red),
        _ => None,
    }
}

/// 把统一入口条目转换为分解 / 升级请求条目
fn to_disenchant_items(items: &[LootActionItem]) -> Vec<loot_actions::DisenchantItem> {
    items
        .iter()
        .map(|i| loot_actions::DisenchantItem {
            loot_id: i.loot_id.clone(),
            count: i.count,
            upgrade_recipe_name: i.upgrade_recipe_name.clone(),
        })
        .collect()
}

/// [D04-e] 同步读取当前登录召唤师 puuid（只借用 watch Ref，不跨 await 持有）。
fn current_self_puuid(status: &watch::Sender<ConnectionStatus>) -> String {
    let guard = status.borrow();
    match &*guard {
        ConnectionStatus::Connected { summoner, .. } => match &**summoner {
            Some(s) => s.puuid.clone(),
            None => String::new(),
        },
        _ => String::new(),
    }
}

/// [D04-e] 从路人集存储只读构造 `puuid → 标签` 映射。
///
/// 对齐旧版 `query_tagged_for_reminder` 的 self_puuid 作用域（self_puuid 为空的记录视为
/// 全局）；C 工单已通过 `SavedPlayersStorage::export_tagged_players` 暴露该数据契约，
/// 本函数只消费、不修改 storage 模块。
fn load_tagged_players_map(self_puuid: &str) -> HashMap<String, String> {
    if self_puuid.is_empty() {
        return HashMap::new();
    }
    let Ok(storage) = SavedPlayersStorage::open_default() else {
        log::warn!("[LiveLcu] 标记提醒：路人集数据库不可用");
        return HashMap::new();
    };
    match storage.export_tagged_players() {
        Ok(records) => records
            .into_iter()
            .filter(|r| r.self_puuid.is_empty() || r.self_puuid.as_str() == self_puuid)
            .filter_map(|r| {
                let tag = r.tag.unwrap_or_default();
                (!tag.is_empty()).then_some((r.puuid, tag))
            })
            .collect(),
        Err(e) => {
            log::warn!("[LiveLcu] 标记提醒：读取标记玩家失败: {}", e);
            HashMap::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_config_sync_startup_runtime_and_reconnect() {
        let asset_cache = Arc::new(AssetCache::new(None));
        let service = LiveLcuService::new(asset_cache);

        // 1. 启动加载自定义项（测试代表：自动接受匹配关闭、延迟 3 秒；自动禁用开启、候选 [1, 2, 3]、延迟 5 秒）
        let mut match_cfg = AutoMatchConfig {
            auto_accept: false,
            auto_accept_delay: 3,
            ..AutoMatchConfig::default()
        };

        let mut bp_cfg = AutoBpConfig {
            auto_ban: true,
            ban_champion_ids: vec![1, 2, 3],
            ban_delay_ms: 5000,
            ..AutoBpConfig::default()
        };

        service.update_auto_match_cfg(match_cfg.clone());
        service.update_auto_bp_cfg(bp_cfg.clone());

        assert!(!service.auto_match_cfg.borrow().auto_accept);
        assert_eq!(service.auto_match_cfg.borrow().auto_accept_delay, 3);
        assert!(service.auto_bp_cfg.borrow().auto_ban);
        assert_eq!(service.auto_bp_cfg.borrow().ban_champion_ids, vec![1, 2, 3]);
        assert_eq!(service.auto_bp_cfg.borrow().ban_delay_ms, 5000);

        // 2. 运行中开启再关闭
        match_cfg.auto_accept = true;
        match_cfg.auto_accept_delay = 0;
        service.update_auto_match_cfg(match_cfg.clone());
        assert!(service.auto_match_cfg.borrow().auto_accept);
        assert_eq!(service.auto_match_cfg.borrow().auto_accept_delay, 0);

        match_cfg.auto_accept = false;
        service.update_auto_match_cfg(match_cfg.clone());
        assert!(!service.auto_match_cfg.borrow().auto_accept);

        bp_cfg.auto_ban = false;
        bp_cfg.ban_champion_ids.clear();
        service.update_auto_bp_cfg(bp_cfg.clone());
        assert!(!service.auto_bp_cfg.borrow().auto_ban);
        assert!(service.auto_bp_cfg.borrow().ban_champion_ids.is_empty());

        // 3. 模拟 LCU 重连：重新引用的 channel 读取到的仍是最新保留的配置
        let reconnected_match_cfg = service.auto_match_cfg.borrow().clone();
        let reconnected_bp_cfg = service.auto_bp_cfg.borrow().clone();
        assert!(!reconnected_match_cfg.auto_accept);
        assert!(!reconnected_bp_cfg.auto_ban);
    }
}
