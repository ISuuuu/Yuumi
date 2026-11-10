// 生涯路由：生涯页渲染与回调、熟练度与战利品动作、他人生涯切换。
use super::*;

impl MainWindow {
    /// 展开 / 收起战绩卡片的详情（基线已有的动作，当前渲染未接线）
    #[allow(dead_code)]
    pub(super) fn toggle_card_expanded(&mut self, idx: usize, cx: &mut Context<Self>) {
        if let Some(m) = self.matches.get_mut(idx) {
            m.expanded = !m.expanded;
            cx.notify();
        }
    }

    pub(super) fn refresh_career(&mut self, _cx: &mut Context<Self>) {
        let live = self.live_service.clone();
        yuumi_core::runtime::spawn_background(async move {
            live.refresh_career_data().await;
        });
    }

    /// 处理近期统计横条的队列筛选与浮层开合
    pub(super) fn apply_career_bar_action(
        &mut self,
        action: CareerBarAction,
        cx: &mut Context<Self>,
    ) {
        match action {
            CareerBarAction::TogglePopup(kind) => {
                self.career_view.popup = if self.career_view.popup == Some(kind) {
                    None
                } else {
                    Some(kind)
                };
            }
            CareerBarAction::ClosePopup => self.career_view.popup = None,
            CareerBarAction::SelectQueue(id) => {
                self.career_view.selected_queue = id;
                self.career_view.popup = None;
            }
        }
        cx.notify();
    }

    /// 更新熟练度页的英雄搜索词
    pub(super) fn set_mastery_query(&mut self, text: String, cx: &mut Context<Self>) {
        self.career_view.mastery_query = text;
        cx.notify();
    }

    /// 应用熟练度页的筛选 / 排序动作
    pub(super) fn apply_mastery_filter(
        &mut self,
        action: MasteryFilterAction,
        cx: &mut Context<Self>,
    ) {
        match action {
            MasteryFilterAction::Chest(value) => self.career_view.mastery_chest = value,
            MasteryFilterAction::Level(value) => self.career_view.mastery_level = value,
            MasteryFilterAction::Sort(value) => self.career_view.mastery_sort = value,
            MasteryFilterAction::ToggleSortDir => {
                self.career_view.mastery_sort_desc = !self.career_view.mastery_sort_desc
            }
            MasteryFilterAction::ToggleDropdown(kind) => {
                let slot = &mut self.career_view.mastery_dropdown;
                *slot = if *slot == Some(kind) {
                    None
                } else {
                    Some(kind)
                };
            }
            MasteryFilterAction::CloseDropdown => self.career_view.mastery_dropdown = None,
        }
        cx.notify();
    }

