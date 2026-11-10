//! 英雄选择组：自动选用/预选、自动禁用、召唤师技能三张卡片与选择器交互。
//!
//! 归属工单 D05/D06（候选配置、选禁延迟、D/F 技能、选择器搜索/多选/确认/取消）。
//! 自 `views/tools.rs` 按工单边界搬移，行为未改动。

use super::*;

impl ToolsView {
    pub(super) fn render_auto_hover_card<F>(&self, config: &AppConfig, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let enabled = config.functions.enable_auto_hover_champion;
        let confirm_timeout = config.functions.auto_select_confirm_on_timeout;
        let open = self.open_cards.contains("autohover");

        let status = if enabled {
            "已开启".to_string()
        } else {
            "未启用".to_string()
        };

        let current_lane = self.hover_active_lane;
        let pick_list = match current_lane {
            "top" => &config.functions.auto_select_champion_top,
            "jug" => &config.functions.auto_select_champion_jug,
            "mid" => &config.functions.auto_select_champion_mid,
            "bot" => &config.functions.auto_select_champion_bot,
            "sup" => &config.functions.auto_select_champion_sup,
            _ => &config.functions.auto_select_champion,
        };

        let act_toggle = on_action.clone();
        let act_hover = on_action.clone();
        let act_timeout = on_action.clone();
        let act_lane = on_action.clone();
        let act_clear = on_action.clone();
        let act_open_picker = on_action.clone();
        let act_close_picker = on_action.clone();
        let act_select_champ = on_action.clone();
        let act_remove_champ = on_action.clone();
        let act_confirm_picker = on_action.clone();
        let hover_search = self.hover_picker_search_live.borrow().clone();
        let hover_search_live = self.hover_picker_search_live.clone();
        let hover_focus = self.hover_picker_focus.clone();

        collapse_card(
            "tools-card-autohover".into(),
            open,
            Some("icons/user.svg"),
            "自动选用/预选英雄",
            "选人阶段优先亮起意向英雄或按分路首选列表自动锁定",
            Some(status),
            move |_ev, win, app| act_toggle(ToolsAction::ToggleCollapse("autohover"), win, app),
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                // 开关 1: 开启自动选用
                .child(setting_row(
                    "开启自动选用",
                    Some("进入选人阶段后自动亮起或选用当前分路预设英雄"),
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_hover(ToolsAction::ToggleAutoHover, win, app)
                        })
                        .child(Switch::new(enabled)),
                ))
                // 开关 2: 倒计时结束前自动确认
                .child(setting_row(
                    "倒计时结束前自动确认",
                    Some("配合亮起意向模式使用，防止因超时未选被判秒退惩罚"),
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_timeout(ToolsAction::ToggleConfirmOnTimeout, win, app)
                        })
                        .child(Switch::new(confirm_timeout)),
                ))
                // 分路选择选项卡
                .child(lane_tabs(current_lane, move |lane, win, app| {
                    act_lane(ToolsAction::SetHoverLane(lane), win, app)
                }))
                // 英雄选择器组件（[D06] 搜索输入 / 确认 / 取消分离）
                .child(champion_picker_searchable(
                    "hover-champ-picker".into(),
                    pick_list,
                    self.open_hover_picker,
                    &hover_search,
                    hover_focus,
                    move |next, window, _app| {
                        *hover_search_live.borrow_mut() = next;
                        window.refresh();
                    },
                    move |_ev, win, app| act_clear(ToolsAction::ClearPickChampion(current_lane), win, app),
                    move |_ev, win, app| act_open_picker(ToolsAction::TogglePicker("hover"), win, app),
                    move |_ev, win, app| {
                        act_confirm_picker(ToolsAction::TogglePicker("confirm_hover"), win, app)
                    },
                    move |_ev, win, app| {
                        act_close_picker(ToolsAction::TogglePicker("cancel_hover"), win, app)
                    },
                    move |cid, win, app| {
                        act_select_champ(
                            ToolsAction::SetPickerChampion {
                                lane: current_lane,
                                is_ban: false,
                                champion_id: cid,
                            },
                            win,
                            app,
                        )
                    },
                    move |cid, win, app| {
                        act_remove_champ(
                            ToolsAction::SetPickerChampion {
                                lane: current_lane,
                                is_ban: false,
                                champion_id: cid,
                            },
                            win,
                            app,
                        )
                    },
                )),
        )
    }

    // 行 4: 自动禁用英雄 (折叠卡 - T-V04)
    pub(super) fn render_auto_ban_card<F>(&self, config: &AppConfig, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let enabled = config.functions.enable_auto_ban_champion;
        let pretend = config.functions.pretend_ban;
        let delay = config.functions.auto_ban_delay;
        let open = self.open_cards.contains("autoban");

        let status = if enabled {
            format!("已开启 · 延迟 {:.1}s", delay)
        } else {
            "未启用".to_string()
        };

        let current_lane = self.ban_active_lane;
        let ban_list = match current_lane {
            "top" => &config.functions.auto_ban_champion_top,
            "jug" => &config.functions.auto_ban_champion_jug,
            "mid" => &config.functions.auto_ban_champion_mid,
            "bot" => &config.functions.auto_ban_champion_bot,
            "sup" => &config.functions.auto_ban_champion_sup,
            _ => &config.functions.auto_ban_champion,
        };

        let act_toggle = on_action.clone();
        let act_switch = on_action.clone();
        let act_pretend = on_action.clone();
        let act_dec = on_action.clone();
        let act_inc = on_action.clone();
        let act_lane = on_action.clone();
        let act_clear = on_action.clone();
        let act_open_picker = on_action.clone();
        let act_close_picker = on_action.clone();
        let act_select_champ = on_action.clone();
        let act_remove_champ = on_action.clone();
        let act_confirm_picker = on_action.clone();
        let ban_search = self.ban_picker_search_live.borrow().clone();
        let ban_search_live = self.ban_picker_search_live.clone();
        let ban_focus = self.ban_picker_focus.clone();

        collapse_card(
            "tools-card-autoban".into(),
            open,
            Some("icons/close.svg"),
            "自动禁用英雄",
            "禁用阶段自动锁定首选 ban 位，防止忘记禁英雄或超时秒退",
            Some(status),
            move |_ev, win, app| act_toggle(ToolsAction::ToggleCollapse("autoban"), win, app),
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(setting_row(
                    "开启自动禁用",
                    Some("进入禁用阶段后自动执行英雄禁用"),
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_switch(ToolsAction::ToggleAutoBan, win, app)
                        })
                        .child(Switch::new(enabled)),
                ))
                .child(setting_row(
                    "假装禁用",
                    Some("先亮起预设禁用英雄，在禁用倒计时末尾才执行确认"),
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_pretend(ToolsAction::TogglePretendBan, win, app)
                        })
                        .child(Switch::new(pretend)),
                ))
                .child(setting_row(
                    "禁用延迟",
                    Some("禁用阶段等待指定秒数后再执行 (0~15 秒，步长 0.5s)"),
                    number_stepper(
                        "stepper-ban-delay".into(),
                        format!("{:.1} 秒", delay),
                        delay > 0.0,
                        delay < 15.0,
                        move |_ev, win, app| {
                            act_dec(ToolsAction::SetAutoBanDelay((delay - 0.5).max(0.0)), win, app)
                        },
                        move |_ev, win, app| {
                            act_inc(ToolsAction::SetAutoBanDelay((delay + 0.5).min(15.0)), win, app)
                        },
                    ),
                ))
                .child(lane_tabs(current_lane, move |lane, win, app| {
                    act_lane(ToolsAction::SetBanLane(lane), win, app)
                }))
                // 英雄选择器组件（[D06] 搜索输入 / 确认 / 取消分离）
                .child(champion_picker_searchable(
                    "ban-champ-picker".into(),
                    ban_list,
                    self.open_ban_picker,
                    &ban_search,
                    ban_focus,
                    move |next, window, _app| {
                        *ban_search_live.borrow_mut() = next;
                        window.refresh();
                    },
                    move |_ev, win, app| act_clear(ToolsAction::ClearBanChampion(current_lane), win, app),
                    move |_ev, win, app| act_open_picker(ToolsAction::TogglePicker("ban"), win, app),
                    move |_ev, win, app| {
                        act_confirm_picker(ToolsAction::TogglePicker("confirm_ban"), win, app)
                    },
                    move |_ev, win, app| {
                        act_close_picker(ToolsAction::TogglePicker("cancel_ban"), win, app)
                    },
                    move |cid, win, app| {
                        act_select_champ(
                            ToolsAction::SetPickerChampion {
                                lane: current_lane,
                                is_ban: true,
                                champion_id: cid,
                            },
                            win,
                            app,
                        )
                    },
                    move |cid, win, app| {
                        act_remove_champ(
                            ToolsAction::SetPickerChampion {
                                lane: current_lane,
                                is_ban: true,
                                champion_id: cid,
                            },
                            win,
                            app,
                        )
                    },
                )),
        )
    }

    // 行 5: 自动设置召唤师技能 (折叠卡 - T-V05)
    pub(super) fn render_auto_spell_card<F>(&self, config: &AppConfig, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let enabled = config.functions.enable_auto_set_spells;
        let open = self.open_cards.contains("autospell");

        let current_lane = self.spell_active_lane;
        let spell_list = match current_lane {
            "top" => &config.functions.auto_set_summoner_spell_top,
            "jug" => &config.functions.auto_set_summoner_spell_jug,
            "mid" => &config.functions.auto_set_summoner_spell_mid,
            "bot" => &config.functions.auto_set_summoner_spell_bot,
            "sup" => &config.functions.auto_set_summoner_spell_sup,
            _ => &config.functions.auto_set_summoner_spell,
        };

        let spell1 = spell_list.first().copied().unwrap_or(4);
        let spell2 = spell_list.get(1).copied().unwrap_or(14);

        let status = if enabled {
            format!("D: {} · F: {}", get_spell_name(spell1), get_spell_name(spell2))
        } else {
            "未启用".to_string()
        };

        let act_toggle = on_action.clone();
        let act_switch = on_action.clone();
        let act_lane = on_action.clone();
        let act_slot_click = on_action.clone();
        let act_select_spell1 = on_action.clone();
        let act_select_spell2 = on_action.clone();
        let act_close = on_action.clone();
        let is_d_slot = self.spell_active_slot == Some("D");

        collapse_card(
            "tools-card-autospell".into(),
            open,
            Some("icons/wrench.svg"),
            "自动设置召唤师技能",
            "进入选人房间时自动校验并配置常用召唤师技能 (D/F 键)",
            Some(status),
            move |_ev, win, app| act_toggle(ToolsAction::ToggleCollapse("autospell"), win, app),
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(setting_row(
                    "开启自动设置技能",
                    Some("进入选人房间时自动将技能切换为当前分路预设"),
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_switch(ToolsAction::ToggleAutoSpell, win, app)
                        })
                        .child(Switch::new(enabled)),
                ))
                .child(lane_tabs(current_lane, move |lane, win, app| {
                    act_lane(ToolsAction::SetSpellLane(lane), win, app)
                }))
                .child(spell_picker(
                    Some(spell1),
                    Some(spell2),
                    self.spell_active_slot,
                    move |slot, win, app| act_slot_click(ToolsAction::TogglePicker(slot), win, app),
                    move |sid, win, app| {
                        act_select_spell1(
                            ToolsAction::SetSpellId {
                                lane: current_lane,
                                is_spell1: is_d_slot,
                                spell_id: sid,
                            },
                            win,
                            app,
                        )
                    },
                    move |slot, win, app| {
                        let is_d = slot == "D";
                        act_select_spell2(
                            ToolsAction::SetSpellId {
                                lane: current_lane,
                                is_spell1: is_d,
                                spell_id: 4,
                            },
                            win,
                            app,
                        )
                    },
                    move |_ev, win, app| act_close(ToolsAction::TogglePicker("close_spell"), win, app),
                )),
        )
    }
}
