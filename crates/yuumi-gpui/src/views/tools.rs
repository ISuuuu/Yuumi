//! 工具箱与自动化流程主视图 (ToolsView)
//!
//! 1:1 复刻 Vue 原版：
//! - `src/views/Tools.vue`
//! - `src/components/tools/AutoAcceptCard.vue`
//! - `src/components/tools/AutoSwapCard.vue`
//! - `src/components/tools/AutoHoverCard.vue`
//! - `src/components/tools/AutoGameflowCard.vue`
//! - `src/components/tools/SpectateCard.vue`
//! - `src/components/tools/FriendCleanerCard.vue`
//! - `src/components/tools/LcuQuickActionsCard.vue` 及 `quickactions/` 下所有子卡片
//!
//! 严格依照 [05-6-GPUI_TOOLS_SETTINGS_STYLE_PARITY.md] 规范实现 21 行：
//! 13 张折叠卡 + 8 个直显行。

pub mod components;
pub mod data;

use crate::components::badge::{Badge, BadgeVariant};
use crate::components::collapse_card::{collapse_card, lane_tabs, number_stepper, setting_row};
use crate::components::switch::Switch;
use crate::theme::Theme;
use components::*;
use data::*;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::collections::HashSet;
use yuumi_core::config::AppConfig;

