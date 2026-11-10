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
        let map_side_label = if let Some(info) = &self.game_info {
            match info.user_side {
                Some(yuumi_core::gameflow::team_builder::MapSide::Blue) => Some("蓝方"),
                Some(yuumi_core::gameflow::team_builder::MapSide::Red) => Some("红方"),
                _ => None,
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
            .bg(Theme::BG_APP)
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
                            this.app_config.general.enable_close_to_tray.unwrap_or(false)
                        });
                        if close_to_tray {
                            w.minimize_window();
                        } else {
                            app.quit();
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
                        self.career_view.summoner.as_ref(),
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
                    ))
                    .child(viewport)
                    // 右下角回到顶部悬浮按钮
                    .when(self.current_route != Route::Home, |this| {
                        let v = view_entity.clone();
                        let is_saved_page = self.current_route == Route::SavedPlayers;
                        let scroll_y = if is_saved_page {
                            -self.saved_players_view.scroll_handle.offset().y
                        } else {
                            -self.main_scroll_handle.offset().y
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
                                    .bg(Theme::CARD_BG)
                                    .border_1()
                                    .border_color(Theme::AURORA_CYAN)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .hover(|s| {
                                        s.bg(Theme::CARD_BG_HOVER)
                                            .border_color(Theme::ACCENT_PRIMARY)
                                    })
                                    .on_mouse_down(MouseButton::Left, move |_ev, _win, app| {
                                        v.update(app, |this, cx| {
                                            if this.current_route == Route::SavedPlayers {
                                                this.saved_players_view
                                                    .scroll_handle
                                                    .set_offset(Point::default());
                                            } else {
                                                this.main_scroll_handle
                                                    .set_offset(Point::default());
                                            }
                                            cx.notify();
                                        });
                                    })
                                    .child(
                                        div()
                                            .text_base()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(Theme::ACCENT_PRIMARY)
                                            .child("↑"),
                                    ),
                            )
                        })
                    })
            )
            // 3. 右下角公告通知弹窗 (NoticePopup)
            .when(self.notice_popup_open, |this| {
                let v = view_entity.clone();
                this.child(
                    div()
                        .absolute()
                        .right(px(20.0))
                        .bottom(px(20.0))
                        .child(notice_popup(
                            "版本更新与运行公告",
                            "当前版本已完整复刻 Vue 纯白水晶极光界面，Direct3D 11 原生极速渲染，内存与启动性能优化完毕。",
                            move |_win, app| {
                                v.update(app, |this, cx| {
                                    this.toggle_notice_popup(false, cx);
                                });
                            },
                        )),
                )
            })
            // 4. OP.GG 入口提示（独立窗口已在原生窗口层打开，这里仅承载临时提示）
            .when_some(self.opgg_hint.clone(), |el, hint| {
                el.child(
                    div()
                        .absolute()
                        .bottom(px(20.0))
                        .left(px(20.0))
                        .px(px(14.0))
                        .py(px(10.0))
                        .rounded(px(8.0))
                        .bg(Theme::GOLD)
                        .shadow_md()
                        .text_size(px(12.0))
                        .text_color(Theme::TEXT_ON_PRIMARY)
                        .child(hint),
                )
            })
    }
}

impl MainWindow {
    pub(super) fn toggle_notice_popup(&mut self, open: bool, cx: &mut Context<Self>) {
        self.notice_popup_open = open;
        cx.notify();
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
            } else if self.current_route == Route::Home {
                self.home_view.reload_paths();
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
