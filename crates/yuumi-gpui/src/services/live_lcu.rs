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
use yuumi_core::parsers::loot::{EssenceBalances, LootInventoryItem};
use yuumi_core::parsers::mastery::ChampionMasteryItem;
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
    ChampionMasteryLoaded(Vec<ChampionMasteryItem>),
    LootSummaryLoaded(EssenceBalances, Vec<LootInventoryItem>),
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

                        // 异步并发拉取召唤师战绩、英雄熟练度与战利品
                        let this_career = self.clone();
                        tokio::spawn(async move {
                            this_career.refresh_career_data().await;
                        });

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
                    self.asset_cache.set_lcu_client(None);
                    *self.client.write().await = None;
                    self.supervisor.cancel_all().await;
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

    async fn handle_ws_event(
        &self,
        client: &yuumi_core::lcu::client::LcuClient,
        event: LcuWsEvent,
    ) {
        match event.uri.as_str() {
            "/lol-gameflow/v1/gameflow-phase" => {
                if let Some(phase_str) = event.data.as_str() {
                    let phase = GamePhase::from(phase_str);
                    let cfg = self.auto_match_cfg.borrow().clone();
                    handle_phase_change(client, phase_str, &cfg, &self.supervisor, &self.lifecycle)
                        .await;
                    let _ = self.event_tx.send(LiveLcuEvent::GamePhaseChanged(phase));
                }
            }
            "/lol-matchmaking/v1/ready-check" => {
                if let Ok(ready) = serde_json::from_value::<ReadyCheckData>(event.data) {
                    let cfg = self.auto_match_cfg.borrow().clone();
                    handle_ready_check(client, &ready, &cfg, &self.supervisor, &self.lifecycle)
                        .await;
                }
            }
            "/lol-champ-select/v1/session" => {
                if let Ok(session) = serde_json::from_value::<ChampSelectSession>(event.data) {
                    let cfg = self.auto_bp_cfg.borrow().clone();
                    handle_champ_select_session(
                        client,
                        &session,
                        &cfg,
                        &self.supervisor,
                        &self.lifecycle,
                    )
                    .await;
                    let _ = self
                        .event_tx
                        .send(LiveLcuEvent::ChampSelectUpdated(session));
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

        // 并发拉取战绩 (近 20 场)
        let c_matches = client.clone();
        let puuid_clone = summoner.puuid.clone();
        let assets_clone = assets.clone();
        let event_tx_matches = self.event_tx.clone();
        let matches_task = tokio::spawn(async move {
            match fetch_match_history(&c_matches, &puuid_clone, Some(0), Some(20), &assets_clone).await {
                Ok(matches) => {
                    log::info!("[LiveLcu] 成功拉取当前召唤师战绩: count={}", matches.len());
                    let _ = event_tx_matches.send(LiveLcuEvent::MatchHistoryLoaded(matches));
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

        let _ = tokio::join!(matches_task, mastery_task, loot_task);
        log::info!("[LiveLcu] 召唤师生涯数据全部拉取完毕并派发完成");
    }
}
