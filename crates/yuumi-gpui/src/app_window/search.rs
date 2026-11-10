// 战绩查询路由：查询、详情、翻页、复制与跨页跳转。
use super::*;
use crate::match_model::ReplayState;

impl MainWindow {
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
        self.search_view.set_searching(true);
        self.search_view.detail_error = None;
        if let Some(gid) = target_game_id {
            self.search_view.selected_game_id = Some(gid);
        }
        cx.notify();

        let live_service = self.live_service.clone();
        let query_name = trimmed.clone();

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
                    // 2. 查询近期战绩 (0..20)
                    match yuumi_core::parsers::match_parser::fetch_match_history(
                        &client,
                        &summoner.puuid,
                        Some(0),
                        Some(19),
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
                                            this.search_view.insert_game_detail(gid, detail);
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
        if self.search_view.details_cache.contains_key(&game_id) {
            // 修复 B04 缺陷：缓存命中分支必须递增请求代数并重置加载/错误状态，
            // 避免已发出的旧网络请求在晚到后覆盖当前选中的缓存详情！
            self.search_view.select_request_id += 1;
            self.search_view.is_loading_detail = false;
            self.search_view.detail_error = None;
            self.search_view.selected_game_id = Some(game_id);
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
                            this.search_view.insert_game_detail(game_id, detail);
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
            log::info!("[MainWindow] 回放正在处理中，忽略重复点击: game_id={}", game_id);
            return;
        }
        if matches!(current, ReplayState::Unsupported) {
            log::warn!("[MainWindow] 回放不可用或已过期: game_id={}", game_id);
            return;
        }

        let live = self.live_service.clone();

        // 已就绪：直接触发客户端播放
        if matches!(current, ReplayState::Watch) {
            cx.spawn(async move |entity, cx| {
                if let Some(client) = live.get_client().await {
                    if let Err(e) = yuumi_core::parsers::replay::watch_game_replay(&client, game_id).await {
                        log::error!("[MainWindow] 启动回放失败: {}", e);
                        let _ = entity.update(cx, |_this, cx| {
                            crate::replay_store::set_replay_state(game_id, ReplayState::Error);
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
                let _ = entity.update(cx, |_this, cx| {
                    crate::replay_store::set_replay_state(game_id, ReplayState::Error);
                    cx.notify();
                });
                return;
            };

            match yuumi_core::parsers::replay::get_replay_status(&client, game_id).await {
                Ok(status) => {
                    let s = status.to_ascii_lowercase();
                    if s == "watch" || s == "found" {
                        let _ = entity.update(cx, |_this, cx| {
                            crate::replay_store::set_replay_state(game_id, ReplayState::Watch);
                            cx.notify();
                        });
                        let _ = yuumi_core::parsers::replay::watch_game_replay(&client, game_id).await;
                        return;
                    } else if s == "unsupported" || s == "incompatible" || s == "missingorexpired" || s == "lost" {
                        let _ = entity.update(cx, |_this, cx| {
                            crate::replay_store::set_replay_state(game_id, ReplayState::Unsupported);
                            cx.notify();
                        });
                        return;
                    } else if s == "downloading" {
                        let _ = entity.update(cx, |_this, cx| {
                            crate::replay_store::set_replay_state(game_id, ReplayState::Downloading);
                            cx.notify();
                        });
                    } else {
                        // 触发下载
                        let _ = entity.update(cx, |_this, cx| {
                            crate::replay_store::set_replay_state(game_id, ReplayState::Downloading);
                            cx.notify();
                        });
                        if let Err(e) = yuumi_core::parsers::replay::download_game_replay(&client, game_id).await {
                            log::error!("[MainWindow] 触发下载回放失败: {}", e);
                            let _ = entity.update(cx, |_this, cx| {
                                crate::replay_store::set_replay_state(game_id, ReplayState::Error);
                                cx.notify();
                            });
                            return;
                        }
                    }
                }
                Err(e) => {
                    log::error!("[MainWindow] 获取回放状态失败: {}", e);
                    let _ = entity.update(cx, |_this, cx| {
                        crate::replay_store::set_replay_state(game_id, ReplayState::Error);
                        cx.notify();
                    });
                    return;
                }
            }

            // 轮询下载进度（最多 40 次，每次 1.5 秒）
            for _ in 1..=40 {
                cx.background_executor().timer(Duration::from_millis(1500)).await;
                if let Ok(st) = yuumi_core::parsers::replay::get_replay_status(&client, game_id).await {
                    let s = st.to_ascii_lowercase();
                    if s == "watch" || s == "found" {
                        let _ = entity.update(cx, |_this, cx| {
                            crate::replay_store::set_replay_state(game_id, ReplayState::Watch);
                            cx.notify();
                        });
                        return;
                    } else if s == "unsupported" || s == "incompatible" || s == "missingorexpired" || s == "lost" {
                        let _ = entity.update(cx, |_this, cx| {
                            crate::replay_store::set_replay_state(game_id, ReplayState::Unsupported);
                            cx.notify();
                        });
                        return;
                    } else if s == "error" {
                        let _ = entity.update(cx, |_this, cx| {
                            crate::replay_store::set_replay_state(game_id, ReplayState::Error);
                            cx.notify();
                        });
                        return;
                    }
                }
            }

            // 超时
            let _ = entity.update(cx, |_this, cx| {
                crate::replay_store::set_replay_state(game_id, ReplayState::Error);
                cx.notify();
            });
        })
        .detach();
    }

    /// 复制指定 Game ID 并短暂展示反馈
    pub(super) fn copy_search_game_id(&mut self, game_id: u64, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(game_id.to_string()));
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
                });
            },
            move |_window, app| {
                view_next.update(app, |this, cx| {
                    this.handle_search_next_page(cx);
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
                });
            },
            move |val, _window, app| {
                view_upload.update(app, |this, cx| {
                    this.search_view.upload_enabled = val;
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
                    if !puuid.is_empty() && puuid != "00000000-0000-0000-0000-000000000000" {
                        this.view_other_career(puuid, name, String::new(), cx);
                    } else if summoner_id > 0 {
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
