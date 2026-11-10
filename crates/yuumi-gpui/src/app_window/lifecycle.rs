// 主窗生命周期接线：LCU 领域事件分派、配置同步、独立窗口（板凳席 / OP.GG）生命周期。
use super::*;

impl MainWindow {
    pub(super) fn handle_live_event(&mut self, event: LiveLcuEvent, cx: &mut Context<Self>) {
        match event {
            LiveLcuEvent::ConnectionChanged(status) => {
                let was_connected = matches!(self.connection_status, ConnectionStatus::Connected { .. });
                if let ConnectionStatus::Connected { summoner, .. } = &status {
                    if let Some(s) = summoner.as_ref() {
                        let is_new_account = self
                            .own_summoner
                            .as_ref()
                            .map(|o| o.puuid != s.puuid)
                            .unwrap_or(false);
                        if is_new_account {
                            // 换号清理上一账号的对局信息与路人集状态
                            self.game_info = None;
                            self.saved_players_view.players.clear();
                            self.saved_players_view.total_count = 0;
                            self.saved_players_view.expanded_puuid = None;
                            self.saved_players_view.expanded_games.clear();
                        }
                        self.own_summoner = Some(s.clone());
                        self.cloud_tasks.upload_matches(s.puuid.clone(), Vec::new(), false);
                        if !self.career_view.is_viewing_other {
                            self.career_view.summoner = Some(s.clone());
                        }
                        self.saved_players_view
                            .set_self_puuid(Some(s.puuid.clone()));
                        // 资料卡头像也要预热，否则 LcuImage 只显示占位文字
                        prewarm_assets(vec![s.icon_path()], cx);
                        // 连接新账号后触发路人身份异步补齐
                        self.backfill_saved_players_identity(cx);
                    }
                    // 首次连接且当前停留在首页时，自动跳转到生涯页（对照 Vue 行为）
                    if !was_connected && self.current_route == Route::Home {
                        self.switch_route(Route::Career, cx);
                    }
                }
                if matches!(status, ConnectionStatus::Disconnected) {
                    // 客户端断开：立即收起悬浮窗，避免残留空窗
                    self.bench_session = None;
                    self.cloud_tasks.set_in_game(false);
                    self.cloud_game_id = None;
                    self.cloud_match_played = false;
                    self.game_info_loading = false;
                    self.close_bench_overlay(cx);
                    // 客户端断开连接时自动返回首页（对照 Vue 行为）
                    if was_connected {
                        self.switch_route(Route::Home, cx);
                    }
                    if !self.app_config.functions.enable_reserve_gameinfo {
                        self.game_info = None;
                    }
                }
                self.connection_status = status;
                if self.current_route == Route::Tft {
                    self.ensure_tft_data(cx);
                }
                cx.notify();
            }
            LiveLcuEvent::MatchHistoryLoaded(matches) => {
                log::info!("[MainWindow] 接收到真实战绩数据: count={}", matches.len());
                let items: Vec<MatchItem> = matches.into_iter().map(MatchItem::new).collect();
                self.stats = CareerHeaderStats::compute(&items);

                // B06 规范：隔离本人刷新与他人搜索，若当前正在查询他人，绝不替换搜索列表
                let viewing_other_in_search = match (
                    &self.search_view.current_summoner_puuid,
                    &self.own_summoner,
                ) {
                    (Some(curr_puuid), Some(own)) => curr_puuid != &own.puuid,
                    _ => false,
                };

                if !viewing_other_in_search {
                    self.search_view.matches = items.clone();

                    // 未搜索场景下（进入战绩查询页自动带出的战绩）也要选中并载入首局详情，
                    // 否则右栏会一直停在「等待详情载入」的空态
                    let keep_selected = self
                        .search_view
                        .selected_game_id
                        .filter(|id| items.iter().any(|m| m.display.game_id == *id));
                    if keep_selected.is_none() {
                        if let Some(first_id) = items.first().map(|m| m.display.game_id) {
                            self.load_match_detail(first_id, cx);
                        }
                    }
                }

                // 异步预热战绩资源：路径必须与渲染同源
                // （装备格渲染用 item_icon_urls，旧逻辑按 item_id 拼 /items/{id}.png 导致缓存永远不命中）
                let mut asset_paths = Vec::new();
                for m in &items {
                    let d = &m.display;
                    for url in [
                        &d.champion_icon_url,
                        &d.spell1_icon_url,
                        &d.spell2_icon_url,
                        &d.rune_icon_url,
                    ] {
                        if !url.is_empty() {
                            asset_paths.push(url.clone());
                        }
                    }
                    asset_paths.extend(d.item_icon_urls.iter().filter(|u| !u.is_empty()).cloned());
                    asset_paths.extend(
                        d.augment_icon_urls
                            .iter()
                            .filter(|u| !u.is_empty())
                            .cloned(),
                    );
                }
                // 20 场战绩里装备/英雄高度重复，去重以免打爆 LCU 请求
                asset_paths.sort();
                asset_paths.dedup();
                prewarm_assets(asset_paths, cx);

                if !self.career_view.is_viewing_other {
                    self.own_matches = items.clone();
                    self.own_stats = self.stats.clone();
                    self.matches = items;
                }
                cx.notify();
            }
            LiveLcuEvent::RankedStatsLoaded(stats) => {
                log::info!(
                    "[MainWindow] 接收到真实排位数据: 队列数={}",
                    stats.queue_map.len()
                );
                if !self.career_view.is_viewing_other {
                    self.own_ranked = Some(stats.clone());
                    self.career_view.ranked = Some(stats);
                }
                cx.notify();
            }
            LiveLcuEvent::ChampionMasteryLoaded(mut masteries) => {
                log::info!(
                    "[MainWindow] 接收到真实熟练度数据: count={}",
                    masteries.len()
                );
                // 解析层不依赖资产字典，英雄名在此用已预加载的资源补全
                if let Some(assets) = self.game_assets.as_ref() {
                    for item in masteries.iter_mut() {
                        if let Some(name) = assets.champions.get(&(item.champion_id as i32)) {
                            item.champion_name = name.clone();
                        }
                    }
                }
                let asset_paths: Vec<String> = masteries
                    .iter()
                    .map(|m| m.champion_icon_url.clone())
                    .filter(|u| !u.is_empty())
                    .collect();
                prewarm_assets(asset_paths, cx);

                self.career_view.mastery_list = masteries;
                cx.notify();
            }
            LiveLcuEvent::LootSummaryLoaded(balances, items) => {
                log::info!(
                    "[MainWindow] 接收到真实战利品数据: 蓝精粹={}, 橙精粹={}, 物品数={}",
                    balances.blue_essence,
                    balances.orange_essence,
                    items.len()
                );
                self.career_view.essences = balances;
                self.career_view.loot_items = items;
                cx.notify();
            }
            LiveLcuEvent::GameDataReady(assets) => {
                self.game_assets = Some(assets);
                cx.notify();
            }
            LiveLcuEvent::GamePhaseChanged(phase) => {
                log::info!("[MainWindow] 接收到游戏阶段更新: {:?}", phase);
                // 与 Vue 的 watch 语义对齐：只在“进入”选人阶段时自动开窗。
                // LCU 会重复推送同一阶段，若按每次通知开窗，用户手动关闭后会被再次拉起。
                let entering_champ_select =
                    phase == GamePhase::ChampSelect && self.current_phase != "ChampSelect";
                self.current_phase = phase.as_str().to_string();
                let in_game = matches!(
                    phase,
                    GamePhase::GameStart | GamePhase::InProgress | GamePhase::Reconnect
                );
                self.cloud_tasks.set_in_game(in_game);
                if in_game {
                    self.cloud_match_played = true;
                } else if matches!(phase.as_str(), "ChampSelect" | "ReadyCheck") {
                    self.cloud_match_played = false;
                    self.cloud_game_id = None;
                } else if matches!(phase.as_str(), "EndOfGame" | "Lobby" | "None")
                    && std::mem::take(&mut self.cloud_match_played)
                {
                    let id = self.cloud_game_id.take();
                    if let Some(player) = self.own_summoner.as_ref() {
                        self.cloud_tasks.upload_finished(player.puuid.clone(), id);
                    }
                }
                if !matches!(
                    self.current_phase.as_str(),
                    "ChampSelect" | "GameStart" | "InProgress" | "Reconnect"
                ) {
                    self.game_info_loading = false;
                }
                // 进入选人阶段按配置自动弹出 OP.GG（已有窗口则聚焦），并重置对局看板
                if phase == GamePhase::ChampSelect {
                    self.game_info = None;
                    self.last_recorded_encounter_game_id = None;
                    // 只在「进入」选人阶段时开窗：LCU 会重复推送同一阶段，
                    // 否则用户手动关闭的窗口会被下一次重复通知再次拉起。
                    if entering_champ_select && self.app_config.functions.auto_show_opgg {
                        self.open_opgg_window(cx);
                    }
                }
                if phase != GamePhase::ChampSelect {
                    // 选人结束（或进入其它阶段）即收起板凳席悬浮窗
                    self.bench_session = None;
                    self.close_bench_overlay(cx);
                }

                // 开局最小化：游戏开始或进入游戏中时自动最小化窗口
                if matches!(phase, GamePhase::GameStart | GamePhase::InProgress)
                    && self.app_config.general.enable_game_start_minimize
                {
                    log::info!("[MainWindow] 检测到开局阶段 {:?} 且开启开局最小化，执行最小化窗口", phase);
                    if let Some(hwnd) = platform::find_main_window_hwnd() {
                        platform::minimize_window(hwnd);
                    }
                }

                // 离开对局阶段（返回大厅/未在对局）：根据配置项决定是否保留上一局看板
                if matches!(
                    phase,
                    GamePhase::None | GamePhase::Lobby | GamePhase::Matchmaking
                ) && !self.app_config.functions.enable_reserve_gameinfo {
                    self.game_info = None;
                }

                // 对局结束（结算阶段）：若存在 10 人快照，自动持久化记录同局相遇玩家
                if matches!(
                    phase,
                    GamePhase::PreEndOfGame | GamePhase::EndOfGame | GamePhase::WaitingForStats
                ) {
                    if let Some(ref info) = self.game_info {
                        // 演示快照与真实结算写入严格隔离！
                        if !info.is_demo {
                            if let Some(ref s) = self.saved_players_view.storage {
                                let self_puuid = self
                                    .career_view
                                    .summoner
                                    .as_ref()
                                    .map(|sum| sum.puuid.as_str())
                                    .unwrap_or("");
                                let game_id = info.game_id.unwrap_or(0);
                                let queue_type = info.queue_id.to_string();

                                // 重复结算事件按 gameId 拦截去重
                                let is_already_recorded = self
                                    .last_recorded_encounter_game_id
                                    .map(|id| id == game_id)
                                    .unwrap_or(false);

                                if !is_already_recorded && !self_puuid.is_empty() && game_id > 0 {
                                    let players_to_record: Vec<yuumi_core::storage::GamePlayerRecord> =
                                        info.players()
                                            .filter(|p| !p.is_local_player && !p.puuid.is_empty())
                                            .map(|p| yuumi_core::storage::GamePlayerRecord {
                                                puuid: p.puuid.clone(),
                                                summoner_name: p.summoner_name.clone(),
                                                tag_line: if p.tag_line.is_empty() {
                                                    None
                                                } else {
                                                    Some(p.tag_line.clone())
                                                },
                                                profile_icon_id: 0,
                                                champion_id: p.champion_id,
                                            })
                                            .collect();

                                    if !players_to_record.is_empty() {
                                        match s.record_encounters(
                                            &players_to_record,
                                            self_puuid,
                                            game_id,
                                            &queue_type,
                                        ) {
                                            Ok(rec) => {
                                                log::info!(
                                                    "[MainWindow] 对局结束自动记录相遇玩家: 共 {} 人",
                                                    rec
                                                );
                                                self.last_recorded_encounter_game_id = Some(game_id);
                                                self.saved_players_view.reload(Some(self_puuid));
                                            }
                                            Err(e) => {
                                                log::warn!("[MainWindow] 自动记录相遇玩家失败: {}", e);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                cx.notify();
            }
            LiveLcuEvent::GameInfoLoading => {
                self.game_info_loading = true;
                cx.notify();
            }
            LiveLcuEvent::GameInfoUpdated(info) => {
                log::info!(
                    "[MainWindow] 对局信息更新: {} 蓝{}人 vs 红{}人",
                    info.phase_label,
                    info.blue_team.len(),
                    info.red_team.len()
                );
                // 预热 10 人英雄头像，避免看板首帧只有占位块
                let asset_paths: Vec<String> = info
                    .players()
                    .filter(|p| p.champion_id > 0)
                    .map(|p| {
                        format!(
                            "/lol-game-data/assets/v1/champion-icons/{}.png",
                            p.champion_id
                        )
                    })
                    .collect();
                prewarm_assets(asset_paths, cx);

                if !info.is_demo
                    && self.cloud_match_played
                    && matches!(self.current_phase.as_str(), "GameStart" | "InProgress" | "Reconnect" | "PreEndOfGame")
                {
                    self.cloud_game_id = info.game_id.filter(|id| *id > 0).or(self.cloud_game_id);
                }
                self.game_info = Some(info);
                self.game_info_loading = false;
                cx.notify();
            }
            LiveLcuEvent::ChampSelectUpdated(session) => {
                if session.is_aram() {
                    self.bench_session = Some(session);
                } else {
                    self.bench_session = None;
                }
                self.sync_bench_overlay(cx);
                cx.notify();
            }
            LiveLcuEvent::LootActionProgress(progress) => {
                let mut state = self
                    .career_view
                    .loot_progress
                    .take()
                    .unwrap_or(LootProgress {
                        current: 0,
                        total: 0,
                        success: 0,
                        current_name: String::new(),
                    });
                state.current = progress.current.max(0) as usize;
                state.total = progress.total.max(0) as usize;
                state.current_name = progress.loot_name;
                if progress.success {
                    state.success += 1;
                }
                self.career_view.loot_progress = Some(state);
                cx.notify();
            }
            LiveLcuEvent::RecentTeammatesLoaded(teammates) => {
                let icon_paths: Vec<String> = teammates.iter().map(|t| t.icon.clone()).collect();
                prewarm_assets(icon_paths, cx);
                self.career_view.recent_teammates = teammates;
                cx.notify();
            }
        }
    }

    /// 依据开关状态与当前选人会话同步大乱斗板凳席悬浮窗
    pub(super) fn sync_bench_overlay(&mut self, cx: &mut Context<Self>) {
        let should_show = self.automation.aram_bench_overlay
            && self
                .bench_session
                .as_ref()
                .map(|session| session.is_aram())
                .unwrap_or(false);

        if !should_show {
            self.close_bench_overlay(cx);
            return;
        }

        let Some(session) = self.bench_session.clone() else {
            return;
        };

        // 已开窗：仅推送最新数据
        if let Some(handle) = &self.overlay_window {
            let assets = self.game_assets.clone();
            let result = handle.update(cx, |view, _window, cx| {
                view.apply_session(&session, assets.as_deref(), cx);
            });
            if result.is_err() {
                log::warn!("[BenchOverlay] 悬浮窗已失效，重新创建");
                self.overlay_window = None;
            } else {
                return;
            }
        }

        self.open_bench_overlay(cx);
    }

    /// 创建置顶工具窗口并灌入当前板凳席数据
    pub(super) fn open_bench_overlay(&mut self, cx: &mut Context<Self>) {
        let service = self.live_service.clone();
        let session = self.bench_session.clone();
        let assets = self.game_assets.clone();

        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                // 恢复上次拖拽后的位置，首次运行落在默认坐标
                origin: load_saved_origin().unwrap_or(Point {
                    x: px(420.0),
                    y: px(140.0),
                }),
                size: Size {
                    width: px(550.0),
                    height: px(70.0),
                },
            })),
            // 无系统装饰，仅保留标题供 Win32 定位到 HWND（置顶 / 工具窗样式）
            titlebar: Some(TitlebarOptions {
                title: Some(BENCH_WINDOW_TITLE.into()),
                appears_transparent: true,
                traffic_light_position: None,
            }),
            kind: WindowKind::Normal,
            focus: false,
            is_movable: true,
            is_resizable: false,
            ..Default::default()
        };

        let new_handle = cx.open_window(window_options, move |_window, cx| {
            cx.new(move |cx| {
                let mut view = BenchOverlayView::new(service);
                if let Some(session) = &session {
                    view.apply_session(session, assets.as_deref(), cx);
                }
                view
            })
        });

        if let Ok(handle) = new_handle {
            self.overlay_window = Some(handle);
            // 异步将板凳席小窗设置为 Windows 原生置顶与工具窗口（隐藏任务栏图标）
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(100));
                if let Some(hwnd) = platform::find_window_by_title(BENCH_WINDOW_TITLE) {
                    platform::set_window_topmost(hwnd, true);
                    platform::set_window_toolwindow(hwnd);
                } else {
                    log::warn!("[BenchOverlay] 未找到悬浮窗 HWND，置顶样式未应用");
                }
            });
        }
    }

    /// 关闭并销毁悬浮窗
    pub(super) fn close_bench_overlay(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.overlay_window.take() {
            let _ = handle.update(cx, |view, window, cx| {
                view.clear(cx);
                window.remove_window();
            });
        }
    }

    /// 打开 OP.GG 独立窗口；未连接 LCU 时给出提示并返回。
    ///
    /// 已有窗口时直接聚焦复用，句柄失效则重建（对照 Vue 的 `getByLabel("opgg")`）。
    pub(super) fn open_opgg_window(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.connection_status, ConnectionStatus::Connected { .. }) {
            self.show_opgg_hint("未连接到英雄联盟客户端，无法打开 OP.GG", cx);
            return;
        }

        if let Some(handle) = &self.opgg_window {
            let alive = handle
                .update(cx, |_view, window, _cx| window.activate_window())
                .is_ok();
            if alive {
                return;
            }
            log::warn!("[OPGG] 窗口句柄已失效，重新创建");
            self.opgg_window = None;
        }

        let service = self.live_service.clone();
        let proxy = yuumi_core::opgg::ProxyConfig {
            enable: self.app_config.general.enable_http_proxy,
            addr: self.app_config.general.http_proxy_addr.clone(),
        };
        let on_top = self.app_config.functions.enable_opgg_on_top;
        let initial_champion = self.initial_opgg_champion();
        let window_options = WindowOptions {
            window_bounds: Some(opgg_window_bounds(cx, self.main_window_bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some(OPGG_WINDOW_TITLE.into()),
                appears_transparent: false,
                traffic_light_position: None,
            }),
            kind: WindowKind::Normal,
            focus: true,
            is_movable: true,
            is_resizable: true,
            ..Default::default()
        };

        match cx.open_window(window_options, move |_window, cx| {
            cx.new(move |cx| OpggWindow::new(service, proxy, initial_champion, cx))
        }) {
            Ok(handle) => {
                self.opgg_window = Some(handle);
                if on_top {
                    // 置顶需等原生窗口就绪后再套用 Win32 样式
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(120));
                        match platform::find_window_by_title(OPGG_WINDOW_TITLE) {
                            Some(hwnd) => platform::set_window_topmost(hwnd, true),
                            None => log::warn!("[OPGG] 未找到窗口 HWND，置顶未应用"),
                        }
                    });
                }
            }
            Err(err) => log::error!("[OPGG] 创建窗口失败: {}", err),
        }
    }

    /// 关闭并销毁 OP.GG 独立窗口
    /// 关闭并销毁 OP.GG 独立窗口（基线已有的动作，当前入口由窗口自身关闭）
    #[allow(dead_code)]
    pub(super) fn close_opgg_window(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.opgg_window.take() {
            let _ = handle.update(cx, |_view, window, _cx| window.remove_window());
        }
    }

    /// 首屏进详情用的英雄：按自动选人配置依次检查通用与五个分路
    pub(super) fn initial_opgg_champion(&self) -> Option<i32> {
        let functions = &self.app_config.functions;
        if !functions.enable_auto_select_champion {
            return None;
        }
        let candidates: [&Vec<i32>; 6] = [
            &functions.auto_select_champion,
            &functions.auto_select_champion_mid,
            &functions.auto_select_champion_top,
            &functions.auto_select_champion_sup,
            &functions.auto_select_champion_jug,
            &functions.auto_select_champion_bot,
        ];
        candidates
            .iter()
            .find_map(|list| list.first().copied().filter(|id| *id > 0))
    }

    /// 显示 OP.GG 入口提示（3 秒后自动消失）
    pub(super) fn show_opgg_hint(&mut self, message: &str, cx: &mut Context<Self>) {
        self.opgg_hint = Some(message.to_string());
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(3)).await;
            let _ = this.update(cx, |this, cx| {
                this.opgg_hint = None;
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn trigger_reconnect(&mut self, cx: &mut Context<Self>) {
        log::info!("[MainWindow] 用户触发重新连接/修复流程");
        let live = self.live_service.clone();
        let phase = self.current_phase.clone();
        yuumi_core::runtime::spawn_background(async move {
            if let Some(client) = live.get_client().await {
                if matches!(phase.as_str(), "InProgress" | "GameStart" | "Reconnect") {
                    // 游戏中 → 调用 reconnect API（指数退避重试最多 5 次）
                    for i in 0..5 {
                        let res = client
                            .request("POST", "/lol-gameflow/v1/reconnect", None, false)
                            .await;
                        if res.is_ok() {
                            log::info!("[MainWindow] 游戏重连 API 调用成功");
                            break;
                        }
                        if i < 4 {
                            tokio::time::sleep(Duration::from_millis(500 * (1 << i))).await;
                        }
                    }
                } else {
                    // 其他阶段 → 请求回到大厅房间阶段并刷新
                    let _ = client
                        .request(
                            "POST",
                            "/lol-gameflow/v1/session/request-lobby-phase",
                            None,
                            false,
                        )
                        .await;
                }
            }
        });
        cx.notify();
    }

    /// 真实拉取 GitHub 公告数据并更新状态
    pub(super) fn fetch_notice(&mut self, cx: &mut Context<Self>) {
        self.notice_loading = true;
        self.notice_error = false;
        cx.notify();

        let proxy_cfg = yuumi_core::opgg::ProxyConfig {
            enable: self.app_config.general.enable_http_proxy,
            addr: self.app_config.general.http_proxy_addr.clone(),
        };

        const NOTICE_URL: &str =
            "https://raw.githubusercontent.com/ISuuuu/Yuumi/main/NOTICE.md";

        cx.spawn(async move |entity, cx| {
            let res = yuumi_core::opgg::fetch_github_text(NOTICE_URL, &proxy_cfg).await;
            let _ = entity.update(cx, |this, cx| {
                this.notice_loading = false;
                match res {
                    Ok(text) => {
                        log::info!("[Notice] 成功拉取 GitHub 公告，字符数: {}", text.len());
                        this.notice_content = Some(text);
                        this.notice_error = false;
                    }
                    Err(e) => {
                        log::warn!("[Notice] 拉取 GitHub 公告失败: {}", e);
                        this.notice_error = true;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 处理系统托盘分发动作
    pub(super) fn handle_tray_action(&mut self, action: TrayAction, cx: &mut Context<Self>) {
        match action {
            TrayAction::ShowWindow => {
                if let Some(hwnd) = platform::find_main_window_hwnd() {
                    platform::restore_and_foreground(hwnd);
                }
                cx.notify();
            }
            TrayAction::ToggleWindow => {
                if let Some(hwnd) = platform::find_main_window_hwnd() {
                    let is_visible = platform::is_window_visible(hwnd);
                    let is_minimized = platform::is_window_minimized(hwnd);
                    let is_foreground = platform::is_foreground_window(hwnd);

                    // 仅当窗口完全可见、非最小化且已是当前前台焦点时，才切换为隐藏
                    if is_visible && !is_minimized && is_foreground {
                        platform::hide_window(hwnd);
                    } else {
                        platform::restore_and_foreground(hwnd);
                    }
                }
                cx.notify();
            }
            TrayAction::Navigate(route) => {
                if let Some(hwnd) = platform::find_main_window_hwnd() {
                    platform::restore_and_foreground(hwnd);
                }
                self.switch_route(route, cx);
                cx.notify();
            }
            TrayAction::Quit => {
                self.quit_application(cx);
            }
        }
    }

    /// 显式退出应用：彻底关闭独立窗口、释放后台资源并退出
    pub(super) fn quit_application(&mut self, cx: &mut Context<Self>) {
        log::info!("[MainWindow] 用户触发显式退出，释放资源并退出应用");
        self.close_bench_overlay(cx);
        self.close_opgg_window(cx);
        cx.quit();
    }

    /// 将全局配置同步进磁盘、Agent 运行时与悬浮窗
    pub(super) fn persist_config(&mut self, cx: &mut Context<Self>) {
        self.automation = self.app_config.to_automation_settings();
        self.search_view.has_upload_api_url =
            !self.app_config.general.upload_api_url.trim().is_empty();
        self.search_view.upload_enabled = self.app_config.functions.upload_enabled;
        self.cloud_tasks.apply(&self.app_config);
        services::logging::set_level(self.app_config.general.log_level);

        let _ = self
            .live_service
            .auto_match_cfg
            .send(self.automation.auto_match.clone());
        let _ = self
            .live_service
            .auto_bp_cfg
            .send(self.automation.auto_bp.clone());

        if let Some(tray) = self.tray.as_mut() {
            let _ = tray.update_menu(self.app_config.functions.hide_tft);
        }

        if let Err(e) = self.app_config.save() {
            log::warn!("[Config] 全局配置持久化失败: {}", e);
        }

        // 悬浮窗开关变更需立即生效
        self.sync_bench_overlay(cx);
        cx.notify();
    }
}

/// OP.GG 窗口默认 760 × 820 逻辑像素，贴靠主窗口所在屏幕的右上角
/// （对照 Vue `openOpggWindow` 的 `x = pos.x + width - 760 - 2, y = pos.y + 2`），
/// 工作区不足时先收敛尺寸，保证标题栏与内容可达。
///
/// 显示器优先取包含主窗中心点的那一块（双屏时跟随主窗），取不到再退回主显示器。
pub(super) fn opgg_window_bounds(cx: &App, anchor: Option<Bounds<Pixels>>) -> WindowBounds {
    let mut width = px(760.0);
    let mut height = px(820.0);
    let display = anchor
        .and_then(|bounds| {
            let center = bounds.center();
            cx.displays().into_iter().find(|display| {
                let screen = display.bounds();
                let right = screen.origin.x + screen.size.width;
                let bottom = screen.origin.y + screen.size.height;
                center.x >= screen.origin.x
                    && center.x <= right
                    && center.y >= screen.origin.y
                    && center.y <= bottom
            })
        })
        .or_else(|| cx.primary_display());
    if let Some(display) = display {
        let bounds = display.bounds();
        width = width.min((bounds.size.width - px(40.0)).max(px(600.0)));
        height = height.min((bounds.size.height - px(80.0)).max(px(520.0)));
        let origin = Point {
            x: bounds.origin.x + bounds.size.width - width - px(2.0),
            y: bounds.origin.y + px(2.0),
        };
        return WindowBounds::Windowed(Bounds {
            origin,
            size: Size { width, height },
        });
    }
    WindowBounds::Windowed(Bounds {
        origin: Point {
            x: px(120.0),
            y: px(60.0),
        },
        size: Size { width, height },
    })
}
