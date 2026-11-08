//! 真实 LCU 实时连接与数据编排管理服务

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
use yuumi_core::lcu::{
    asset_cache::AssetCache,
    game_data::{fetch_game_data_assets, GameDataAssets},
    monitor::{detect_lcu, LcuConnectionInfo},
    ws::{start_ws, LcuWsEvent, LcuWsHandle},
};
use yuumi_core::parsers::game_info::{
    fetch_live_game_info, fetch_ranked_stats, ChampSelectSnapshot, GameInfoSource,
    GameflowSessionSnapshot, LiveGameInfo, RankedStats,
};
use yuumi_core::parsers::loot::{EssenceBalances, LootInventoryItem};
use yuumi_core::parsers::mastery::ChampionMasteryItem;
use yuumi_core::parsers::match_parser::{fetch_match_history, MatchDisplay};
use yuumi_core::parsers::summoner::{fetch_current_summoner, SummonerInfo};

use crate::services::agent_runtime::{AgentRuntime, BpAgentEvent, MatchAgentEvent};

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
    LootSummaryLoaded(EssenceBalances, Vec<LootInventoryItem>),
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
    pub supervisor: Arc<AgentSupervisor>,
    pub lifecycle: Arc<LifecycleManager>,
    pub asset_cache: Arc<AssetCache>,
    pub auto_match_cfg: Arc<watch::Sender<AutoMatchConfig>>,
    pub auto_bp_cfg: Arc<watch::Sender<AutoBpConfig>>,
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
        let (match_tx, _) = watch::channel(AutoMatchConfig::default());
        let (bp_tx, _) = watch::channel(AutoBpConfig::default());

        Self {
            status: Arc::new(status_tx),
            event_tx,
            supervisor: Arc::new(AgentSupervisor::new()),
            lifecycle: Arc::new(LifecycleManager::new()),
            asset_cache,
            auto_match_cfg: Arc::new(match_tx),
            auto_bp_cfg: Arc::new(bp_tx),
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

    pub fn subscribe_status(&self) -> watch::Receiver<ConnectionStatus> {
        self.status.subscribe()
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
                        *self.agent_runtime.write().await = Some(runtime);

                        // 重连后若正处于对局，补拉当前 10 人对局信息
                        let phase = self.get_current_phase().await;
                        self.schedule_game_info_refresh(phase, Duration::ZERO).await;

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
                    let _ = self
                        .event_tx
                        .send(LiveLcuEvent::ChampSelectUpdated(session));
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
            _ => {}
        }
    }

    /// 刷新当前登录召唤师的生涯数据（最近 20 场战绩、英雄熟练度、战利品与精粹）
    pub async fn refresh_career_data(&self) {
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
            "[LiveLcu] 开始并发拉取召唤师 [{}] 的生涯对局历史、熟练度与战利品...",
            summoner.full_name()
        );

        // 并发拉取战绩 (近 20 场，endIndex 为闭区间故取 19)
        let c_matches = client.clone();
        let puuid_clone = summoner.puuid.clone();
        let assets_clone = assets.clone();
        let this_matches = self.clone();
        let matches_task = tokio::spawn(async move {
            match fetch_match_history(&c_matches, &puuid_clone, Some(0), Some(19), &assets_clone)
                .await
            {
                Ok(mut matches) => {
                    matches.truncate(CAREER_MATCH_LIMIT);
                    log::info!("[LiveLcu] 成功拉取当前召唤师战绩: count={}", matches.len());
                    // 同步缓存本人近期战绩，供宿命对局判定复用
                    *this_matches.recent_matches.write().await = matches.clone();
                    let _ = this_matches
                        .event_tx
                        .send(LiveLcuEvent::MatchHistoryLoaded(matches));
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

        let source = match phase {
            "ChampSelect" => match client.get("/lol-champ-select/v1/session").await {
                Ok(value) => serde_json::from_value::<ChampSelectSnapshot>(value)
                    .ok()
                    .and_then(|snapshot| GameInfoSource::from_champ_select(&snapshot)),
                Err(e) => {
                    log::debug!("[LiveLcu] 读取选人会话失败: {}", e);
                    None
                }
            },
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

        let Some(source) = source else {
            log::debug!("[LiveLcu] 当前阶段（{}）暂无可用对局会话", phase);
            return;
        };

        let key = source.refresh_key();
        if self.game_info.read().await.is_some() && *self.last_game_info_key.read().await == key {
            return;
        }

        let assets = self.get_game_data_assets().await;
        let summoner = fetch_current_summoner(&client).await.ok();
        let (Some(assets), Some(summoner)) = (assets, summoner) else {
            log::debug!("[LiveLcu] 召唤师或资源字典未就绪，跳过对局信息拉取");
            return;
        };

        let self_summoner_id = summoner.summoner_id.and_then(|id| i64::try_from(id).ok());
        let recent = self.get_recent_matches().await;

        log::info!(
            "[LiveLcu] 开始拉取对局信息: phase={}, queue={}, 玩家={}",
            source.phase,
            source.queue_id,
            source.puuids().len()
        );
        let _ = self.event_tx.send(LiveLcuEvent::GameInfoLoading);

        let info = fetch_live_game_info(
            &client,
            &summoner.puuid,
            self_summoner_id,
            source,
            &recent,
            &assets,
        )
        .await;

        log::info!(
            "[LiveLcu] 对局信息就绪: {} vs {}",
            info.blue_team.len(),
            info.red_team.len()
        );

        let info = Arc::new(info);
        *self.last_game_info_key.write().await = key;
        *self.game_info.write().await = Some(info.clone());
        let _ = self.event_tx.send(LiveLcuEvent::GameInfoUpdated(info));
    }
}
