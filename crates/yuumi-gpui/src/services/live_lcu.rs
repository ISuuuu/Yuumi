//! 真实 LCU 实时连接与数据编排管理服务

use std::sync::Arc;
use tokio::sync::{broadcast, watch};
use tokio::time::{sleep, Duration};
use yuumi_core::agents::{
    auto_bp::{handle_champ_select_session, ChampSelectSession},
    auto_match::{handle_phase_change, handle_ready_check, ReadyCheckData},
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
        }
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<LiveLcuEvent> {
        self.event_tx.subscribe()
    }

    pub fn subscribe_status(&self) -> watch::Receiver<ConnectionStatus> {
        self.status.subscribe()
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
                    log::info!("[LiveLcu] 检测到本地 LCU 进程: pid={}, port={}", info.pid, info.port);
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
                            let assets = fetch_game_data_assets(&client).await;
                            let arc_assets = Arc::new(assets);
                            game_data_assets = Some(arc_assets.clone());
                            let _ = self.event_tx.send(LiveLcuEvent::GameDataReady(arc_assets));
                        }

                        // 获取当前召唤师
                        let summoner = fetch_current_summoner(&client).await.ok();
                        let _ = self.status.send(ConnectionStatus::Connected {
                            info: info.clone(),
                            summoner: Box::new(summoner.clone()),
                        });
                        let _ = self.event_tx.send(LiveLcuEvent::ConnectionChanged(ConnectionStatus::Connected {
                            info: info.clone(),
                            summoner: Box::new(summoner.clone()),
                        }));

                        // 若已获取召唤师，拉取最近战绩
                        if let (Some(s), Some(assets)) = (&summoner, &game_data_assets) {
                            if let Ok(matches) = fetch_match_history(&client, &s.puuid, Some(0), Some(19), assets).await {
                                let _ = self.event_tx.send(LiveLcuEvent::MatchHistoryLoaded(matches));
                            }
                        }

                        // 建立 WebSocket 连接
                        let ws_handle = start_ws(info.port, info.token.clone());
                        let mut ws_rx = ws_handle.subscribe_events();
                        let this_clone = self.clone();
                        let client_clone = client.clone();

                        tokio::spawn(async move {
                            while let Ok(event) = ws_rx.recv().await {
                                this_clone.handle_ws_event(&client_clone, event).await;
                            }
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
                    self.supervisor.cancel_all().await;
                    let _ = self.lifecycle.on_phase_changed("None").await;
                    let _ = self.status.send(ConnectionStatus::Disconnected);
                    let _ = self.event_tx.send(LiveLcuEvent::ConnectionChanged(ConnectionStatus::Disconnected));
                }
                _ => {}
            }

            sleep(Duration::from_secs(2)).await;
        }
    }

    async fn handle_ws_event(&self, client: &yuumi_core::lcu::client::LcuClient, event: LcuWsEvent) {
        match event.uri.as_str() {
            "/lol-gameflow/v1/gameflow-phase" => {
                if let Some(phase_str) = event.data.as_str() {
                    let phase = GamePhase::from(phase_str);
                    let cfg = self.auto_match_cfg.borrow().clone();
                    handle_phase_change(client, phase_str, &cfg, &self.supervisor, &self.lifecycle).await;
                    let _ = self.event_tx.send(LiveLcuEvent::GamePhaseChanged(phase));
                }
            }
            "/lol-matchmaking/v1/ready-check" => {
                if let Ok(ready) = serde_json::from_value::<ReadyCheckData>(event.data) {
                    let cfg = self.auto_match_cfg.borrow().clone();
                    handle_ready_check(client, &ready, &cfg, &self.supervisor, &self.lifecycle).await;
                }
            }
            "/lol-champ-select/v1/session" => {
                if let Ok(session) = serde_json::from_value::<ChampSelectSession>(event.data) {
                    let cfg = self.auto_bp_cfg.borrow().clone();
                    handle_champ_select_session(client, &session, &cfg, &self.supervisor, &self.lifecycle).await;
                    let _ = self.event_tx.send(LiveLcuEvent::ChampSelectUpdated(session));
                }
            }
            _ => {}
        }
    }
}
