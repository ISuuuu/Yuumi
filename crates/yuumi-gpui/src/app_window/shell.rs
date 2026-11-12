// 主窗渲染入口：标题栏、侧栏、路由分派、回到顶部、公告弹窗与 OP.GG 提示。
use super::*;

impl Render for MainWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _is_overlay_open = self.overlay_window.is_some();
        // 记录主窗所在区域：OP.GG 独立窗按主窗所在屏幕定位（对照 Vue 跟随主窗口）
        self.main_window_bounds = Some(window.bounds());
        let connected = matches!(self.connection_status, ConnectionStatus::Connected { .. });
        let phase = if connected {
            phase_label(self.current_phase.as_str())
        } else {
            None
        };
        let is_active_game = connected
            && matches!(
                self.current_phase.as_str(),
                "ChampSelect" | "GameStart" | "InProgress" | "Reconnect"
            );
        let map_side_label = if is_active_game {
            if let Some(info) = &self.game_info {
                match info.user_side {
                    Some(yuumi_core::gameflow::team_builder::MapSide::Blue) => {
                        Some(crate::i18n::t("titlebar.blueSide"))
                    }
                    Some(yuumi_core::gameflow::team_builder::MapSide::Red) => {
                        Some(crate::i18n::t("titlebar.redSide"))
                    }
                    _ => None,
                }
            } else {
                None
            }
        } else {
            None
        };
        let show_back = !self.route_history.is_empty();
        let view_entity = cx.entity().clone();

        // 主视口：首页、路人集、生涯、对局信息、查询各自内部独立滚动，
        // 仅 Tools 与 Settings 启用全局外层滚动
        let viewport = div()
            .id("main-scroll-view")
            .flex_1()
            .h_full()
            .min_w(px(0.0))
            .track_scroll(&self.main_scroll_handle)
            .when(
                matches!(self.current_route, Route::Tools | Route::Settings),
                |el| el.overflow_y_scroll(),
            );

        let viewport = match self.current_route {
            Route::Home => self.render_home_route(viewport, window, cx),
            Route::Career => self.render_career_route(viewport, window, cx),
            Route::GameInfo => self.render_game_info_route(viewport, window, cx),
            Route::Search => self.render_search_route(viewport, window, cx),
            Route::SavedPlayers => self.render_saved_players_route(viewport, window, cx),
            Route::Tools => self.render_tools_route(viewport, window, cx),
            Route::Settings => self.render_settings_route(viewport, window, cx),
            Route::Tft => self.render_tft_route(viewport, window, cx),
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(if self.mica_active {
                Theme::BG_APP().opacity(0.75)
            } else {
                Theme::BG_APP()
            })
            .font_family(Theme::font_family())
            // 1. 无边框自定义标题栏（Logo + Yummi + 阶段徽章 + 窗口按钮）
            .child(self.titlebar.render(
                show_back,
                phase,
                map_side_label,
                window,
                {
                    let entity = view_entity.clone();
                    move |_w, app| {
                        entity.update(app, |this, cx| this.go_back(cx));
                    }
                },
                |w, _app| {
                    w.minimize_window();
                },
                {
                    let entity = view_entity.clone();
                    move |w, app| {
                        let close_to_tray = entity.update(app, |this, _cx| {
                            this.app_config
                                .general
                                .enable_close_to_tray
                                .unwrap_or(false)
                        });
                        if close_to_tray {
                            if let Some(hwnd) = platform::find_main_window_hwnd() {
                                platform::hide_window(hwnd);
                            } else {
                                w.minimize_window();
                            }
                        } else {
                            entity.update(app, |this, cx| {
                                this.quit_application(cx);
                            });
                        }
                    }
                },
            ))
            // 2. 主体区域：64/210 导航侧栏 + 主路由视口
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .overflow_hidden()
                    .child(Sidebar::render(
                        self.current_route,
                        self.sidebar_expanded,
                        &self.app_config,
                        // 名片固定展示登录账号信息：查看他人生涯时不得跟随切换
                        self.own_summoner.as_ref(),
                        "",
                        self.has_update,
                        {
                            let entity = view_entity.clone();
                            move |route, _window, app| {
                                entity.update(app, |this, cx| {
                                    this.switch_route(route, cx);
                                });
                            }
                        },
                        {
                            let entity = view_entity.clone();
                            move |_window, app| {
                                entity.update(app, |this, cx| {
                                    this.toggle_sidebar(cx);
                                });
                            }
                        },
                        {
                            let entity = view_entity.clone();
                            move |_window, app| {
                                entity.update(app, |this, cx| {
                                    this.open_opgg_window(cx);
                                });
                            }
                        },
                        {
                            let entity = view_entity.clone();
                            move |_window, app| {
                                entity.update(app, |this, cx| {
                                    this.trigger_reconnect(cx);
                                });
                            }
                        },
                        {
                            let entity = view_entity.clone();
                            move |_window, app| {
                                entity.update(app, |this, cx| {
                                    this.toggle_notice_popup(true, cx);
                                });
                            }
                        },
                        {
                            let entity = view_entity.clone();
                            move |_window, app| {
                                entity.update(app, |this, cx| {
                                    this.back_to_my_career(cx);
                                    this.switch_route(Route::Career, cx);
                                });
                            }
                        },
                    ))
                    .child(viewport)
                    // 右下角回到顶部悬浮按钮
                    .when(self.current_route != Route::Home, |this| {
                        let v = view_entity.clone();
                        let scroll_y = match self.current_route {
                            Route::SavedPlayers => {
                                -self.saved_players_view.scroll_handle.offset().y
                            }
                            Route::Career => match self.career_view.active_tab {
                                views::career::CareerTab::Matches => {
                                    -self.career_view.matches_scroll_handle.offset().y
                                }
                                views::career::CareerTab::Loot => {
                                    -self.career_view.loot_scroll_handle.offset().y
                                }
                                views::career::CareerTab::Mastery => {
                                    -self.career_view.mastery_scroll_handle.offset().y
                                }
                            },
                            // [P1-2] TFT 页各页签内部滚动，取当前激活页签的句柄
                            Route::Tft => -self.tft_view.active_scroll_handle().offset().y,
                            _ => -self.main_scroll_handle.offset().y,
                        };
                        let show_btn = scroll_y > px(180.0);
                        this.when(show_btn, |panel| {
                            panel.child(
                                div()
                                    .absolute()
                                    .bottom(px(32.0))
                                    .right(px(32.0))
                                    .size(px(40.0))
                                    .rounded_full()
                                    .bg(Theme::CARD_BG())
                                    .border_1()
                                    .border_color(Theme::AURORA_CYAN())
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .hover(|s| {
                                        s.bg(Theme::CARD_BG_HOVER())
                                            .border_color(Theme::ACCENT_PRIMARY())
                                    })
                                    .on_mouse_down(MouseButton::Left, move |_ev, _win, app| {
                                        v.update(app, |this, cx| {
                                            match this.current_route {
                                                Route::SavedPlayers => {
                                                    this.saved_players_view
                                                        .scroll_handle
                                                        .set_offset(Point::default());
                                                }
                                                Route::Career => {
                                                    match this.career_view.active_tab {
                                                        views::career::CareerTab::Matches => {
                                                            this.career_view
                                                                .matches_scroll_handle
                                                                .set_offset(Point::default());
                                                        }
                                                        views::career::CareerTab::Loot => {
                                                            this.career_view
                                                                .loot_scroll_handle
                                                                .set_offset(Point::default());
                                                        }
                                                        views::career::CareerTab::Mastery => {
                                                            this.career_view
                                                                .mastery_scroll_handle
                                                                .set_offset(Point::default());
                                                        }
                                                    }
                                                }
                                                // [P1-2] TFT 页签内部滚动复位
                                                Route::Tft => {
                                                    this.tft_view
                                                        .active_scroll_handle()
                                                        .set_offset(Point::default());
                                                }
                                                _ => {
                                                    this.main_scroll_handle
                                                        .set_offset(Point::default());
                                                }
                                            }
                                            cx.notify();
                                        });
                                    })
                                    .child(
                                        div()
                                            .text_base()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(Theme::ACCENT_PRIMARY())
                                            .child("↑"),
                                    ),
                            )
                        })
                    })
                    // ── 页面模态弹窗（脱离内部滚动视口，覆盖可见工作区居中展示） ──
                    .when(
                        self.current_route == Route::Tools && self.tools_view.show_badges_modal,
                        |this| {
                            let v_close = view_entity.clone();
                            let v_toggle = view_entity.clone();
                            let v_clear = view_entity.clone();
                            let v_save = view_entity.clone();
                            this.child(crate::views::tools::components::badges_modal(
                                &self.tools_view.selected_badges,
                                &self.tools_view.selected_title,
                                move |_ev, _win, app| {
                                    v_close.update(app, |this, cx| {
                                        this.handle_tools_action(
                                            ToolsAction::ToggleBadgesModal(false),
                                            cx,
                                        );
                                    });
                                },
                                move |bid, _win, app| {
                                    v_toggle.update(app, |this, cx| {
                                        this.handle_tools_action(
                                            ToolsAction::ToggleBadgeItem(bid),
                                            cx,
                                        );
                                    });
                                },
                                move |_ev, _win, app| {
                                    v_clear.update(app, |this, cx| {
                                        this.handle_tools_action(ToolsAction::ClearBadgesModal, cx);
                                    });
                                },
                                move |_ev, _win, app| {
                                    v_save.update(app, |this, cx| {
                                        this.handle_tools_action(
                                            ToolsAction::SaveBadgesPreferences,
                                            cx,
                                        );
                                    });
                                },
                            ))
                        },
                    )
                    .when(
                        self.current_route == Route::Settings && self.settings_view.show_changelog,
                        |this| {
                            let v = view_entity.clone();
                            this.child(self.settings_view.render_changelog_modal(
                                move |action, win, app| {
                                    v.update(app, |this, cx| {
                                        this.handle_settings_action(action, win, cx);
                                    });
                                },
                            ))
                        },
                    ),
            )
            // 3. 右下角公告通知弹窗 (NoticePopup)
            .when(self.notice_popup_open, |this| {
                let v = view_entity.clone();
                let v_retry = view_entity.clone();
                let content_str = self.notice_content.as_deref().unwrap_or("");
                this.child(
                    div()
                        .absolute()
                        .right(px(20.0))
                        .bottom(px(20.0))
                        .child(notice_popup(
                            crate::i18n::t("noticePage.title"),
                            self.notice_loading,
                            self.notice_error,
                            content_str,
                            move |_win, app| {
                                v.update(app, |this, cx| {
                                    this.toggle_notice_popup(false, cx);
                                });
                            },
                            move |_win, app| {
                                v_retry.update(app, |this, cx| {
                                    this.fetch_notice(cx);
                                    this.schedule_notice_dismiss(cx);
                                });
                            },
                        )),
                )
            })
            .child(self.render_update_surface(&view_entity))
            // 4. 窗口级 Toast 浮层（最后绘制，z 序高于页面与应用内弹窗；不拦截鼠标）
            .child(components::toast::toast_overlay(self.toast_stack.read(cx)))
    }
}