    /// 应用战利品页签的筛选 / 多选 / 下拉动作
    ///
    /// 开启、全部开启、分解、重铸、升级解锁属于 LCU 写操作，按钮已做禁用态并标注
    /// 「服务层待接」；待服务层就绪后在 `LootAction` 补写动作变体并在此调用
    /// `LiveLcuService` 的写方法，完成后再 `refresh_career_data()` 回拉库存与余额。
    pub(super) fn apply_loot_action(&mut self, action: LootAction, cx: &mut Context<Self>) {
        match action {
            LootAction::ToggleDropdown(kind) => {
                let slot = &mut self.career_view.loot_dropdown;
                *slot = if *slot == Some(kind) {
                    None
                } else {
                    Some(kind)
                };
            }
            LootAction::CloseDropdown => self.career_view.loot_dropdown = None,
            LootAction::SetType(value) => {
                self.career_view.loot_filter_type = value;
                self.career_view.loot_dropdown = None;
                // 对照 Vue watch(filterType)：切换类型自动清空选择
                self.career_view.loot_selected.clear();
            }
            LootAction::SetOwned(value) => {
                self.career_view.loot_filter_owned = value;
                self.career_view.loot_dropdown = None;
            }
            LootAction::SetValueBasis(value) => {
                self.career_view.loot_filter_value_basis = value;
                self.career_view.loot_dropdown = None;
            }
            LootAction::SetOperator(value) => {
                self.career_view.loot_filter_operator = value;
                self.career_view.loot_dropdown = None;
            }
            LootAction::SetValueInput(text) => self.career_view.loot_value_input = text,
            LootAction::ClearValueInput => self.career_view.loot_value_input.clear(),
            LootAction::ToggleSelect(id) => {
                let selected = &mut self.career_view.loot_selected;
                if let Some(pos) = selected.iter().position(|x| x == &id) {
                    selected.remove(pos);
                } else {
                    selected.push(id);
                }
            }
            LootAction::SelectAllFiltered => {
                let ids: Vec<String> = self
                    .career_view
                    .filtered_loot()
                    .into_iter()
                    .map(|item| item.loot_id.clone())
                    .collect();
                self.career_view.loot_selected = ids;
            }
            LootAction::ClearSelection => self.career_view.loot_selected.clear(),
            LootAction::OpenQuantity(modal) => {
                self.career_view.loot_modal = Some(modal);
            }
            LootAction::OpenConfirm(modal) => {
                self.career_view.loot_modal = Some(modal);
            }
            LootAction::Confirm(modal) => {
                self.career_view.loot_modal = None;
                let live = self.live_service.clone();
                match modal {
                    LootModal::Quantity {
                        recipe_name,
                        ingredients,
                        quantity,
                        action,
                        ..
                    } => {
                        let item = LootActionItem {
                            recipe_name,
                            ingredients,
                            count: quantity,
                            ..Default::default()
                        };
                        let kind = match action {
                            views::career::LootQuantityAction::Open => LootActionKind::Open,
                            views::career::LootQuantityAction::Craft => LootActionKind::Open,
                        };
                        yuumi_core::runtime::spawn_background(async move {
                            let _ = live.run_loot_action(kind, vec![item]).await;
                            live.refresh_career_data().await;
                        });
                    }
                    LootModal::Confirm { action_kind, .. } => {
                        let selected_ids = self.career_view.loot_selected.clone();
                        self.career_view.loot_selected.clear();

                        let items: Vec<LootActionItem> = if action_kind
                            == LootActionKind::SmartOpenAll
                        {
                            self.career_view
                                .loot_items
                                .iter()
                                .filter(|i| i.is_openable)
                                .map(|i| LootActionItem {
                                    loot_id: i.loot_id.clone(),
                                    name: i.name.clone(),
                                    count: i.count,
                                    recipe_name: format!("{}_open", i.loot_id),
                                    ingredients: vec![i.loot_id.clone()],
                                    ..Default::default()
                                })
                                .collect()
                        } else {
                            self.career_view
                                .loot_items
                                .iter()
                                .filter(|i| selected_ids.contains(&i.loot_id))
                                .map(|i| LootActionItem {
                                    loot_id: i.loot_id.clone(),
                                    name: i.item_desc.clone(),
                                    count: i.count,
                                    recipe_name: i.upgrade_recipe_name.clone().unwrap_or_default(),
                                    ingredients: vec![i.loot_id.clone()],
                                    ..Default::default()
                                })
                                .collect()
                        };

                        yuumi_core::runtime::spawn_background(async move {
                            let _ = live.run_loot_action(action_kind, items).await;
                            live.refresh_career_data().await;
                        });
                    }
                }
            }
            LootAction::CloseModal => self.career_view.loot_modal = None,
            LootAction::CloseProgress => self.career_view.loot_progress = None,
        }
        cx.notify();
    }