/// 工具箱交互动作枚举
#[derive(Clone, Debug)]
pub enum ToolsAction {
    /// 展开/收起指定卡片
    ToggleCollapse(&'static str),

    // ─── 英雄选择阶段 (Champ Select) ───
    ToggleAutoAccept,
    SetAutoAcceptDelay(u32),
    ToggleCeilSwap,
    ToggleChampTrade,
    ToggleAutoPick,
    ToggleAutoHover,
    ToggleConfirmOnTimeout,
    SetHoverLane(&'static str),
    ClearPickChampion(&'static str),
    ToggleAutoBan,
    TogglePretendBan,
    SetAutoBanDelay(f64),
    SetBanLane(&'static str),
    ClearBanChampion(&'static str),
    ToggleAutoSpell,
    SetSpellLane(&'static str),
    SetSpellId {
        lane: &'static str,
        is_spell1: bool,
        spell_id: i32,
    },

    // ─── 游戏阶段 (Gameflow) ───
    ToggleAutoHandleInvite,
    ToggleAutoHonor,
    ToggleAutoPlayAgain,
    ToggleAramTeamSide,
    ToggleAramTeamSideVisible,
    ToggleTagReminder,
    SpectateSummonerNameInput(String),
    LaunchSpectate,
    ScanFriends,
    SetCleanerDays(u32),
    BatchDeleteOfflineFriends,
    ToggleBenchOverlay,
    OpenBenchOverlayWindow,
    ToggleAutoReconnect,
    ToggleAutoCreateLobby,
    SetDefaultGameMode(u32),
    CreateLobbyNow,
    ToggleLockGameSettings,

    // ─── 客户端小工具 (Client Utilities) ───
    FixClientWindow,
    RestartClient,

    // ─── 个人主页与社交设置 (Profile & Social) ───
    SignatureInput(String),
    ApplySignature,
    ProfileBgSkinInput(String),
    ApplyProfileBackground,
    SetSpoofQueue(String),
    SetSpoofTier(String),
    SetSpoofDivision(String),
    ApplyRankSpoof,
    SetOnlineStatus(String),
    ApplyOnlineStatus,
    ClearBadges,
    ClearBorder,

    // ─── 新增交互动作 ───
    TogglePicker(&'static str),
    SetPickerChampion {
        lane: &'static str,
        is_ban: bool,
        champion_id: i32,
    },
    ToggleDropdownMenu(&'static str),
    SetSpectateMethod(String),
    SetCleanerFilter(String),
    ToggleSelectCleanerFriend(String),
    SelectAllCleanerFriends,
    ClearSelectedCleanerFriends,
    SelectBgChampion(i32),
    SelectBgSkin(i32),
    ToggleBadgesModal(bool),
    ToggleBadgeItem(String),
    ClearBadgesModal,
    SaveBadgesPreferences,
}

/// 工具箱视图状态
pub struct ToolsView {
    /// 当前展开的卡片标识集合
    pub open_cards: HashSet<String>,
    /// 自动秒选/预选当前选中的分路
    pub hover_active_lane: &'static str,
    /// 自动禁用当前选中的分路
    pub ban_active_lane: &'static str,
    /// 自动技能当前选中的分路
    pub spell_active_lane: &'static str,

    // 英雄选择器展开与过滤状态
    pub open_hover_picker: bool,
    pub open_ban_picker: bool,
    pub open_bg_champion_picker: bool,
    pub open_bg_skin_modal: bool,
    pub hover_picker_search: String,
    pub ban_picker_search: String,

    // 技能槽位与展开状态
    pub spell_active_slot: Option<&'static str>,

    // 表单状态输入缓存
    pub spectate_name_buffer: String,
    pub spectate_method: String,
    pub cleaner_days: u32,
    pub cleaner_has_scanned: bool,
    pub cleaner_filter: String,
    pub cleaner_search: String,
    pub cleaner_selected_ids: HashSet<String>,

    pub signature_buffer: String,
    pub profile_skin_id_buffer: String,
    pub bg_selected_champion: Option<i32>,
    pub bg_selected_skin_id: Option<i32>,

    pub spoof_queue: String,
    pub spoof_tier: String,
    pub spoof_division: String,
    pub online_status: String,

    // 游戏配置只读锁定状态
    pub is_game_settings_locked: bool,

    // 下拉浮层开闭状态
    pub open_lobby_mode_dropdown: bool,
    pub open_spectate_method_dropdown: bool,
    pub open_cleaner_filter_dropdown: bool,
    pub open_spoof_queue_dropdown: bool,
    pub open_spoof_tier_dropdown: bool,
    pub open_spoof_division_dropdown: bool,

    // 自定义装配弹窗状态
    pub show_badges_modal: bool,
    pub selected_badges: Vec<String>,
    pub selected_title: String,

    // 临时操作反馈文本
    pub status_feedback: Option<(String, bool)>,
}

impl Default for ToolsView {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolsView {
    pub fn new() -> Self {
        let mut open_cards = HashSet::new();
        // 默认展开高频卡片：自动接受对局
        open_cards.insert("autoaccept".to_string());

        let mut cleaner_selected = HashSet::new();
        cleaner_selected.insert("f1".to_string());
        cleaner_selected.insert("f2".to_string());

        let mut badges = Vec::new();
        badges.push("b1".to_string());
        badges.push("b2".to_string());

        Self {
            open_cards,
            hover_active_lane: "default",
            ban_active_lane: "default",
            spell_active_lane: "default",

            open_hover_picker: false,
            open_ban_picker: false,
            open_bg_champion_picker: false,
            open_bg_skin_modal: false,
            hover_picker_search: String::new(),
            ban_picker_search: String::new(),

            spell_active_slot: None,

            spectate_name_buffer: String::new(),
            spectate_method: "LCU".to_string(),
            cleaner_days: 90,
            cleaner_has_scanned: false,
            cleaner_filter: "days180".to_string(),
            cleaner_search: String::new(),
            cleaner_selected_ids: cleaner_selected,

            signature_buffer: String::new(),
            profile_skin_id_buffer: String::new(),
            bg_selected_champion: Some(157),
            bg_selected_skin_id: Some(157003),

            spoof_queue: "RANKED_SOLO_5x5".to_string(),
            spoof_tier: "CHALLENGER".to_string(),
            spoof_division: "I".to_string(),
            online_status: "chat".to_string(),

            is_game_settings_locked: false,

            open_lobby_mode_dropdown: false,
            open_spectate_method_dropdown: false,
            open_cleaner_filter_dropdown: false,
            open_spoof_queue_dropdown: false,
            open_spoof_tier_dropdown: false,
            open_spoof_division_dropdown: false,

            show_badges_modal: false,
            selected_badges: badges,
            selected_title: "t1".to_string(),

            status_feedback: None,
        }
    }

    /// 切换卡片展开折叠
    pub fn toggle_card(&mut self, card_name: &str) {
        if self.open_cards.contains(card_name) {
            self.open_cards.remove(card_name);
        } else {
            self.open_cards.insert(card_name.to_string());
        }
    }

    /// 设置临时状态反馈
    pub fn set_feedback(&mut self, msg: impl Into<String>, is_error: bool) {
        self.status_feedback = Some((msg.into(), is_error));
    }

    /// 主渲染入口（1000px 居中单列，严格 21 行）
    pub fn render_panel<F>(
        &self,
        config: &AppConfig,
        connected: bool,
        _window: &mut Window,
        on_action: F,
    ) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        div()
            .id("tools-scroll-view")
            .size_full()
            .overflow_y_scroll()
            .p(px(24.0))
            // 离线禁用样式：保持可滚动查看，但整页置灰并禁用未连接时的操作
            .when(!connected, |this| this.opacity(0.6))
            .child(
                div()
                    .max_w(px(1000.0))
                    .w_full()
                    .mx_auto()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    // 页面标题栏
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .mb(px(8.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(10.0))
                                    .child(
                                        div()
                                            .text_size(px(20.0))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(Theme::TEXT_PRIMARY)
                                            .child("其他功能"),
                                    )
                                    .when(!connected, |this| {
                                        this.child(
                                            Badge::new("LCU 未连接 · 部分功能受限")
                                                .variant(BadgeVariant::Loss),
                                        )
                                    }),
                            )
                            .when_some(self.status_feedback.as_ref(), |this, (msg, is_err)| {
                                this.child(Badge::new(msg.clone()).variant(if *is_err {
                                    BadgeVariant::Loss
                                } else {
                                    BadgeVariant::Primary
                                }))
                            }),
                    )
                    // ─────────────────────────────────────────────────────────────
                    // 1. 英雄选择组 - 5 张折叠卡
                    // ─────────────────────────────────────────────────────────────
                    .child(self.render_group_header("英雄选择"))
                    // 行 1: 折叠「自动接受对局」
                    .child(self.render_auto_accept_card(config, on_action.clone()))
                    // 行 2: 折叠「自动接受交换请求」
                    .child(self.render_auto_swap_card(config, on_action.clone()))
                    // 行 3: 折叠「自动选用/预选英雄」
                    .child(self.render_auto_hover_card(config, on_action.clone()))
                    // 行 4: 折叠「自动禁用英雄」
                    .child(self.render_auto_ban_card(config, on_action.clone()))
                    // 行 5: 折叠「自动设置召唤师技能」
                    .child(self.render_auto_spell_card(config, on_action.clone()))
                    // ─────────────────────────────────────────────────────────────
                    // 2. 游戏组 - 4 张折叠卡 + 3 个直显行
                    // ─────────────────────────────────────────────────────────────
                    .child(self.render_group_header("游戏"))
                    // 行 6: 折叠「自动游戏流程」
                    .child(self.render_auto_gameflow_card(config, on_action.clone()))
                    // 行 7: 折叠「观战」
                    .child(self.render_spectate_card(on_action.clone()))
                    // 行 8: 折叠「好友管理与僵尸号清理」
                    .child(self.render_friend_cleaner_card(on_action.clone()))
                    // 行 9: 直显板凳席开关 (贴合上方)
                    .child(self.render_bench_overlay_row(config, on_action.clone()))
                    // 行 10: 直显自动重连开关 (贴合下方)
                    .child(self.render_auto_reconnect_row(config, on_action.clone()))
                    // 行 11: 折叠「自动创建房间」
                    .child(self.render_auto_create_lobby_card(config, on_action.clone()))
                    // 行 12: 直显锁定游戏设置开关 (独立卡片)
                    .child(self.render_lock_game_settings_row(on_action.clone()))
                    // ─────────────────────────────────────────────────────────────
                    // 3. 客户端组 - 2 个直显行 (贴合)
                    // ─────────────────────────────────────────────────────────────
                    .child(self.render_group_header("客户端"))
                    // 行 13: 直显「修复客户端窗口」 (贴合上方)
                    .child(self.render_fix_window_row(on_action.clone()))
                    // 行 14: 直显「重启客户端」 (贴合下方)
                    .child(self.render_restart_client_row(on_action.clone()))
                    // ─────────────────────────────────────────────────────────────
                    // 4. 个人主页组 - 4 张折叠卡 + 3 个直显行
                    // ─────────────────────────────────────────────────────────────
                    .child(self.render_group_header("个人主页"))
                    // 行 15: 折叠「个性签名」
                    .child(self.render_signature_card(on_action.clone()))
                    // 行 16: 折叠「主页背景」
                    .child(self.render_profile_bg_card(on_action.clone()))
                    // 行 17: 折叠「段位伪装/展示」
                    .child(self.render_rank_spoof_card(on_action.clone()))
                    // 行 18: 折叠「在线状态」
                    .child(self.render_online_status_card(on_action.clone()))
                    // 行 19: 直显「装配称号与挑战勋章」 (贴合上方)
                    .child(self.render_badges_preferences_row(on_action.clone()))
                    // 行 20: 直显「卸下全部勋章」 (贴合中间)
                    .child(self.render_clear_badges_row(on_action.clone()))
                    // 行 21: 直显「卸下头像边框」 (贴合下方)
                    .child(self.render_clear_border_row(on_action.clone()))
                    .child(div().h(px(32.0))),
            )
            // 模态弹窗：自定义称号与挑战勋章装配
            .when(self.show_badges_modal, |this| {
                let act_close = on_action.clone();
                let act_toggle_b = on_action.clone();
                let act_clear = on_action.clone();
                let act_save = on_action.clone();

                this.child(badges_modal(
                    &self.selected_badges,
                    &self.selected_title,
                    move |_ev, win, app| act_close(ToolsAction::ToggleBadgesModal(false), win, app),
                    move |bid, win, app| act_toggle_b(ToolsAction::ToggleBadgeItem(bid.to_string()), win, app),
                    move |_ev, win, app| act_clear(ToolsAction::ClearBadgesModal, win, app),
                    move |_ev, win, app| act_save(ToolsAction::SaveBadgesPreferences, win, app),
                ))
            })
    }

    /// 分组标题小组件
    fn render_group_header(&self, title: &'static str) -> Div {
        div()
            .text_size(px(12.0))
            .font_weight(FontWeight::BOLD)
            .text_color(Theme::TEXT_MUTED)
            .mt(px(12.0))
            .mb(px(2.0))
            .px(px(4.0))
            .child(title)
    }

    // ─────────────────────────────────────────────────────────────
    // 英雄选择组卡片实现
    // ─────────────────────────────────────────────────────────────

    // 行 1: 自动接受对局 (折叠卡)
    fn render_auto_accept_card<F>(&self, config: &AppConfig, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let enabled = config.functions.enable_auto_accept_matching;
        let delay = config.functions.auto_accept_matching_delay;
        let open = self.open_cards.contains("autoaccept");

        let status = if enabled {
            format!("已开启 · 延迟 {} 秒", delay)
        } else {
            "未启用".to_string()
        };

        let act_toggle = on_action.clone();
        let act_switch = on_action.clone();
        let act_dec = on_action.clone();
        let act_inc = on_action.clone();

        collapse_card(
            "tools-card-autoaccept".into(),
            open,
            Some("icons/gamepad.svg"),
            "自动接受对局",
            "匹配到对局后自动在设定延迟确认就绪，防止错过排位",
            Some(status),
            move |_ev, win, app| act_toggle(ToolsAction::ToggleCollapse("autoaccept"), win, app),
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(setting_row(
                    "延迟接受时间",
                    Some("在确认对局前等待指定秒数 (0~11 秒)，模拟自然点击"),
                    number_stepper(
                        "stepper-accept-delay".into(),
                        format!("{} 秒", delay),
                        delay > 0,
                        delay < 11,
                        move |_ev, win, app| {
                            act_dec(ToolsAction::SetAutoAcceptDelay(delay.saturating_sub(1)), win, app)
                        },
                        move |_ev, win, app| {
                            act_inc(ToolsAction::SetAutoAcceptDelay((delay + 1).min(11)), win, app)
                        },
                    ),
                ))
                .child(setting_row(
                    "启用自动接受对局",
                    Some("开启后检测到匹配成功事件将自动点击接受"),
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_switch(ToolsAction::ToggleAutoAccept, win, app)
                        })
                        .child(Switch::new(enabled)),
                )),
        )
    }

    // 行 2: 自动接受交换请求 (折叠卡)
    fn render_auto_swap_card<F>(&self, config: &AppConfig, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let ceil = config.functions.auto_accept_ceil_swap;
        let champ = config.functions.auto_accept_champ_trade;
        let open = self.open_cards.contains("autoswap");

        let status = if ceil || champ {
            "已开启".to_string()
        } else {
            "未启用".to_string()
        };

        let act_toggle = on_action.clone();
        let act_ceil = on_action.clone();
        let act_champ = on_action.clone();

        collapse_card(
            "tools-card-autoswap".into(),
            open,
            Some("icons/menu.svg"),
            "自动接受交换请求",
            "队友发起楼层或英雄交换请求时自动确认，无需手动点击",
            Some(status),
            move |_ev, win, app| act_toggle(ToolsAction::ToggleCollapse("autoswap"), win, app),
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(setting_row(
                    "自动同意交换楼层",
                    Some("队友发起选人楼层顺序调整请求时自动同意"),
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_ceil(ToolsAction::ToggleCeilSwap, win, app)
                        })
                        .child(Switch::new(ceil)),
                ))
                .child(setting_row(
                    "自动同意交换英雄",
                    Some("大乱斗或常规模式中队友请求交换英雄时自动同意"),
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_champ(ToolsAction::ToggleChampTrade, win, app)
                        })
                        .child(Switch::new(champ)),
                )),
        )
    }

    // 行 3: 自动选用/预选英雄 (折叠卡 - T-V04)
    fn render_auto_hover_card<F>(&self, config: &AppConfig, on_action: F) -> impl IntoElement
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
                // 英雄选择器组件
                .child(champion_picker(
                    "hover-champ-picker".into(),
                    pick_list,
                    self.open_hover_picker,
                    &self.hover_picker_search,
                    move |_ev, win, app| act_clear(ToolsAction::ClearPickChampion(current_lane), win, app),
                    move |_ev, win, app| act_open_picker(ToolsAction::TogglePicker("hover"), win, app),
                    move |_ev, win, app| act_close_picker(ToolsAction::TogglePicker("hover"), win, app),
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
                    move |_cid, win, app| {
                        act_remove_champ(ToolsAction::ClearPickChampion(current_lane), win, app)
                    },
                )),
        )
    }

    // 行 4: 自动禁用英雄 (折叠卡 - T-V04)
    fn render_auto_ban_card<F>(&self, config: &AppConfig, on_action: F) -> impl IntoElement
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
                // 英雄选择器组件
                .child(champion_picker(
                    "ban-champ-picker".into(),
                    ban_list,
                    self.open_ban_picker,
                    &self.ban_picker_search,
                    move |_ev, win, app| act_clear(ToolsAction::ClearBanChampion(current_lane), win, app),
                    move |_ev, win, app| act_open_picker(ToolsAction::TogglePicker("ban"), win, app),
                    move |_ev, win, app| act_close_picker(ToolsAction::TogglePicker("ban"), win, app),
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
                    move |_cid, win, app| {
                        act_remove_champ(ToolsAction::ClearBanChampion(current_lane), win, app)
                    },
                )),
        )
    }

    // 行 5: 自动设置召唤师技能 (折叠卡 - T-V05)
    fn render_auto_spell_card<F>(&self, config: &AppConfig, on_action: F) -> impl IntoElement
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

    // ─────────────────────────────────────────────────────────────
    // 游戏组卡片与直显行实现
    // ─────────────────────────────────────────────────────────────

    // 行 6: 自动游戏流程 (折叠卡 - T-V06)
    fn render_auto_gameflow_card<F>(&self, config: &AppConfig, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let invite = config.functions.enable_auto_handle_invite;
        let honor = config.functions.enable_auto_honor;
        let again = config.functions.enable_auto_play_again;
        let aram_side = config.functions.enable_auto_aram_team_side;
        let team_visible = config.functions.aram_team_side_visible_to_team;
        let tag_remind = config.functions.enable_auto_tag_reminder;
        let open = self.open_cards.contains("autogameflow");

        let any_enabled = invite || honor || again || aram_side || tag_remind;

        let act_toggle = on_action.clone();
        let act_invite = on_action.clone();
        let act_honor = on_action.clone();
        let act_again = on_action.clone();
        let act_aram = on_action.clone();
        let act_team = on_action.clone();
        let act_tag = on_action.clone();

        collapse_card(
            "tools-card-autogameflow".into(),
            open,
            Some("icons/gamepad.svg"),
            "自动游戏流程与大厅",
            "自动接受组队邀请、游戏结算自动点赞、再来一局及 ARAM 阵营预测",
            Some(if any_enabled { "已配置" } else { "未启用" }.to_string()),
            move |_ev, win, app| act_toggle(ToolsAction::ToggleCollapse("autogameflow"), win, app),
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(setting_row(
                    "自动接受好友组队邀请",
                    Some("收到游戏好友发起的组队邀请时自动同意并进入房间"),
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_invite(ToolsAction::ToggleAutoHandleInvite, win, app)
                        })
                        .child(Switch::new(invite)),
                ))
                .child(setting_row(
                    "结算自动点赞队友",
                    Some("游戏结束后自动为表现最佳或随机队友点赞，获取荣誉点数"),
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_honor(ToolsAction::ToggleAutoHonor, win, app)
                        })
                        .child(Switch::new(honor)),
                ))
                .child(setting_row(
                    "结算自动「再来一局」",
                    Some("游戏结算页面停留指定时间后自动点击再来一局返回房间"),
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_again(ToolsAction::ToggleAutoPlayAgain, win, app)
                        })
                        .child(Switch::new(again)),
                ))
                .child(setting_row(
                    "极地大乱斗报边与阵营预测",
                    Some("选人阶段提前获知己方位于蓝色方或红色方并发送聊天消息"),
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_aram(ToolsAction::ToggleAramTeamSide, win, app)
                        })
                        .child(Switch::new(aram_side)),
                ))
                .child(setting_row(
                    "阵营预测仅队伍可见",
                    Some("预测阵营仅发送给友方队伍通道，不向全局大厅透漏"),
                    div()
                        .cursor_pointer()
                        .when(aram_side, |s| {
                            s.on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                act_team(ToolsAction::ToggleAramTeamSideVisible, win, app)
                            })
                        })
                        .child(Switch::new(team_visible)),
                ))
                .child(setting_row(
                    "选人阶段带标记玩家提醒",
                    Some("进入选人房间时若检测到带有标签的路人或好友，发送聊天栏提示"),
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_tag(ToolsAction::ToggleTagReminder, win, app)
                        })
                        .child(Switch::new(tag_remind)),
                )),
        )
    }

    // 行 7: 实时直接观战召唤师 (折叠卡 - T-V07)
    fn render_spectate_card<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let open = self.open_cards.contains("spectate");
        let act_toggle = on_action.clone();
        let act_launch = on_action.clone();
        let act_toggle_dd = on_action.clone();
        let act_select_method = on_action.clone();

        collapse_card(
            "tools-card-spectate".into(),
            open,
            Some("icons/search.svg"),
            "实时直接观战召唤师",
            "输入游戏玩家召唤师名称或 Riot ID，直接调用游戏客户端引擎启动观战",
            Some("点击展开".to_string()),
            move |_ev, win, app| act_toggle(ToolsAction::ToggleCollapse("spectate"), win, app),
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                // 行 1: 召唤师输入框带内嵌后缀按钮
                .child(setting_row(
                    "召唤师名称",
                    Some("输入目标召唤师昵称#TAG，直接启动对局实时观战"),
                    suffix_input(
                        "input-spectate-name".into(),
                        &self.spectate_name_buffer,
                        "输入游戏昵称#TAG...",
                        "启动观战",
                        false,
                        300.0,
                        move |_ev, win, app| act_launch(ToolsAction::LaunchSpectate, win, app),
                    ),
                ))
                // 行 2: 120px 启动方式下拉框
                .child(setting_row(
                    "启动方式",
                    Some("选择通过 LCU 原生客户端接口或 CMD 独立拉起游戏本体"),
                    select_dropdown(
                        "dropdown-spectate-method",
                        120.0,
                        if self.spectate_method == "CMD" { "CMD" } else { "LCU API" }.to_string(),
                        self.open_spectate_method_dropdown,
                        false,
                        vec![("LCU", "LCU API"), ("CMD", "CMD")],
                        move |win, app| act_toggle_dd(ToolsAction::ToggleDropdownMenu("spectate_method"), win, app),
                        move |val, win, app| act_select_method(ToolsAction::SetSpectateMethod(val.to_string()), win, app),
                    ),
                )),
        )
    }

    // 行 8: 好友管理与僵尸号清理 (折叠卡 - T-V08)
    fn render_friend_cleaner_card<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let open = self.open_cards.contains("friendcleaner");
        let act_toggle = on_action.clone();
        let act_scan = on_action.clone();
        let act_del = on_action.clone();
        let act_toggle_filter = on_action.clone();
        let act_select_filter = on_action.clone();
        let act_select_all = on_action.clone();
        let act_clear_sel = on_action.clone();

        let filter_label = match self.cleaner_filter.as_str() {
            "days360" => "超过 360 天未打游戏 (23)",
            "days90" => "超过 90 天未打游戏 (62)",
            "never" => "从未打过游戏 (5)",
            "hidden" => "🔒 战绩已隐藏 (8)",
            "all" => "全部好友 (92)",
            _ => "超过 180 天未打游戏 (48)",
        };

        collapse_card(
            "tools-card-friendcleaner".into(),
            open,
            Some("icons/user.svg"),
            "好友管理与僵尸号清理",
            "智能检索超过设定天数未上线的游戏好友，支持一键批量移除",
            Some(if self.cleaner_has_scanned {
                format!("已扫描 · 筛选出 {} 人", MOCK_FRIENDS.len())
            } else {
                "未扫描".to_string()
            }),
            move |_ev, win, app| act_toggle(ToolsAction::ToggleCollapse("friendcleaner"), win, app),
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                // 未扫描态：仅显示扫描好友按钮
                .when(!self.cleaner_has_scanned, |this| {
                    let scan_cb = act_scan.clone();
                    this.child(
                        div()
                            .p(px(16.0))
                            .rounded_md()
                            .bg(Theme::HOVER_BG)
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(Theme::TEXT_PRIMARY)
                                            .child("扫描好友列表并自动检测每位好友的最后活跃时间"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.5))
                                            .text_color(Theme::TEXT_MUTED)
                                            .child("穿透查询最近对局与熟练度时间，保护隐藏战绩玩家不被误删"),
                                    ),
                            )
                            .child(
                                div()
                                    .px(px(16.0))
                                    .py(px(6.0))
                                    .rounded_md()
                                    .bg(Theme::ACCENT_PRIMARY)
                                    .text_size(px(12.5))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(gpui::white())
                                    .cursor_pointer()
                                    .hover(|s| s.opacity(0.85))
                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                        scan_cb(ToolsAction::ScanFriends, win, app)
                                    })
                                    .child("开始扫描好友"),
                            ),
                    )
                })
                // 已扫描态：操作栏 + 列表
                .when(self.cleaner_has_scanned, |this| {
                    let scan_cb = act_scan.clone();
                    this.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(10.0))
                            // 顶行：重新扫描 + 250px 筛选下拉 + 170px 搜索框
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap(px(10.0))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(10.0))
                                            .child(
                                                div()
                                                    .px(px(12.0))
                                                    .py(px(5.0))
                                                    .rounded_md()
                                                    .bg(Theme::ACCENT_PRIMARY)
                                                    .text_size(px(12.0))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(gpui::white())
                                                    .cursor_pointer()
                                                    .hover(|s| s.opacity(0.85))
                                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                                        scan_cb(ToolsAction::ScanFriends, win, app)
                                                    })
                                                    .child("重新扫描好友"),
                                            )
                                            .child(select_dropdown(
                                                "dropdown-cleaner-filter",
                                                250.0,
                                                filter_label.to_string(),
                                                self.open_cleaner_filter_dropdown,
                                                false,
                                                vec![
                                                    ("days180", "超过 180 天未打游戏 (48)"),
                                                    ("days360", "超过 360 天未打游戏 (23)"),
                                                    ("days90", "超过 90 天未打游戏 (62)"),
                                                    ("never", "从未打过游戏 (5)"),
                                                    ("hidden", "🔒 战绩已隐藏 (8)"),
                                                    ("all", "全部好友 (92)"),
                                                ],
                                                move |win, app| act_toggle_filter(ToolsAction::ToggleDropdownMenu("cleaner_filter"), win, app),
                                                move |val, win, app| act_select_filter(ToolsAction::SetCleanerFilter(val.to_string()), win, app),
                                            )),
                                    )
                                    .child(
                                        div()
                                            .w(px(170.0))
                                            .h(px(28.0))
                                            .rounded_md()
                                            .border_1()
                                            .border_color(Theme::CARD_BORDER)
                                            .bg(Theme::CARD_BG)
                                            .px(px(8.0))
                                            .flex()
                                            .items_center()
                                            .text_size(px(11.5))
                                            .text_color(Theme::TEXT_MUTED)
                                            .child("搜索游戏名/备注..."),
                                    ),
                            )
                            // 次行：全选、清空勾选、批量删除按钮
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(8.0))
                                            .child(
                                                div()
                                                    .px(px(10.0))
                                                    .py(px(4.0))
                                                    .rounded_md()
                                                    .border_1()
                                                    .border_color(Theme::CARD_BORDER)
                                                    .bg(Theme::CARD_BG)
                                                    .text_size(px(11.5))
                                                    .text_color(Theme::TEXT_SECONDARY)
                                                    .cursor_pointer()
                                                    .hover(|s| s.bg(Theme::HOVER_BG))
                                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                                        act_select_all(ToolsAction::SelectAllCleanerFriends, win, app)
                                                    })
                                                    .child("勾选当前列表"),
                                            )
                                            .child(
                                                div()
                                                    .px(px(10.0))
                                                    .py(px(4.0))
                                                    .rounded_md()
                                                    .border_1()
                                                    .border_color(Theme::CARD_BORDER)
                                                    .bg(Theme::CARD_BG)
                                                    .text_size(px(11.5))
                                                    .text_color(Theme::TEXT_SECONDARY)
                                                    .cursor_pointer()
                                                    .hover(|s| s.bg(Theme::HOVER_BG))
                                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                                        act_clear_sel(ToolsAction::ClearSelectedCleanerFriends, win, app)
                                                    })
                                                    .child("清空勾选"),
                                            ),
                                    )
                                    .child(danger_button(
                                        format!("批量删除选中的 {} 人", self.cleaner_selected_ids.len()),
                                        move |_ev, win, app| act_del(ToolsAction::BatchDeleteOfflineFriends, win, app),
                                    )),
                            )
                            // 好友列表
                            .child(
                                div()
                                    .id("cleaner-friends-list-scroll")
                                    .w_full()
                                    .max_h(px(220.0))
                                    .overflow_y_scroll()
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.0))
                                    .children(MOCK_FRIENDS.iter().map(|f| {
                                        let fid = f.id.to_string();
                                        let is_selected = self.cleaner_selected_ids.contains(&fid);
                                        let act_check = on_action.clone();

                                        div()
                                            .w_full()
                                            .px(px(12.0))
                                            .py(px(6.0))
                                            .rounded_md()
                                            .border_1()
                                            .border_color(if is_selected {
                                                Theme::SIDEBAR_ACTIVE_BORDER
                                            } else {
                                                Theme::CARD_BORDER
                                            })
                                            .bg(if is_selected {
                                                Theme::ACCENT_LIGHT
                                            } else {
                                                Theme::CARD_BG
                                            })
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .cursor_pointer()
                                            .hover(|s| s.border_color(Theme::ACCENT_PRIMARY))
                                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                                act_check(ToolsAction::ToggleSelectCleanerFriend(fid.clone()), win, app)
                                            })
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(10.0))
                                                    .child(
                                                        div()
                                                            .size(px(16.0))
                                                            .rounded(px(3.0))
                                                            .border_1()
                                                            .border_color(if is_selected {
                                                                Theme::ACCENT_PRIMARY
                                                            } else {
                                                                Theme::CARD_BORDER
                                                            })
                                                            .bg(if is_selected {
                                                                Theme::ACCENT_PRIMARY
                                                            } else {
                                                                Theme::CARD_BG
                                                            })
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .text_size(px(10.0))
                                                            .text_color(gpui::white())
                                                            .child(if is_selected { "✓" } else { "" }),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(12.5))
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(Theme::TEXT_PRIMARY)
                                                            .child(f.name),
                                                    )
                                                    .when(!f.note.is_empty(), |s| {
                                                        s.child(
                                                            div()
                                                                .text_size(px(11.0))
                                                                .text_color(Theme::TEXT_MUTED)
                                                                .child(format!("({})", f.note)),
                                                        )
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.5))
                                                    .text_color(Theme::LOSS_TEXT)
                                                    .child(format!("离线 {} 天", f.days)),
                                            )
                                    })),
                            ),
                    )
                }),
        )
    }

    // 行 9: 大乱斗板凳席悬浮窗 (直显行 - T-V02)
    fn render_bench_overlay_row<F>(&self, config: &AppConfig, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let enabled = config.functions.enable_bench_overlay;
        let act_switch = on_action.clone();

        direct_card_row(
            "tools-row-benchoverlay".into(),
            CardGroupPosition::Top,
            "icons/gamepad.svg",
            "大乱斗板凳席独立悬浮窗",
            "极地大乱斗选人时弹出轻量透明置顶悬浮窗，直观呈现全队掷点板凳席英雄",
            div()
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                    act_switch(ToolsAction::ToggleBenchOverlay, win, app)
                })
                .child(Switch::new(enabled)),
        )
    }

    // 行 10: 掉线自动重连 (直显行 - T-V02)
    fn render_auto_reconnect_row<F>(&self, config: &AppConfig, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let enabled = config.functions.enable_auto_reconnect;
        let act_switch = on_action.clone();

        direct_card_row(
            "tools-row-autoreconnect".into(),
            CardGroupPosition::Bottom,
            "icons/restore.svg",
            "掉线自动重新连接",
            "检测到对局意外断线或客户端处于 Reconnect 状态时，自动触发重连",
            div()
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                    act_switch(ToolsAction::ToggleAutoReconnect, win, app)
                })
                .child(Switch::new(enabled)),
        )
    }

    // 行 11: 自动创建房间 (折叠卡 - T-V09)
    fn render_auto_create_lobby_card<F>(&self, config: &AppConfig, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let enabled = config.functions.enable_auto_create_lobby;
        let mode_id = config.functions.default_game_mode;
        let open = self.open_cards.contains("autocreatelobby");

        let mode_name = GAME_MODES
            .iter()
            .find(|(id, _)| *id == mode_id)
            .map(|(_, n)| *n)
            .unwrap_or("海克斯大乱斗");

        let act_toggle = on_action.clone();
        let act_switch = on_action.clone();
        let act_toggle_dd = on_action.clone();
        let act_select_mode = on_action.clone();

        collapse_card(
            "tools-card-autocreatelobby".into(),
            open,
            Some("icons/home.svg"),
            "自动创建房间",
            "客户端连接后或快捷一键创建常用对局模式房间",
            Some(if enabled {
                format!("已开启 · {}", mode_name)
            } else {
                "未启用".to_string()
            }),
            move |_ev, win, app| act_toggle(ToolsAction::ToggleCollapse("autocreatelobby"), win, app),
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(setting_row(
                    "开启自动创建房间",
                    Some("在游戏处于大厅空闲时自动进入指定模式组队房间"),
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_switch(ToolsAction::ToggleAutoCreateLobby, win, app)
                        })
                        .child(Switch::new(enabled)),
                ))
                // 140px 默认房间模式下拉框（关闭时呈禁用态）
                .child(setting_row(
                    "默认房间模式",
                    Some("选择常用游戏队列或模式"),
                    select_dropdown(
                        "dropdown-lobby-mode",
                        140.0,
                        mode_name.to_string(),
                        self.open_lobby_mode_dropdown,
                        !enabled,
                        GAME_MODE_OPTIONS.to_vec(),
                        move |win, app| act_toggle_dd(ToolsAction::ToggleDropdownMenu("lobby_mode"), win, app),
                        move |val, win, app| {
                            if let Ok(mid) = val.parse::<u32>() {
                                act_select_mode(ToolsAction::SetDefaultGameMode(mid), win, app);
                            }
                        },
                    ),
                )),
        )
    }

    // 行 12: 锁定游戏设置 (直显行 - T-V02)
    fn render_lock_game_settings_row<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let locked = self.is_game_settings_locked;
        let act_switch = on_action.clone();

        direct_card_row(
            "tools-row-locksettings".into(),
            CardGroupPosition::Single,
            "icons/lock.svg",
            "锁定游戏设置 (只读保护)",
            "锁定 Game.cfg 与 PersistedSettings.json 为只读，防止客户端意外重置键位与画质",
            div()
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                    act_switch(ToolsAction::ToggleLockGameSettings, win, app)
                })
                .child(Switch::new(locked)),
        )
    }

    // ─────────────────────────────────────────────────────────────
    // 客户端管理组直显行实现 (T-V01)
    // ─────────────────────────────────────────────────────────────

    // 行 13: 修复客户端窗口 (直显行)
    fn render_fix_window_row<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let act_fix = on_action.clone();

        direct_card_row(
            "tools-row-fixwindow".into(),
            CardGroupPosition::Top,
            "icons/maximize.svg",
            "修复客户端窗口大小与位置",
            "重置因分辨率错位导致的黑边，一键恢复 1600x900 100% 原始缩放并居中",
            div()
                .px(px(16.0))
                .py(px(5.0))
                .rounded_md()
                .border_1()
                .border_color(Theme::CARD_BORDER)
                .bg(Theme::CARD_BG)
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(Theme::TEXT_PRIMARY)
                .cursor_pointer()
                .hover(|s| s.bg(Theme::HOVER_BG).border_color(Theme::ACCENT_PRIMARY))
                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                    act_fix(ToolsAction::FixClientWindow, win, app)
                })
                .child("修复"),
        )
    }

    // 行 14: 重启客户端 (直显行)
    fn render_restart_client_row<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let act_restart = on_action.clone();

        direct_card_row(
            "tools-row-restartclient".into(),
            CardGroupPosition::Bottom,
            "icons/restore.svg",
            "重启客户端 (LeagueClientUx)",
            "安全重启英雄联盟前端渲染进程，解决界面卡死、白屏或无响应",
            div()
                .px(px(16.0))
                .py(px(5.0))
                .rounded_md()
                .border_1()
                .border_color(Theme::CARD_BORDER)
                .bg(Theme::CARD_BG)
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(Theme::TEXT_PRIMARY)
                .cursor_pointer()
                .hover(|s| s.bg(Theme::HOVER_BG).border_color(Theme::ACCENT_PRIMARY))
                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                    act_restart(ToolsAction::RestartClient, win, app)
                })
                .child("重启"),
        )
    }

    // ─────────────────────────────────────────────────────────────
    // 个人主页与社交装扮组实现 (T-V10, T-V11, T-V03)
    // ─────────────────────────────────────────────────────────────

    // 行 15: 自定义个性签名 (折叠卡 - T-V10)
    fn render_signature_card<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let open = self.open_cards.contains("signature");
        let act_toggle = on_action.clone();
        let act_apply = on_action.clone();

        collapse_card(
            "tools-card-signature".into(),
            open,
            Some("icons/copy.svg"),
            "修改个人签名",
            "修改好友列表中展示的个性化状态签名文本",
            Some("点击展开".to_string()),
            move |_ev, win, app| act_toggle(ToolsAction::ToggleCollapse("signature"), win, app),
            setting_row(
                "输入新的个性化签名:",
                Some("留空将清空当前展示的状态签名"),
                suffix_input(
                    "input-signature".into(),
                    &self.signature_buffer,
                    "输入新签名...",
                    "保存签名",
                    false,
                    300.0,
                    move |_ev, win, app| act_apply(ToolsAction::ApplySignature, win, app),
                ),
            ),
        )
    }

    // 行 16: 修改个人主页背景 (折叠卡 - T-V10)
    fn render_profile_bg_card<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let open = self.open_cards.contains("profilebg");
        let act_toggle = on_action.clone();
        let act_open_picker = on_action.clone();
        let act_close_picker = on_action.clone();
        let act_select_champ = on_action.clone();
        let act_select_skin = on_action.clone();

        let current_champ_id = self.bg_selected_champion.unwrap_or(157);
        let champ_ids = vec![current_champ_id];

        collapse_card(
            "tools-card-profilebg".into(),
            open,
            Some("icons/user.svg"),
            "修改个人主页背景",
            "设置个人生涯资料卡片背景展示的英雄及皮肤",
            Some("点击展开".to_string()),
            move |_ev, win, app| act_toggle(ToolsAction::ToggleCollapse("profilebg"), win, app),
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(Theme::TEXT_MUTED)
                        .child("选择背景展示的英雄与皮肤立绘:"),
                )
                // 单英雄选择器
                .child(champion_picker(
                    "bg-champ-picker".into(),
                    &champ_ids,
                    self.open_bg_champion_picker,
                    "",
                    move |_ev, _win, _app| {},
                    move |_ev, win, app| act_open_picker(ToolsAction::TogglePicker("bg_champ"), win, app),
                    move |_ev, win, app| act_close_picker(ToolsAction::TogglePicker("bg_champ"), win, app),
                    move |cid, win, app| act_select_champ(ToolsAction::SelectBgChampion(cid), win, app),
                    move |_cid, _win, _app| {},
                ))
                // 皮肤预览与选择
                .child(
                    div()
                        .p(px(12.0))
                        .rounded_md()
                        .border_1()
                        .border_color(Theme::CARD_BORDER)
                        .bg(Theme::HOVER_BG)
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.0))
                                .child(
                                    div()
                                        .size(px(36.0))
                                        .rounded(px(4.0))
                                        .bg(Theme::ACCENT_LIGHT)
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_size(px(14.0))
                                        .child("🖼️"),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(2.0))
                                        .child(
                                            div()
                                                .text_size(px(13.0))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(Theme::TEXT_PRIMARY)
                                                .child("当前已选皮肤: 黑夜使者 亚索"),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(11.0))
                                                .text_color(Theme::TEXT_MUTED)
                                                .child("皮肤 ID: 157003 · 支持一键更换并应用到名片"),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .px(px(14.0))
                                .py(px(5.0))
                                .rounded_md()
                                .bg(Theme::ACCENT_PRIMARY)
                                .text_size(px(12.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(gpui::white())
                                .cursor_pointer()
                                .hover(|s| s.opacity(0.85))
                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                    act_select_skin(ToolsAction::SelectBgSkin(157003), win, app)
                                })
                                .child("应用此皮肤背景"),
                        ),
                ),
        )
    }

    // 行 17: 修改段位展示 (折叠卡 - T-V11)
    fn render_rank_spoof_card<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let open = self.open_cards.contains("rankspoof");
        let act_toggle = on_action.clone();
        let act_apply = on_action.clone();
        let act_toggle_q = on_action.clone();
        let act_toggle_t = on_action.clone();
        let act_toggle_d = on_action.clone();
        let act_select_q = on_action.clone();
        let act_select_t = on_action.clone();
        let act_select_d = on_action.clone();

        let queue_label = SPOOF_QUEUES
            .iter()
            .find(|(q, _)| *q == self.spoof_queue)
            .map(|(_, n)| *n)
            .unwrap_or("单双排位");

        let tier_label = SPOOF_TIERS
            .iter()
            .find(|(t, _)| *t == self.spoof_tier)
            .map(|(_, n)| *n)
            .unwrap_or("最强王者");

        // 王者、宗师、大师、未定级时小段禁用
        let division_disabled = matches!(
            self.spoof_tier.as_str(),
            "CHALLENGER" | "GRANDMASTER" | "MASTER" | "UNRANKED"
        );

        collapse_card(
            "tools-card-rankspoof".into(),
            open,
            Some("icons/bookmark.svg"),
            "修改生涯与卡片段位展示",
            "修改好友聊天中展示的伪装段位图标与阶位",
            Some("点击展开".to_string()),
            move |_ev, win, app| act_toggle(ToolsAction::ToggleCollapse("rankspoof"), win, app),
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                // 行 1: 140px 队列下拉
                .child(setting_row(
                    "选择展示队列",
                    Some("选择要在个人卡片上模拟的排位赛模式"),
                    select_dropdown(
                        "dropdown-spoof-queue",
                        140.0,
                        queue_label.to_string(),
                        self.open_spoof_queue_dropdown,
                        false,
                        SPOOF_QUEUES.to_vec(),
                        move |win, app| act_toggle_q(ToolsAction::ToggleDropdownMenu("spoof_queue"), win, app),
                        move |val, win, app| act_select_q(ToolsAction::SetSpoofQueue(val.to_string()), win, app),
                    ),
                ))
                // 行 2: 段位与阶位 (130px 段位 + 80px 小段 + 应用按钮)
                .child(setting_row(
                    "段位与阶位",
                    Some("设定好友名片与浮窗预览呈现的段位阶层"),
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(select_dropdown(
                            "dropdown-spoof-tier",
                            130.0,
                            tier_label.to_string(),
                            self.open_spoof_tier_dropdown,
                            false,
                            SPOOF_TIERS.to_vec(),
                            move |win, app| act_toggle_t(ToolsAction::ToggleDropdownMenu("spoof_tier"), win, app),
                            move |val, win, app| act_select_t(ToolsAction::SetSpoofTier(val.to_string()), win, app),
                        ))
                        .child(select_dropdown(
                            "dropdown-spoof-div",
                            80.0,
                            self.spoof_division.clone(),
                            self.open_spoof_division_dropdown,
                            division_disabled,
                            SPOOF_DIVISIONS.iter().map(|&d| (d, d)).collect(),
                            move |win, app| act_toggle_d(ToolsAction::ToggleDropdownMenu("spoof_division"), win, app),
                            move |val, win, app| act_select_d(ToolsAction::SetSpoofDivision(val.to_string()), win, app),
                        ))
                        .child(
                            div()
                                .px(px(12.0))
                                .py(px(5.0))
                                .rounded_md()
                                .bg(Theme::ACCENT_PRIMARY)
                                .text_size(px(12.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(gpui::white())
                                .cursor_pointer()
                                .hover(|s| s.opacity(0.85))
                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                    act_apply(ToolsAction::ApplyRankSpoof, win, app)
                                })
                                .child("应用伪装段位"),
                        ),
                )),
        )
    }

    // 行 18: 修改在线状态 (折叠卡 - T-V11)
    fn render_online_status_card<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let open = self.open_cards.contains("onlinestatus");
        let act_toggle = on_action.clone();
        let act_chat = on_action.clone();
        let act_away = on_action.clone();
        let act_offline = on_action.clone();

        let status_desc = match self.online_status.as_str() {
            "away" => "当前状态: 离开 (Away)",
            "offline" | "mobile" => "当前状态: 离线 (Offline)",
            _ => "当前状态: 在线 (Online)",
        };

        collapse_card(
            "tools-card-onlinestatus".into(),
            open,
            Some("icons/menu.svg"),
            "修改在线状态",
            "自由切换在好友列表中的显示状态 (在线 / 离开 / 移动端离线)",
            Some(status_desc.to_string()),
            move |_ev, win, app| act_toggle(ToolsAction::ToggleCollapse("onlinestatus"), win, app),
            setting_row(
                "在线状态",
                Some("切换你在好友列表中的当前状态呈现"),
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(self.render_status_btn("设为在线", self.online_status == "chat", move |win, app| {
                        act_chat(ToolsAction::SetOnlineStatus("chat".to_string()), win, app)
                    }))
                    .child(self.render_status_btn("设为离开", self.online_status == "away", move |win, app| {
                        act_away(ToolsAction::SetOnlineStatus("away".to_string()), win, app)
                    }))
                    .child(self.render_status_btn("设为离线", self.online_status == "offline", move |win, app| {
                        act_offline(ToolsAction::SetOnlineStatus("offline".to_string()), win, app)
                    })),
            ),
        )
    }

    fn render_status_btn<F>(&self, label: &'static str, active: bool, on_click: F) -> Div
    where
        F: Fn(&mut Window, &mut App) + 'static + Clone,
    {
        div()
            .px(px(14.0))
            .py(px(5.0))
            .rounded_md()
            .border_1()
            .border_color(if active {
                Theme::ACCENT_PRIMARY
            } else {
                Theme::CARD_BORDER
            })
            .bg(if active {
                Theme::ACCENT_LIGHT
            } else {
                Theme::CARD_BG
            })
            .text_size(px(12.0))
            .font_weight(if active {
                FontWeight::BOLD
            } else {
                FontWeight::MEDIUM
            })
            .text_color(if active {
                Theme::ACCENT_PRIMARY
            } else {
                Theme::TEXT_PRIMARY
            })
            .cursor_pointer()
            .hover(|s| s.border_color(Theme::ACCENT_PRIMARY))
            .on_mouse_down(MouseButton::Left, move |_ev, win, app| on_click(win, app))
            .child(label)
    }

    // 行 19: 自定义装配 (直显行 - T-V03)
    fn render_badges_preferences_row<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let act_open = on_action.clone();

        direct_card_row(
            "tools-row-badges".into(),
            CardGroupPosition::Top,
            "icons/bookmark.svg",
            "装配称号与挑战勋章",
            "自定义佩戴个人名片的称号与 3 枚挑战勋章",
            div()
                .px(px(14.0))
                .py(px(5.0))
                .rounded_md()
                .border_1()
                .border_color(Theme::ACCENT_PRIMARY)
                .bg(Theme::CARD_BG)
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(Theme::ACCENT_PRIMARY)
                .cursor_pointer()
                .hover(|s| s.bg(Theme::ACCENT_LIGHT))
                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                    act_open(ToolsAction::ToggleBadgesModal(true), win, app)
                })
                .child("自定义装配"),
        )
    }

    // 行 20: 卸下全部勋章 (直显行 - T-V03)
    fn render_clear_badges_row<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let act_clear = on_action.clone();

        direct_card_row(
            "tools-row-clearbadges".into(),
            CardGroupPosition::Middle,
            "icons/close.svg",
            "卸下全部勋章",
            "一键卸下所有已佩戴的挑战勋章",
            danger_button("卸下", move |_ev, win, app| {
                act_clear(ToolsAction::ClearBadges, win, app)
            }),
        )
    }

    // 行 21: 卸下头像边框 (直显行 - T-V03)
    fn render_clear_border_row<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let act_clear = on_action.clone();

        direct_card_row(
            "tools-row-clearborder".into(),
            CardGroupPosition::Bottom,
            "icons/close.svg",
            "卸下头像边框",
            "移除个人主页中配置的旗帜与头像边框",
            danger_button("卸下", move |_ev, win, app| {
                act_clear(ToolsAction::ClearBorder, win, app)
            }),
        )
    }
}