impl MainWindow {
    pub(super) fn toggle_notice_popup(&mut self, open: bool, cx: &mut Context<Self>) {
        self.notice_popup_open = open;
        if open {
            if self.notice_content.is_none() && !self.notice_loading {
                self.fetch_notice(cx);
            }
            self.schedule_notice_dismiss(cx);
        } else {
            self.notice_dismiss_epoch = self.notice_dismiss_epoch.wrapping_add(1);
        }
        cx.notify();
    }

    /// 启动公告弹窗自动关闭倒计时（对照 Vue AUTO_DISMISS_MS = 8000：8 秒后平滑自动隐藏）
    pub(super) fn schedule_notice_dismiss(&mut self, cx: &mut Context<Self>) {
        self.notice_dismiss_epoch = self.notice_dismiss_epoch.wrapping_add(1);
        let epoch = self.notice_dismiss_epoch;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(8)).await;
            let _ = this.update(cx, |this, cx| {
                if this.notice_dismiss_epoch == epoch && this.notice_popup_open {
                    this.toggle_notice_popup(false, cx);
                }
            });
        })
        .detach();
    }

    pub(super) fn switch_route(&mut self, route: Route, cx: &mut Context<Self>) {
        if route != self.current_route {
            self.route_history.push(self.current_route);
            if self.route_history.len() > 20 {
                self.route_history.remove(0);
            }
            self.current_route = route;
            self.main_scroll_handle.set_offset(Point::default());
            if self.current_route == Route::SavedPlayers {
                self.reload_saved_players(cx);
                // 对照 Vue 版 refreshRoster：每次进入路人集都补齐缺失的召唤师身份
                self.backfill_saved_players_identity(cx);
            } else if self.current_route == Route::Home {
                self.sync_home_paths();
            } else if self.current_route == Route::Tft {
                self.ensure_tft_data(cx);
            }
        }
        cx.notify();
    }

    /// 返回上一路由（标题栏返回键）
    pub(super) fn go_back(&mut self, cx: &mut Context<Self>) {
        if let Some(previous) = self.route_history.pop() {
            self.current_route = previous;
            cx.notify();
        }
    }

    /// 折叠 / 展开侧栏
    pub(super) fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_expanded = !self.sidebar_expanded;
        cx.notify();
    }

    /// 首页路由渲染：客户端路径选择与启动。
    pub(super) fn render_home_route(
        &mut self,
        el: Stateful<Div>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let view_entity = cx.entity().clone();
        let view_select = view_entity.clone();
        let view_launch = view_entity.clone();
        el.child(self.home_view.render(
            &self.connection_status,
            move |path, _win, app| {
                view_select.update(app, |this, cx| {
                    this.home_view.selected_path = Some(path);
                    cx.notify();
                });
            },
            move |_win, app| {
                view_launch.update(app, |this, cx| {
                    let path = this.home_view.selected_path.clone();
                    let wegame = this.home_view.wegame_path.clone();
                    log::info!("[MainWindow] 用户点击启动客户端, path: {:?}", path);
                    let res = services::launch_lol_client(
                        path.as_deref(),
                        if wegame.is_empty() {
                            None
                        } else {
                            Some(&wegame)
                        },
                    );
                    match res {
                        Ok(_) => {
                            log::info!("[MainWindow] 客户端启动调用成功");
                            this.home_view.launch_message =
                                Some(("正在启动英雄联盟客户端...".to_string(), false));
                        }
                        Err(e) => {
                            log::warn!("[MainWindow] 客户端启动失败: {}", e);
                            this.home_view.launch_message =
                                Some((format!("启动失败: {}", e), true));
                        }
                    }
                    cx.notify();
                });
            },
        ))
    }
}
