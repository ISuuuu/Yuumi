// 生涯路由：生涯页渲染与回调、熟练度与战利品动作、他人生涯切换。
use super::*;
use crate::views::career::loot_category;
use yuumi_core::parsers::loot::LootInventoryItem;
use yuumi_core::parsers::loot_actions as loot_core;

impl MainWindow {
    /// 展开 / 收起战绩卡片的详情（基线已有的动作，当前渲染未接线）
    #[allow(dead_code)]
    pub(super) fn toggle_card_expanded(&mut self, idx: usize, cx: &mut Context<Self>) {
        if let Some(m) = self.matches.get_mut(idx) {
            m.expanded = !m.expanded;
            cx.notify();
        }
    }

    pub(super) fn refresh_career(&mut self, cx: &mut Context<Self>) {
        // B01 规范：若当前正在查看他人生涯，刷新应重新拉取该他人的战绩与资料，而不是拉取登录账号
        if self.career_view.is_viewing_other {
            if let Some(ref s) = self.career_view.summoner {
                let puuid = s.puuid.clone();
                let name = s.full_name();
                let icon = s.icon_path();
                self.view_other_career(puuid, name, icon, cx);
                return;
            }
        }

        let live = self.live_service.clone();
        let limit = (self.app_config.functions.career_games_number as usize).clamp(1, 50);
        yuumi_core::runtime::spawn_background(async move {
            live.refresh_career_data_with_limit(limit).await;
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
                let opening = self.career_view.popup != Some(kind);
                self.career_view.popup = if opening { Some(kind) } else { None };
                // 对齐 Vue：面板打开时按当前查看的召唤师补齐队友数据
                if opening && kind == CareerPopup::Teammates {
                    self.ensure_recent_teammates(cx);
                }
            }
            CareerBarAction::ClosePopup => self.career_view.popup = None,
            CareerBarAction::SelectQueue(id) => {
                self.career_view.selected_queue = id;
                self.career_view.popup = None;
            }
        }
        cx.notify();
    }

