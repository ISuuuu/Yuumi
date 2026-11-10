// 路人集路由：首屏拉取、分页加载与相遇记录写入接线。
use super::*;

impl MainWindow {
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
                        let total = storage
                            .count_players(self_p.as_deref(), filter.as_deref(), search.as_deref())
                            .unwrap_or(0);
                        let list = storage
                            .query_players(
                                self_p.as_deref(),
                                filter.as_deref(),
                                search.as_deref(),
                                views::saved_players::SAVED_PLAYERS_PAGE_SIZE,
                                0,
                            )
                            .unwrap_or_default();
                        (total, list)
                    })
                    .await
                    .unwrap_or((0, Vec::new()))
                })
                .await;

            if let Ok((total, list)) = res {
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
                    let targets = storage
                        .query_missing_identity_players()
                        .unwrap_or_default();
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
                            let profile_icon_id = info
                                .get("profileIconId")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0) as i32;

                            if !tag_line.is_empty() || !summoner_name.is_empty() || profile_icon_id > 0 {
                                updates.push((puuid, self_puuid, tag_line, summoner_name, profile_icon_id));
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
                        this.saved_players_view.toggle_pin(&puuid);
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
                        this.saved_players_view.save_edit(&puuid);
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
                        this.saved_players_view.confirm_delete(&puuid);
                        this.reload_saved_players(cx);
                    }
                    SavedPlayersAction::ExportTagged => {
                        this.saved_players_view.export_tagged_to_file();
                    }
                    SavedPlayersAction::ImportTagged => {
                        this.saved_players_view.import_tagged_from_file();
                        this.reload_saved_players(cx);
                        this.backfill_saved_players_identity(cx);
                    }
                    SavedPlayersAction::LoadMoreGames => {
                        this.saved_players_view.load_more_games();
                    }
                    SavedPlayersAction::DismissToast => {
                        this.saved_players_view.toast_message = None;
                    }
                }
                cx.notify();
            });
        };

        el.child(self.saved_players_view.render_panel(window, on_action))
    }
}
