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

pub mod client_ops;
pub mod components;
pub mod data;
pub mod personalization;
pub mod selectors;
pub mod spectate;

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

    /// 好友扫描结果（来自 LCU `/lol-chat/v1/friends` 真实数据） // [D01]
    pub cleaner_friends: Vec<crate::services::friend_cleaner::CleanerFriend>, // [D01]
    /// 好友扫描进行中 // [D01]
    pub cleaner_scanning: bool, // [D01]
    /// 好友搜索框文本（文本框回调直接写入，渲染时参与真实过滤） // [D01]
    pub cleaner_search_live: std::rc::Rc<std::cell::RefCell<String>>, // [D01]
    /// 观战名称输入框焦点（首次点击惰性创建） // [D01]
    pub spectate_name_focus: std::rc::Rc<std::cell::OnceCell<FocusHandle>>, // [D01]
    /// 好友搜索框焦点（首次点击惰性创建） // [D01]
    pub cleaner_search_focus: std::rc::Rc<std::cell::OnceCell<FocusHandle>>, // [D01]

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

    /// 真实皮肤列表（选英雄后由 services::profile_background::load_skins 回写） // [D09/D11]
    pub bg_skins: Vec<crate::services::profile_background::SkinInfo>, // [D09/D11]
    /// 皮肤预览索引（上一个/下一个循环，取消零写入） // [D09/D11]
    pub bg_preview_index: usize, // [D09/D11]
    /// 皮肤列表加载中 // [D09/D11]
    pub bg_loading: bool, // [D09/D11]
    /// 加载/写入失败文案（含重试入口） // [D09/D11]
    pub bg_error: Option<String>, // [D09/D11]
    /// 回读到的当前主页背景皮肤 ID // [D09/D11]
    pub bg_current_skin_id: Option<i32>, // [D09/D11]
    /// 打开装配弹层时的选中快照（取消回滚，零写入） // [D09/D11]
    pub badges_snapshot_badges: Vec<String>, // [D09/D11]
    pub badges_snapshot_title: String, // [D09/D11]
    /// 清除勋章 / 清除边框的二次确认框 // [D09/D11]
    pub confirm_clear_badges: bool, // [D09/D11]
    pub confirm_clear_border: bool,    // [D09/D11]

    // 临时操作反馈文本
    pub status_feedback: Option<(String, bool)>,

    /// 选用/禁用选择器搜索词（文本输入回调直接写入，渲染时参与过滤） // [D04-D07]
    pub hover_picker_search_live: std::rc::Rc<std::cell::RefCell<String>>, // [D04-D07]
    pub ban_picker_search_live: std::rc::Rc<std::cell::RefCell<String>>, // [D04-D07]
    /// 选择器搜索框焦点（首次点击惰性创建） // [D04-D07]
    pub hover_picker_focus: std::rc::Rc<std::cell::OnceCell<FocusHandle>>, // [D04-D07]
    pub ban_picker_focus: std::rc::Rc<std::cell::OnceCell<FocusHandle>>, // [D04-D07]
    /// 打开选择器前的 FunctionsConfig 快照（取消回滚，零写入） // [D04-D07]
    pub picker_backup: Option<Box<yuumi_core::config::FunctionsConfig>>, // [D04-D07]
}

impl Default for ToolsView {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolsView {
    pub fn new() -> Self {
        let open_cards = HashSet::new();

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
            cleaner_selected_ids: HashSet::new(),

            cleaner_friends: Vec::new(), // [D01]
            cleaner_scanning: false,     // [D01]
            cleaner_search_live: std::rc::Rc::new(std::cell::RefCell::new(String::new())), // [D01]
            spectate_name_focus: std::rc::Rc::new(std::cell::OnceCell::new()), // [D01]
            cleaner_search_focus: std::rc::Rc::new(std::cell::OnceCell::new()), // [D01]

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
            // 不再使用 b1/b2/t1 模拟数据：打开弹层时由真实偏好回读覆盖 // [D09/D11]
            selected_badges: Vec::new(),   // [D09/D11]
            selected_title: String::new(), // [D09/D11]

            bg_skins: Vec::new(),                 // [D09/D11]
            bg_preview_index: 0,                  // [D09/D11]
            bg_loading: false,                    // [D09/D11]
            bg_error: None,                       // [D09/D11]
            bg_current_skin_id: None,             // [D09/D11]
            badges_snapshot_badges: Vec::new(),   // [D09/D11]
            badges_snapshot_title: String::new(), // [D09/D11]
            confirm_clear_badges: false,          // [D09/D11]
            confirm_clear_border: false,          // [D09/D11]

            status_feedback: None,

            hover_picker_search_live: std::rc::Rc::new(std::cell::RefCell::new(String::new())), // [D04-D07]
            ban_picker_search_live: std::rc::Rc::new(std::cell::RefCell::new(String::new())), // [D04-D07]
            hover_picker_focus: std::rc::Rc::new(std::cell::OnceCell::new()), // [D04-D07]
            ban_picker_focus: std::rc::Rc::new(std::cell::OnceCell::new()),   // [D04-D07]
            picker_backup: None,                                              // [D04-D07]
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
                                            .text_color(Theme::TEXT_PRIMARY())
                                            .child("其他功能"),
                                    )
                                    .when(!connected, |this| {
                                        this.child(
                                            Badge::new("LCU 未连接 · 所有功能已禁用")
                                                .variant(BadgeVariant::Loss),
                                        )
                                    }),
                            ),
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
    }

    /// 分组标题小组件
    fn render_group_header(&self, title: &'static str) -> Div {
        div()
            .text_size(px(12.0))
            .font_weight(FontWeight::BOLD)
            .text_color(Theme::TEXT_MUTED())
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
                            act_dec(
                                ToolsAction::SetAutoAcceptDelay(delay.saturating_sub(1)),
                                win,
                                app,
                            )
                        },
                        move |_ev, win, app| {
                            act_inc(
                                ToolsAction::SetAutoAcceptDelay((delay + 1).min(11)),
                                win,
                                app,
                            )
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
            Some(
                if any_enabled {
                    "已配置"
                } else {
                    "未启用"
                }
                .to_string(),
            ),
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
            move |_ev, win, app| {
                act_toggle(ToolsAction::ToggleCollapse("autocreatelobby"), win, app)
            },
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
                        move |win, app| {
                            act_toggle_dd(ToolsAction::ToggleDropdownMenu("lobby_mode"), win, app)
                        },
                        move |val, win, app| {
                            if let Ok(mid) = val.parse::<u32>() {
                                act_select_mode(ToolsAction::SetDefaultGameMode(mid), win, app);
                            }
                        },
                    ),
                )),
        )
    }
}
