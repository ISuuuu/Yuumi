// 战绩查询路由：查询、详情、翻页、复制与跨页跳转。
use super::*;
use crate::match_model::{GameDetail, ReplayState};
use crate::views::search::ParticipantRankInfo;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Instant;
use yuumi_core::parsers::game_info::{fetch_ranked_stats_batch, RankedQueueEntry};

/// 详情段位 TTL 缓存（对齐旧版 RANK_TTL = 5 分钟）
const RANK_TTL: std::time::Duration = std::time::Duration::from_secs(5 * 60);

/// 模块级段位缓存: puuid -> (段位项, 缓存时刻)。全应用一份，切对局/换玩家时命中免重复请求。
static RANK_CACHE: LazyLock<Mutex<HashMap<String, (RankedQueueEntry, Instant)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 段位项转玩家行徽章信息
fn rank_badge_info(entry: &RankedQueueEntry) -> ParticipantRankInfo {
    ParticipantRankInfo {
        tier: entry.tier.clone(),
        rank_text: yuumi_core::parsers::game_info::rank_tier_label(&entry.tier, &entry.division),
    }
}

/// 收集战绩列表需要预热的图标路径（与渲染同源：英雄/技能/符文/装备/海克斯）
fn match_list_asset_paths(items: &[MatchItem]) -> Vec<String> {
    let mut paths = Vec::new();
    for m in items {
        let d = &m.display;
        for url in [
            &d.champion_icon_url,
            &d.spell1_icon_url,
            &d.spell2_icon_url,
            &d.rune_icon_url,
        ] {
            if !url.is_empty() {
                paths.push(url.clone());
            }
        }
        paths.extend(d.item_icon_urls.iter().filter(|u| !u.is_empty()).cloned());
        paths.extend(
            d.augment_icon_urls
                .iter()
                .filter(|u| !u.is_empty())
                .cloned(),
        );
    }
    paths.sort();
    paths.dedup();
    paths
}

/// 收集对局详情 10 人需要预热的图标路径（英雄/技能/符文/装备/饰品）
fn match_detail_asset_paths(detail: &GameDetail) -> Vec<String> {
    let mut paths = Vec::new();
    for p in detail.blue.players.iter().chain(detail.red.players.iter()) {
        for url in [
            &p.champion_icon_url,
            &p.spell1_icon_url,
            &p.spell2_icon_url,
            &p.rune_icon_url,
            &p.ward_icon_url,
        ] {
            if !url.is_empty() {
                paths.push(url.clone());
            }
        }
        paths.extend(p.item_icon_urls.iter().filter(|u| !u.is_empty()).cloned());
    }
    paths.sort();
    paths.dedup();
    paths
}

