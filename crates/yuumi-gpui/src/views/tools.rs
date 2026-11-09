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
//! 严格依照 [05-5-GPUI_VUE_PAGE_PARITY.md] 第 12.3 节 T01~T07 规范实现。

use crate::components::badge::{Badge, BadgeVariant};
use crate::components::collapse_card::{
    action_button, collapse_card, lane_tabs, number_stepper, setting_row,
};
use crate::components::switch::Switch;
use crate::theme::Theme;
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
}

/// 常用召唤师技能列表定义 (技能 ID, 中文名)
pub const SUMMONER_SPELLS: [(i32, &str); 9] = [
    (4, "闪现"),
    (14, "引燃"),
    (7, "治疗"),
    (11, "惩戒"),
    (12, "传送"),
    (21, "屏障"),
    (3, "虚弱"),
    (1, "净化"),
    (6, "疾跑"),
];

/// 常用游戏模式选项定义
pub const GAME_MODES: [(u32, &str); 5] = [
    (2400, "海克斯大乱斗"),
    (450, "极地大乱斗"),
    (430, "匹配模式 (自选)"),
    (420, "单双排位"),
    (440, "灵活排位"),
];

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

    // 表单状态输入缓存
    pub spectate_name_buffer: String,
    pub cleaner_days: u32,
    pub signature_buffer: String,
    pub profile_skin_id_buffer: String,

    pub spoof_queue: String,
    pub spoof_tier: String,
    pub spoof_division: String,
    pub online_status: String,

    // 游戏配置只读锁定状态
    pub is_game_settings_locked: bool,

    // 下拉浮层开闭状态
    pub open_lobby_mode_dropdown: bool,
    pub open_spoof_queue_dropdown: bool,
    pub open_spoof_tier_dropdown: bool,
    pub open_spoof_division_dropdown: bool,
    pub open_online_status_dropdown: bool,
    pub open_spell1_dropdown: bool,
    pub open_spell2_dropdown: bool,

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

        Self {
            open_cards,
            hover_active_lane: "default",
            ban_active_lane: "default",
            spell_active_lane: "default",

            spectate_name_buffer: String::new(),
            cleaner_days: 90,
            signature_buffer: String::new(),
            profile_skin_id_buffer: String::new(),

            spoof_queue: "RANKED_SOLO_5x5".to_string(),
            spoof_tier: "CHALLENGER".to_string(),
            spoof_division: "I".to_string(),
            online_status: "chat".to_string(),

            is_game_settings_locked: false,

            open_lobby_mode_dropdown: false,
            open_spoof_queue_dropdown: false,
            open_spoof_tier_dropdown: false,
            open_spoof_division_dropdown: false,
            open_online_status_dropdown: false,
            open_spell1_dropdown: false,
            open_spell2_dropdown: false,

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

    /// 主渲染入口（1000px 居中单列）
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
                    .gap(px(12.0))
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
                    // ─── 1. 英雄选择组 ───
                    .child(self.render_group_header("英雄选择 (Champ Select)"))
                    .child(self.render_auto_accept_card(config, on_action.clone()))
                    .child(self.render_auto_swap_card(config, on_action.clone()))
                    .child(self.render_auto_hover_card(config, on_action.clone()))
                    .child(self.render_auto_ban_card(config, on_action.clone()))
                    .child(self.render_auto_spell_card(config, on_action.clone()))
                    // ─── 2. 游戏组 ───
                    .child(self.render_group_header("游戏自动化与实用工具 (Game & Lobby)"))
                    .child(self.render_auto_gameflow_card(config, on_action.clone()))
                    .child(self.render_spectate_card(on_action.clone()))
                    .child(self.render_friend_cleaner_card(on_action.clone()))
                    .child(self.render_bench_overlay_card(config, on_action.clone()))
                    .child(self.render_auto_reconnect_card(config, on_action.clone()))
                    .child(self.render_auto_create_lobby_card(config, on_action.clone()))
                    .child(self.render_lock_game_settings_card(on_action.clone()))
                    // ─── 3. 客户端组 ───
                    .child(self.render_group_header("客户端管理 (Client Operations)"))
                    .child(self.render_fix_window_card(on_action.clone()))
                    .child(self.render_restart_client_card(on_action.clone()))
                    // ─── 4. 个人主页组 ───
                    .child(self.render_group_header("个人主页与社交装扮 (Profile & Social)"))
                    .child(self.render_signature_card(on_action.clone()))
                    .child(self.render_profile_bg_card(on_action.clone()))
                    .child(self.render_rank_spoof_card(on_action.clone()))
                    .child(self.render_online_status_card(on_action.clone()))
                    .child(self.render_badges_card(on_action.clone()))
                    .child(self.render_clear_border_card(on_action.clone()))
                    .child(div().h(px(32.0))),
            )
    }

    /// 分组标题小组件
    fn render_group_header(&self, title: &'static str) -> Div {
        div()
            .text_size(px(12.0))
            .font_weight(FontWeight::BOLD)
            .text_color(Theme::TEXT_MUTED)
            .mt(px(8.0))
            .mb(px(2.0))
            .px(px(4.0))
            .child(title)
    }

    // ─── 卡片 1: 自动接受对局 ───
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
                    "启用自动接受对局",
                    Some("开启后检测到匹配成功事件将自动点击接受"),
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_switch(ToolsAction::ToggleAutoAccept, win, app)
                        })
                        .child(Switch::new(enabled)),
                ))
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
                )),
        )
    }

    // ─── 卡片 2: 自动接受交换请求 ───
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

    // ─── 卡片 3: 自动选用/预选英雄 ───
    fn render_auto_hover_card<F>(&self, config: &AppConfig, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let enabled = config.functions.enable_auto_select_champion;
        let hover_only = config.functions.enable_auto_hover_champion;
        let confirm_timeout = config.functions.auto_select_confirm_on_timeout;
        let open = self.open_cards.contains("autohover");

        let status = if enabled || hover_only {
            if hover_only {
                "预选模式".to_string()
            } else {
                "秒选锁定".to_string()
            }
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
        let act_switch = on_action.clone();
        let act_hover = on_action.clone();
        let act_timeout = on_action.clone();
        let act_lane = on_action.clone();
        let act_clear = on_action.clone();

        collapse_card(
            "tools-card-autohover".into(),
            open,
            Some("icons/user.svg"),
            "自动秒选/预选英雄",
            "选人阶段优先亮起意向英雄或按分路首选列表自动锁定",
            Some(status),
            move |_ev, win, app| act_toggle(ToolsAction::ToggleCollapse("autohover"), win, app),
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(setting_row(
                    "开启自动选用",
                    Some("进入选人阶段后自动选用预设英雄"),
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_switch(ToolsAction::ToggleAutoPick, win, app)
                        })
                        .child(Switch::new(enabled)),
                ))
                .child(setting_row(
                    "仅预选/亮起意向 (不自动锁定)",
                    Some("在选用阶段仅亮起英雄供队友参考，不直接锁定"),
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_hover(ToolsAction::ToggleAutoHover, win, app)
                        })
                        .child(Switch::new(hover_only)),
                ))
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
                // 英雄槽位展示
                .child(
                    div()
                        .p(px(12.0))
                        .rounded_md()
                        .bg(Theme::HOVER_BG)
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
                                        .text_size(px(12.5))
                                        .text_color(Theme::TEXT_SECONDARY)
                                        .child(format!(
                                            "当前【{}】配置: {}",
                                            match current_lane {
                                                "top" => "上单",
                                                "jug" => "打野",
                                                "mid" => "中单",
                                                "bot" => "下路",
                                                "sup" => "辅助",
                                                _ => "通用默认",
                                            },
                                            if pick_list.is_empty() {
                                                "未配置 (无意向)".to_string()
                                            } else {
                                                format!("英雄 ID: {:?}", pick_list)
                                            }
                                        )),
                                ),
                        )
                        .child(action_button("清空该路英雄", false, move |_ev, win, app| {
                            act_clear(ToolsAction::ClearPickChampion(current_lane), win, app)
                        })),
                ),
        )
    }

    // ─── 卡片 4: 自动禁用英雄 ───
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

        collapse_card(
            "tools-card-autoban".into(),
            open,
            Some("icons/close.svg"),
            "自动秒禁英雄",
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
                    "假装禁用 (亮起不确定，等到最后几秒再禁)",
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
                            act_dec(
                                ToolsAction::SetAutoBanDelay((delay - 0.5).max(0.0)),
                                win,
                                app,
                            )
                        },
                        move |_ev, win, app| {
                            act_inc(
                                ToolsAction::SetAutoBanDelay((delay + 0.5).min(15.0)),
                                win,
                                app,
                            )
                        },
                    ),
                ))
                .child(lane_tabs(current_lane, move |lane, win, app| {
                    act_lane(ToolsAction::SetBanLane(lane), win, app)
                }))
                .child(
                    div()
                        .p(px(12.0))
                        .rounded_md()
                        .bg(Theme::HOVER_BG)
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(12.5))
                                .text_color(Theme::TEXT_SECONDARY)
                                .child(format!(
                                    "当前【{}】禁用列表: {}",
                                    match current_lane {
                                        "top" => "上单",
                                        "jug" => "打野",
                                        "mid" => "中单",
                                        "bot" => "下路",
                                        "sup" => "辅助",
                                        _ => "通用默认",
                                    },
                                    if ban_list.is_empty() {
                                        "未配置 (无禁用)".to_string()
                                    } else {
                                        format!("英雄 ID: {:?}", ban_list)
                                    }
                                )),
                        )
                        .child(action_button("清空该路禁用", false, move |_ev, win, app| {
                            act_clear(ToolsAction::ClearBanChampion(current_lane), win, app)
                        })),
                ),
        )
    }

    // ─── 卡片 5: 自动设置召唤师技能 ───
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

        let spell_name = |id: i32| {
            SUMMONER_SPELLS
                .iter()
                .find(|(i, _)| *i == id)
                .map(|(_, n)| *n)
                .unwrap_or("未知")
        };

        let status = if enabled {
            format!("D: {} · F: {}", spell_name(spell1), spell_name(spell2))
        } else {
            "未启用".to_string()
        };

        let act_toggle = on_action.clone();
        let act_switch = on_action.clone();
        let act_lane = on_action.clone();

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
                .child(
                    div()
                        .p(px(12.0))
                        .rounded_md()
                        .bg(Theme::HOVER_BG)
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(16.0))
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .text_color(Theme::TEXT_PRIMARY)
                                        .child(format!("D 键: 【{}】", spell_name(spell1))),
                                )
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .text_color(Theme::TEXT_PRIMARY)
                                        .child(format!("F 键: 【{}】", spell_name(spell2))),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .children(SUMMONER_SPELLS.iter().take(4).map(|(id, name)| {
                                    let id_val = *id;
                                    let act_d = on_action.clone();
                                    action_button(
                                        format!("D:{}", name),
                                        spell1 == id_val,
                                        move |_ev, win, app| {
                                            act_d(
                                                ToolsAction::SetSpellId {
                                                    lane: current_lane,
                                                    is_spell1: true,
                                                    spell_id: id_val,
                                                },
                                                win,
                                                app,
                                            )
                                        },
                                    )
                                })),
                        ),
                ),
        )
    }

    // ─── 卡片 6: 自动游戏流程 ───
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
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_team(ToolsAction::ToggleAramTeamSideVisible, win, app)
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

    // ─── 卡片 7: 实时观战 ───
    fn render_spectate_card<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let open = self.open_cards.contains("spectate");
        let act_toggle = on_action.clone();
        let act_launch = on_action.clone();

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
                .items_center()
                .gap(px(10.0))
                .child(
                    div()
                        .flex_1()
                        .p(px(8.0))
                        .rounded_md()
                        .border_1()
                        .border_color(Theme::CARD_BORDER)
                        .bg(Theme::CARD_BG)
                        .text_size(px(13.0))
                        .text_color(Theme::TEXT_PRIMARY)
                        .child(if self.spectate_name_buffer.is_empty() {
                            "请输入要观战的召唤师昵称#TAG...".to_string()
                        } else {
                            self.spectate_name_buffer.clone()
                        }),
                )
                .child(action_button("启动观战", true, move |_ev, win, app| {
                    act_launch(ToolsAction::LaunchSpectate, win, app)
                })),
        )
    }

    // ─── 卡片 8: 好友管理与僵尸号清理 ───
    fn render_friend_cleaner_card<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let open = self.open_cards.contains("friendcleaner");
        let act_toggle = on_action.clone();
        let act_scan = on_action.clone();
        let act_del = on_action.clone();

        collapse_card(
            "tools-card-friendcleaner".into(),
            open,
            Some("icons/user.svg"),
            "好友管理与僵尸号清理",
            "智能检索超过设定天数未上线的游戏好友，支持一键批量移除",
            Some(format!("筛选 {} 天未上线", self.cleaner_days)),
            move |_ev, win, app| act_toggle(ToolsAction::ToggleCollapse("friendcleaner"), win, app),
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child({
                    let cur_days = self.cleaner_days;
                    let act_cleaner_dec = on_action.clone();
                    let act_cleaner_inc = on_action.clone();
                    setting_row(
                        "离线天数阈值",
                        Some("筛选离线超过该天数的好友作为待清理候选"),
                        number_stepper(
                            "stepper-cleaner-days".into(),
                            format!("{} 天", cur_days),
                            cur_days > 30,
                            cur_days < 365,
                            move |_ev, win, app| {
                                act_cleaner_dec(
                                    ToolsAction::SetCleanerDays(cur_days.saturating_sub(30).max(30)),
                                    win,
                                    app,
                                )
                            },
                            move |_ev, win, app| {
                                act_cleaner_inc(
                                    ToolsAction::SetCleanerDays((cur_days + 30).min(365)),
                                    win,
                                    app,
                                )
                            },
                        ),
                    )
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap(px(8.0))
                        .child(action_button("扫描离线好友", false, move |_ev, win, app| {
                            act_scan(ToolsAction::ScanFriends, win, app)
                        }))
                        .child(action_button(
                            "批量删除已选好友",
                            true,
                            move |_ev, win, app| {
                                act_del(ToolsAction::BatchDeleteOfflineFriends, win, app)
                            },
                        )),
                ),
        )
    }

    // ─── 卡片 9: 大乱斗板凳席独立悬浮窗 ───
    fn render_bench_overlay_card<F>(&self, config: &AppConfig, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let enabled = config.functions.enable_bench_overlay;
        let open = self.open_cards.contains("benchoverlay");

        let act_toggle = on_action.clone();
        let act_switch = on_action.clone();
        let act_open_win = on_action.clone();

        collapse_card(
            "tools-card-benchoverlay".into(),
            open,
            Some("icons/gamepad.svg"),
            "大乱斗板凳席独立悬浮窗",
            "极地大乱斗选人时弹出轻量透明置顶悬浮窗，直观呈现全队掷点板凳席英雄",
            Some(if enabled { "已开启" } else { "未启用" }.to_string()),
            move |_ev, win, app| act_toggle(ToolsAction::ToggleCollapse("benchoverlay"), win, app),
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(setting_row(
                    "开启大乱斗板凳席悬浮窗",
                    Some("选人阶段自动唤出独立置顶小窗口，支持快捷捡英雄"),
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_switch(ToolsAction::ToggleBenchOverlay, win, app)
                        })
                        .child(Switch::new(enabled)),
                ))
                .child(setting_row(
                    "快捷唤出/预览",
                    Some("立即打开悬浮窗窗口测试位置与透明度效果"),
                    action_button("打开板凳席悬浮窗", false, move |_ev, win, app| {
                        act_open_win(ToolsAction::OpenBenchOverlayWindow, win, app)
                    }),
                )),
        )
    }

    // ─── 卡片 10: 掉线自动重连 ───
    fn render_auto_reconnect_card<F>(&self, config: &AppConfig, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let enabled = config.functions.enable_auto_reconnect;
        let open = self.open_cards.contains("autoreconnect");

        let act_toggle = on_action.clone();
        let act_switch = on_action.clone();

        collapse_card(
            "tools-card-autoreconnect".into(),
            open,
            Some("icons/restore.svg"),
            "掉线自动重新连接",
            "检测到对局意外断线或客户端处于 Reconnect 状态时，自动触发重连",
            Some(if enabled { "已开启" } else { "未启用" }.to_string()),
            move |_ev, win, app| act_toggle(ToolsAction::ToggleCollapse("autoreconnect"), win, app),
            setting_row(
                "开启掉线自动重连",
                Some("掉线后在安全间隔内持续尝试重新进入游戏客户端，无需人工干预"),
                div()
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                        act_switch(ToolsAction::ToggleAutoReconnect, win, app)
                    })
                    .child(Switch::new(enabled)),
            ),
        )
    }

    // ─── 卡片 11: 自动创建房间 ───
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
            .unwrap_or("自定义模式");

        let act_toggle = on_action.clone();
        let act_switch = on_action.clone();
        let act_create = on_action.clone();

        collapse_card(
            "tools-card-autocreatelobby".into(),
            open,
            Some("icons/home.svg"),
            "自动创建房间",
            "客户端连接后或快捷一键创建常用对局模式房间",
            Some(if enabled {
                format!("已配置 · {}", mode_name)
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
                .child(setting_row(
                    "默认房间模式",
                    Some("选择常用游戏队列或模式"),
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .px(px(12.0))
                                .py(px(5.0))
                                .rounded_md()
                                .border_1()
                                .border_color(Theme::CARD_BORDER)
                                .bg(Theme::CARD_BG)
                                .text_size(px(12.5))
                                .child(mode_name),
                        )
                        .child(action_button("立即创建", true, move |_ev, win, app| {
                            act_create(ToolsAction::CreateLobbyNow, win, app)
                        })),
                )),
        )
    }

    // ─── 卡片 12: 锁定游戏设置 ───
    fn render_lock_game_settings_card<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let locked = self.is_game_settings_locked;
        let open = self.open_cards.contains("lockgamesettings");
        let act_toggle = on_action.clone();
        let act_lock = on_action.clone();

        collapse_card(
            "tools-card-locksettings".into(),
            open,
            Some("icons/lock.svg"),
            "锁定游戏设置 (只读保护)",
            "锁定 Game.cfg 与 PersistedSettings.json 为只读，防止客户端意外重置键位与画质",
            Some(if locked { "只读保护中" } else { "未锁定" }.to_string()),
            move |_ev, win, app| act_toggle(ToolsAction::ToggleCollapse("lockgamesettings"), win, app),
            setting_row(
                "配置锁定状态",
                Some(if locked {
                    "游戏配置文件已加只读锁，客户端无法修改设置"
                } else {
                    "配置文件可写，正常允许保存改动"
                }),
                action_button(
                    if locked { "解除只读锁定" } else { "一键锁定配置" },
                    !locked,
                    move |_ev, win, app| act_lock(ToolsAction::ToggleLockGameSettings, win, app),
                ),
            ),
        )
    }

    // ─── 卡片 13: 修复客户端窗口 ───
    fn render_fix_window_card<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let open = self.open_cards.contains("fixwindow");
        let act_toggle = on_action.clone();
        let act_fix = on_action.clone();

        collapse_card(
            "tools-card-fixwindow".into(),
            open,
            Some("icons/maximize.svg"),
            "修复客户端窗口大小与位置",
            "重置因分辨率错位导致的黑边，一键恢复 1600x900 100% 原始缩放并居中",
            Some("点击展开".to_string()),
            move |_ev, win, app| act_toggle(ToolsAction::ToggleCollapse("fixwindow"), win, app),
            setting_row(
                "一键还原窗口",
                Some("调用 Win32 原生窗口重排接口，修复英雄联盟主界面缩放异常"),
                action_button("立即修复窗口", true, move |_ev, win, app| {
                    act_fix(ToolsAction::FixClientWindow, win, app)
                }),
            ),
        )
    }

    // ─── 卡片 14: 重启客户端 ───
    fn render_restart_client_card<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let open = self.open_cards.contains("restartclient");
        let act_toggle = on_action.clone();
        let act_restart = on_action.clone();

        collapse_card(
            "tools-card-restartclient".into(),
            open,
            Some("icons/restore.svg"),
            "重启客户端 (LeagueClientUx)",
            "安全重启英雄联盟前端渲染进程，解决界面卡死、白屏或无响应",
            Some("点击展开".to_string()),
            move |_ev, win, app| act_toggle(ToolsAction::ToggleCollapse("restartclient"), win, app),
            setting_row(
                "重启前端渲染进程",
                Some("调用 RiotClient kill-and-restart-ux 接口，不退出游戏本体"),
                action_button("重启客户端前端", false, move |_ev, win, app| {
                    act_restart(ToolsAction::RestartClient, win, app)
                }),
            ),
        )
    }

    // ─── 卡片 15: 自定义个性签名 ───
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
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .child(
                    div()
                        .flex_1()
                        .p(px(8.0))
                        .rounded_md()
                        .border_1()
                        .border_color(Theme::CARD_BORDER)
                        .bg(Theme::CARD_BG)
                        .text_size(px(13.0))
                        .text_color(Theme::TEXT_PRIMARY)
                        .child(if self.signature_buffer.is_empty() {
                            "输入要展示的新个性签名...".to_string()
                        } else {
                            self.signature_buffer.clone()
                        }),
                )
                .child(action_button("保存签名", true, move |_ev, win, app| {
                    act_apply(ToolsAction::ApplySignature, win, app)
                })),
        )
    }

    // ─── 卡片 16: 修改个人资料主页背景 ───
    fn render_profile_bg_card<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let open = self.open_cards.contains("profilebg");
        let act_toggle = on_action.clone();
        let act_apply = on_action.clone();

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
                .items_center()
                .gap(px(10.0))
                .child(
                    div()
                        .flex_1()
                        .p(px(8.0))
                        .rounded_md()
                        .border_1()
                        .border_color(Theme::CARD_BORDER)
                        .bg(Theme::CARD_BG)
                        .text_size(px(13.0))
                        .text_color(Theme::TEXT_PRIMARY)
                        .child(if self.profile_skin_id_buffer.is_empty() {
                            "请输入英雄皮肤 ID (例如 103001 为九尾妖狐皮肤)...".to_string()
                        } else {
                            self.profile_skin_id_buffer.clone()
                        }),
                )
                .child(action_button("应用背景", true, move |_ev, win, app| {
                    act_apply(ToolsAction::ApplyProfileBackground, win, app)
                })),
        )
    }

    // ─── 卡片 17: 修改段位展示 ───
    fn render_rank_spoof_card<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let open = self.open_cards.contains("rankspoof");
        let act_toggle = on_action.clone();
        let act_apply = on_action.clone();

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
                .items_center()
                .gap(px(10.0))
                .child(
                    div()
                        .px(px(12.0))
                        .py(px(6.0))
                        .rounded_md()
                        .border_1()
                        .border_color(Theme::CARD_BORDER)
                        .bg(Theme::CARD_BG)
                        .text_size(px(12.5))
                        .child("单双排位 · 最强王者 (I)".to_string()),
                )
                .child(action_button("应用伪装段位", true, move |_ev, win, app| {
                    act_apply(ToolsAction::ApplyRankSpoof, win, app)
                })),
        )
    }

    // ─── 卡片 18: 修改在线状态 ───
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
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(action_button("设为在线", self.online_status == "chat", move |_ev, win, app| {
                    act_chat(ToolsAction::SetOnlineStatus("chat".to_string()), win, app)
                }))
                .child(action_button("设为离开", self.online_status == "away", move |_ev, win, app| {
                    act_away(ToolsAction::SetOnlineStatus("away".to_string()), win, app)
                }))
                .child(action_button("设为离线", self.online_status == "offline", move |_ev, win, app| {
                    act_offline(ToolsAction::SetOnlineStatus("offline".to_string()), win, app)
                })),
        )
    }

    // ─── 卡片 19: 成就勋章与称号 ───
    fn render_badges_card<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let open = self.open_cards.contains("badges");
        let act_toggle = on_action.clone();
        let act_clear = on_action.clone();

        collapse_card(
            "tools-card-badges".into(),
            open,
            Some("icons/bookmark.svg"),
            "成就勋章与称号管理",
            "配置个人资料卡上展示的 3 枚成就勋章与头衔称号",
            Some("点击展开".to_string()),
            move |_ev, win, app| act_toggle(ToolsAction::ToggleCollapse("badges"), win, app),
            setting_row(
                "清空成就勋章展示",
                Some("一键将生涯卡片上的全部 3 枚成就勋章槽位重置为空"),
                action_button("清空所有勋章", false, move |_ev, win, app| {
                    act_clear(ToolsAction::ClearBadges, win, app)
                }),
            ),
        )
    }

    // ─── 卡片 20: 清除个人边框 ───
    fn render_clear_border_card<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(ToolsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let open = self.open_cards.contains("border");
        let act_toggle = on_action.clone();
        let act_clear = on_action.clone();

        collapse_card(
            "tools-card-border".into(),
            open,
            Some("icons/close.svg"),
            "清除生涯与头像边框",
            "移除个人生涯中配置的旗帜与个人头像外围边框装饰",
            Some("点击展开".to_string()),
            move |_ev, win, app| act_toggle(ToolsAction::ToggleCollapse("border"), win, app),
            setting_row(
                "重置个人边框",
                Some("调用 update-player-preferences 清除 bannerId 边框装饰"),
                action_button("一键清除边框", false, move |_ev, win, app| {
                    act_clear(ToolsAction::ClearBorder, win, app)
                }),
            ),
        )
    }
}
