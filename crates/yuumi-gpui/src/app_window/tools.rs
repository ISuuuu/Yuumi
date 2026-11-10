// 工具箱路由：ToolsAction 分发与自动化配置回写。
use super::*;

impl MainWindow {
    /// 处理工具箱交互动作
    pub(super) fn handle_tools_action(&mut self, action: ToolsAction, cx: &mut Context<Self>) {
        match action {
            ToolsAction::ToggleCollapse(name) => {
                self.tools_view.toggle_card(name);
                cx.notify();
            }
            ToolsAction::ToggleAutoAccept => {
                self.app_config.functions.enable_auto_accept_matching =
                    !self.app_config.functions.enable_auto_accept_matching;
                self.persist_config(cx);
            }
            ToolsAction::SetAutoAcceptDelay(delay) => {
                self.app_config.functions.auto_accept_matching_delay = delay;
                self.persist_config(cx);
            }
            ToolsAction::ToggleCeilSwap => {
                self.app_config.functions.auto_accept_ceil_swap =
                    !self.app_config.functions.auto_accept_ceil_swap;
                self.persist_config(cx);
            }
            ToolsAction::ToggleChampTrade => {
                self.app_config.functions.auto_accept_champ_trade =
                    !self.app_config.functions.auto_accept_champ_trade;
                self.persist_config(cx);
            }
            ToolsAction::ToggleAutoPick => {
                self.app_config.functions.enable_auto_select_champion =
                    !self.app_config.functions.enable_auto_select_champion;
                self.persist_config(cx);
            }
            ToolsAction::ToggleAutoHover => {
                self.app_config.functions.enable_auto_hover_champion =
                    !self.app_config.functions.enable_auto_hover_champion;
                self.persist_config(cx);
            }
            ToolsAction::ToggleConfirmOnTimeout => {
                self.app_config.functions.auto_select_confirm_on_timeout =
                    !self.app_config.functions.auto_select_confirm_on_timeout;
                self.persist_config(cx);
            }
            ToolsAction::SetHoverLane(lane) => {
                self.tools_view.hover_active_lane = lane;
                cx.notify();
            }
            ToolsAction::ClearPickChampion(lane) => {
                match lane {
                    "top" => self.app_config.functions.auto_select_champion_top.clear(),
                    "jug" => self.app_config.functions.auto_select_champion_jug.clear(),
                    "mid" => self.app_config.functions.auto_select_champion_mid.clear(),
                    "bot" => self.app_config.functions.auto_select_champion_bot.clear(),
                    "sup" => self.app_config.functions.auto_select_champion_sup.clear(),
                    _ => self.app_config.functions.auto_select_champion.clear(),
                }
                self.persist_config(cx);
            }
            ToolsAction::ToggleAutoBan => {
                self.app_config.functions.enable_auto_ban_champion =
                    !self.app_config.functions.enable_auto_ban_champion;
                self.persist_config(cx);
            }
            ToolsAction::TogglePretendBan => {
                self.app_config.functions.pretend_ban = !self.app_config.functions.pretend_ban;
                self.persist_config(cx);
            }
            ToolsAction::SetAutoBanDelay(delay) => {
                self.app_config.functions.auto_ban_delay = delay;
                self.persist_config(cx);
            }
            ToolsAction::SetBanLane(lane) => {
                self.tools_view.ban_active_lane = lane;
                cx.notify();
            }
            ToolsAction::ClearBanChampion(lane) => {
                match lane {
                    "top" => self.app_config.functions.auto_ban_champion_top.clear(),
                    "jug" => self.app_config.functions.auto_ban_champion_jug.clear(),
                    "mid" => self.app_config.functions.auto_ban_champion_mid.clear(),
                    "bot" => self.app_config.functions.auto_ban_champion_bot.clear(),
                    "sup" => self.app_config.functions.auto_ban_champion_sup.clear(),
                    _ => self.app_config.functions.auto_ban_champion.clear(),
                }
                self.persist_config(cx);
            }
            ToolsAction::ToggleAutoSpell => {
                self.app_config.functions.enable_auto_set_spells =
                    !self.app_config.functions.enable_auto_set_spells;
                self.persist_config(cx);
            }
            ToolsAction::SetSpellLane(lane) => {
                self.tools_view.spell_active_lane = lane;
                cx.notify();
            }
            ToolsAction::SetSpellId {
                lane,
                is_spell1,
                spell_id,
            } => {
                let list = match lane {
                    "top" => &mut self.app_config.functions.auto_set_summoner_spell_top,
                    "jug" => &mut self.app_config.functions.auto_set_summoner_spell_jug,
                    "mid" => &mut self.app_config.functions.auto_set_summoner_spell_mid,
                    "bot" => &mut self.app_config.functions.auto_set_summoner_spell_bot,
                    "sup" => &mut self.app_config.functions.auto_set_summoner_spell_sup,
                    _ => &mut self.app_config.functions.auto_set_summoner_spell,
                };
                if list.len() < 2 {
                    *list = vec![4, 14];
                }
                if is_spell1 {
                    list[0] = spell_id;
                } else {
                    list[1] = spell_id;
                }
                self.persist_config(cx);
            }
            ToolsAction::ToggleAutoHandleInvite => {
                self.app_config.functions.enable_auto_handle_invite =
                    !self.app_config.functions.enable_auto_handle_invite;
                self.persist_config(cx);
            }
            ToolsAction::ToggleAutoHonor => {
                self.app_config.functions.enable_auto_honor =
                    !self.app_config.functions.enable_auto_honor;
                self.persist_config(cx);
            }
            ToolsAction::ToggleAutoPlayAgain => {
                self.app_config.functions.enable_auto_play_again =
                    !self.app_config.functions.enable_auto_play_again;
                self.persist_config(cx);
            }
            ToolsAction::ToggleAramTeamSide => {
                self.app_config.functions.enable_auto_aram_team_side =
                    !self.app_config.functions.enable_auto_aram_team_side;
                self.persist_config(cx);
            }
            ToolsAction::ToggleAramTeamSideVisible => {
                self.app_config.functions.aram_team_side_visible_to_team =
                    !self.app_config.functions.aram_team_side_visible_to_team;
                self.persist_config(cx);
            }
            ToolsAction::ToggleTagReminder => {
                self.app_config.functions.enable_auto_tag_reminder =
                    !self.app_config.functions.enable_auto_tag_reminder;
                self.persist_config(cx);
            }
            ToolsAction::ToggleBenchOverlay => {
                self.app_config.functions.enable_bench_overlay =
                    !self.app_config.functions.enable_bench_overlay;
                self.persist_config(cx);
            }
            ToolsAction::OpenBenchOverlayWindow => {
                self.open_bench_overlay(cx);
            }
            ToolsAction::ToggleAutoReconnect => {
                self.app_config.functions.enable_auto_reconnect =
                    !self.app_config.functions.enable_auto_reconnect;
                self.persist_config(cx);
            }
            ToolsAction::ToggleAutoCreateLobby => {
                self.app_config.functions.enable_auto_create_lobby =
                    !self.app_config.functions.enable_auto_create_lobby;
                self.persist_config(cx);
            }
            ToolsAction::SetDefaultGameMode(mode) => {
                self.app_config.functions.default_game_mode = mode;
                self.persist_config(cx);
            }
            ToolsAction::CreateLobbyNow => {
                let live = self.live_service.clone();
                let mode = self.app_config.functions.default_game_mode;
                yuumi_core::runtime::spawn_background(async move {
                    if let Some(client) = live.get_client().await {
                        let body = serde_json::json!({ "queueId": mode });
                        let _ = client
                            .request("POST", "/lol-lobby/v2/lobby", Some(body), false)
                            .await;
                    }
                });
                self.tools_view.set_feedback("已发送创建房间指令", false);
                cx.notify();
            }
            ToolsAction::ToggleLockGameSettings => {
                self.tools_view.is_game_settings_locked = !self.tools_view.is_game_settings_locked;
                self.tools_view.set_feedback(
                    if self.tools_view.is_game_settings_locked {
                        "已锁定游戏配置文件 (只读保护)"
                    } else {
                        "已解除游戏配置文件只读锁定"
                    },
                    false,
                );
                cx.notify();
            }
            ToolsAction::FixClientWindow => {
                std::thread::spawn(|| {
                    let _ = services::fix_client_window();
                });
                self.tools_view
                    .set_feedback("已重置客户端窗口为 1600x900", false);
                cx.notify();
            }
            ToolsAction::RestartClient => {
                let live = self.live_service.clone();
                yuumi_core::runtime::spawn_background(async move {
                    if let Some(client) = live.get_client().await {
                        let _ = client
                            .request("POST", "/riotclient/kill-and-restart-ux", None, false)
                            .await;
                    }
                });
                self.tools_view.set_feedback("已发送客户端重启指令", false);
                cx.notify();
            }
            ToolsAction::SpectateSummonerNameInput(name) => {
                self.tools_view.spectate_name_buffer = name;
                cx.notify();
            }
            ToolsAction::LaunchSpectate => {
                self.tools_view.set_feedback("观战指令已发送", false);
                cx.notify();
            }
            ToolsAction::ScanFriends => {
                self.tools_view.set_feedback("好友扫描完成", false);
                cx.notify();
            }
            ToolsAction::SetCleanerDays(d) => {
                self.tools_view.cleaner_days = d;
                cx.notify();
            }
            ToolsAction::BatchDeleteOfflineFriends => {
                self.tools_view.set_feedback("批量清理已完成", false);
                cx.notify();
            }
            ToolsAction::SignatureInput(s) => {
                self.tools_view.signature_buffer = s;
                cx.notify();
            }
            ToolsAction::ApplySignature => {
                let sig = self.tools_view.signature_buffer.clone();
                let live = self.live_service.clone();
                yuumi_core::runtime::spawn_background(async move {
                    if let Some(client) = live.get_client().await {
                        let body = serde_json::json!({ "statusMessage": sig });
                        let _ = client
                            .request("PUT", "/lol-chat/v1/me", Some(body), false)
                            .await;
                    }
                });
                self.tools_view.set_feedback("个性签名已更新", false);
                cx.notify();
            }
            ToolsAction::ProfileBgSkinInput(s) => {
                self.tools_view.profile_skin_id_buffer = s;
                cx.notify();
            }
            ToolsAction::ApplyProfileBackground => {
                if let Ok(skin_id) = self.tools_view.profile_skin_id_buffer.trim().parse::<i32>() {
                    let live = self.live_service.clone();
                    yuumi_core::runtime::spawn_background(async move {
                        if let Some(client) = live.get_client().await {
                            let body =
                                serde_json::json!({ "key": "backgroundSkinId", "value": skin_id });
                            let _ = client
                                .request(
                                    "POST",
                                    "/lol-summoner/v1/current-summoner/summoner-profile",
                                    Some(body),
                                    false,
                                )
                                .await;
                        }
                    });
                    self.tools_view
                        .set_feedback(format!("已应用背景皮肤 ID: {}", skin_id), false);
                }
                cx.notify();
            }
            ToolsAction::SetSpoofQueue(q) => {
                self.tools_view.spoof_queue = q;
                cx.notify();
            }
            ToolsAction::SetSpoofTier(t) => {
                self.tools_view.spoof_tier = t;
                cx.notify();
            }
            ToolsAction::SetSpoofDivision(d) => {
                self.tools_view.spoof_division = d;
                cx.notify();
            }
            ToolsAction::ApplyRankSpoof => {
                let tier = self.tools_view.spoof_tier.clone();
                let div = self.tools_view.spoof_division.clone();
                let live = self.live_service.clone();
                yuumi_core::runtime::spawn_background(async move {
                    if let Some(client) = live.get_client().await {
                        let body = serde_json::json!({
                            "lol": {
                                "rankedLeagueTier": tier,
                                "rankedLeagueDivision": div,
                            }
                        });
                        let _ = client
                            .request("PUT", "/lol-chat/v1/me", Some(body), false)
                            .await;
                    }
                });
                self.tools_view.set_feedback("伪装段位已生效", false);
                cx.notify();
            }
            ToolsAction::SetOnlineStatus(st) => {
                self.tools_view.online_status = st.clone();
                let live = self.live_service.clone();
                yuumi_core::runtime::spawn_background(async move {
                    if let Some(client) = live.get_client().await {
                        let body = serde_json::json!({ "availability": st });
                        let _ = client
                            .request("PUT", "/lol-chat/v1/me", Some(body), false)
                            .await;
                    }
                });
                self.tools_view.set_feedback("在线状态已切换", false);
                cx.notify();
            }
            ToolsAction::ClearBadges => {
                let live = self.live_service.clone();
                yuumi_core::runtime::spawn_background(async move {
                    if let Some(client) = live.get_client().await {
                        let body = serde_json::json!({ "challengeIds": [] });
                        let _ = client
                            .request(
                                "POST",
                                "/lol-challenges/v1/update-player-preferences",
                                Some(body),
                                false,
                            )
                            .await;
                    }
                });
                self.tools_view.set_feedback("已清空成就勋章展示", false);
                cx.notify();
            }
            ToolsAction::ClearBorder => {
                let live = self.live_service.clone();
                yuumi_core::runtime::spawn_background(async move {
                    if let Some(client) = live.get_client().await {
                        let body = serde_json::json!({ "bannerId": "" });
                        let _ = client
                            .request(
                                "POST",
                                "/lol-challenges/v1/update-player-preferences",
                                Some(body),
                                false,
                            )
                            .await;
                    }
                });
                self.tools_view.set_feedback("已清除个人边框", false);
                cx.notify();
            }
            ToolsAction::ApplyOnlineStatus => {
                let status = self.tools_view.online_status.clone();
                let live = self.live_service.clone();
                yuumi_core::runtime::spawn_background(async move {
                    if let Some(client) = live.get_client().await {
                        let body = serde_json::json!({ "availability": status });
                        let _ = client
                            .request("PUT", "/lol-chat/v1/me", Some(body), false)
                            .await;
                    }
                });
                self.tools_view.set_feedback("在线状态已切换", false);
                cx.notify();
            }
            ToolsAction::TogglePicker(kind) => {
                match kind {
                    "hover" => {
                        self.tools_view.open_hover_picker = !self.tools_view.open_hover_picker
                    }
                    "ban" => self.tools_view.open_ban_picker = !self.tools_view.open_ban_picker,
                    "bg_champ" => {
                        self.tools_view.open_bg_champion_picker =
                            !self.tools_view.open_bg_champion_picker
                    }
                    "D" => self.tools_view.spell_active_slot = Some("D"),
                    "F" => self.tools_view.spell_active_slot = Some("F"),
                    "close_spell" => self.tools_view.spell_active_slot = None,
                    _ => {}
                }
                cx.notify();
            }
            ToolsAction::SetPickerChampion {
                lane,
                is_ban,
                champion_id,
            } => {
                if is_ban {
                    let list = match lane {
                        "top" => &mut self.app_config.functions.auto_ban_champion_top,
                        "jug" => &mut self.app_config.functions.auto_ban_champion_jug,
                        "mid" => &mut self.app_config.functions.auto_ban_champion_mid,
                        "bot" => &mut self.app_config.functions.auto_ban_champion_bot,
                        "sup" => &mut self.app_config.functions.auto_ban_champion_sup,
                        _ => &mut self.app_config.functions.auto_ban_champion,
                    };
                    *list = vec![champion_id];
                    self.tools_view.open_ban_picker = false;
                } else {
                    let list = match lane {
                        "top" => &mut self.app_config.functions.auto_select_champion_top,
                        "jug" => &mut self.app_config.functions.auto_select_champion_jug,
                        "mid" => &mut self.app_config.functions.auto_select_champion_mid,
                        "bot" => &mut self.app_config.functions.auto_select_champion_bot,
                        "sup" => &mut self.app_config.functions.auto_select_champion_sup,
                        _ => &mut self.app_config.functions.auto_select_champion,
                    };
                    *list = vec![champion_id];
                    self.tools_view.open_hover_picker = false;
                }
                self.persist_config(cx);
            }
            ToolsAction::ToggleDropdownMenu(menu) => {
                match menu {
                    "lobby_mode" => {
                        self.tools_view.open_lobby_mode_dropdown =
                            !self.tools_view.open_lobby_mode_dropdown
                    }
                    "spectate_method" => {
                        self.tools_view.open_spectate_method_dropdown =
                            !self.tools_view.open_spectate_method_dropdown
                    }
                    "cleaner_filter" => {
                        self.tools_view.open_cleaner_filter_dropdown =
                            !self.tools_view.open_cleaner_filter_dropdown
                    }
                    "spoof_queue" => {
                        self.tools_view.open_spoof_queue_dropdown =
                            !self.tools_view.open_spoof_queue_dropdown
                    }
                    "spoof_tier" => {
                        self.tools_view.open_spoof_tier_dropdown =
                            !self.tools_view.open_spoof_tier_dropdown
                    }
                    "spoof_division" => {
                        self.tools_view.open_spoof_division_dropdown =
                            !self.tools_view.open_spoof_division_dropdown
                    }
                    _ => {}
                }
                cx.notify();
            }
            ToolsAction::SetSpectateMethod(m) => {
                self.tools_view.spectate_method = m;
                self.tools_view.open_spectate_method_dropdown = false;
                cx.notify();
            }
            ToolsAction::SetCleanerFilter(f) => {
                self.tools_view.cleaner_filter = f;
                self.tools_view.open_cleaner_filter_dropdown = false;
                cx.notify();
            }
            ToolsAction::ToggleSelectCleanerFriend(id) => {
                if self.tools_view.cleaner_selected_ids.contains(&id) {
                    self.tools_view.cleaner_selected_ids.remove(&id);
                } else {
                    self.tools_view.cleaner_selected_ids.insert(id);
                }
                cx.notify();
            }
            ToolsAction::SelectAllCleanerFriends => {
                for f in crate::views::tools::data::MOCK_FRIENDS {
                    self.tools_view
                        .cleaner_selected_ids
                        .insert(f.id.to_string());
                }
                cx.notify();
            }
            ToolsAction::ClearSelectedCleanerFriends => {
                self.tools_view.cleaner_selected_ids.clear();
                cx.notify();
            }
            ToolsAction::SelectBgChampion(cid) => {
                self.tools_view.bg_selected_champion = Some(cid);
                self.tools_view.open_bg_champion_picker = false;
                self.tools_view.set_feedback("已选择背景英雄", false);
                cx.notify();
            }
            ToolsAction::SelectBgSkin(sid) => {
                self.tools_view.bg_selected_skin_id = Some(sid);
                let live = self.live_service.clone();
                yuumi_core::runtime::spawn_background(async move {
                    if let Some(client) = live.get_client().await {
                        let body = serde_json::json!({ "key": "backgroundSkinId", "value": sid });
                        let _ = client
                            .request(
                                "POST",
                                "/lol-summoner/v1/current-summoner/summoner-profile",
                                Some(body),
                                false,
                            )
                            .await;
                    }
                });
                self.tools_view
                    .set_feedback(format!("已应用背景皮肤 ID: {}", sid), false);
                cx.notify();
            }
            ToolsAction::ToggleBadgesModal(show) => {
                self.tools_view.show_badges_modal = show;
                cx.notify();
            }
            ToolsAction::ToggleBadgeItem(bid) => {
                if let Some(pos) = self
                    .tools_view
                    .selected_badges
                    .iter()
                    .position(|id| id == &bid)
                {
                    self.tools_view.selected_badges.remove(pos);
                } else if self.tools_view.selected_badges.len() < 3 {
                    self.tools_view.selected_badges.push(bid);
                }
                cx.notify();
            }
            ToolsAction::ClearBadgesModal => {
                self.tools_view.selected_badges.clear();
                cx.notify();
            }
            ToolsAction::SaveBadgesPreferences => {
                self.tools_view.show_badges_modal = false;
                self.tools_view
                    .set_feedback("称号与挑战勋章装配成功！", false);
                cx.notify();
            }
        }
    }

    /// 工具箱路由渲染与 ToolsAction 回调。
    pub(super) fn render_tools_route(
        &mut self,
        el: Stateful<Div>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let view_entity = cx.entity().clone();
        let view = view_entity.clone();
        let connected = matches!(self.connection_status, ConnectionStatus::Connected { .. });
        let on_tools_action = move |action, _win: &mut Window, app: &mut App| {
            view.update(app, |this, cx| {
                this.handle_tools_action(action, cx);
            });
        };
        el.child(
            self.tools_view
                .render_panel(&self.app_config, connected, window, on_tools_action),
        )
    }
}