impl MainWindow {
    /// 对局详情玩家段位后台拉取（对照 Vue loadRanksInBackground）。
    /// 关闭「对局信息显示段位」开关时不发任何请求；部分玩家查询失败不影响基础详情展示。
    pub(super) fn load_detail_ranks(&mut self, detail: &GameDetail, cx: &mut Context<Self>) {
        if !self.app_config.functions.show_tier_in_game_info {
            return;
        }
        let puuids: Vec<String> = detail
            .blue
            .players
            .iter()
            .chain(detail.red.players.iter())
            .map(|p| p.puuid.clone())
            .filter(|puuid| !puuid.is_empty())
            .collect();
        if puuids.is_empty() {
            return;
        }

        // 先吃模块级 TTL 缓存，仅未命中的 puuid 才发请求
        let mut results: HashMap<String, ParticipantRankInfo> = HashMap::new();
        let now = Instant::now();
        let pending: Vec<String>;
        {
            let cache = RANK_CACHE.lock().unwrap();
            pending = puuids
                .into_iter()
                .filter(|puuid| match cache.get(puuid) {
                    Some((entry, cached_at)) if now.duration_since(*cached_at) < RANK_TTL => {
                        results.insert(puuid.clone(), rank_badge_info(entry));
                        false
                    }
                    _ => true,
                })
                .collect();
        }

        self.search_view.rank_request_id += 1;
        let req_id = self.search_view.rank_request_id;
        self.search_view.participant_ranks = results.clone();
        if pending.is_empty() {
            cx.notify();
            return;
        }

        let live_service = self.live_service.clone();
        cx.spawn(async move |entity, cx| {
            let Some(client) = live_service.get_client().await else {
                return;
            };
            let fetched = fetch_ranked_stats_batch(&client, pending).await;
            let now = Instant::now();
            {
                let mut cache = RANK_CACHE.lock().unwrap();
                for (puuid, entry) in &fetched {
                    cache.insert(puuid.clone(), (entry.clone(), now));
                }
            }
            let _ = entity.update(cx, |this, cx| {
                if this.search_view.rank_request_id == req_id {
                    results.extend(
                        fetched
                            .iter()
                            .map(|(puuid, e)| (puuid.clone(), rank_badge_info(e))),
                    );
                    this.search_view.participant_ranks = results;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// 批量上传结果转 Toast（对照 Vue D15-D18）
    pub(super) fn show_upload_outcome(
        &mut self,
        outcome: services::cloud_tasks::UploadOutcome,
        cx: &mut Context<Self>,
    ) {
        use services::cloud_tasks::UploadOutcome;
        match outcome {
            UploadOutcome::Uploaded(count) => {
                self.toast(ToastKind::Success, format!("已上传 {} 场对局", count), cx);
            }
            UploadOutcome::Partial(success, failed) => {
                self.toast(
                    ToastKind::Warning,
                    format!("已上传 {} 场对局，{} 场失败", success, failed),
                    cx,
                );
            }
            UploadOutcome::AllFailed(failed) => {
                self.toast(
                    ToastKind::Error,
                    format!("上传失败: {} 场对局均未上传成功", failed),
                    cx,
                );
            }
            UploadOutcome::Failed(error) => {
                self.toast(ToastKind::Error, format!("上传异常: {}", error), cx);
            }
            UploadOutcome::AllExisted => {
                self.toast(ToastKind::Info, "所有对局已存在，无需上传", cx);
            }
        }
        cx.notify();
    }

    fn upload_current_search_page(&self) {
        let Some(owner) = self.own_summoner.as_ref() else {
            log::debug!("[Upload] 跳过：本机召唤师未连接");
            return;
        };
        if !self.app_config.functions.upload_enabled
            || self.search_view.current_summoner_puuid.as_deref() != Some(owner.puuid.as_str())
        {
            log::debug!("[Upload] 跳过：未勾选上传或当前查询非本人战绩");
            return;
        }
        let (items, _, _) = self.search_view.paged_filtered_matches();
        let ids: Vec<u64> = items.iter().map(|item| item.display.game_id).collect();
        log::info!("[Upload] 查询触发批量上传: {} 场对局", ids.len());
        self.cloud_tasks
            .upload_matches(owner.puuid.clone(), ids, true);
    }
    /// 控制战绩查询页队列下拉的开合
    pub(super) fn set_search_queue_menu(&mut self, open: bool, cx: &mut Context<Self>) {
        self.search_view.queue_menu_open = open;
        cx.notify();
    }

    /// 跳转到战绩查询并定位指定对局（对照 Vue `goToMatchDetail`）
    pub(super) fn navigate_to_search(
        &mut self,
        query: &str,
        target_game_id: Option<u64>,
        cx: &mut Context<Self>,
    ) {
        self.perform_search_with_target(query, target_game_id, cx);
        self.switch_route(Route::Search, cx);
    }

    pub(super) fn perform_search(&mut self, query: &str, cx: &mut Context<Self>) {
        self.perform_search_with_target(query, None, cx);
    }

    pub(super) fn perform_search_with_target(
        &mut self,
        query: &str,
        target_game_id: Option<u64>,
        cx: &mut Context<Self>,
    ) {
        let trimmed = query.trim().to_string();
        if trimmed.is_empty() {
            return;
        }

        // 防双击与连击：正在搜索中则忽略重复请求
        if self.search_view.is_searching {
            return;
        }

        // 推进搜索世代代数，防止快速连续搜索时慢响应回填覆盖新结果
        self.search_view.search_generation += 1;
        let current_gen = self.search_view.search_generation;

        self.search_view.show_history = false;
        self.search_view.set_search_query(&trimmed);
        self.search_view.search_query_dirty = false;
        self.search_view.set_searching(true);
        self.search_view.detail_error = None;
        if let Some(gid) = target_game_id {
            // [P1-3] 记录跳转目标：set_search_results 命中后按列表下标自动翻页定位
            self.search_view.target_select_game_id = Some(gid);
            self.search_view.selected_game_id = Some(gid);
        }
        cx.notify();

        let live_service = self.live_service.clone();
        let query_name = trimmed.clone();
        // [P1-3] 拉取范围至少覆盖生涯页设置的对局数量：生涯第 11~50 场跳转查询页时才能命中
        let fetch_end = self
            .app_config
            .functions
            .career_games_number
            .max(20)
            .saturating_sub(1);

        cx.spawn(async move |entity, cx| {
            let client_opt = live_service.get_client().await;
            let assets_opt = live_service.get_game_data_assets().await;

            let (client, assets) = match (client_opt, assets_opt) {
                (Some(c), Some(a)) => (c, a),
                _ => {
                    let _ = entity.update(cx, |this, cx| {
                        if this.search_view.search_generation == current_gen {
                            this.search_view
                                .set_search_error("客户端未连接或离线中，请先启动英雄联盟");
                            cx.notify();
                        }
                    });
                    return;
                }
            };

            // 1. 查询召唤师信息
            match yuumi_core::parsers::summoner::fetch_summoner_by_name(&client, &query_name).await
            {
                Ok(summoner) => {
                    let full_name = summoner.full_name();
                    // 2. 查询近期战绩 (0..=career_games_number.max(20)-1)
                    match yuumi_core::parsers::match_parser::fetch_match_history(
                        &client,
                        &summoner.puuid,
                        Some(0),
                        Some(fetch_end),
                        &assets,
                    )
                    .await
                    {
                        Ok(matches) => {
                            let items: Vec<MatchItem> =
                                matches.into_iter().map(MatchItem::new).collect();
                            let selected_game_id = target_game_id
                                .filter(|tgid| items.iter().any(|m| m.display.game_id == *tgid))
                                .or_else(|| items.first().map(|m| m.display.game_id));

                            let _ = entity.update(cx, |this, cx| {
                                if this.search_view.search_generation != current_gen {
                                    return;
                                }
                                // B03 规范：仅在查询成功后才将规范的 Riot ID 存入历史记录
                                this.search_view.save_to_history(&full_name);
                                this.search_view.set_search_results(
                                    full_name,
                                    summoner.puuid.clone(),
                                    summoner.summoner_id.unwrap_or(0),
                                    items,
                                );
                                // 列表图标异步预热（与渲染同源路径），避免右栏只显示占位文字
                                let list_paths = match_list_asset_paths(&this.search_view.matches);
                                prewarm_assets(list_paths, cx);
                                this.upload_current_search_page();
                                if let Some(gid) = selected_game_id {
                                    this.search_view.selected_game_id = Some(gid);
                                }
                                cx.notify();
                            });

                            // 3. 异步拉取详情填入 details_cache
                            if let Some(gid) = selected_game_id {
                                if let Ok(detail) =
                                    yuumi_core::parsers::match_detail::fetch_game_detail(
                                        &client, gid, &assets,
                                    )
                                    .await
                                {
                                    let _ = entity.update(cx, |this, cx| {
                                        if this.search_view.search_generation == current_gen {
                                            this.search_view
                                                .insert_game_detail(gid, detail.clone());
                                            this.load_detail_ranks(&detail, cx);
                                            // 详情 10 人全套图标预热
                                            prewarm_assets(match_detail_asset_paths(&detail), cx);
                                            cx.notify();
                                        }
                                    });
                                }
                            }
                        }
                        Err(e) => {
                            let _ = entity.update(cx, |this, cx| {
                                if this.search_view.search_generation == current_gen {
                                    this.search_view
                                        .set_search_error(format!("获取战绩失败: {}", e));
                                    cx.notify();
                                }
                            });
                        }
                    }
                }
                Err(e) => {
                    let _ = entity.update(cx, |this, cx| {
                        if this.search_view.search_generation == current_gen {
                            this.search_view
                                .set_search_error(format!("未找到召唤师: {}", e));
                            cx.notify();
                        }
                    });
                }
            }
        })
        .detach();
    }

    pub(super) fn load_match_detail(&mut self, game_id: u64, cx: &mut Context<Self>) {
        if let Some(cached) = self.search_view.details_cache.get(&game_id).cloned() {
            // 修复 B04 缺陷：缓存命中分支必须递增请求代数并重置加载/错误状态，
            // 避免已发出的旧网络请求在晚到后覆盖当前选中的缓存详情！
            self.search_view.select_request_id += 1;
            self.search_view.is_loading_detail = false;
            self.search_view.detail_error = None;
            self.search_view.selected_game_id = Some(game_id);
            self.load_detail_ranks(&cached, cx);
            cx.notify();
            return;
        }

        self.search_view.select_request_id += 1;
        let req_id = self.search_view.select_request_id;
        self.search_view.selected_game_id = Some(game_id);
        self.search_view.is_loading_detail = true;
        self.search_view.detail_error = None;
        cx.notify();

        let live_service = self.live_service.clone();
        cx.spawn(async move |entity, cx| {
            let client_opt = live_service.get_client().await;
            let assets_opt = live_service.get_game_data_assets().await;

            if let (Some(client), Some(assets)) = (client_opt, assets_opt) {
                if let Ok(detail) =
                    yuumi_core::parsers::match_detail::fetch_game_detail(&client, game_id, &assets)
                        .await
                {
                    let _ = entity.update(cx, |this, cx| {
                        if this.search_view.select_request_id == req_id {
                            this.search_view.insert_game_detail(game_id, detail.clone());
                            this.load_detail_ranks(&detail, cx);
                            // 详情 10 人全套图标预热
                            prewarm_assets(match_detail_asset_paths(&detail), cx);
                            this.search_view.is_loading_detail = false;
                            this.search_view.detail_error = None;
                            cx.notify();
                        }
                    });
                    return;
                }
            }

            let _ = entity.update(cx, |this, cx| {
                if this.search_view.select_request_id == req_id {
                    log::warn!("[MainWindow] 对局详情拉取失败: game_id={}", game_id);
                    this.search_view.is_loading_detail = false;
                    this.search_view.detail_error = Some("获取对局详情失败，请重试".to_string());
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// 统一对局回放处理状态机（支持检查状态、下载进度轮询、自动观看与重试）
    pub(super) fn handle_replay(&mut self, game_id: u64, cx: &mut Context<Self>) {
        let current = crate::replay_store::get_replay_state(game_id);
        if matches!(current, ReplayState::Checking | ReplayState::Downloading) {
            log::info!(
                "[MainWindow] 回放正在处理中，忽略重复点击: game_id={}",
                game_id
            );
            return;
        }
        if matches!(current, ReplayState::Unsupported) {
            log::warn!("[MainWindow] 回放不可用或已过期: game_id={}", game_id);
            return;
        }

        let live = self.live_service.clone();

        // 已就绪：直接触发客户端播放
        if matches!(current, ReplayState::Watch) {
            // 对照 Vue D07：已就绪点击播放
            self.toast(ToastKind::Info, "正在启动对局回放...", cx);
            cx.spawn(async move |entity, cx| {
                if let Some(client) = live.get_client().await {
                    if let Err(e) =
                        yuumi_core::parsers::replay::watch_game_replay(&client, game_id).await
                    {
                        log::error!("[MainWindow] 启动回放失败: {}", e);
                        let _ = entity.update(cx, |this, cx| {
                            crate::replay_store::set_replay_state(game_id, ReplayState::Error);
                            // 对照 Vue D08
                            this.toast(ToastKind::Error, format!("启动回放失败: {}", e), cx);
                            cx.notify();
                        });
                    }
                }
            })
            .detach();
            return;
        }

        // Idle 或 Error：先进入 Checking
        crate::replay_store::set_replay_state(game_id, ReplayState::Checking);
        cx.notify();

        cx.spawn(async move |entity, cx| {
            let Some(client) = live.get_client().await else {
                log::warn!("[MainWindow] LCU 客户端未连接，无法获取回放");
                let _ = entity.update(cx, |this, cx| {
                    crate::replay_store::set_replay_state(game_id, ReplayState::Error);
                    this.toast(ToastKind::Error, "客户端未连接，无法获取回放", cx);
                    cx.notify();
                });
                return;
            };

            match yuumi_core::parsers::replay::get_replay_status(&client, game_id).await {
                Ok(status) => {
                    let s = status.to_ascii_lowercase();
                    if s == "watch" || s == "found" {
                        let _ = entity.update(cx, |this, cx| {
                            crate::replay_store::set_replay_state(game_id, ReplayState::Watch);
                            // 对照 Vue D09：已就绪直接播放
                            this.toast(ToastKind::Info, "回放已下载就绪，正在启动...", cx);
                            cx.notify();
                        });
                        if let Err(e) =
                            yuumi_core::parsers::replay::watch_game_replay(&client, game_id).await
                        {
                            let _ = entity.update(cx, |this, cx| {
                                // 对照 Vue D10
                                this.toast(ToastKind::Error, format!("启动回放失败: {}", e), cx);
                                cx.notify();
                            });
                        }
                        return;
                    } else if s == "unsupported"
                        || s == "incompatible"
                        || s == "missingorexpired"
                        || s == "lost"
                    {
                        let _ = entity.update(cx, |this, cx| {
                            crate::replay_store::set_replay_state(
                                game_id,
                                ReplayState::Unsupported,
                            );
                            // 对照 Vue D06
                            this.toast(ToastKind::Warning, "该对局回放已过期或不可用", cx);
                            cx.notify();
                        });
                        return;
                    } else if s == "downloading" {
                        let _ = entity.update(cx, |this, cx| {
                            crate::replay_store::set_replay_state(
                                game_id,
                                ReplayState::Downloading,
                            );
                            // 对照 Vue D12：云端已在下载
                            this.toast(ToastKind::Info, "回放正在下载中...", cx);
                            cx.notify();
                        });
                    } else {
                        // 触发下载
                        let _ = entity.update(cx, |this, cx| {
                            crate::replay_store::set_replay_state(
                                game_id,
                                ReplayState::Downloading,
                            );
                            // 对照 Vue D13
                            this.toast(ToastKind::Info, "已开始下载回放文件...", cx);
                            cx.notify();
                        });
                        if let Err(e) =
                            yuumi_core::parsers::replay::download_game_replay(&client, game_id)
                                .await
                        {
                            log::error!("[MainWindow] 触发下载回放失败: {}", e);
                            let _ = entity.update(cx, |this, cx| {
                                crate::replay_store::set_replay_state(game_id, ReplayState::Error);
                                // 对照 Vue D04
                                this.toast(ToastKind::Error, format!("下载回放失败: {}", e), cx);
                                cx.notify();
                            });
                            return;
                        }
                    }
                }
                Err(e) => {
                    log::error!("[MainWindow] 获取回放状态失败: {}", e);
                    let _ = entity.update(cx, |this, cx| {
                        crate::replay_store::set_replay_state(game_id, ReplayState::Error);
                        // 对照 Vue D14
                        this.toast(ToastKind::Error, format!("获取回放失败: {}", e), cx);
                        cx.notify();
                    });
                    return;
                }
            }

            // 轮询下载进度（最多 40 次，每次 1.5 秒）
            for _ in 1..=40 {
                cx.background_executor()
                    .timer(Duration::from_millis(1500))
                    .await;
                if let Ok(st) =
                    yuumi_core::parsers::replay::get_replay_status(&client, game_id).await
                {
                    let s = st.to_ascii_lowercase();
                    if s == "watch" || s == "found" {
                        let _ = entity.update(cx, |this, cx| {
                            crate::replay_store::set_replay_state(game_id, ReplayState::Watch);
                            // 对照 Vue D02
                            this.toast(ToastKind::Success, "回放下载完成，可点击观看", cx);
                            cx.notify();
                        });
                        return;
                    } else if s == "unsupported"
                        || s == "incompatible"
                        || s == "missingorexpired"
                        || s == "lost"
                    {
                        let _ = entity.update(cx, |this, cx| {
                            crate::replay_store::set_replay_state(
                                game_id,
                                ReplayState::Unsupported,
                            );
                            this.toast(ToastKind::Warning, "该对局回放已过期或不可用", cx);
                            cx.notify();
                        });
                        return;
                    } else if s == "error" {
                        let _ = entity.update(cx, |this, cx| {
                            crate::replay_store::set_replay_state(game_id, ReplayState::Error);
                            // 对照 Vue D04
                            this.toast(ToastKind::Error, "下载回放失败，请稍后重试", cx);
                            cx.notify();
                        });
                        return;
                    }
                }
            }

            // 超时
            let _ = entity.update(cx, |this, cx| {
                crate::replay_store::set_replay_state(game_id, ReplayState::Error);
                // 对照 Vue D01
                this.toast(ToastKind::Warning, "下载回放超时，请重试", cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// 复制指定 Game ID 并短暂展示反馈
    pub(super) fn copy_search_game_id(&mut self, game_id: u64, cx: &mut Context<Self>) {
        // 对照 Vue D20：以实际写入结果决定提示（剪贴板被占用/失败可感知）
        if let Err(e) = platform::try_write_clipboard(&game_id.to_string()) {
            self.toast(ToastKind::Error, format!("复制对局 ID 失败: {}", e), cx);
            cx.notify();
            return;
        }
        self.toast(
            ToastKind::Success,
            format!("游戏 ID: {} 已复制到剪贴板", game_id),
            cx,
        );
        self.search_view.copied_feedback = Some(game_id);
        cx.notify();

        cx.spawn(async move |entity, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1500))
                .await;
            let _ = entity.update(cx, |this, cx| {
                if this.search_view.copied_feedback == Some(game_id) {
                    this.search_view.copied_feedback = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// 战绩查询翻到上一页并自动定位当页首局详情
    pub(super) fn handle_search_prev_page(&mut self, cx: &mut Context<Self>) {
        if self.search_view.current_page > 1 {
            self.search_view.current_page -= 1;
            let (paged, _, _) = self.search_view.paged_filtered_matches();
            if let Some(first) = paged.first() {
                let gid = first.display.game_id;
                self.load_match_detail(gid, cx);
            }
            cx.notify();
        }
    }

    /// 战绩查询翻到下一页并自动定位当页首局详情
    pub(super) fn handle_search_next_page(&mut self, cx: &mut Context<Self>) {
        let (_, _, has_more) = self.search_view.paged_filtered_matches();
        if has_more {
            self.search_view.current_page += 1;
            let (paged, _, _) = self.search_view.paged_filtered_matches();
            if let Some(first) = paged.first() {
                let gid = first.display.game_id;
                self.load_match_detail(gid, cx);
            }
            cx.notify();
        }
    }

    /// 战绩查询路由渲染与动作回调。
    pub(super) fn render_search_route(
        &mut self,
        el: Stateful<Div>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let view_entity = cx.entity().clone();
        let view = view_entity.clone();
        let view_sq = view.clone();
        let view_del_hist = view.clone();
        let view_prev = view.clone();
        let view_next = view.clone();
        let view_career = view.clone();
        let view_upload = view.clone();
        let view_copy = view.clone();
        let view_player = view.clone();
        let view_replay = view.clone();
        let view_retry = view.clone();

        let on_select_match = {
            let v = view.clone();
            move |game_id: u64, _win: &mut Window, app: &mut App| {
                v.update(app, |this, cx| {
                    this.load_match_detail(game_id, cx);
                });
            }
        };
        let on_filter_queue = {
            let v = view.clone();
            move |qid: Option<i32>, _win: &mut Window, app: &mut App| {
                v.update(app, |this, cx| {
                    this.search_view.active_queue_filter = qid;
                    this.search_view.current_page = 1;
                    this.upload_current_search_page();
                    let (paged, _, _) = this.search_view.paged_filtered_matches();
                    if let Some(first) = paged.first() {
                        let gid = first.display.game_id;
                        this.load_match_detail(gid, cx);
                    } else {
                        // B04 规范：空筛选时清除选中的对局与详情，保持左右栏状态一致
                        this.search_view.selected_game_id = None;
                        this.search_view.last_detail = None;
                        this.search_view.detail_error = None;
                    }
                    cx.notify();
                });
            }
        };
        let on_search = {
            let v = view.clone();
            move |query: &str, _win: &mut Window, app: &mut App| {
                let q = query.to_string();
                v.update(app, |this, cx| {
                    this.perform_search(&q, cx);
                });
            }
        };
        let on_input_change = {
            let v = view.clone();
            move |new_text: String, _win: &mut Window, app: &mut App| {
                v.update(app, |this, cx| {
                    this.search_view.set_search_query(new_text);
                    this.search_view.search_query_dirty = true;
                    this.search_view.show_history = true;
                    cx.notify();
                });
            }
        };

        let view_open_hist = view.clone();
        let view_close_hist = view.clone();

        let connected = matches!(self.connection_status, ConnectionStatus::Connected { .. });

        el.overflow_hidden().h_full().child(self.search_view.render(
            connected,
            on_select_match,
            on_filter_queue,
            on_search,
            on_input_change,
            self.search_view.focus_handle.is_focused(window),
            move |_window, app| {
                view_open_hist.update(app, |this, cx| {
                    this.search_view.show_history = true;
                    this.search_view.queue_menu_open = false;
                    cx.notify();
                });
            },
            move |open, _window, app| {
                view_sq.update(app, |this, cx| this.set_search_queue_menu(open, cx));
            },
            move |entry, _window, app| {
                let e = entry.to_string();
                view_del_hist.update(app, |this, cx| {
                    this.search_view.remove_from_history(&e);
                    cx.notify();
                });
            },
            move |_window, app| {
                view_close_hist.update(app, |this, cx| {
                    this.search_view.show_history = false;
                    cx.notify();
                });
            },
            move |_window, app| {
                view_prev.update(app, |this, cx| {
                    this.handle_search_prev_page(cx);
                    this.upload_current_search_page();
                });
            },
            move |_window, app| {
                view_next.update(app, |this, cx| {
                    this.handle_search_next_page(cx);
                    this.upload_current_search_page();
                });
            },
            move |_window, app| {
                view_career.update(app, |this, cx| {
                    let current_target = this.search_view.current_summoner_puuid.clone();
                    let self_puuid = this.own_summoner.as_ref().map(|s| s.puuid.clone());
                    if let Some(target) = current_target {
                        if Some(&target) != self_puuid.as_ref() {
                            let name = this
                                .search_view
                                .current_summoner_name
                                .clone()
                                .unwrap_or_default();
                            this.view_other_career(target, name, String::new(), cx);
                            return;
                        }
                    }
                    this.back_to_my_career(cx);
                    this.switch_route(Route::Career, cx);
                });
            },
            move |val, _window, app| {
                view_upload.update(app, |this, cx| {
                    this.app_config.functions.upload_enabled = val;
                    this.persist_config(cx);
                    if val {
                        this.upload_current_search_page();
                    }
                    cx.notify();
                });
            },
            move |game_id, _window, app| {
                view_copy.update(app, |this, cx| {
                    this.copy_search_game_id(game_id, cx);
                });
            },
            move |puuid, summoner_id, name, _window, app| {
                view_player.update(app, |this, cx| {
                    let is_bot = summoner_id == 0
                        && (puuid.is_empty() || puuid == "00000000-0000-0000-0000-000000000000");
                    if !is_bot && !name.trim().is_empty() {
                        this.perform_search(&name, cx);
                    }
                });
            },
            move |game_id, _window, app| {
                view_replay.update(app, |this, cx| {
                    this.handle_replay(game_id, cx);
                });
            },
            move |game_id, _window, app| {
                view_retry.update(app, |this, cx| {
                    this.load_match_detail(game_id, cx);
                });
            },
        ))
    }
}