    /// 复制当前召唤师的 Riot ID，并在资料卡上短暂回显反馈
    pub(super) fn copy_riot_id(&mut self, cx: &mut Context<Self>) {
        let text = match self.career_view.summoner.as_ref() {
            Some(summoner) => summoner.full_name(),
            None => return,
        };

        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.career_view.copied = true;
        cx.notify();

        cx.spawn(async move |entity, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1500))
                .await;
            let _ = entity.update(cx, |this, cx| {
                this.career_view.copied = false;
                cx.notify();
            });
        })
        .detach();
    }

    /// 跳转到当前召唤师的战绩查询（对照 Vue `Career.vue` 的 `goToHistory`）
    pub(super) fn open_own_history(&mut self, cx: &mut Context<Self>) {
        let name = match self.career_view.summoner.as_ref() {
            Some(summoner) => summoner.full_name(),
            None => return,
        };

        self.navigate_to_search(&name, None, cx);
    }

    /// 进入查看他人生涯模式
    pub(super) fn view_other_career(
        &mut self,
        puuid: String,
        name: String,
        _icon: String,
        cx: &mut Context<Self>,
    ) {
        let (game_name, tag_line) = if let Some((g, t)) = name.split_once('#') {
            (g.to_string(), Some(t.to_string()))
        } else {
            (name.clone(), None)
        };

        let other_summoner = SummonerInfo {
            puuid: puuid.clone(),
            display_name: Some(name.clone()),
            game_name: Some(game_name),
            tag_line,
            summoner_level: Some(0),
            profile_icon_id: Some(29),
            privacy: None,
            percent_complete_for_next_level: Some(0),
            account_id: None,
            summoner_id: None,
            xp_since_last_level: None,
            xp_until_next_level: None,
        };

        self.career_view.is_viewing_other = true;
        self.career_view.active_tab = views::career::CareerTab::Matches;
        self.career_view.summoner = Some(other_summoner);
        self.career_view.ranked = None;
        self.matches.clear();
        self.stats = CareerHeaderStats::compute(&[]);
        cx.notify();

        let live = self.live_service.clone();
        let target_name = name.clone();
        cx.spawn(async move |entity, cx| {
            if let Some(client) = live.get_client().await {
                let assets_opt = live.get_game_data_assets().await;
                if let Ok(real_s) =
                    yuumi_core::parsers::summoner::fetch_summoner_by_name(&client, &target_name)
                        .await
                {
                    let _ = entity.update(cx, |this, cx| {
                        if this.career_view.is_viewing_other {
                            this.career_view.summoner = Some(real_s);
                            cx.notify();
                        }
                    });
                }
                if let Ok(stats) =
                    yuumi_core::parsers::game_info::fetch_ranked_stats(&client, &puuid).await
                {
                    let _ = entity.update(cx, |this, cx| {
                        if this.career_view.is_viewing_other {
                            this.career_view.ranked = Some(stats);
                            cx.notify();
                        }
                    });
                }
                if let Some(assets) = assets_opt {
                    if let Ok(matches) = yuumi_core::parsers::match_parser::fetch_match_history(
                        &client,
                        &puuid,
                        Some(0),
                        Some(19),
                        &assets,
                    )
                    .await
                    {
                        let items: Vec<MatchItem> =
                            matches.into_iter().map(MatchItem::new).collect();
                        let _ = entity.update(cx, |this, cx| {
                            if this.career_view.is_viewing_other {
                                this.stats = CareerHeaderStats::compute(&items);
                                this.matches = items;
                                cx.notify();
                            }
                        });
                    }
                }
            }
        })
        .detach();
    }

    /// 返回我的生涯模式并恢复本人数据
    pub(super) fn back_to_my_career(&mut self, cx: &mut Context<Self>) {
        self.career_view.is_viewing_other = false;
        self.career_view.active_tab = views::career::CareerTab::Matches;
        if let Some(own) = self.own_summoner.clone() {
            self.career_view.summoner = Some(own);
        }
        self.matches = self.own_matches.clone();
        self.stats = self.own_stats.clone();
        self.career_view.ranked = self.own_ranked.clone();
        self.refresh_career(cx);
        cx.notify();
    }

    /// 生涯路由渲染与动作回调。
    pub(super) fn render_career_route(
        &mut self,
        el: Stateful<Div>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let view_entity = cx.entity().clone();
        let stats = &self.stats;
        let view = view_entity.clone();
        let view_select = view_entity.clone();
        let view_refresh = view_entity.clone();
        let view_copy = view_entity.clone();
        let view_history = view_entity.clone();
        let view_back = view_entity.clone();
        let view_teammate = view_entity.clone();
        let view_query = view_entity.clone();
        let view_filter = view_entity.clone();
        let view_bar = view_entity.clone();
        let view_loot = view_entity.clone();
        let is_connected = matches!(self.connection_status, ConnectionStatus::Connected { .. });
        el.child(self.career_view.render(
            &self.matches,
            stats,
            is_connected,
            move |target_tab, _window, app| {
                view.update(app, |this, cx| {
                    this.career_view.active_tab = target_tab;
                    this.career_view.popup = None;
                    cx.notify();
                });
            },
            move |game_id, _window, app| {
                view_select.update(app, |this, cx| {
                    let summoner_name = this
                        .career_view
                        .summoner
                        .as_ref()
                        .map(|s| s.full_name())
                        .unwrap_or_default();
                    this.navigate_to_search(&summoner_name, Some(game_id), cx);
                });
            },
            move |_window, app| {
                view_refresh.update(app, |this, cx| {
                    this.refresh_career(cx);
                });
            },
            move |_window, app| {
                view_copy.update(app, |this, cx| this.copy_riot_id(cx));
            },
            move |_window, app| {
                view_history.update(app, |this, cx| this.open_own_history(cx));
            },
            move |_window, app| {
                view_back.update(app, |this, cx| {
                    this.back_to_my_career(cx);
                });
            },
            move |tm, _window, app| {
                view_teammate.update(app, |this, cx| {
                    this.career_view.popup = None;
                    if !tm.puuid.is_empty() {
                        this.view_other_career(tm.puuid, tm.name, tm.icon, cx);
                    } else {
                        this.navigate_to_search(&tm.name, None, cx);
                    }
                });
            },
            &self.career_mastery_focus,
            self.career_mastery_focus.is_focused(window),
            move |text: String, _window, app| {
                view_query.update(app, |this, cx| this.set_mastery_query(text, cx));
            },
            move |action, _window, app| {
                view_filter.update(app, |this, cx| this.apply_mastery_filter(action, cx));
            },
            move |action: CareerBarAction, _window, app| {
                view_bar.update(app, |this, cx| {
                    if let CareerBarAction::SelectQueue(_) = &action {
                        this.career_view
                            .matches_scroll_handle
                            .set_offset(Point::default());
                    }
                    this.apply_career_bar_action(action, cx);
                });
            },
            &self.loot_value_focus,
            self.loot_value_focus.is_focused(window),
            move |action: LootAction, _window, app| {
                view_loot.update(app, |this, cx| this.apply_loot_action(action, cx));
            },
        ))
    }
}
