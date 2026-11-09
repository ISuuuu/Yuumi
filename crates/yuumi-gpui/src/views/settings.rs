//! 全局设置主视图 (SettingsView)
//!
//! 1:1 复刻 Vue 原版：
//! - `src/views/Settings.vue`
//! - `src/components/settings/LolPathSection.vue`
//! - `src/components/settings/CloudServiceSection.vue`
//! - `src/components/settings/ScreenshotSection.vue`
//! - `src/components/settings/PersonalizationSection.vue`
//! - `src/components/settings/UpdateSection.vue`
//! - `src/components/settings/AboutSection.vue`
//!
//! 严格依照 [05-5-GPUI_VUE_PAGE_PARITY.md] 第 12.4 节 F01~F08 规范实现。

use crate::components::badge::{Badge, BadgeVariant};
use crate::components::card::Card;
use crate::components::collapse_card::{
    action_button, collapse_card, number_stepper, setting_row, settings_card_item,
};
use crate::components::switch::Switch;
use crate::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::collections::HashSet;
use yuumi_core::config::{AppConfig, WEGAME_MARKER};

/// 设置项交互动作枚举
#[derive(Clone, Debug)]
pub enum SettingsAction {
    /// 展开/收起指定折叠分组
    ToggleCollapse(&'static str),

    // ─── 1. 功能 (Features) ───
    SetApiConcurrency(u32),
    SetCareerGames(u32),
    ToggleGameInfoFilter,
    ToggleReserveGameInfo,
    ToggleShowTierInGameInfo,

    // ─── 2. OP.GG ───
    ToggleAutoShowOpgg,
    TogglePinOpgg,

    // ─── 3. 通用设置 (General) ───
    DetectLolPath,
    SelectLolFolder,
    AddWegamePath,
    RemoveLolPath(usize),
    ToggleStartLolWithApp,
    ClearCache,
    ToggleCloseToTray,
    ToggleMinimizeOnGameStart,

    // 云服务与菜单
    SetUploadApiUrl(String),
    ToggleLcuRealtime,
    SetLcuUserId(String),
    ToggleSignalrHub,
    SetSignalrServer(String),
    SetSignalrUser(String),
    ToggleHideTft,
    ToggleHideSavedPlayers,

    // 截图
    ToggleScreenshotOnMultikill,
    ToggleScreenshotLevel(u32),
    SelectScreenshotFolder,
    OpenScreenshotFolder,

    // ─── 4. 日志 (Log) ───
    SetLogLevel(u32),
    OpenLogFolder,

    // ─── 5. 个性化 (Personalization) ───
    ToggleMica,
    SetThemeMode(String),
    SetThemeColor(String),
    ResetThemeColor,
    ResetCardColors,
    ResetDeathsColors,
    SetDpiScale(String),
    SetLanguage(String),

    // ─── 6. 更新与代理 (Update & Proxy) ───
    CheckUpdate,
    ToggleHttpProxy,
    SetHttpProxyAddr(String),
}

/// 主题色预设色板
pub const THEME_COLOR_PRESETS: [(&str, &str); 4] = [
    ("#009faa", "极光青"),
    ("#1890ff", "极光蓝"),
    ("#722ed1", "幻影紫"),
    ("#fa8c16", "暖阳金"),
];

/// 日志等级定义
pub const LOG_LEVELS: [(u32, &str); 3] = [(0, "Debug"), (1, "Info"), (2, "Error")];

/// 设置视图状态
pub struct SettingsView {
    /// 展开的折叠面板标识集合
    pub open_sections: HashSet<String>,

    // 输入缓冲
    pub upload_url_buffer: String,
    pub lcu_user_id_buffer: String,
    pub signalr_server_buffer: String,
    pub signalr_user_buffer: String,
    pub proxy_addr_buffer: String,

    // 下拉浮层状态
    pub open_log_level_dropdown: bool,
    pub open_theme_mode_dropdown: bool,
    pub open_scale_dropdown: bool,
    pub open_lang_dropdown: bool,

