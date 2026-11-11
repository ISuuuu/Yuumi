// 路人集路由：首屏拉取、分页加载与相遇记录写入接线。
use super::*;

impl MainWindow {
    /// [P2-1] 后台打开路人集 SQLite（建库 / WAL / 迁移检查均为阻塞 I/O），
    /// 打开完成后回写 `storage` 并触发首屏异步加载。启动与首帧渲染 0 同步磁盘 I/O。
    pub(super) fn init_saved_players_storage(&mut self, cx: &mut Context<Self>) {
        if self.saved_players_view.storage.is_some() {
            return;
        }
        let handle = yuumi_core::runtime::tokio_handle();
        cx.spawn(async move |entity, cx| {
            let Some(handle) = handle else {
                return;
            };
            let opened = handle
                .spawn(async move {
                    tokio::task::spawn_blocking(
                        yuumi_core::storage::SavedPlayersStorage::open_default,
                    )
                    .await
                    .unwrap_or_else(|e| Err(e.to_string()))
                })
                .await
                .unwrap_or_else(|e| Err(e.to_string()));

            let _ = entity.update(cx, |this, cx| {
                match opened {
                    Ok(storage) => {
                        this.saved_players_view.storage = Some(storage);
                        // 打开就绪后按当前 self_puuid 异步加载首屏
                        this.reload_saved_players(cx);
                    }
                    Err(e) => {
                        log::error!("[MainWindow] 路人集数据库打开失败: {}", e);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 异步拉取路人集首屏（完全剥离到 Tokio 阻塞池，0 阻塞 UI 主线程）
    pub(super) fn reload_saved_players(&mut self, cx: &mut Context<Self>) {
        let storage = self.saved_players_view.storage.clone();
        let Some(storage) = storage else {
            return;
        };

        let req_id = self.saved_players_view.next_request_seq();
        let filter = if self.saved_players_view.active_tag_filter == "全部" {
            None
        } else {
            Some(self.saved_players_view.active_tag_filter.clone())
        };
        let search = if self.saved_players_view.search_query.trim().is_empty() {
            None
        } else {
            Some(self.saved_players_view.search_query.trim().to_string())
        };
        let self_p = self.saved_players_view.self_puuid.clone();

        let handle = yuumi_core::runtime::tokio_handle();
        cx.spawn(async move |entity, cx| {
            let Some(handle) = handle else {
                return;
            };
            let res = handle
                .spawn(async move {
                    tokio::task::spawn_blocking(move || {
                        // 对照 Vue F01：首屏加载失败不再静默吞掉，交由 Toast 呈现
                        let total = storage
                            .count_players(self_p.as_deref(), filter.as_deref(), search.as_deref())
                            .map_err(|e| e.to_string())?;
                        let list = storage
                            .query_players(
                                self_p.as_deref(),
                                filter.as_deref(),
                                search.as_deref(),
                                views::saved_players::SAVED_PLAYERS_PAGE_SIZE,
                                0,
                            )
                            .map_err(|e| e.to_string())?;
                        Ok((total, list))
                    })
                    .await
                    .unwrap_or(Err("后台查询中断".to_string()))
                })
                .await;

            // 统一为 Result<(u32, Vec<SavedPlayer>), String>：后台任务中断视为加载失败
            let res = res.unwrap_or_else(|_| Err("后台查询中断".to_string()));
            match res {
                Ok((total, list)) => {
                    let _ = entity.update(cx, |this, cx| {
                        if this.saved_players_view.request_seq != req_id {
                            return;
                        }
                        let mut asset_paths = Vec::new();
                        for p in &list {
                            asset_paths.push(p.profile_icon_url());
                            if let Some(champ) = p.champion_icon_url() {
                                asset_paths.push(champ);
                            }
                        }
                        prewarm_assets(asset_paths, cx);

                        this.saved_players_view.current_page = 1;
                        this.saved_players_view.total_count = total;
                        this.saved_players_view.players = list;
                        this.saved_players_view.is_loading_more = false;
                        cx.notify();
                    });
                }
                Err(e) => {
                    let _ = entity.update(cx, |this, cx| {
                        this.saved_players_view.is_loading_more = false;
                        // 对照 Vue F01：首屏加载失败以 Toast 呈现
                        this.toast(ToastKind::Error, format!("路人集加载失败: {}", e), cx);
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    /// 异步拉取路人集下一页（完全剥离到 Tokio 阻塞池，0 阻塞 UI 主线程）
    pub(super) fn load_more_saved_players(&mut self, cx: &mut Context<Self>) {
        if self.saved_players_view.is_loading_more
            || self.saved_players_view.players.len() >= self.saved_players_view.total_count as usize
        {
            return;
        }

        self.saved_players_view.is_loading_more = true;
        let storage = self.saved_players_view.storage.clone();
        let Some(storage) = storage else {
            self.saved_players_view.is_loading_more = false;
            return;
        };

        let next_page = self.saved_players_view.current_page + 1;
        let offset = (next_page - 1) * views::saved_players::SAVED_PLAYERS_PAGE_SIZE;
        let filter = if self.saved_players_view.active_tag_filter == "全部" {
            None
        } else {
            Some(self.saved_players_view.active_tag_filter.clone())
        };
        let search = if self.saved_players_view.search_query.trim().is_empty() {
            None
        } else {
            Some(self.saved_players_view.search_query.trim().to_string())
        };
        let self_p = self.saved_players_view.self_puuid.clone();
        let req_id = self.saved_players_view.request_seq;

        let handle = yuumi_core::runtime::tokio_handle();
        cx.spawn(async move |entity, cx| {
            let Some(handle) = handle else {
                return;
            };
            let res = handle
                .spawn(async move {
                    tokio::task::spawn_blocking(move || {
                        storage.query_players(
                            self_p.as_deref(),
                            filter.as_deref(),
                            search.as_deref(),
                            views::saved_players::SAVED_PLAYERS_PAGE_SIZE,
                            offset,
                        )
                    })
                    .await
                    .unwrap_or_else(|e| Err(e.to_string()))
                })
                .await;

            if let Ok(Ok(more)) = res {
                let _ = entity.update(cx, |this, cx| {
                    if this.saved_players_view.request_seq != req_id {
                        return;
                    }
                    let mut asset_paths = Vec::new();
                    for p in &more {
                        asset_paths.push(p.profile_icon_url());
                        if let Some(champ) = p.champion_icon_url() {
                            asset_paths.push(champ);
                        }
                    }
                    prewarm_assets(asset_paths, cx);

                    this.saved_players_view.players.extend(more);
                    this.saved_players_view.current_page = next_page;
                    this.saved_players_view.is_loading_more = false;
                    cx.notify();
                });
            } else {
                let _ = entity.update(cx, |this, _cx| {
                    this.saved_players_view.is_loading_more = false;
                });
            }
        })
        .detach();
    }

    /// 异步通过 LCU 回填路人集中缺失的召唤师名称、TagLine 与头像
    pub(super) fn backfill_saved_players_identity(&mut self, cx: &mut Context<Self>) {
        let storage = self.saved_players_view.storage.clone();
        let Some(storage) = storage else {
            return;
        };
        let live_service = self.live_service.clone();
        let handle = yuumi_core::runtime::tokio_handle();
        cx.spawn(async move |entity, cx| {
            let Some(handle) = handle else {
                return;
            };
            let updated = handle
                .spawn(async move {
                    let targets = storage.query_missing_identity_players().unwrap_or_default();
                    if targets.is_empty() {
                        return 0;
                    }
                    let client = live_service.get_client().await;
                    let Some(client) = client else {
                        return 0;
                    };

                    let mut updates = Vec::new();
                    for (puuid, self_puuid) in targets {
                        if puuid.is_empty() {
                            continue;
                        }
                        let path = format!("/lol-summoner/v2/summoners/puuid/{}", puuid);
                        if let Ok(info) = client.get(&path).await {
                            let tag_line = info
                                .get("tagLine")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let summoner_name = info
                                .get("displayName")
                                .and_then(|v| v.as_str())
                                .filter(|s| !s.is_empty())
                                .or_else(|| {
                                    info.get("gameName")
                                        .and_then(|v| v.as_str())
                                        .filter(|s| !s.is_empty())
                                })
                                .unwrap_or("")
                                .to_string();
                            let profile_icon_id =
                                info.get("profileIconId")
                                    .and_then(|v| v.as_i64())
                                    .unwrap_or(0) as i32;

                            if !tag_line.is_empty()
                                || !summoner_name.is_empty()
                                || profile_icon_id > 0
                            {
                                updates.push((
                                    puuid,
                                    self_puuid,
                                    tag_line,
                                    summoner_name,
                                    profile_icon_id,
                                ));
                            }
                        }
                    }
                    storage.batch_update_identities(&updates).unwrap_or(0)
                })
                .await
                .unwrap_or(0);

            if updated > 0 {
                let _ = entity.update(cx, |this, cx| {
                    log::info!("[MainWindow] 成功回填 {} 名路人身份信息", updated);
                    this.reload_saved_players(cx);
                });
            }
        })
        .detach();
    }

    /// 路人集路由渲染与 SavedPlayersAction 分发。
    pub(super) fn render_saved_players_route(
        &mut self,
        el: Stateful<Div>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let view_entity = cx.entity().clone();
        let view = view_entity.clone();
        let on_action = move |action: views::saved_players::SavedPlayersAction,
                              _win: &mut Window,
                              app: &mut App| {
            view.update(app, |this, cx| {
                use views::saved_players::SavedPlayersAction;
                match action {
                    SavedPlayersAction::FilterTag(tag) => {
                        this.saved_players_view.active_tag_filter = tag;
                        this.reload_saved_players(cx);
                    }
                    SavedPlayersAction::TogglePin(puuid) => {
                        if let Err(msg) = this.saved_players_view.toggle_pin(&puuid) {
                            this.toast(ToastKind::Error, msg, cx);
                        }
                    }
                    SavedPlayersAction::ToggleExpand(puuid) => {
                        this.saved_players_view.toggle_expand(&puuid);
                    }
                    SavedPlayersAction::RemovePlayer(puuid) => {
                        let _ = this.saved_players_view.remove_player(&puuid);
                    }
                    SavedPlayersAction::StartEdit(puuid) => {
                        this.saved_players_view.start_edit(&puuid);
                    }
                    SavedPlayersAction::SaveEdit(puuid) => {
                        match this.saved_players_view.save_edit(&puuid) {
                            Ok(()) => this.toast(ToastKind::Success, "标记保存成功", cx),
                            Err(msg) => this.toast(ToastKind::Error, msg, cx),
                        }
                    }
                    SavedPlayersAction::CancelEdit => {
                        this.saved_players_view.cancel_edit();
                    }
                    SavedPlayersAction::SetEditTag(tag) => {
                        this.saved_players_view.set_editing_tag(&tag);
                    }
                    SavedPlayersAction::LoadMore => {
                        this.load_more_saved_players(cx);
                    }
                    SavedPlayersAction::SearchInput(q) => {
                        this.saved_players_view.search_query = q;
                        this.reload_saved_players(cx);
                    }
                    SavedPlayersAction::SearchSubmit => {
                        this.reload_saved_players(cx);
                    }
                    SavedPlayersAction::SearchClear => {
                        this.saved_players_view.search_query.clear();
                        this.reload_saved_players(cx);
                    }
                    SavedPlayersAction::EditTagInput(t) => {
                        this.saved_players_view.editing_tag_buffer = t;
                    }
                    SavedPlayersAction::EditNotesInput(n) => {
                        this.saved_players_view.editing_notes_buffer = n;
                    }
                    SavedPlayersAction::AskDelete(puuid) => {
                        this.saved_players_view.ask_delete(&puuid);
                    }
                    SavedPlayersAction::CancelDelete => {
                        this.saved_players_view.cancel_delete();
                    }
                    SavedPlayersAction::ConfirmDelete(puuid) => {
                        match this.saved_players_view.confirm_delete(&puuid) {
                            Ok(()) => this.toast(ToastKind::Success, "玩家已删除", cx),
                            Err(msg) => this.toast(ToastKind::Error, msg, cx),
                        }
                        this.reload_saved_players(cx);
                    }
                    SavedPlayersAction::ExportTagged => {
                        match this.saved_players_view.export_tagged_to_file() {
                            Some(Ok(path)) => {
                                this.toast(ToastKind::Success, format!("已导出到 {}", path), cx)
                            }
                            Some(Err(msg)) => this.toast(ToastKind::Error, msg, cx),
                            None => {} // 用户取消文件对话框
                        }
                    }
                    SavedPlayersAction::ImportTagged => {
                        match this.saved_players_view.import_tagged_from_file() {
                            Some(Ok(count)) => this.toast(
                                ToastKind::Success,
                                format!("成功导入 {} 名玩家", count),
                                cx,
                            ),
                            Some(Err(msg)) => this.toast(ToastKind::Error, msg, cx),
                            None => {} // 用户取消文件对话框
                        }
                        this.reload_saved_players(cx);
                        this.backfill_saved_players_identity(cx);
                    }
                    SavedPlayersAction::LoadMoreGames => {
                        if let Err(msg) = this.saved_players_view.load_more_games() {
                            this.toast(ToastKind::Error, msg, cx);
                        }
                    }
                }
                cx.notify();
            });
        };

        el.child(self.saved_players_view.render_panel(window, on_action))
    }
}
