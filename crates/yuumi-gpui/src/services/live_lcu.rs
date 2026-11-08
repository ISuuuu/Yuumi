//! 真实 LCU 实时连接与数据编排管理服务

use std::sync::Arc;
use tokio::sync::{broadcast, watch};
use tokio::time::{sleep, Duration};
use yuumi_core::agents::{
    auto_bp::{request_bench_swap, request_reroll, ChampSelectSession},
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
use yuumi_core::parsers::match_parser::{fetch_match_history, MatchDisplay};
use yuumi_core::parsers::summoner::{fetch_current_summoner, SummonerInfo};

use crate::services::agent_runtime::{AgentRuntime, BpAgentEvent, MatchAgentEvent};

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

                        // 若已获取召唤师，拉取最近战绩
                        if let (Some(s), Some(assets)) = (&summoner, &game_data_assets) {
                            if let Ok(matches) =
                                fetch_match_history(&client, &s.puuid, Some(0), Some(19), assets)
                                    .await
                            {
                                let _ = self
                                    .event_tx
                                    .send(LiveLcuEvent::MatchHistoryLoaded(matches));
                            }
                        }

                        // 启动 Agent 后台 Worker（与本次连接同生命周期）
                        let runtime = AgentRuntime::start(
                            client.clone(),
                            self.supervisor.clone(),
                            self.lifecycle.clone(),
                            self.auto_match_cfg.clone(),
                            self.auto_bp_cfg.clone(),
                        );
                        *self.agent_runtime.write().await = Some(runtime);

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
                if let Some(phase_str) = event.data.as_str() {
                    if let Some(d) = &dispatcher {
                        d.dispatch_match(MatchAgentEvent::PhaseChanged(phase_str.to_string()));
                    }
                    let phase = GamePhase::from(phase_str);
                    let _ = self.event_tx.send(LiveLcuEvent::GamePhaseChanged(phase));
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
            }
            _ => {}
        }
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
}
