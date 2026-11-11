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
                    "top" => {
                        self.automation.auto_bp.pick_champion_ids_top.clear();
                        self.app_config.functions.auto_select_champion_top.clear();
                    }
                    "jug" => {
                        self.automation.auto_bp.pick_champion_ids_jug.clear();
                        self.app_config.functions.auto_select_champion_jug.clear();
                    }
                    "mid" => {
                        self.automation.auto_bp.pick_champion_ids_mid.clear();
                        self.app_config.functions.auto_select_champion_mid.clear();
                    }
                    "bot" => {
                        self.automation.auto_bp.pick_champion_ids_bot.clear();
                        self.app_config.functions.auto_select_champion_bot.clear();
                    }
                    "sup" => {
                        self.automation.auto_bp.pick_champion_ids_sup.clear();
                        self.app_config.functions.auto_select_champion_sup.clear();
                    }
                    _ => {
                        self.automation.auto_bp.pick_champion_ids.clear();
                        self.app_config.functions.auto_select_champion.clear();
                    }
                }
                if self.tools_view.open_hover_picker {
                    // 选择器展开编辑中：作为临时修改等待「完成」确认才落盘
                    cx.notify();
                } else {
                    // 收起状态直接点清空：立即落盘保存
                    self.persist_config(cx);
                }
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
                    "top" => {
                        self.automation.auto_bp.ban_champion_ids_top.clear();
                        self.app_config.functions.auto_ban_champion_top.clear();
                    }
                    "jug" => {
                        self.automation.auto_bp.ban_champion_ids_jug.clear();
                        self.app_config.functions.auto_ban_champion_jug.clear();
                    }
                    "mid" => {
                        self.automation.auto_bp.ban_champion_ids_mid.clear();
                        self.app_config.functions.auto_ban_champion_mid.clear();
                    }
                    "bot" => {
                        self.automation.auto_bp.ban_champion_ids_bot.clear();
                        self.app_config.functions.auto_ban_champion_bot.clear();
                    }
                    "sup" => {
                        self.automation.auto_bp.ban_champion_ids_sup.clear();
                        self.app_config.functions.auto_ban_champion_sup.clear();
                    }
                    _ => {
                        self.automation.auto_bp.ban_champion_ids.clear();
                        self.app_config.functions.auto_ban_champion.clear();
                    }
                }
                if self.tools_view.open_ban_picker {
                    // 选择器展开编辑中：作为临时修改等待「完成」确认才落盘
                    cx.notify();
                } else {
                    // 收起状态直接点清空：立即落盘保存
                    self.persist_config(cx);
                }
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
                // [D07-a] 用户主动打开：清除本会话隐藏标志，允许再次显示
                crate::bench_overlay::reset_bench_user_hidden();
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
                let live = self.live_service.clone();
                cx.spawn(async move |_entity, _cx| {
                    live.reset_lobby_state().await;
                })
                .detach();
            }
            ToolsAction::SetDefaultGameMode(mode) => {
                self.app_config.functions.default_game_mode = mode;
                self.persist_config(cx);
                let live = self.live_service.clone();
                cx.spawn(async move |_entity, _cx| {
                    live.reset_lobby_state().await;
                })
                .detach();
            }
            ToolsAction::CreateLobbyNow => {
                let live = self.live_service.clone();
                let mode = self.app_config.functions.default_game_mode;
                self.tools_view.set_feedback("正在创建房间…", false);
                cx.notify();
                cx.spawn(async move |entity, cx| {
                    let result = match live.get_client().await {
                        Some(client) => {
                            let body = serde_json::json!({ "queueId": mode });
                            client
                                .request("POST", "/lol-lobby/v2/lobby", Some(body), false)
                                .await
                                .map(|_| "房间已创建".to_string())
                        }
                        None => Err("客户端未连接，创建房间未执行".to_string()),
                    };
                    let _ = entity.update(cx, |this, cx| {
                        match result {
                            Ok(msg) => this.toast(ToastKind::Success, msg, cx),
                            Err(e) => {
                                this.toast(ToastKind::Error, format!("创建房间失败: {}", e), cx)
                            }
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
            ToolsAction::ToggleLockGameSettings => {
                let paths = self.app_config.general.lol_path.clone();
                let target = !self.tools_view.is_game_settings_locked;
                match services::set_game_settings_readonly(&paths, target) {
                    Ok(msg) => {
                        // 回读真实属性，避免界面状态与磁盘状态漂移
                        self.tools_view.is_game_settings_locked =
                            services::game_settings_readonly_state(&paths);
                        self.toast(ToastKind::Success, msg, cx);
                    }
                    Err(e) => {
                        self.tools_view.is_game_settings_locked =
                            services::game_settings_readonly_state(&paths);
                        self.toast(
                            ToastKind::Error,
                            format!("修改游戏设置只读属性失败: {}", e),
                            cx,
                        );
                    }
                }
                cx.notify();
            }
            ToolsAction::FixClientWindow => {
                self.tools_view.set_feedback("正在修复客户端窗口…", false);
                cx.notify();
                cx.spawn(async move |entity, cx| {
                    let result = cx
                        .background_executor()
                        .spawn(async { services::fix_client_window() })
                        .await;
                    let _ = entity.update(cx, |this, cx| {
                        match result {
                            Ok(msg) => this.toast(ToastKind::Success, msg, cx),
                            Err(e) => this.toast(
                                ToastKind::Error,
                                format!("修复客户端窗口失败: {}", e),
                                cx,
                            ),
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
            ToolsAction::RestartClient => {
                self.tools_view.set_feedback("正在重启客户端…", false);
                cx.notify();
                let live = self.live_service.clone();
                cx.spawn(async move |entity, cx| {
                    let result = match live.get_client().await {
                        Some(client) => client
                            .request("POST", "/riotclient/kill-and-restart-ux", None, false)
                            .await
                            .map(|_| "已发送客户端重启指令".to_string()),
                        None => Err("客户端未连接，无法重启".to_string()),
                    };
                    let _ = entity.update(cx, |this, cx| {
                        match result {
                            Ok(msg) => this.toast(ToastKind::Success, msg, cx),
                            Err(e) => {
                                this.toast(ToastKind::Error, format!("重启客户端失败: {}", e), cx)
                            }
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
            ToolsAction::SpectateSummonerNameInput(name) => {
                self.tools_view.spectate_name_buffer = name;
                cx.notify();
            }
            ToolsAction::LaunchSpectate => {
                let name = self.tools_view.spectate_name_buffer.clone();
                if name.trim().is_empty() {
                    self.toast(ToastKind::Warning, "请输入召唤师名称", cx);
                    cx.notify();
                    return;
                }
                let method =
                    services::spectate::SpectateMethod::from_ui(&self.tools_view.spectate_method);
                let lol_path = self
                    .app_config
                    .general
                    .lol_path
                    .iter()
                    .find(|p| p.as_str() != services::WEGAME_MARKER)
                    .cloned();
                let live = self.live_service.clone();
                self.tools_view.set_feedback("正在启动观战…", false);
                cx.notify();
                cx.spawn(async move |entity, cx| {
                    let result = match live.get_client().await {
                        Some(client) => {
                            services::spectate::launch_spectate(
                                &client,
                                &name,
                                method,
                                lol_path.as_deref(),
                            )
                            .await
                        }
                        None => Err("客户端未连接，请先启动英雄联盟".to_string()),
                    };
                    let _ = entity.update(cx, |this, cx| {
                        match result {
                            Ok(msg) => this.toast(ToastKind::Success, msg, cx),
                            Err(e) => this.toast(ToastKind::Error, e, cx),
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
            ToolsAction::ScanFriends => {
                if self.tools_view.cleaner_scanning {
                    return;
                }
                self.tools_view.cleaner_scanning = true;
                self.tools_view.set_feedback("正在扫描好友列表…", false);
                cx.notify();
                let live = self.live_service.clone();
                cx.spawn(async move |entity, cx| {
                    let result = match live.get_client().await {
                        Some(client) => services::friend_cleaner::scan_friends(&client).await,
                        None => Err("客户端未连接，无法读取好友列表".to_string()),
                    };
                    let _ = entity.update(cx, |this, cx| {
                        this.tools_view.cleaner_scanning = false;
                        match result {
                            Ok(friends) => {
                                let count = friends.len();
                                this.tools_view.cleaner_friends = friends;
                                this.tools_view.cleaner_has_scanned = true;
                                this.tools_view.cleaner_selected_ids.clear();
                                this.toast(
                                    ToastKind::Success,
                                    format!("扫描完成，共获取 {} 位好友并完成战绩分析", count),
                                    cx,
                                );
                            }
                            Err(e) => {
                                this.tools_view.cleaner_has_scanned = false;
                                this.toast(ToastKind::Error, format!("扫描好友失败: {}", e), cx);
                            }
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
            ToolsAction::SetCleanerDays(d) => {
                self.tools_view.cleaner_days = d;
                cx.notify();
            }
            ToolsAction::BatchDeleteOfflineFriends => {
                let ids: Vec<String> = self
                    .tools_view
                    .cleaner_selected_ids
                    .iter()
                    .cloned()
                    .collect();
                if ids.is_empty() {
                    self.toast(ToastKind::Info, "未勾选任何好友，已取消删除", cx);
                    cx.notify();
                    return;
                }
                self.tools_view.set_feedback("正在删除选中的好友…", false);
                cx.notify();
                let live = self.live_service.clone();
                cx.spawn(async move |entity, cx| {
                    let outcome = match live.get_client().await {
                        Some(client) => {
                            services::friend_cleaner::delete_friends(&client, &ids).await
                        }
                        None => {
                            let _ = entity.update(cx, |this, cx| {
                                this.toast(ToastKind::Error, "客户端未连接，删除未执行", cx);
                                cx.notify();
                            });
                            return;
                        }
                    };
                    let _ = entity.update(cx, |this, cx| {
                        let deleted = outcome.deleted.len();
                        let failed = outcome.failed.len();
                        this.tools_view
                            .cleaner_friends
                            .retain(|friend| !outcome.deleted.contains(&friend.id));
                        this.tools_view.cleaner_selected_ids.clear();
                        if failed == 0 {
                            this.toast(
                                ToastKind::Success,
                                format!("成功清理 {} 位好友", deleted),
                                cx,
                            );
                        } else {
                            let first_error = outcome
                                .failed
                                .first()
                                .map(|(_, err)| err.clone())
                                .unwrap_or_default();
                            this.toast(
                                ToastKind::Error,
                                format!(
                                    "删除完成：成功 {} 位，失败 {} 位（{}）",
                                    deleted, failed, first_error
                                ),
                                cx,
                            );
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
            ToolsAction::SignatureInput(s) => {
                self.tools_view.signature_buffer = s;
                cx.notify();
            }
            ToolsAction::ApplySignature => {
                let sig = self.tools_view.signature_buffer.clone();
                let live = self.live_service.clone();
                self.tools_view.set_feedback("正在保存个性签名…", false);
                cx.notify();
                cx.spawn(async move |entity, cx| {
                    let result = match live.get_client().await {
                        Some(client) => {
                            let body = serde_json::json!({ "statusMessage": sig });
                            client
                                .request("PUT", "/lol-chat/v1/me", Some(body), false)
                                .await
                                .map(|_| "个性签名已更新".to_string())
                        }
                        None => Err("客户端未连接，签名未写入".to_string()),
                    };
                    let _ = entity.update(cx, |this, cx| {
                        match result {
                            Ok(msg) => this.toast(ToastKind::Success, msg, cx),
                            Err(e) => {
                                this.toast(ToastKind::Error, format!("保存签名失败: {}", e), cx)
                            }
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
            // D09：主页背景弹层指令通道（选英雄加载、上一个/下一个、取消、重试）
            // 全部只改预览态，零写入；仅 ApplyProfileBackground 才真正写 LCU。
            ToolsAction::ProfileBgSkinInput(cmd) => {
                match cmd.as_str() {
                    "open" => {
                        if self.tools_view.bg_skins.is_empty() {
                            self.toast(ToastKind::Warning, "请先选择英雄以加载真实皮肤列表", cx);
                        } else {
                            self.tools_view.open_bg_skin_modal = true;
                            self.tools_view.bg_error = None;
                        }
                    }
                    "cancel" | "close" => {
                        // 取消零写入：关闭弹层并把预览索引恢复到当前回读的背景
                        self.tools_view.open_bg_skin_modal = false;
                        if let Some(cur) = self.tools_view.bg_current_skin_id {
                            if let Some(pos) =
                                self.tools_view.bg_skins.iter().position(|s| s.id == cur)
                            {
                                self.tools_view.bg_preview_index = pos;
                            }
                        }
                        self.tools_view.bg_error = None;
                    }
                    "prev" => {
                        let n = self.tools_view.bg_skins.len();
                        if n > 0 {
                            self.tools_view.bg_preview_index =
                                (self.tools_view.bg_preview_index + n - 1) % n;
                        }
                    }
                    "next" => {
                        let n = self.tools_view.bg_skins.len();
                        if n > 0 {
                            self.tools_view.bg_preview_index =
                                (self.tools_view.bg_preview_index + 1) % n;
                        }
                    }
                    "retry" => {
                        if let Some(cid) = self.tools_view.bg_selected_champion {
                            self.spawn_skins_load(cid, cx);
                        }
                    }
                    _ => {
                        // 兼容原义：纯文本只进输入缓存，不写 LCU
                        self.tools_view.profile_skin_id_buffer = cmd;
                    }
                }
                cx.notify();
            }
            ToolsAction::ApplyProfileBackground => {
                // 确认才写入：取预览态当前皮肤 ID，写入并回读，按真实结果反馈
                let preview_idx = self
                    .tools_view
                    .bg_preview_index
                    .min(self.tools_view.bg_skins.len().saturating_sub(1));
                let skin_id = match self.tools_view.bg_skins.get(preview_idx) {
                    Some(s) => s.id,
                    None => {
                        self.toast(ToastKind::Warning, "没有可应用的皮肤，请先选择英雄", cx);
                        cx.notify();
                        return;
                    }
                };
                let live = self.live_service.clone();
                self.tools_view.set_feedback("正在应用主页背景…", false);
                cx.notify();
                cx.spawn(async move |entity, cx| {
                    let result = match live.get_client().await {
                        Some(client) => {
                            crate::services::profile_background::apply_skin(&client, skin_id).await
                        }
                        None => Err("客户端未连接，背景未写入".to_string()),
                    };
                    let _ = entity.update(cx, |this, cx| {
                        match result {
                            Ok(readback) => {
                                this.tools_view.bg_current_skin_id = readback;
                                this.tools_view.bg_error = None;
                                this.tools_view.open_bg_skin_modal = false;
                                this.toast(
                                    ToastKind::Success,
                                    format!("已应用背景皮肤 ID: {}（已回读）", skin_id),
                                    cx,
                                );
                            }
                            Err(e) => {
                                // 失败保留预览弹层与选择，便于直接重试
                                this.tools_view.bg_error = Some(e.clone());
                                this.toast(
                                    ToastKind::Error,
                                    format!("应用背景皮肤失败: {}", e),
                                    cx,
                                );
                            }
                        }
                        cx.notify();
                    });
                })
                .detach();
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
                let queue = self.tools_view.spoof_queue.clone();
                let tier = self.tools_view.spoof_tier.clone();
                let div = self.tools_view.spoof_division.clone();
                let live = self.live_service.clone();
                self.tools_view.set_feedback("正在应用段位展示…", false);
                cx.notify();
                cx.spawn(async move |entity, cx| {
                    let result = match live.get_client().await {
                        Some(client) => {
                            // 对齐旧版 RankSpoofCard：队列 / 段位 / 小段一并提交
                            let body = serde_json::json!({
                                "lol": {
                                    "rankedLeagueQueue": queue,
                                    "rankedLeagueTier": tier,
                                    "rankedLeagueDivision": div,
                                }
                            });
                            client
                                .request("PUT", "/lol-chat/v1/me", Some(body), false)
                                .await
                                .map(|_| ())
                        }
                        None => Err("客户端未连接，段位未写入".to_string()),
                    };
                    let _ = entity.update(cx, |this, cx| {
                        match result {
                            Ok(()) => this.toast(ToastKind::Success, "伪装段位已生效", cx),
                            Err(e) => {
                                this.toast(ToastKind::Error, format!("伪装段位失败: {}", e), cx)
                            }
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
            ToolsAction::SetOnlineStatus(st) => {
                self.tools_view.online_status = st.clone();
                let live = self.live_service.clone();
                self.tools_view.set_feedback("正在切换在线状态…", false);
                cx.notify();
                cx.spawn(async move |entity, cx| {
                    let result = match live.get_client().await {
                        Some(client) => {
                            let body = serde_json::json!({ "availability": st });
                            client
                                .request("PUT", "/lol-chat/v1/me", Some(body), false)
                                .await
                                .map(|_| "在线状态已切换".to_string())
                        }
                        None => Err("客户端未连接，状态未写入".to_string()),
                    };
                    let _ = entity.update(cx, |this, cx| {
                        match result {
                            Ok(msg) => this.toast(ToastKind::Success, msg, cx),
                            // 按真实请求结果反馈，不再提前报成功
                            Err(e) => {
                                this.toast(ToastKind::Error, format!("切换在线状态失败: {}", e), cx)
                            }
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
            // D11-e：首次点击只展开二次确认框（零写入），确认后才发真实请求
            ToolsAction::ClearBadges => {
                if !self.tools_view.confirm_clear_badges {
                    self.tools_view.confirm_clear_badges = true;
                    cx.notify();
                    return;
                }
                self.tools_view.confirm_clear_badges = false;
                let live = self.live_service.clone();
                self.tools_view.set_feedback("正在清除成就勋章…", false);
                cx.notify();
                cx.spawn(async move |entity, cx| {
                    // 只清 challengeIds，请求体带当前 title / bannerAccent 保护称号与头像框
                    let result = match live.get_client().await {
                        Some(client) => crate::services::challenges::clear_badges(&client)
                            .await
                            .map(|_| "已清空成就勋章展示".to_string()),
                        None => Err("客户端未连接，清除未执行".to_string()),
                    };
                    let _ = entity.update(cx, |this, cx| {
                        match result {
                            Ok(msg) => this.toast(ToastKind::Success, msg, cx),
                            Err(e) => {
                                this.toast(ToastKind::Error, format!("清除勋章失败: {}", e), cx)
                            }
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
            ToolsAction::ClearBorder => {
                if !self.tools_view.confirm_clear_border {
                    self.tools_view.confirm_clear_border = true;
                    cx.notify();
                    return;
                }
                self.tools_view.confirm_clear_border = false;
                let live = self.live_service.clone();
                self.tools_view.set_feedback("正在清除个人边框…", false);
                cx.notify();
                cx.spawn(async move |entity, cx| {
                    // 对齐旧版 ClearBorderCard.vue：PUT /lol-regalia/v2/current-regalia，
                    // 仅清边框，不影响挑战勋章与称号（该前缀已在白名单内扩充）
                    let result = match live.get_client().await {
                        Some(client) => crate::services::challenges::clear_border(&client)
                            .await
                            .map(|_| "已清除个人头像边框".to_string()),
                        None => Err("客户端未连接，清除未执行".to_string()),
                    };
                    let _ = entity.update(cx, |this, cx| {
                        match result {
                            Ok(msg) => this.toast(ToastKind::Success, msg, cx),
                            Err(e) => {
                                this.toast(ToastKind::Error, format!("清除边框失败: {}", e), cx)
                            }
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
            ToolsAction::ApplyOnlineStatus => {
                // [D10-c] 收敛为单一写入路径：本分支无 UI 触发，且曾与 SetOnlineStatus 重复写
                // PUT /lol-chat/v1/me {availability}（丢弃结果、提前报成功）。保留该分支
                // （既有死代码不删），委托给 SetOnlineStatus 的活路径，避免同端点重复写入。
                let status = self.tools_view.online_status.clone();
                self.handle_tools_action(ToolsAction::SetOnlineStatus(status), cx);
            }
            ToolsAction::TogglePicker(kind) => {
                match kind {
                    "hover" => {
                        // [D06-d] 打开前保存 FunctionsConfig 快照，供取消零写入回滚
                        if !self.tools_view.open_hover_picker {
                            self.tools_view.picker_backup =
                                Some(Box::new(self.app_config.functions.clone()));
                        }
                        self.tools_view.open_hover_picker = !self.tools_view.open_hover_picker
                    }
                    "ban" => {
                        if !self.tools_view.open_ban_picker {
                            self.tools_view.picker_backup =
                                Some(Box::new(self.app_config.functions.clone()));
                        }
                        self.tools_view.open_ban_picker = !self.tools_view.open_ban_picker;
                    }
                    // [D06-c] 「完成」才落盘，清空临时快照
                    "confirm_hover" => {
                        self.tools_view.open_hover_picker = false;
                        self.tools_view.picker_backup = None;
                        self.persist_config(cx);
                    }
                    "confirm_ban" => {
                        self.tools_view.open_ban_picker = false;
                        self.tools_view.picker_backup = None;
                        self.persist_config(cx);
                    }
                    // [D06-d] 「✕ / 收起」为取消：恢复打开前配置，零写入
                    "cancel_hover" => {
                        if let Some(backup) = self.tools_view.picker_backup.take() {
                            self.app_config.functions = *backup;
                            self.automation = self.app_config.to_automation_settings();
                        }
                        self.tools_view.open_hover_picker = false;
                    }
                    "cancel_ban" => {
                        if let Some(backup) = self.tools_view.picker_backup.take() {
                            self.app_config.functions = *backup;
                            self.automation = self.app_config.to_automation_settings();
                        }
                        self.tools_view.open_ban_picker = false;
                    }
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
                // [D05-a][D06-b] 以运行时 AutomationSettings 为权威修改候选，再同步回 AppConfig
                {
                    let list: &mut Vec<i32> = match (is_ban, lane) {
                        (true, "top") => &mut self.automation.auto_bp.ban_champion_ids_top,
                        (true, "jug") => &mut self.automation.auto_bp.ban_champion_ids_jug,
                        (true, "mid") => &mut self.automation.auto_bp.ban_champion_ids_mid,
                        (true, "bot") => &mut self.automation.auto_bp.ban_champion_ids_bot,
                        (true, "sup") => &mut self.automation.auto_bp.ban_champion_ids_sup,
                        (true, _) => &mut self.automation.auto_bp.ban_champion_ids,
                        (false, "top") => &mut self.automation.auto_bp.pick_champion_ids_top,
                        (false, "jug") => &mut self.automation.auto_bp.pick_champion_ids_jug,
                        (false, "mid") => &mut self.automation.auto_bp.pick_champion_ids_mid,
                        (false, "bot") => &mut self.automation.auto_bp.pick_champion_ids_bot,
                        (false, "sup") => &mut self.automation.auto_bp.pick_champion_ids_sup,
                        (false, _) => &mut self.automation.auto_bp.pick_champion_ids,
                    };
                    // [D06-b] Vue ChampionPicker 语义：点已选移除、未达上限追加、达上限忽略
                    if let Some(pos) = list.iter().position(|id| *id == champion_id) {
                        list.remove(pos);
                    } else if list.len() < crate::views::tools::components::PICKER_MAX_COUNT {
                        list.push(champion_id);
                    }
                }
                // [D05-a] sync_automation_settings 的真实消费点：运行时配置回写磁盘配置模型
                self.app_config.sync_automation_settings(&self.automation);
                // [D06-c] 选择不落盘，等待「完成」确认；面板保持展开以便继续多选
                cx.notify();
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
                // 只勾选当前筛选 + 搜索命中的好友（对齐 Vue `selectAllFiltered`）
                let now = services::friend_cleaner::now_ms();
                let query = self.tools_view.cleaner_search_live.borrow().clone();
                let ids: Vec<String> = services::friend_cleaner::filtered_friends(
                    &self.tools_view.cleaner_friends,
                    &self.tools_view.cleaner_filter,
                    &query,
                    now,
                )
                .into_iter()
                .map(|friend| friend.id.clone())
                .collect();
                self.tools_view.cleaner_selected_ids.extend(ids);
                cx.notify();
            }
            ToolsAction::ClearSelectedCleanerFriends => {
                self.tools_view.cleaner_selected_ids.clear();
                cx.notify();
            }
            // D09-c：选英雄自动拉真实皮肤列表并回读当前背景，零写入
            ToolsAction::SelectBgChampion(cid) => {
                self.tools_view.bg_selected_champion = Some(cid);
                self.tools_view.open_bg_champion_picker = false;
                self.tools_view.open_bg_skin_modal = false;
                self.spawn_skins_load(cid, cx);
            }
            // D09-b：缩略图点击只改预览态，消除「选皮肤即写」
            ToolsAction::SelectBgSkin(sid) => {
                self.tools_view.bg_selected_skin_id = Some(sid);
                if let Some(pos) = self.tools_view.bg_skins.iter().position(|s| s.id == sid) {
                    self.tools_view.bg_preview_index = pos;
                }
                cx.notify();
            }
            // D11-b/e：打开弹层回读真实称号/勋章与当前偏好并快照；取消回滚零写入
            ToolsAction::ToggleBadgesModal(show) => {
                if show {
                    self.tools_view.badges_snapshot_badges =
                        self.tools_view.selected_badges.clone();
                    self.tools_view.badges_snapshot_title = self.tools_view.selected_title.clone();
                    self.tools_view.show_badges_modal = true;
                    crate::views::tools::personalization::update_badges_view_state(|s| {
                        s.badge_search.clear();
                        s.title_search.clear();
                        s.tier_filter = "all".to_string();
                        s.error = None;
                    });
                    self.spawn_badges_load(cx);
                } else {
                    // 取消：回滚临时选择，零写入 LCU
                    self.tools_view.selected_badges =
                        self.tools_view.badges_snapshot_badges.clone();
                    self.tools_view.selected_title = self.tools_view.badges_snapshot_title.clone();
                    self.tools_view.show_badges_modal = false;
                    crate::views::tools::personalization::update_badges_view_state(|s| {
                        s.loading = false;
                        s.error = None;
                    });
                    cx.notify();
                }
            }
            // D11：弹层通用指令通道（badges_modal 的 on_toggle_badge 与确认框共用）
            // "<id>" / "remove:idx" / "title:id" / "search:text" / "titlesearch:text"
            // "filter:value" / "retry" / "cancel-clear"
            ToolsAction::ToggleBadgeItem(cmd) => {
                if let Some(rest) = cmd.strip_prefix("search:") {
                    let text = rest.to_string();
                    crate::views::tools::personalization::update_badges_view_state(|s| {
                        s.badge_search = text;
                    });
                } else if let Some(rest) = cmd.strip_prefix("titlesearch:") {
                    let text = rest.to_string();
                    crate::views::tools::personalization::update_badges_view_state(|s| {
                        s.title_search = text;
                    });
                } else if let Some(rest) = cmd.strip_prefix("filter:") {
                    let value = rest.to_string();
                    crate::views::tools::personalization::update_badges_view_state(|s| {
                        s.tier_filter = value;
                    });
                } else if let Some(rest) = cmd.strip_prefix("title:") {
                    self.tools_view.selected_title = rest.to_string();
                } else if let Some(rest) = cmd.strip_prefix("remove:") {
                    if let Ok(idx) = rest.parse::<usize>() {
                        if idx < self.tools_view.selected_badges.len() {
                            self.tools_view.selected_badges.remove(idx);
                        }
                    }
                } else if cmd == "cancel-clear" {
                    self.tools_view.confirm_clear_badges = false;
                    self.tools_view.confirm_clear_border = false;
                } else if cmd == "retry" {
                    self.spawn_badges_load(cx);
                } else {
                    // 纯勋章 ID：切换装配（最多 3 枚）
                    if let Some(pos) = self
                        .tools_view
                        .selected_badges
                        .iter()
                        .position(|id| id == &cmd)
                    {
                        self.tools_view.selected_badges.remove(pos);
                    } else if self.tools_view.selected_badges.len() < 3 {
                        self.tools_view.selected_badges.push(cmd);
                    } else {
                        // 对照 Vue G13：最多佩戴 3 枚挑战勋章
                        self.toast(ToastKind::Warning, "最多只能佩戴 3 枚挑战勋章", cx);
                    }
                }
                cx.notify();
            }
            ToolsAction::ClearBadgesModal => {
                self.tools_view.selected_badges.clear();
                self.tools_view.selected_title.clear();
                cx.notify();
            }
            ToolsAction::SaveBadgesPreferences => {
                let title = self.tools_view.selected_title.clone();
                let challenge_ids = self.tools_view.selected_badges.clone();
                let live = self.live_service.clone();
                self.tools_view
                    .set_feedback("正在保存称号与勋章装配…", false);
                cx.notify();
                cx.spawn(async move |entity, cx| {
                    // 真实写入 + 回读当前偏好，成功才关弹层，失败保留选择与弹层便于重试
                    let result: Result<(String, Vec<String>), String> =
                        match live.get_client().await {
                            Some(client) => {
                                match crate::services::challenges::save_preferences(
                                    &client,
                                    &title,
                                    &challenge_ids,
                                )
                                .await
                                {
                                    Ok(()) => {
                                        crate::services::challenges::read_preferences(&client).await
                                    }
                                    Err(e) => Err(e),
                                }
                            }
                            None => Err("客户端未连接，装配未写入".to_string()),
                        };
                    let _ = entity.update(cx, |this, cx| {
                        match result {
                            Ok((read_title, read_ids)) => {
                                this.tools_view.selected_title = read_title;
                                this.tools_view.selected_badges = read_ids;
                                this.tools_view.badges_snapshot_title =
                                    this.tools_view.selected_title.clone();
                                this.tools_view.badges_snapshot_badges =
                                    this.tools_view.selected_badges.clone();
                                this.tools_view.show_badges_modal = false;
                                this.toast(
                                    ToastKind::Success,
                                    "称号与挑战勋章装配成功（已回读）",
                                    cx,
                                );
                            }
                            Err(e) => {
                                crate::views::tools::personalization::update_badges_view_state(
                                    |s| {
                                        s.error = Some(e.clone());
                                    },
                                );
                                this.toast(ToastKind::Error, format!("装配失败: {}", e), cx);
                            }
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
        }
    }

    /// D09：选英雄后异步读取真实皮肤列表 + 回读当前主页背景，写回视图（零写入）
    fn spawn_skins_load(&mut self, champion_id: i32, cx: &mut Context<Self>) {
        let live = self.live_service.clone();
        self.tools_view.bg_loading = true;
        self.tools_view.bg_error = None;
        cx.notify();
        cx.spawn(async move |entity, cx| {
            let result = match live.get_client().await {
                Some(client) => {
                    let skins =
                        crate::services::profile_background::load_skins(&client, champion_id).await;
                    let current =
                        crate::services::profile_background::read_current_background(&client).await;
                    match (skins, current) {
                        (Ok(list), Ok(cur)) => Ok((list, cur)),
                        (Err(e), _) | (_, Err(e)) => Err(e),
                    }
                }
                None => Err("客户端未连接，无法读取皮肤列表".to_string()),
            };
            let _ = entity.update(cx, |this, cx| {
                this.tools_view.bg_loading = false;
                match result {
                    Ok((list, cur)) => {
                        this.tools_view.bg_current_skin_id = cur;
                        this.tools_view.bg_preview_index = cur
                            .and_then(|id| list.iter().position(|s| s.id == id))
                            .unwrap_or(0);
                        this.tools_view.bg_skins = list;
                        this.tools_view.bg_error = None;
                    }
                    Err(e) => {
                        this.tools_view.bg_skins.clear();
                        this.tools_view.bg_error = Some(e);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// D11：打开弹层 / 重试时异步读取真实称号、勋章与当前偏好，写回视图与弹层状态
    fn spawn_badges_load(&mut self, cx: &mut Context<Self>) {
        let live = self.live_service.clone();
        crate::views::tools::personalization::update_badges_view_state(|s| {
            s.loading = true;
            s.error = None;
        });
        cx.notify();
        cx.spawn(async move |entity, cx| {
            let result = match live.get_client().await {
                Some(client) => crate::services::challenges::load(&client).await,
                None => Err("客户端未连接，无法读取称号与勋章".to_string()),
            };
            let _ = entity.update(cx, |this, cx| {
                match result {
                    Ok(data) => {
                        crate::views::tools::personalization::update_badges_view_state(|s| {
                            s.owned_badges = data.badges;
                            s.owned_titles = data.titles;
                            s.loading = false;
                            s.error = None;
                        });
                        // 回读的真实当前佩戴覆盖临时选择（不重置打开时的取消快照）
                        this.tools_view.selected_badges = data.current_badges;
                        this.tools_view.selected_title = data.current_title;
                    }
                    Err(e) => {
                        crate::views::tools::personalization::update_badges_view_state(|s| {
                            s.loading = false;
                            s.error = Some(e.clone());
                        });
                        // 对照 Vue G12：称号与勋章数据加载失败以 Toast 呈现
                        this.toast(ToastKind::Error, e, cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
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