    // 状态反馈提示
    pub status_feedback: Option<(String, bool)>,
}

impl Default for SettingsView {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsView {
    pub fn new() -> Self {
        let mut open_sections = HashSet::new();
        // 默认折叠项保持收起，按需点击展开
        open_sections.insert("lolpath".to_string());

        Self {
            open_sections,
            upload_url_buffer: String::new(),
            lcu_user_id_buffer: String::new(),
            signalr_server_buffer: String::new(),
            signalr_user_buffer: String::new(),
            proxy_addr_buffer: "127.0.0.1:7897".to_string(),

            open_log_level_dropdown: false,
            open_theme_mode_dropdown: false,
            open_scale_dropdown: false,
            open_lang_dropdown: false,

            status_feedback: None,
        }
    }

    /// 切换折叠分组
    pub fn toggle_section(&mut self, section_name: &str) {
        if self.open_sections.contains(section_name) {
            self.open_sections.remove(section_name);
        } else {
            self.open_sections.insert(section_name.to_string());
        }
    }

    /// 设置临时状态反馈
    pub fn set_feedback(&mut self, msg: impl Into<String>, is_error: bool) {
        self.status_feedback = Some((msg.into(), is_error));
    }

    /// 主渲染入口（800px 居中单列）
    pub fn render_panel<F>(
        &self,
        config: &AppConfig,
        _window: &mut Window,
        on_action: F,
    ) -> impl IntoElement
    where
        F: Fn(SettingsAction, &mut Window, &mut App) + 'static + Clone,
    {
        div()
            .id("settings-scroll-view")
            .size_full()
            .overflow_y_scroll()
            .p(px(24.0))
            .child(
                div()
                    .max_w(px(800.0))
                    .w_full()
                    .mx_auto()
                    .flex()
                    .flex_col()
                    .gap(px(14.0))
                    // 页面标题
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .mb(px(8.0))
                            .child(
                                div()
                                    .text_size(px(20.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child("设置"),
                            )
                            .when_some(self.status_feedback.as_ref(), |this, (msg, is_err)| {
                                this.child(Badge::new(msg.clone()).variant(if *is_err {
                                    BadgeVariant::Loss
                                } else {
                                    BadgeVariant::Primary
                                }))
                            }),
                    )
                    // ─── 1. 功能组 ───
                    .child(self.render_group_header("功能 (Features)"))
                    .child(self.render_features_group(config, on_action.clone()))
                    // ─── 2. OP.GG 组 ───
                    .child(self.render_group_header("OP.GG 辅助窗口"))
                    .child(self.render_opgg_group(config, on_action.clone()))
                    // ─── 3. 通用设置组 ───
                    .child(self.render_group_header("通用设置 (General)"))
                    .child(self.render_general_group(config, on_action.clone()))
                    // ─── 4. 日志维护组 ───
                    .child(self.render_group_header("日志与故障排查 (Diagnostics)"))
                    .child(self.render_log_group(config, on_action.clone()))
                    // ─── 5. 个性化设置组 ───
                    .child(self.render_group_header("外观与个性化 (Personalization)"))
                    .child(self.render_personalization_group(config, on_action.clone()))
                    // ─── 6. 软件更新组 ───
                    .child(self.render_group_header("软件更新 (Updates)"))
                    .child(self.render_update_group(config, on_action.clone()))
                    // ─── 7. 网络代理 ───
                    .child(self.render_group_header("网络代理 (Proxy)"))
                    .child(self.render_proxy_group(config, on_action.clone()))
                    // ─── 8. 关于 ───
                    .child(self.render_group_header("关于应用 (About)"))
                    .child(self.render_about_group(config))
                    .child(div().h(px(32.0))),
            )
    }

    /// 分组小标题
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

    // ─── 1. 功能设置组 (F01) ───
    fn render_features_group<F>(&self, config: &AppConfig, on_action: F) -> impl IntoElement
    where
        F: Fn(SettingsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let f = &config.functions;
        let cur_concurr = f.api_concurrency_number;
        let cur_games = f.career_games_number;
        let act_concurr_dec = on_action.clone();
        let act_concurr_inc = on_action.clone();
        let act_games_dec = on_action.clone();
        let act_games_inc = on_action.clone();
        let act_filter = on_action.clone();
        let act_reserve = on_action.clone();
        let act_tier = on_action.clone();

        Card::new().overflow_hidden().child(
            div()
                .flex()
                .flex_col()
                // LCU API 并发数
                .child(settings_card_item(
                    "LCU API 并发数",
                    Some("调整与客户端并发交互时的线程/许可数量 (1~10，默认 5)"),
                    true,
                    number_stepper(
                        "stepper-api-concurr".into(),
                        cur_concurr.to_string(),
                        cur_concurr > 1,
                        cur_concurr < 10,
                        move |_ev, win, app| {
                            act_concurr_dec(
                                SettingsAction::SetApiConcurrency(
                                    cur_concurr.saturating_sub(1).max(1),
                                ),
                                win,
                                app,
                            )
                        },
                        move |_ev, win, app| {
                            act_concurr_inc(
                                SettingsAction::SetApiConcurrency(
                                    (cur_concurr + 1).min(10),
                                ),
                                win,
                                app,
                            )
                        },
                    ),
                ))
                // 默认对局数量
                .child(settings_card_item(
                    "默认拉取战绩局数",
                    Some("生涯与战绩查询页面单次向 LCU 请求的历史场次数目 (1~100 局，步长 5)"),
                    true,
                    number_stepper(
                        "stepper-career-games".into(),
                        format!("{} 局", cur_games),
                        cur_games > 5,
                        cur_games < 100,
                        move |_ev, win, app| {
                            act_games_dec(
                                SettingsAction::SetCareerGames(
                                    cur_games.saturating_sub(5).max(5),
                                ),
                                win,
                                app,
                            )
                        },
                        move |_ev, win, app| {
                            act_games_inc(
                                SettingsAction::SetCareerGames(
                                    (cur_games + 5).min(100),
                                ),
                                win,
                                app,
                            )
                        },
                    ),
                ))
                // 对局信息过滤
                .child(settings_card_item(
                    "对局信息自动过滤人机与训练对局",
                    Some("战绩列表中自动剔除新手教程、人机对战以及训练营自定义场次"),
                    true,
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_filter(SettingsAction::ToggleGameInfoFilter, win, app)
                        })
                        .child(Switch::new(f.game_info_filter)),
                ))
                // 保留对局信息
                .child(settings_card_item(
                    "游戏结算后保留对局信息",
                    Some("游戏结束后保留上一局 10 人面板数据，防止过早被大厅状态重置清空"),
                    true,
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_reserve(SettingsAction::ToggleReserveGameInfo, win, app)
                        })
                        .child(Switch::new(f.enable_reserve_gameinfo)),
                ))
                // 详情显示段位
                .child(settings_card_item(
                    "在对局详情中展示段位徽章",
                    Some("在战绩查询面板与对局玩家列表中展示单双/灵活排位段位"),
                    false,
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_tier(SettingsAction::ToggleShowTierInGameInfo, win, app)
                        })
                        .child(Switch::new(f.show_tier_in_game_info)),
                )),
        )
    }

    // ─── 2. OP.GG 组 (F02) ───
    fn render_opgg_group<F>(&self, config: &AppConfig, on_action: F) -> impl IntoElement
    where
        F: Fn(SettingsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let f = &config.functions;
        let act_show = on_action.clone();
        let act_top = on_action.clone();

        Card::new().overflow_hidden().child(
            div()
                .flex()
                .flex_col()
                .child(settings_card_item(
                    "进入选人阶段自动呼出 OP.GG 窗口",
                    Some("检测到进入 Champ Select 时自动弹出独立英雄符文出装辅助窗口"),
                    true,
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_show(SettingsAction::ToggleAutoShowOpgg, win, app)
                        })
                        .child(Switch::new(f.auto_show_opgg)),
                ))
                .child(settings_card_item(
                    "OP.GG 辅助窗口始终置顶",
                    Some("将独立 OP.GG 窗口固定在最前端，方便在游戏加载界面查看对位数据"),
                    false,
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_top(SettingsAction::TogglePinOpgg, win, app)
                        })
                        .child(Switch::new(f.enable_opgg_on_top)),
                )),
        )
    }

    // ─── 3. 通用设置组 (F03 ~ F05) ───
    fn render_general_group<F>(&self, config: &AppConfig, on_action: F) -> impl IntoElement
    where
        F: Fn(SettingsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let g = &config.general;
        let f = &config.functions;
        let open_path = self.open_sections.contains("lolpath");
        let open_cloud = self.open_sections.contains("cloud");
        let open_hide = self.open_sections.contains("hidemenu");
        let open_shot = self.open_sections.contains("screenshot");

        let act_toggle_path = on_action.clone();
        let act_detect = on_action.clone();
        let act_select_folder = on_action.clone();
        let act_add_wegame = on_action.clone();

        let act_start_lol = on_action.clone();
        let act_clear_cache = on_action.clone();
        let act_tray = on_action.clone();
        let act_min = on_action.clone();

        let act_toggle_cloud = on_action.clone();
        let act_lcu_rt = on_action.clone();
        let act_sig_hub = on_action.clone();

        let act_toggle_hide = on_action.clone();
        let act_tft = on_action.clone();
        let act_saved = on_action.clone();

        let act_toggle_shot = on_action.clone();
        let act_shot_switch = on_action.clone();
        let act_shot_pick = on_action.clone();
        let act_shot_open = on_action.clone();

        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            // 客户端路径管理折叠卡片
            .child(collapse_card(
                "settings-card-lolpath".into(),
                open_path,
                Some("icons/home.svg"),
                "客户端安装路径管理",
                "配置国服/外服英雄联盟客户端安装路径或 WeGame 启动项",
                Some(format!("已配置 {} 条路径", g.lol_path.len())),
                move |_ev, win, app| {
                    act_toggle_path(SettingsAction::ToggleCollapse("lolpath"), win, app)
                },
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    // 现有路径列表
                    .children(g.lol_path.iter().enumerate().map(|(idx, path)| {
                        let is_wegame = path == WEGAME_MARKER;
                        let act_rm = on_action.clone();
                        div()
                            .p(px(10.0))
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
                                        Badge::new(if is_wegame {
                                            "WeGame"
                                        } else {
                                            "常规客户端"
                                        })
                                        .variant(if is_wegame {
                                            BadgeVariant::Primary
                                        } else {
                                            BadgeVariant::Default
                                        }),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.5))
                                            .text_color(Theme::TEXT_PRIMARY)
                                            .child(if is_wegame {
                                                g.wegame_path.as_deref().unwrap_or("未配置 WeGame 路径").to_string()
                                            } else {
                                                path.clone()
                                            }),
                                    ),
                            )
                            .child(action_button("移除", false, move |_ev, win, app| {
                                act_rm(SettingsAction::RemoveLolPath(idx), win, app)
                            }))
                    }))
                    // 操作按钮栏
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap(px(8.0))
                            .child(action_button("自动检测路径", false, move |_ev, win, app| {
                                act_detect(SettingsAction::DetectLolPath, win, app)
                            }))
                            .child(action_button("手动选择目录", false, move |_ev, win, app| {
                                act_select_folder(SettingsAction::SelectLolFolder, win, app)
                            }))
                            .child(action_button("添加 WeGame 启动项", false, move |_ev, win, app| {
                                act_add_wegame(SettingsAction::AddWegamePath, win, app)
                            })),
                    ),
            ))
            // 常规通用选项面板
            .child(
                Card::new().overflow_hidden().child(
                    div()
                        .flex()
                        .flex_col()
                        .child(settings_card_item(
                            "随应用启动客户端",
                            Some("打开 Yuumi 时自动拉起配置的首选英雄联盟客户端"),
                            true,
                            div()
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                    act_start_lol(SettingsAction::ToggleStartLolWithApp, win, app)
                                })
                                .child(Switch::new(g.enable_start_lol_with_app)),
                        ))
                        .child(settings_card_item(
                            "清理客户端本地缓存",
                            Some("清除 CEF 浏览器缓存及应用静态资源，释放磁盘占用并解决卡死"),
                            true,
                            action_button("清除缓存", false, move |_ev, win, app| {
                                act_clear_cache(SettingsAction::ClearCache, win, app)
                            }),
                        ))
                        .child(settings_card_item(
                            "关闭窗口时最小化到系统托盘",
                            Some("点击窗口右上角关闭按钮时保留后台进程，通过托盘图标唤回"),
                            true,
                            div()
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                    act_tray(SettingsAction::ToggleCloseToTray, win, app)
                                })
                                .child(Switch::new(g.enable_close_to_tray.unwrap_or(true))),
                        ))
                        .child(settings_card_item(
                            "对局开始时主窗口自动最小化",
                            Some("检测到游戏加载完成进入对局时，自动最小化避免遮挡视线"),
                            false,
                            div()
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                    act_min(SettingsAction::ToggleMinimizeOnGameStart, win, app)
                                })
                                .child(Switch::new(g.enable_game_start_minimize)),
                        )),
                ),
            )
            // 云服务与对局上传折叠卡片
            .child(collapse_card(
                "settings-card-cloud".into(),
                open_cloud,
                Some("icons/menu.svg"),
                "云端服务与对局上传",
                "配置比赛战绩上传接口、LCU 实时查询网关与 SignalR 远程控制反代",
                Some(if !g.upload_api_url.is_empty() {
                    "已配置上传"
                } else {
                    "未配置"
                }.to_string()),
                move |_ev, win, app| {
                    act_toggle_cloud(SettingsAction::ToggleCollapse("cloud"), win, app)
                },
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(setting_row(
                        "对局上传 API 接口",
                        Some("游戏结束后将战绩指标推送到远程分析接口"),
                        div()
                            .p(px(8.0))
                            .rounded_md()
                            .border_1()
                            .border_color(Theme::CARD_BORDER)
                            .bg(Theme::CARD_BG)
                            .text_size(px(12.5))
                            .text_color(Theme::TEXT_PRIMARY)
                            .child(if g.upload_api_url.is_empty() {
                                "https://lol.369900.xyz".to_string()
                            } else {
                                g.upload_api_url.clone()
                            }),
                    ))
                    .child(setting_row(
                        "开启 LCU 实时查询服务",
                        Some("允许通过云端网关查询对局玩家实时数据"),
                        div()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                act_lcu_rt(SettingsAction::ToggleLcuRealtime, win, app)
                            })
                            .child(Switch::new(f.lcu_realtime_enabled)),
                    ))
                    .child(setting_row(
                        "开启 SignalR 远程反代",
                        Some("开启基于 WebSocket/SignalR 的远程移动端与控制面板"),
                        div()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                act_sig_hub(SettingsAction::ToggleSignalrHub, win, app)
                            })
                            .child(Switch::new(g.enable_signalr_hub)),
                    )),
            ))
            // 隐藏菜单折叠卡片
            .child(collapse_card(
                "settings-card-hidemenu".into(),
                open_hide,
                Some("icons/menu.svg"),
                "隐藏菜单项定制",
                "控制侧边栏导航中云顶之弈 (TFT) 与路人集模块的显隐状态",
                Some(if f.hide_tft || f.hide_saved_players {
                    "已隐藏部分项"
                } else {
                    "全部显示"
                }.to_string()),
                move |_ev, win, app| {
                    act_toggle_hide(SettingsAction::ToggleCollapse("hidemenu"), win, app)
                },
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(setting_row(
                        "隐藏云顶之弈 (TFT) 模块",
                        Some("从左侧导航栏移除 TFT 入口"),
                        div()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                act_tft(SettingsAction::ToggleHideTft, win, app)
                            })
                            .child(Switch::new(f.hide_tft)),
                    ))
                    .child(setting_row(
                        "隐藏路人集模块",
                        Some("从左侧导航栏移除曾同局玩家标记与相遇历史入口"),
                        div()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                act_saved(SettingsAction::ToggleHideSavedPlayers, win, app)
                            })
                            .child(Switch::new(f.hide_saved_players)),
                    )),
            ))
            // 多杀截图折叠卡片
            .child(collapse_card(
                "settings-card-screenshot".into(),
                open_shot,
                Some("icons/copy.svg"),
                "游戏内精彩瞬间截图",
                "监听游戏内多杀事件（三杀/四杀/五杀/超神）并自动抓取屏幕截图存档",
                Some(if f.enable_screenshot_on_multikill {
                    "已开启"
                } else {
                    "未启用"
                }.to_string()),
                move |_ev, win, app| {
                    act_toggle_shot(SettingsAction::ToggleCollapse("screenshot"), win, app)
                },
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(setting_row(
                        "开启多杀自动截图",
                        Some("在对局内触发连杀事件时自动截图保存到本地"),
                        div()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                act_shot_switch(SettingsAction::ToggleScreenshotOnMultikill, win, app)
                            })
                            .child(Switch::new(f.enable_screenshot_on_multikill)),
                    ))
                    .child(setting_row(
                        "连杀触发等级",
                        Some("选择自动截图的目标事件级别"),
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(action_button("三杀 (Triple)", true, |_ev, _w, _a| {}))
                            .child(action_button("四杀 (Quadra)", true, |_ev, _w, _a| {}))
                            .child(action_button("五杀 (Penta)", true, |_ev, _w, _a| {}))
                            .child(action_button("超神 (Godlike)", true, |_ev, _w, _a| {})),
                    ))
                    .child(setting_row(
                        "截图保存目录",
                        Some("设置截图图片的本地存储路径"),
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(action_button("更改保存目录", false, move |_ev, win, app| {
                                act_shot_pick(SettingsAction::SelectScreenshotFolder, win, app)
                            }))
                            .child(action_button("打开截图文件夹", false, move |_ev, win, app| {
                                act_shot_open(SettingsAction::OpenScreenshotFolder, win, app)
                            })),
                    )),
            ))
    }

    // ─── 4. 日志维护组 (F05) ───
    fn render_log_group<F>(&self, config: &AppConfig, on_action: F) -> impl IntoElement
    where
        F: Fn(SettingsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let level = config.general.log_level;
        let act_open = on_action.clone();

        Card::new().overflow_hidden().child(
            div()
                .flex()
                .flex_col()
                .child(settings_card_item(
                    "运行日志记录等级",
                    Some("调整日志详细程度，排查问题时建议设为 Debug (默认 Info)"),
                    true,
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .children(LOG_LEVELS.iter().map(|(lvl, name)| {
                            let l = *lvl;
                            let act = on_action.clone();
                            action_button(*name, level == l, move |_ev, win, app| {
                                act(SettingsAction::SetLogLevel(l), win, app)
                            })
                        })),
                ))
                .child(settings_card_item(
                    "应用运行日志文件",
                    Some("打开本地日志保存目录，方便查看排查网络与 LCU 联调记录"),
                    false,
                    action_button("打开日志目录", false, move |_ev, win, app| {
                        act_open(SettingsAction::OpenLogFolder, win, app)
                    }),
                )),
        )
    }

    // ─── 5. 个性化设置组 (F06) ───
    fn render_personalization_group<F>(&self, config: &AppConfig, on_action: F) -> impl IntoElement
    where
        F: Fn(SettingsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let p = &config.personalization;
        let act_mica = on_action.clone();
        let act_reset_color = on_action.clone();
        let act_reset_card = on_action.clone();

        Card::new().overflow_hidden().child(
            div()
                .flex()
                .flex_col()
                // Mica 背景特效
                .child(settings_card_item(
                    "Windows 11 Mica 材质特效",
                    Some("开启云母毛玻璃水晶半透背景，适配系统桌面壁纸"),
                    true,
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_mica(SettingsAction::ToggleMica, win, app)
                        })
                        .child(Switch::new(p.mica_enabled)),
                ))
                // 主题模式
                .child(settings_card_item(
                    "界面主题模式",
                    Some("支持浅色水晶与深色暗夜模式切换"),
                    true,
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(action_button("跟随系统", p.theme_mode == "Auto", |_ev, _w, _a| {}))
                        .child(action_button("浅色极光", p.theme_mode == "Light", |_ev, _w, _a| {}))
                        .child(action_button("深色暗夜", p.theme_mode == "Dark", |_ev, _w, _a| {})),
                ))
                // 主题色
                .child(settings_card_item(
                    "极光主题品牌色",
                    Some("选择或自定义应用高亮主色调"),
                    true,
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .children(THEME_COLOR_PRESETS.iter().map(|(hex, name)| {
                            let h = hex.to_string();
                            let is_active = p.theme_color.eq_ignore_ascii_case(hex);
                            let act = on_action.clone();
                            action_button(*name, is_active, move |_ev, win, app| {
                                act(SettingsAction::SetThemeColor(h.clone()), win, app)
                            })
                        }))
                        .child(action_button("重置", false, move |_ev, win, app| {
                            act_reset_color(SettingsAction::ResetThemeColor, win, app)
                        })),
                ))
                // 对局卡片胜负颜色
                .child(settings_card_item(
                    "战绩胜负与重开卡片配色",
                    Some("胜负重开战绩卡背景透明色阶定制"),
                    true,
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(div().size(px(16.0)).rounded_sm().bg(rgb(0x39b01b)))
                                .child(div().text_size(px(12.0)).child("胜利")),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(div().size(px(16.0)).rounded_sm().bg(rgb(0xd3190c)))
                                .child(div().text_size(px(12.0)).child("失败")),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(div().size(px(16.0)).rounded_sm().bg(rgb(0xa2a2a2)))
                                .child(div().text_size(px(12.0)).child("重开")),
                        )
                        .child(action_button("恢复默认", false, move |_ev, win, app| {
                            act_reset_card(SettingsAction::ResetCardColors, win, app)
                        })),
                ))
                // 界面缩放
                .child(settings_card_item(
                    "界面缩放比例 (DPI Scale)",
                    Some("针对不同高分辨率显示屏调整 UI 整体缩放比例"),
                    true,
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(action_button("自动 (Auto)", true, |_ev, _w, _a| {}))
                        .child(action_button("100%", false, |_ev, _w, _a| {}))
                        .child(action_button("125%", false, |_ev, _w, _a| {})),
                ))
                // 语言选择
                .child(settings_card_item(
                    "应用显示语言 (Language)",
                    Some("切换应用多语言国际化文案"),
                    false,
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(action_button("简体中文", true, |_ev, _w, _a| {}))
                        .child(action_button("English", false, |_ev, _w, _a| {})),
                )),
        )
    }

    // ─── 6. 软件更新组 (F07) ───
    fn render_update_group<F>(&self, _config: &AppConfig, on_action: F) -> impl IntoElement
    where
        F: Fn(SettingsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let act_check = on_action.clone();

        Card::new().overflow_hidden().child(
            settings_card_item(
                "当前应用版本: v2.5.0 原生 GPUI 重构版",
                Some("基于 Rust + GPUI 原生构建，享受 Direct3D 11 极速 GPU 渲染与极致内存优化"),
                false,
                action_button("检查更新", true, move |_ev, win, app| {
                    act_check(SettingsAction::CheckUpdate, win, app)
                }),
            ),
        )
    }

    // ─── 7. 网络代理 (F08) ───
    fn render_proxy_group<F>(&self, config: &AppConfig, on_action: F) -> impl IntoElement
    where
        F: Fn(SettingsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let g = &config.general;
        let open_proxy = self.open_sections.contains("httpproxy");
        let act_toggle = on_action.clone();
        let act_switch = on_action.clone();

        collapse_card(
            "settings-card-proxy".into(),
            open_proxy,
            Some("icons/settings.svg"),
            "HTTP 网络代理设置",
            "配置 GitHub 更新检查及外部资源访问的代理服务器地址",
            Some(if g.enable_http_proxy {
                "已启用"
            } else {
                "未启用"
            }.to_string()),
            move |_ev, win, app| {
                act_toggle(SettingsAction::ToggleCollapse("httpproxy"), win, app)
            },
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(setting_row(
                    "开启 HTTP 代理",
                    Some("启用本地或局域网代理加速 GitHub Release 及外部请求"),
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_switch(SettingsAction::ToggleHttpProxy, win, app)
                        })
                        .child(Switch::new(g.enable_http_proxy)),
                ))
                .child(setting_row(
                    "代理服务器地址",
                    Some("格式形如 127.0.0.1:7897"),
                    div()
                        .p(px(8.0))
                        .rounded_md()
                        .border_1()
                        .border_color(Theme::CARD_BORDER)
                        .bg(Theme::CARD_BG)
                        .text_size(px(12.5))
                        .child(if g.http_proxy_addr.is_empty() {
                            "127.0.0.1:7897".to_string()
                        } else {
                            g.http_proxy_addr.clone()
                        }),
                )),
        )
    }

    // ─── 8. 关于 ───
    fn render_about_group(&self, _config: &AppConfig) -> impl IntoElement {
        Card::new().overflow_hidden().child(
            div()
                .p(px(16.0))
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(Theme::TEXT_PRIMARY)
                        .child("Yuumi - 英雄联盟原生极光对局助手"),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(Theme::TEXT_MUTED)
                        .child("基于 GPUI 原生 Direct3D 11 / Rust 引擎重构，纯白水晶极光设计语言。"),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(Theme::TEXT_MUTED)
                        .child("代码仓库: https://github.com/ikim/Yuumi · 开源协议: MIT License"),
                ),
        )
    }
}