    /// 打开「最近队友」面板时，按当前查看的召唤师补齐队友数据。
    ///
    /// 对齐 Vue：面板打开时若缓存与当前对象不匹配就重新统计（切换他人、换对局后都会刷新），
    /// 避免展示上一位召唤师或上一批对局的队友。
    pub(super) fn ensure_recent_teammates(&mut self, cx: &mut Context<Self>) {
        let Some(puuid) = self.career_view.summoner.as_ref().map(|s| s.puuid.clone()) else {
            return;
        };
        if self.career_view.recent_teammates_loading
            || self.career_view.recent_teammates_for.as_deref() == Some(puuid.as_str())
        {
            return;
        }
        let game_ids: Vec<u64> = self.matches.iter().map(|m| m.display.game_id).collect();
        if game_ids.is_empty() {
            return;
        }

        // 切换查看对象后旧数据立即失效，不要继续展示别人的队友
        self.career_view.recent_teammates.clear();
        self.career_view.recent_teammates_loading = true;
        cx.notify();

        let live = self.live_service.clone();
        let target = puuid.clone();
        cx.spawn(async move |entity, cx| {
            let Some(client) = live.get_client().await else {
                let _ = entity.update(cx, |this, cx| {
                    this.career_view.recent_teammates_loading = false;
                    cx.notify();
                });
                return;
            };
            let tagged = std::collections::HashMap::new();
            match yuumi_core::parsers::teammates::fetch_recent_teammates(
                &client, &game_ids, &target, &tagged,
            )
            .await
            {
                Ok(teammates) => {
                    let icons: Vec<String> = teammates.iter().map(|t| t.icon.clone()).collect();
                    let _ = entity.update(cx, |this, cx| {
                        // 拉取期间又切换了查看对象，丢弃本次结果
                        let still_current =
                            this.career_view.summoner.as_ref().map(|s| s.puuid.as_str())
                                == Some(target.as_str());
                        if still_current {
                            prewarm_assets(icons, cx);
                            this.career_view.recent_teammates = teammates;
                            this.career_view.recent_teammates_for = Some(target.clone());
                        }
                        this.career_view.recent_teammates_loading = false;
                        cx.notify();
                    });
                }
                Err(e) => {
                    log::warn!("[MainWindow] 统计最近队友失败: {}", e);
                    let _ = entity.update(cx, |this, cx| {
                        this.career_view.recent_teammates_loading = false;
                        cx.notify();
                    });
                }
            }
        })
        .detach();
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

    /// 应用战利品页签的筛选 / 多选 / 下拉 / 批量动作。
    ///
    /// 对照 Vue `useLoot`：所有批量动作都先做前置校验（数量、钥匙、精粹、英雄拥有状态），
    /// 校验通过才弹确认框，确认后在后台执行并逐条刷新进度面板。
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
            LootAction::RequestSmartOpenAll => {
                let total_keys = self.loot_key_count();
                let batches = loot_core::build_smart_open_batches(
                    &self.career_view.loot_openables,
                    total_keys,
                );
                let total: i32 = batches.iter().map(|b| b.count).sum();
                if total <= 0 {
                    self.toast(ToastKind::Error, "没有可开启的战利品".to_string(), cx);
                    cx.notify();
                    return;
                }
                let key_cost: i32 = batches
                    .iter()
                    .filter(|b| b.ingredients.len() > 1)
                    .map(|b| b.count)
                    .sum();
                self.career_view.loot_modal = Some(LootModal::Confirm {
                    action_kind: LootActionKind::SmartOpenAll,
                    title: "一键开启全部战利品".to_string(),
                    message: format!(
                        "将自动开启 {} 个战利品：优先用钥匙碎片锻造钥匙，再按优先级开启宝箱与法球。",
                        total
                    ),
                    details: vec![
                        ("战利品种类".to_string(), format!("{} 种", batches.len())),
                        ("预计开启".to_string(), format!("{} 个", total)),
                        (
                            "消耗钥匙".to_string(),
                            format!("{} 把（当前 {}）", key_cost, total_keys),
                        ),
                    ],
                });
            }
            LootAction::RequestDisenchant => {
                let selected = self.selected_loot_items();
                if selected.is_empty() {
                    self.toast(ToastKind::Error, "没有选中任何碎片".to_string(), cx);
                    cx.notify();
                    return;
                }
                let mut blue = 0i64;
                let mut orange = 0i64;
                for item in &selected {
                    let gain = item.disenchant_value as i64 * item.count as i64;
                    if loot_category(item) == "CHAMPION" {
                        blue += gain;
                    } else {
                        orange += gain;
                    }
                }
                self.career_view.loot_modal = Some(LootModal::Confirm {
                    action_kind: LootActionKind::Disenchant,
                    title: "批量分解确认".to_string(),
                    message: format!(
                        "确定要分解选中的 {} 种碎片吗？此操作无法撤销。",
                        selected.len()
                    ),
                    details: vec![
                        ("所选碎片种类".to_string(), format!("{} 种", selected.len())),
                        ("预计蓝色精粹".to_string(), format!("+{}", blue)),
                        ("预计橙色精粹".to_string(), format!("+{}", orange)),
                    ],
                });
            }
            LootAction::RequestReroll => {
                let selected = self.selected_loot_items();
                let (ids, remaining) = plan_reroll_ids(&selected);
                if ids.is_empty() {
                    self.toast(
                        ToastKind::Error,
                        "没有可重铸的战利品组合（同类碎片需满 3 个）".to_string(),
                        cx,
                    );
                    cx.notify();
                    return;
                }
                self.career_view.loot_modal = Some(LootModal::Confirm {
                    action_kind: LootActionKind::Reroll,
                    title: "三合一重铸确认".to_string(),
                    message: "将按碎片分类自动分组，每 3 个合成 1 个永久物品。".to_string(),
                    details: vec![
                        ("重铸组数".to_string(), format!("{} 组", ids.len() / 3)),
                        ("消耗碎片".to_string(), format!("{} 个", ids.len())),
                        ("保留未配对".to_string(), format!("{} 个", remaining)),
                    ],
                });
            }
            LootAction::RequestUpgrade => {
                let selected = self.selected_loot_items();
                let unowned: Vec<LootInventoryItem> = selected
                    .into_iter()
                    .filter(|item| {
                        !item.item_status.eq_ignore_ascii_case("OWNED")
                            && !is_excluded_category(item)
                    })
                    .collect();
                if unowned.is_empty() {
                    self.toast(
                        ToastKind::Error,
                        "没有选中任何未拥有的战利品进行操作！".to_string(),
                        cx,
                    );
                    cx.notify();
                    return;
                }

                // 对照 Vue：皮肤碎片对应的英雄未拥有时无法升级
                let missing_champions: Vec<String> = unowned
                    .iter()
                    .filter(|item| {
                        loot_category(item) == "SKIN"
                            && !item.parent_item_status.eq_ignore_ascii_case("OWNED")
                    })
                    .map(|item| item.item_desc.clone())
                    .collect();
                if !missing_champions.is_empty() {
                    self.toast(
                        ToastKind::Error,
                        format!(
                            "未拥有以下皮肤对应的英雄：[{}]，请先在游戏内解锁该英雄！",
                            missing_champions.join("、")
                        ),
                        cx,
                    );
                    cx.notify();
                    return;
                }

                let mut total_blue = 0i64;
                let mut total_orange = 0i64;
                for item in &unowned {
                    if item.upgrade_essence_cost <= 0 {
                        continue;
                    }
                    let cost = item.upgrade_essence_cost as i64 * item.count as i64;
                    if loot_category(item) == "CHAMPION" {
                        total_blue += cost;
                    } else {
                        total_orange += cost;
                    }
                }
                if total_blue > self.career_view.essences.blue_essence as i64 {
                    self.toast(
                        ToastKind::Error,
                        format!(
                            "蓝色精粹不足！需要 🔵 {}，当前拥有 🔵 {}",
                            total_blue, self.career_view.essences.blue_essence
                        ),
                        cx,
                    );
                    cx.notify();
                    return;
                }
                if total_orange > self.career_view.essences.orange_essence as i64 {
                    self.toast(
                        ToastKind::Error,
                        format!(
                            "橙色精粹不足！需要 🟠 {}，当前拥有 🟠 {}",
                            total_orange, self.career_view.essences.orange_essence
                        ),
                        cx,
                    );
                    cx.notify();
                    return;
                }

                let has_upgrade = unowned.iter().any(|i| i.upgrade_essence_cost > 0);
                let has_unlock = unowned.iter().any(|i| i.upgrade_essence_cost <= 0);
                let description = if has_upgrade && has_unlock {
                    "消耗精粹升级部分碎片，其余永久道具将免费解锁"
                } else if has_unlock {
                    "将免费解锁 / 激活永久道具并添加到收藏"
                } else {
                    "将消耗对应精粹并解锁为永久版"
                };
                let mut details = vec![("待处理种类".to_string(), format!("{} 种", unowned.len()))];
                if total_blue > 0 {
                    details.push(("预计消耗蓝色精粹".to_string(), format!("🔵 {}", total_blue)));
                }
                if total_orange > 0 {
                    details.push((
                        "预计消耗橙色精粹".to_string(),
                        format!("🟠 {}", total_orange),
                    ));
                }
                details.push(("操作说明".to_string(), description.to_string()));

                let title = if has_unlock && !has_upgrade {
                    "解锁选中项"
                } else if has_upgrade && !has_unlock {
                    "升级选中项"
                } else {
                    "升级 / 解锁选中项"
                };
                self.career_view.loot_modal = Some(LootModal::Confirm {
                    action_kind: LootActionKind::Upgrade,
                    title: title.to_string(),
                    message: format!(
                        "确定要将选中的 {} 个未拥有项目进行升级或解锁吗？",
                        unowned.len()
                    ),
                    details,
                });
            }
            LootAction::Confirm(modal) => {
                self.career_view.loot_modal = None;
                match modal {
                    LootModal::Quantity {
                        name,
                        quantity,
                        max_quantity,
                        recipe_name,
                        ingredients,
                        action,
                        ..
                    } => {
                        if quantity <= 0 || quantity > max_quantity {
                            self.toast(ToastKind::Error, "数量超出可执行范围".to_string(), cx);
                            cx.notify();
                            return;
                        }
                        let item = LootActionItem {
                            recipe_name,
                            ingredients,
                            count: quantity,
                            ..Default::default()
                        };
                        let title = match action {
                            crate::views::career::LootQuantityAction::Open => "正在开启战利品",
                            crate::views::career::LootQuantityAction::Craft => "正在合成钥匙",
                        };
                        self.start_loot_action(
                            LootActionKind::Open,
                            vec![item],
                            title.to_string(),
                            quantity as usize,
                            name,
                            cx,
                        );
                    }
                    LootModal::Confirm { action_kind, .. } => {
                        let (items, title, total, first_name) = match action_kind {
                            LootActionKind::SmartOpenAll => {
                                let batches = loot_core::build_smart_open_batches(
                                    &self.career_view.loot_openables,
                                    self.loot_key_count(),
                                );
                                let total: i32 = batches.iter().map(|b| b.count).sum();
                                let items: Vec<LootActionItem> = batches
                                    .iter()
                                    .map(|b| LootActionItem {
                                        loot_id: b.loot_id.clone(),
                                        name: b.name.clone(),
                                        count: b.count,
                                        recipe_name: b.recipe_name.clone(),
                                        ingredients: b.ingredients.clone(),
                                        upgrade_recipe_name: None,
                                    })
                                    .collect();
                                (
                                    items,
                                    "正在一键开启战利品".to_string(),
                                    total.max(0) as usize,
                                    batches.first().map(|b| b.name.clone()).unwrap_or_default(),
                                )
                            }
                            LootActionKind::Disenchant => {
                                let selected = self.selected_loot_items();
                                let total = selected.len();
                                let first = selected
                                    .first()
                                    .map(|i| i.item_desc.clone())
                                    .unwrap_or_default();
                                let items: Vec<LootActionItem> = selected
                                    .iter()
                                    .map(|i| LootActionItem {
                                        loot_id: i.loot_id.clone(),
                                        name: i.item_desc.clone(),
                                        count: i.count,
                                        ..Default::default()
                                    })
                                    .collect();
                                (items, "正在分解碎片".to_string(), total, first)
                            }
                            LootActionKind::Reroll => {
                                let selected = self.selected_loot_items();
                                let (ids, _) = plan_reroll_ids(&selected);
                                let total = ids.len() / 3;
                                let first = ids.first().cloned().unwrap_or_default();
                                let items: Vec<LootActionItem> = ids
                                    .into_iter()
                                    .map(|loot_id| LootActionItem {
                                        loot_id,
                                        count: 1,
                                        ..Default::default()
                                    })
                                    .collect();
                                (items, "正在三合一重铸".to_string(), total, first)
                            }
                            LootActionKind::Upgrade => {
                                let selected = self.selected_loot_items();
                                let unowned: Vec<LootInventoryItem> = selected
                                    .into_iter()
                                    .filter(|item| {
                                        !item.item_status.eq_ignore_ascii_case("OWNED")
                                            && !is_excluded_category(item)
                                    })
                                    .collect();
                                let total = unowned.len();
                                let first = unowned
                                    .first()
                                    .map(|i| i.item_desc.clone())
                                    .unwrap_or_default();
                                let items: Vec<LootActionItem> = unowned
                                    .iter()
                                    .map(|i| LootActionItem {
                                        loot_id: i.loot_id.clone(),
                                        name: i.item_desc.clone(),
                                        count: i.count,
                                        upgrade_recipe_name: i.upgrade_recipe_name.clone(),
                                        ..Default::default()
                                    })
                                    .collect();
                                (items, "正在升级 / 解锁碎片".to_string(), total, first)
                            }
                            LootActionKind::Open => (Vec::new(), String::new(), 0, String::new()),
                        };

                        if items.is_empty() || total == 0 {
                            self.toast(ToastKind::Error, "没有可执行的项目".to_string(), cx);
                            cx.notify();
                            return;
                        }
                        self.start_loot_action(action_kind, items, title, total, first_name, cx);
                    }
                }
            }
            LootAction::CloseModal => self.career_view.loot_modal = None,
            LootAction::CloseProgress => {
                self.career_view.loot_progress = None;
                self.career_view.loot_busy = false;
            }
        }
        cx.notify();
    }

    /// 预热战利品页所有瓦片图（库存碎片 + 可开启战利品）
    pub(super) fn prewarm_loot_assets(&self, cx: &mut Context<Self>) {
        let mut paths: Vec<String> = self
            .career_view
            .loot_items
            .iter()
            .filter_map(|item| item.tile_path.clone())
            .collect();
        paths.extend(
            self.career_view
                .loot_openables
                .iter()
                .filter_map(|loot| loot.tile_path.clone()),
        );
        paths.retain(|p| !p.is_empty());
        paths.sort();
        paths.dedup();
        prewarm_assets(paths, cx);
    }

    /// 当前可用钥匙总数（海克斯钥匙 + 杰作钥匙）
    fn loot_key_count(&self) -> i32 {
        self.career_view
            .loot_items
            .iter()
            .filter(|i| i.loot_id == "MATERIAL_key" || i.loot_id == "MATERIAL_key_premium")
            .map(|i| i.count)
            .sum()
    }

    /// 选中项对应的库存条目
    fn selected_loot_items(&self) -> Vec<LootInventoryItem> {
        self.career_view
            .loot_items
            .iter()
            .filter(|i| self.career_view.loot_selected.contains(&i.loot_id))
            .cloned()
            .collect()
    }

    /// 启动一次战利品写动作：先置进度面板与忙碌态，再后台执行并逐条刷新
    fn start_loot_action(
        &mut self,
        kind: LootActionKind,
        items: Vec<LootActionItem>,
        title: String,
        total: usize,
        first_name: String,
        cx: &mut Context<Self>,
    ) {
        self.career_view.loot_progress = Some(LootProgress {
            title: title.clone(),
            current: 0,
            total,
            success: 0,
            failed: 0,
            current_name: first_name,
            last_message: String::new(),
        });
        self.career_view.loot_busy = true;

        let live = self.live_service.clone();
        cx.spawn(async move |entity, cx| {
            let result = live.run_loot_action(kind, items).await;
            let _ = entity.update(cx, |this, cx| {
                this.career_view.loot_busy = false;
                match result {
                    Ok(progress) => {
                        let success = progress.iter().filter(|p| p.success).count();
                        let failed = progress.len().saturating_sub(success);
                        if let Some(state) = this.career_view.loot_progress.as_mut() {
                            state.current = progress.len().max(state.current);
                            state.total = state.total.max(progress.len());
                            state.success = success;
                            state.failed = failed;
                        }
                        if failed == 0 {
                            this.career_view.loot_selected.clear();
                        } else {
                            this.toast(
                                ToastKind::Error,
                                format!("{}：{} 项处理失败", title, failed),
                                cx,
                            );
                        }
                    }
                    Err(e) => {
                        this.career_view.loot_progress = None;
                        let label = match kind {
                            LootActionKind::Disenchant => "分解失败",
                            LootActionKind::Upgrade => "升级失败",
                            LootActionKind::Reroll => "重铸失败",
                            LootActionKind::SmartOpenAll | LootActionKind::Open => "开箱失败",
                        };
                        log::error!("[MainWindow] 执行战利品动作失败: {}", e);
                        this.toast(ToastKind::Error, format!("{}: {}", label, e), cx);
                    }
                }
                cx.notify();
            });
            live.refresh_career_data().await;
        })
        .detach();
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

        let placeholder_icon = other_summoner.icon_path();

        self.career_view.is_viewing_other = true;
        self.career_view.active_tab = views::career::CareerTab::Matches;
        self.career_view.summoner = Some(other_summoner);
        self.career_view.ranked = None;
        // 队友数据属于被查看的召唤师，切换对象后立即失效
        self.career_view.recent_teammates.clear();
        self.career_view.recent_teammates_for = None;
        self.career_view.recent_teammates_loading = false;
        self.matches.clear();
        self.stats = CareerHeaderStats::compute(&[]);
        cx.notify();

        // 从战绩查询 / 对局信息等页面点击他人召唤师时需切换到生涯路由，否则停留在原页面
        self.switch_route(Route::Career, cx);
        // LcuImage 只读已预热缓存，切换查看对象后必须重新预热，否则头像一直是占位块
        prewarm_assets(vec![placeholder_icon], cx);

        let live = self.live_service.clone();
        let target_name = name.clone();
        let limit = (self.app_config.functions.career_games_number as usize).clamp(1, 50);
        cx.spawn(async move |entity, cx| {
            if let Some(client) = live.get_client().await {
                let assets_opt = live.get_game_data_assets().await;
                // P0-4：有 puuid 优先按 puuid 查询；失败或无 puuid 且名字非空时回退按名查询
                let real_summoner = if !puuid.trim().is_empty() {
                    match yuumi_core::parsers::summoner::fetch_summoner_by_puuid(&client, &puuid)
                        .await
                    {
                        Ok(s) => Some(s),
                        Err(e) => {
                            log::warn!("[MainWindow] 按 puuid 查询召唤师失败，回退按名查询: {}", e);
                            if target_name.trim().is_empty() {
                                None
                            } else {
                                yuumi_core::parsers::summoner::fetch_summoner_by_name(
                                    &client,
                                    &target_name,
                                )
                                .await
                                .ok()
                            }
                        }
                    }
                } else if !target_name.trim().is_empty() {
                    yuumi_core::parsers::summoner::fetch_summoner_by_name(&client, &target_name)
                        .await
                        .ok()
                } else {
                    None
                };
                if let Some(real_s) = real_summoner {
                    let real_icon = real_s.icon_path();
                    let _ = entity.update(cx, |this, cx| {
                        if this.career_view.is_viewing_other {
                            this.career_view.summoner = Some(real_s);
                            // 真实头像替换占位图标后同样需要预热
                            prewarm_assets(vec![real_icon], cx);
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
                    let end_idx = (limit as u32).saturating_sub(1);
                    if let Ok(mut matches) = yuumi_core::parsers::match_parser::fetch_match_history(
                        &client,
                        &puuid,
                        Some(0),
                        Some(end_idx),
                        &assets,
                    )
                    .await
                    {
                        matches.truncate(limit);
                        let items: Vec<MatchItem> =
                            matches.into_iter().map(MatchItem::new).collect();
                        let _ = entity.update(cx, |this, cx| {
                            if this.career_view.is_viewing_other {
                                // P0-2：战绩列表图标预热，路径与渲染同源（对齐 MatchHistoryLoaded 入口）
                                let mut asset_paths = Vec::new();
                                for m in &items {
                                    let d = &m.display;
                                    for url in [
                                        &d.champion_icon_url,
                                        &d.spell1_icon_url,
                                        &d.spell2_icon_url,
                                        &d.rune_icon_url,
                                    ] {
                                        if !url.is_empty() {
                                            asset_paths.push(url.clone());
                                        }
                                    }
                                    asset_paths.extend(
                                        d.item_icon_urls.iter().filter(|u| !u.is_empty()).cloned(),
                                    );
                                    asset_paths.extend(
                                        d.augment_icon_urls
                                            .iter()
                                            .filter(|u| !u.is_empty())
                                            .cloned(),
                                    );
                                }
                                // 战绩里英雄/装备高度重复，去重以免打爆 LCU 请求
                                asset_paths.sort();
                                asset_paths.dedup();
                                this.stats = CareerHeaderStats::compute(&items);
                                this.matches = items;
                                prewarm_assets(asset_paths, cx);
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
        self.career_view.recent_teammates.clear();
        self.career_view.recent_teammates_for = None;
        self.career_view.recent_teammates_loading = false;
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
        let view_replay = view_entity.clone();
        let is_connected = matches!(self.connection_status, ConnectionStatus::Connected { .. });
        el.child(self.career_view.render(
            &self.matches,
            stats,
            is_connected,
            move |target_tab, _window, app| {
                view.update(app, |this, cx| {
                    this.career_view.active_tab = target_tab;
                    this.career_view.popup = None;
                    // 战利品瓦片图依赖异步预热，切页时补一次（含上次失败的路径）
                    if target_tab == views::career::CareerTab::Loot {
                        this.prewarm_loot_assets(cx);
                    }
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
            move |game_id, _window, app| {
                view_replay.update(app, |this, cx| {
                    this.handle_replay(game_id, cx);
                });
            },
        ))
    }
}

/// 兑换/升级动作需要排除的分类（材料、宝箱、法球、精粹）
fn is_excluded_category(item: &LootInventoryItem) -> bool {
    matches!(
        loot_category(item),
        "MATERIAL" | "CHEST" | "ORB" | "CURRENCY"
    )
}

/// 三合一重铸计划（对照 Vue `handleBatchReroll`）
///
/// 按 displayCategories 分组，每组按 3 个取整，返回平铺的 lootId 列表与未配对剩余数。
fn plan_reroll_ids(items: &[LootInventoryItem]) -> (Vec<String>, i32) {
    let mut groups: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for item in items {
        if item.loot_type.to_uppercase().contains("STATSTONE") || is_excluded_category(item) {
            continue;
        }
        let entry = groups.entry(item.display_categories.clone()).or_default();
        for _ in 0..item.count.max(0) {
            entry.push(item.loot_id.clone());
        }
    }

    let mut ids = Vec::new();
    let mut remaining = 0i32;
    for (_, list) in groups {
        let usable = list.len() - list.len() % 3;
        ids.extend(list[..usable].iter().cloned());
        remaining += (list.len() % 3) as i32;
    }
    (ids, remaining)
}
