// 对局信息路由：视图模式、队伍页签、演示预览与玩家跳转接线。
use super::*;

impl MainWindow {
    /// 加载高保真对局演示预览数据（优先真实快照，兜底演示）
    pub(super) fn load_game_info_demo(&mut self, cx: &mut Context<Self>) {
        let info = std::sync::Arc::new(
            yuumi_core::parsers::game_info::LiveGameInfo::load_snapshot_or_demo(),
        );
        // P0-2：演示看板的英雄图标预热（列头/概览卡/常用英雄/历史战绩卡，路径与渲染同源）
        let mut asset_paths: Vec<String> = Vec::new();
        for p in info.players() {
            if p.champion_id > 0 {
                asset_paths.push(format!(
                    "/lol-game-data/assets/v1/champion-icons/{}.png",
                    p.champion_id
                ));
            }
            for c in &p.top_champions {
                if c.champion_id > 0 {
                    asset_paths.push(format!(
                        "/lol-game-data/assets/v1/champion-icons/{}.png",
                        c.champion_id
                    ));
                }
            }
            for m in &p.matches {
                if m.champion_id > 0 {
                    asset_paths.push(format!(
                        "/lol-game-data/assets/v1/champion-icons/{}.png",
                        m.champion_id
                    ));
                }
            }
        }
        asset_paths.sort();
        asset_paths.dedup();
        self.game_info = Some(info);
        prewarm_assets(asset_paths, cx);
        cx.notify();
    }

    pub(super) fn switch_game_info_view_mode(
        &mut self,
        mode: GameInfoViewMode,
        cx: &mut Context<Self>,
    ) {
        self.game_info_view_mode = mode;
        cx.notify();
    }

    pub(super) fn switch_game_info_team_tab(
        &mut self,
        tab: GameInfoTeamTab,
        cx: &mut Context<Self>,
    ) {
        self.game_info_active_tab = tab;
        cx.notify();
    }

    /// 退出演示预览模式
    pub(super) fn exit_game_info_demo(&mut self, cx: &mut Context<Self>) {
        if let Some(info) = &self.game_info {
            if info.is_demo {
                self.game_info = None;
                cx.notify();
            }
        }
    }

    /// 对局信息路由渲染：视图模式、队伍页签与玩家跳转回调。
    pub(super) fn render_game_info_route(
        &mut self,
        el: Stateful<Div>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let view_entity = cx.entity().clone();
        let connected = matches!(self.connection_status, ConnectionStatus::Connected { .. });
        let view_mode = self.game_info_view_mode;
        let active_tab = self.game_info_active_tab;
        let view_mode_cb = view_entity.clone();
        let view_tab_cb = view_entity.clone();
        let view_career_cb = view_entity.clone();
        let view_search_cb = view_entity.clone();
        let view_load_demo = view_entity.clone();
        let view_exit_demo = view_entity.clone();
        let game_info_filter = self.app_config.functions.game_info_filter;
        el.overflow_hidden().h_full().child(GameInfoView::render(
            self.game_info.as_deref(),
            self.current_phase.as_str(),
            self.game_info_loading,
            connected,
            view_mode,
            active_tab,
            game_info_filter,
            move |mode, _window, app| {
                view_mode_cb.update(app, |this, cx| {
                    this.switch_game_info_view_mode(mode, cx);
                });
            },
            move |tab, _window, app| {
                view_tab_cb.update(app, |this, cx| {
                    this.switch_game_info_team_tab(tab, cx);
                });
            },
            move |target, _window, app| {
                view_career_cb.update(app, |this, cx| {
                    // P0-4：对局信息点击玩家时携带显示名，puuid 查询失败可按名回退
                    match target {
                        Some((puuid, name)) => {
                            this.view_other_career(puuid, name, String::new(), cx);
                        }
                        None => this.back_to_my_career(cx),
                    }
                });
            },
            move |name, tag, _window, app| {
                view_search_cb.update(app, |this, cx| {
                    let query = match tag {
                        Some(t) if !t.is_empty() => {
                            format!("{}#{}", name, t)
                        }
                        _ => name,
                    };
                    this.navigate_to_search(&query, None, cx);
                });
            },
            move |_event, _window, app| {
                view_load_demo.update(app, |this, cx| {
                    this.load_game_info_demo(cx);
                });
            },
            move |_event, _window, app| {
                view_exit_demo.update(app, |this, cx| {
                    this.exit_game_info_demo(cx);
                });
            },
            window.viewport_size().width,
        ))
    }
}
