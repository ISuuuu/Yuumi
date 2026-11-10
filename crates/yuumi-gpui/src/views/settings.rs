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
//! 严格依照 [05-6-GPUI_TOOLS_SETTINGS_STYLE_PARITY.md] F-V01~F-V08 规范实现。

use crate::components::card::Card;
use crate::components::collapse_card::{
    action_button, checkbox, collapse_card, number_stepper, setting_row, settings_card_item,
};
use crate::components::dropdown::{select_dropdown, select_dropdown_item};
use crate::components::switch::Switch;
use crate::components::text_field::{text_field, TextFieldOptions};
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

    /// 展开/关闭下拉菜单或浮层
    ToggleDropdown(&'static str),

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
    EditFInput(&'static str, String),
    SaveFInput(&'static str),
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
    SetWinCardColor(String),
    SetLoseCardColor(String),
    SetRemakeCardColor(String),
    SetLightDeathsColor(String),
    SetDarkDeathsColor(String),
    ResetThemeColor,
    ResetCardColors,
    ResetDeathsColors,
    SetDpiScale(String),
    SetLanguage(String),

    // ─── 6. 更新与代理 (Update & Proxy) ───
    CheckUpdate,
    ToggleCheckUpdate,
    GoToReleases,
    ToggleHttpProxy,
    SetHttpProxyAddr(String),

    // ─── 7. 关于 ───
    ToggleChangelog,
    RefreshChangelog,
    OpenRepo,
}

/// 推荐调色盘配置
pub const THEME_PALETTE: [(&str, &str); 8] = [
    ("#009FAA", "极光青"),
    ("#1890FF", "极光蓝"),
    ("#722ED1", "幻影紫"),
    ("#FA8C16", "暖阳金"),
    ("#52C41A", "极光绿"),
    ("#F5222D", "烈焰红"),
    ("#EB2F96", "樱花粉"),
    ("#13C2C2", "青碧色"),
];

pub const WIN_CARD_PALETTE: [(&str, &str); 4] = [
    ("#3339B01B", "默认绿"),
    ("#4039B01B", "加深绿"),
    ("#2539B01B", "淡雅绿"),
    ("#3352C41A", "翠青绿"),
];

pub const LOSE_CARD_PALETTE: [(&str, &str); 4] = [
    ("#33D3190C", "默认红"),
    ("#40D3190C", "加深红"),
    ("#25D3190C", "淡雅红"),
    ("#33F5222D", "绯红"),
];

pub const REMAKE_CARD_PALETTE: [(&str, &str); 4] = [
    ("#33A2A2A2", "默认灰"),
    ("#40A2A2A2", "加深灰"),
    ("#25A2A2A2", "淡雅灰"),
    ("#338C8C8C", "冷月灰"),
];

pub const LIGHT_DEATH_PALETTE: [(&str, &str); 4] = [
    ("#FFB60000", "默认深红"),
    ("#FFD3190C", "猩红"),
    ("#FFCF1322", "暗赤"),
    ("#FFF5222D", "鲜红"),
];

pub const DARK_DEATH_PALETTE: [(&str, &str); 4] = [
    ("#FFFF8D8D", "默认浅红"),
    ("#FFFF7875", "柔粉红"),
    ("#FFFFA39E", "浅桃红"),
    ("#FFFFBB96", "浅珊瑚"),
];

/// 预设主题色（兼容外部引用）
pub const THEME_COLOR_PRESETS: [(&str, &str); 4] = [
    ("#009faa", "极光青"),
    ("#1890ff", "极光蓝"),
    ("#722ed1", "幻影紫"),
    ("#fa8c16", "暖阳金"),
];

/// 解析 6 位或 8 位十六进制颜色字符为 gpui::Rgba
pub fn parse_hex_color(hex: &str) -> Rgba {
    let s = hex.trim().trim_start_matches('#');
    if s.len() == 6 {
        if let Ok(val) = u32::from_str_radix(s, 16) {
            return rgba((val << 8) | 0xff);
        }
    } else if s.len() == 8 {
        // Vue: #AARRGGBB
        if let Ok(val) = u32::from_str_radix(s, 16) {
            let a = (val >> 24) & 0xff;
            let rgb = val & 0x00ffffff;
            return rgba((rgb << 8) | a);
        }
    }
    rgba(0x009faaff)
}

/// 清除/展示输入框
fn render_color_picker(
    id: ElementId,
    width_px: f32,
    color_hex: &str,
    open: bool,
    palette: &[(&'static str, &'static str)],
    on_toggle: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static + Clone,
    on_select: impl Fn(String, &mut Window, &mut App) + 'static + Clone,
) -> impl IntoElement {
    let swatch_color = parse_hex_color(color_hex);
    let hex_display = color_hex.to_uppercase();

    div()
        .id(id)
        .relative()
        .child(
            div()
                .w(px(width_px))
                .h(px(28.0))
                .px(px(8.0))
                .rounded_md()
                .border_1()
                .border_color(if open {
                    Theme::SIDEBAR_ACTIVE_BORDER
                } else {
                    Theme::CARD_BORDER
                })
                .bg(Theme::CARD_BG)
                .flex()
                .items_center()
                .gap(px(8.0))
                .cursor_pointer()
                .hover(|s| s.border_color(Theme::ACCENT_PRIMARY))
                .on_mouse_down(MouseButton::Left, on_toggle)
                // 色块预览
                .child(
                    div()
                        .w(px(18.0))
                        .h(px(16.0))
                        .rounded(px(3.0))
                        .border_1()
                        .border_color(Theme::CARD_BORDER)
                        .bg(swatch_color),
                )
                // 色值文本
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(Theme::TEXT_SECONDARY)
                        .child(hex_display),
                ),
        )
        .when(open, |this| {
            this.child(
                deferred(
                    div()
                        .absolute()
                        .top_full()
                        .right_0()
                        .mt(px(4.0))
                        .p(px(8.0))
                        .rounded_lg()
                        .border_1()
                        .border_color(Theme::CARD_BORDER)
                        .bg(rgb(0xffffff))
                        .shadow_lg()
                        .occlude()
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(Theme::TEXT_MUTED)
                                .child("选择推荐色阶:"),
                        )
                        .child(div().flex().flex_wrap().gap(px(6.0)).w(px(180.0)).children(
                            palette.iter().map(|(hex, _name)| {
                                    let h = hex.to_string();
                                    let c = parse_hex_color(hex);
                                    let is_cur = color_hex.eq_ignore_ascii_case(hex);
                                    let sel = on_select.clone();
                                    div()
                                        .size(px(24.0))
                                        .rounded_md()
                                        .border_1()
                                        .border_color(if is_cur {
                                            Theme::ACCENT_PRIMARY
                                        } else {
                                            Theme::CARD_BORDER
                                        })
                                        .bg(c)
                                        .cursor_pointer()
                                        .hover(|s| s.opacity(0.8))
                                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                            sel(h.clone(), win, app)
                                        })
                            }),
                        )),
                )
                .with_priority(25),
            )
        })
}

/// 设置视图状态
pub struct SettingsView {
    /// 展开的折叠面板标识集合
    pub open_sections: HashSet<String>,

    // 下拉浮层状态
    pub open_log_level_dropdown: bool,
    pub cloud_status: yuumi_core::signalr::HubStatus,
    pub open_theme_mode_dropdown: bool,
    pub open_scale_dropdown: bool,
    pub open_lang_dropdown: bool,

    // 色盘浮层状态
    pub open_theme_color_picker: bool,
    pub open_win_color_picker: bool,
    pub open_lose_color_picker: bool,
    pub open_remake_color_picker: bool,
    pub open_light_death_color_picker: bool,
    pub open_dark_death_color_picker: bool,

    // 更新日志弹窗
    pub show_changelog: bool,
    pub release_loading: bool,
    pub release_notes: Vec<yuumi_core::update::ReleaseEntry>,
    pub release_error: Option<String>,
    pub f_inputs: [String; 4],
    f_focus: Option<[FocusHandle; 4]>,

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
            open_log_level_dropdown: false,
            cloud_status: yuumi_core::signalr::HubStatus::Disabled,
            open_theme_mode_dropdown: false,
            open_scale_dropdown: false,
            open_lang_dropdown: false,

            open_theme_color_picker: false,
            open_win_color_picker: false,
            open_lose_color_picker: false,
            open_remake_color_picker: false,
            open_light_death_color_picker: false,
            open_dark_death_color_picker: false,

            show_changelog: false,
            release_loading: false,
            release_notes: Vec::new(),
            release_error: None,
            f_inputs: Default::default(),
            f_focus: None,
            status_feedback: None,
        }
    }

    pub fn bind_f_inputs(&mut self, config: &AppConfig, cx: &mut App) {
        self.f_inputs = [
            config.general.upload_api_url.clone(),
            config.general.signalr_user_id.clone(),
            config.general.http_proxy_addr.clone(),
            config.functions.screenshot_save_path.clone(),
        ];
        self.f_focus = Some(std::array::from_fn(|_| cx.focus_handle()));
    }

    fn f_input<F>(
        &self,
        index: usize,
        key: &'static str,
        placeholder: &'static str,
        on_action: F,
    ) -> Div
    where
        F: Fn(SettingsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let Some(focus) = self.f_focus.as_ref().map(|f| &f[index]) else {
            return div();
        };
        let edit = on_action.clone();
        let clear = on_action.clone();
        let submit = on_action.clone();
        let save = on_action.clone();
        div()
            .flex()
            .items_center()
            .gap(px(6.0))
            .child(text_field(
                key,
                &self.f_inputs[index],
                placeholder,
                px(260.0),
                focus,
                false,
                TextFieldOptions::default(),
                move |value, win, app| edit(SettingsAction::EditFInput(key, value), win, app),
                move |win, app| {
                    clear(SettingsAction::EditFInput(key, String::new()), win, app);
                    clear(SettingsAction::SaveFInput(key), win, app);
                },
                move |_, win, app| submit(SettingsAction::SaveFInput(key), win, app),
            ))
            .child(action_button("保存", false, move |_, win, app| {
                save(SettingsAction::SaveFInput(key), win, app)
            }))
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

    /// 主渲染入口（1000px 居中单列，与其他功能页保持一致）
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
            .relative()
            .child(
                div()
                    .max_w(px(1000.0))
                    .w_full()
                    .mx_auto()
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    // 页面标题 (去除多余状态徽章)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .mb(px(8.0))
                            .child(
                                div()
                                    .text_size(px(22.0))
                                    .font_weight(FontWeight::EXTRA_BOLD)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child("设置"),
                            ),
                    )
                    // ─── 1. 功能 ───
                    .child(self.render_group_header("功能"))
                    .child(self.render_features_group(config, on_action.clone()))
                    // ─── 2. OP.GG ───
                    .child(self.render_group_header("OP.GG"))
                    .child(self.render_opgg_group(config, on_action.clone()))
                    // ─── 3. 通用设置 ───
                    .child(self.render_group_header("通用设置"))
                    .child(self.render_general_group(config, on_action.clone()))
                    // ─── 4. 日志 ───
                    .child(self.render_group_header("日志"))
                    .child(self.render_log_group(config, on_action.clone()))
                    // ─── 5. 个性化 ───
                    .child(self.render_group_header("个性化"))
                    .child(self.render_personalization_group(config, on_action.clone()))
                    // ─── 6. 软件更新 ───
                    .child(self.render_group_header("软件更新"))
                    .child(self.render_update_group(config, on_action.clone()))
                    // ─── 7. 网络代理 ───
                    .child(self.render_proxy_group(config, on_action.clone()))
                    // ─── 8. 关于 ───
                    .child(self.render_group_header("关于"))
                    .child(self.render_about_group(config, on_action.clone()))
                    .child(div().h(px(32.0))),
            )
            .when(self.show_changelog, |this| {
                this.child(self.render_changelog_modal(on_action.clone()))
            })
    }

    /// 分组小标题（全中文、统一上边距与粗体）
    fn render_group_header(&self, title: &'static str) -> Div {
        div()
            .text_size(px(12.5))
            .font_weight(FontWeight::BOLD)
            .text_color(Theme::TEXT_MUTED)
            .mt(px(20.0))
            .mb(px(4.0))
            .px(px(6.0))
            .child(title)
    }

    // ─── 1. 功能设置组 (F-V01) ───
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

        Card::new().rounded(px(12.0)).overflow_hidden().child(
            div()
                .flex()
                .flex_col()
                // LCU API 并发数 (140px 数值居左，减加居右)
                .child(settings_card_item(
                    "LCU API 并发数",
                    Some("该值越大数据加载速度越快，但越可能引起客户端闪退"),
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
                                SettingsAction::SetApiConcurrency((cur_concurr + 1).min(10)),
                                win,
                                app,
                            )
                        },
                    ),
                ))
                // 默认对局数量 (140px 数值居左，减加居右)
                .child(settings_card_item(
                    "默认对局数量",
                    Some("调整在个人生涯界面中显示的最大对局数量"),
                    true,
                    number_stepper(
                        "stepper-career-games".into(),
                        cur_games.to_string(),
                        cur_games > 5,
                        cur_games < 100,
                        move |_ev, win, app| {
                            act_games_dec(
                                SettingsAction::SetCareerGames(cur_games.saturating_sub(5).max(5)),
                                win,
                                app,
                            )
                        },
                        move |_ev, win, app| {
                            act_games_inc(
                                SettingsAction::SetCareerGames((cur_games + 5).min(100)),
                                win,
                                app,
                            )
                        },
                    ),
                ))
                // 对局信息过滤 (直显开关)
                .child(settings_card_item(
                    "对局信息过滤",
                    Some("基于你当前游戏模式（地图/队列）筛选战绩，只显示相同模式的玩家历史数据"),
                    true,
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_filter(SettingsAction::ToggleGameInfoFilter, win, app)
                        })
                        .child(Switch::new(f.game_info_filter)),
                ))
                // 保留对局信息 (直显开关)
                .child(settings_card_item(
                    "保留对局信息界面内容",
                    Some("保留上一局的对局信息内容直到下一次对局开始"),
                    true,
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_reserve(SettingsAction::ToggleReserveGameInfo, win, app)
                        })
                        .child(Switch::new(f.enable_reserve_gameinfo)),
                ))
                // 详情显示段位 (直显开关)
                .child(settings_card_item(
                    "对局详情中显示段位",
                    Some("在搜索界面对局详情界面中显示段位，启动该选项将影响加载界面的速度"),
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

    // ─── 2. OP.GG 组 (F-V01) ───
    fn render_opgg_group<F>(&self, config: &AppConfig, on_action: F) -> impl IntoElement
    where
        F: Fn(SettingsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let f = &config.functions;
        let act_show = on_action.clone();
        let act_top = on_action.clone();

        Card::new().rounded(px(12.0)).overflow_hidden().child(
            div()
                .flex()
                .flex_col()
                .child(settings_card_item(
                    "自动显示 OP.GG 窗口",
                    Some("在英雄选择开始时自动显示 OP.GG 窗口"),
                    true,
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_show(SettingsAction::ToggleAutoShowOpgg, win, app)
                        })
                        .child(Switch::new(f.auto_show_opgg)),
                ))
                .child(settings_card_item(
                    "置顶 OP.GG 窗口",
                    Some("在英雄选择时将 OP.GG 窗口置顶"),
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

    // ─── 3. 通用设置组 (F-V01 ~ F-V04) ───
    fn render_general_group<F>(&self, config: &AppConfig, on_action: F) -> impl IntoElement
    where
        F: Fn(SettingsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let g = &config.general;
        let f = &config.functions;
        let open_path = self.open_sections.contains("lolpath");
        let open_cloud = self.open_sections.contains("upload_and_signalr");
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
            // 客户端路径管理折叠卡片 (F-V02)
            .child(collapse_card(
                "settings-card-lolpath".into(),
                open_path,
                None,
                "英雄联盟客户端路径",
                "设置英雄联盟客户端所在目录的路径",
                Some(if g.lol_path.is_empty() {
                    "未设置".to_string()
                } else {
                    format!("已设置 {} 个路径", g.lol_path.len())
                }),
                move |_ev, win, app| {
                    act_toggle_path(SettingsAction::ToggleCollapse("lolpath"), win, app)
                },
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    // 空列表提示
                    .when(g.lol_path.is_empty(), |this| {
                        this.child(
                            div()
                                .w_full()
                                .py(px(12.0))
                                .text_size(px(12.5))
                                .text_color(Theme::TEXT_MUTED)
                                .text_align(TextAlign::Center)
                                .child("暂无客户端路径"),
                        )
                    })
                    // 现有路径列表 (输入展示 + 标记 + 移除)
                    .children(g.lol_path.iter().enumerate().map(|(idx, path)| {
                        let is_wegame = path == WEGAME_MARKER;
                        let act_rm = on_action.clone();
                        div()
                            .p(px(8.0))
                            .px(px(12.0))
                            .rounded_md()
                            .border_1()
                            .border_color(Theme::CARD_BORDER)
                            .bg(Theme::CARD_BG)
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .flex_1()
                                    .when(is_wegame, |this| {
                                        this.child(
                                            div()
                                                .px(px(6.0))
                                                .py(px(2.0))
                                                .rounded_sm()
                                                .bg(Theme::ACCENT_LIGHT)
                                                .text_size(px(11.0))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(Theme::ACCENT_PRIMARY)
                                                .child("WeGame"),
                                        )
                                    })
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(if is_wegame && g.wegame_path.is_none() {
                                                Theme::TEXT_MUTED
                                            } else {
                                                Theme::TEXT_PRIMARY
                                            })
                                            .child(if is_wegame {
                                                g.wegame_path
                                                    .as_deref()
                                                    .unwrap_or("WeGame 安装路径（可手动修改）")
                                                    .to_string()
                                            } else {
                                                path.clone()
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .size(px(20.0))
                                    .rounded_full()
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(11.0))
                                    .text_color(Theme::TEXT_MUTED)
                                    .hover(|s| s.bg(Theme::HOVER_BG).text_color(Theme::LOSS_TEXT))
                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                        act_rm(SettingsAction::RemoveLolPath(idx), win, app)
                                    })
                                    .child("✕"),
                            )
                    }))
                    // 底部操作按钮按原顺序
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap(px(8.0))
                            .child(action_button(
                                "自动检测",
                                false,
                                move |_ev, win, app| {
                                act_detect(SettingsAction::DetectLolPath, win, app)
                                },
                            ))
                            .child(action_button(
                                "浏览文件夹",
                                false,
                                move |_ev, win, app| {
                                act_select_folder(SettingsAction::SelectLolFolder, win, app)
                                },
                            ))
                            .child(action_button(
                                "添加 WeGame 启动项",
                                false,
                                move |_ev, win, app| {
                                act_add_wegame(SettingsAction::AddWegamePath, win, app)
                                },
                            )),
                    ),
            ))
            // 常规通用直显面板 (启动客户端、清除缓存、关闭到托盘、游戏开始最小化)
            .child(
                Card::new().rounded(px(12.0)).overflow_hidden().child(
                    div()
                        .flex()
                        .flex_col()
                        .child(settings_card_item(
                            "启动时自动运行 LOL",
                            Some("Yuumi 启动后自动检测并运行英雄联盟客户端"),
                            true,
                            div()
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                    act_start_lol(SettingsAction::ToggleStartLolWithApp, win, app)
                                })
                                .child(Switch::new(g.enable_start_lol_with_app)),
                        ))
                        .child(settings_card_item(
                            "清除缓存",
                            Some("删除所有游戏资源的缓存（建议在游戏资源有更新时使用）"),
                            true,
                            action_button("删除", false, move |_ev, win, app| {
                                act_clear_cache(SettingsAction::ClearCache, win, app)
                            }),
                        ))
                        .child(settings_card_item(
                            "关闭到托盘",
                            Some("点击关闭按钮时将窗口隐藏到系统托盘区域"),
                            true,
                            div()
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                    act_tray(SettingsAction::ToggleCloseToTray, win, app)
                                })
                                .child(Switch::new(g.enable_close_to_tray.unwrap_or(true))),
                        ))
                        .child(settings_card_item(
                            "启动时最小化",
                            Some("软件启动后自动隐藏主窗口并最小化到系统托盘"),
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
            // 云端服务折叠卡片 (F-V03)
            .child(collapse_card(
                "settings-card-cloud".into(),
                open_cloud,
                None,
                "云端服务",
                "LCU 实时数据上传及 SignalR 实时查询服务",
                Some({
                    let upload_st = if g.upload_api_url.is_empty() {
                        "未配置"
                    } else {
                        "已配置上传"
                    };
                    if f.lcu_realtime_enabled {
                        format!("{} / {}", upload_st, self.cloud_status.label())
                    } else {
                        upload_st.to_string()
                    }
                }),
                move |_ev, win, app| {
                    act_toggle_cloud(
                        SettingsAction::ToggleCollapse("upload_and_signalr"),
                        win,
                        app,
                    )
                },
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    // 1. 服务器 API 地址
                    .child(setting_row(
                        "服务器 API 地址:",
                        None,
                        self.f_input(0, "f-upload", "http://example.com", on_action.clone()),
                    ))
                    // 2. LCU 实时服务开关
                    .child(setting_row(
                        "LCU 实时查询:",
                        None,
                        div()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                act_lcu_rt(SettingsAction::ToggleLcuRealtime, win, app)
                            })
                            .child(Switch::new(f.lcu_realtime_enabled)),
                    ))
                    // 3. userid 输入
                    .child(setting_row(
                        "userid:",
                        None,
                        self.f_input(1, "f-user", "留空默认使用 lcu_user_001", on_action.clone()),
                    )),
            ))
            // 隐藏菜单折叠卡片 (F-V04)
            .child(collapse_card(
                "settings-card-hidemenu".into(),
                open_hide,
                None,
                "隐藏菜单",
                "在侧边栏中隐藏选中的菜单入口",
                Some(
                    if f.hide_tft || f.hide_saved_players {
                    "已启用"
                } else {
                    "未启用"
                    }
                    .to_string(),
                ),
                move |_ev, win, app| {
                    act_toggle_hide(SettingsAction::ToggleCollapse("hidemenu"), win, app)
                },
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(setting_row(
                        "云顶之弈",
                        None,
                        div()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                act_tft(SettingsAction::ToggleHideTft, win, app)
                            })
                            .child(Switch::new(f.hide_tft)),
                    ))
                    .child(setting_row(
                        "路人集",
                        None,
                        div()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                act_saved(SettingsAction::ToggleHideSavedPlayers, win, app)
                            })
                            .child(Switch::new(f.hide_saved_players)),
                    )),
            ))
            // 自动截图组 (F-V04)
            .child(self.render_group_header("自动截图"))
            .child(collapse_card(
                "settings-card-screenshot".into(),
                open_shot,
                None,
                "多杀与超神自动截图",
                "多杀或超神时自动截图。游戏视频设置须为「无边框」或「窗口化」以防黑屏",
                Some(
                    if f.enable_screenshot_on_multikill {
                    "已启用"
                } else {
                    "未启用"
                    }
                    .to_string(),
                ),
                move |_ev, win, app| {
                    act_toggle_shot(SettingsAction::ToggleCollapse("screenshot"), win, app)
                },
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .child(setting_row(
                        "多杀与超神自动截图",
                        None,
                        div()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                act_shot_switch(
                                    SettingsAction::ToggleScreenshotOnMultikill,
                                    win,
                                    app,
                                )
                            })
                            .child(Switch::new(f.enable_screenshot_on_multikill)),
                    ))
                    .when(f.enable_screenshot_on_multikill, |this| {
                        let act_lvl3 = on_action.clone();
                        let act_lvl4 = on_action.clone();
                        let act_lvl5 = on_action.clone();
                        let act_lvl8 = on_action.clone();
                        this.child(setting_row(
                            "截图触发事件",
                            None,
                            div()
                                .flex()
                                .items_center()
                                .gap(px(14.0))
                                .child(checkbox(
                                    "cb-triple".into(),
                                    "三杀",
                                    f.screenshot_on_multikill_levels.contains(&3),
                                    move |_ev, win, app| {
                                        act_lvl3(SettingsAction::ToggleScreenshotLevel(3), win, app)
                                    },
                                ))
                                .child(checkbox(
                                    "cb-quadra".into(),
                                    "四杀",
                                    f.screenshot_on_multikill_levels.contains(&4),
                                    move |_ev, win, app| {
                                        act_lvl4(SettingsAction::ToggleScreenshotLevel(4), win, app)
                                    },
                                ))
                                .child(checkbox(
                                    "cb-penta".into(),
                                    "五杀",
                                    f.screenshot_on_multikill_levels.contains(&5),
                                    move |_ev, win, app| {
                                        act_lvl5(SettingsAction::ToggleScreenshotLevel(5), win, app)
                                    },
                                ))
                                .child(checkbox(
                                    "cb-godlike".into(),
                                    "超神",
                                    f.screenshot_on_multikill_levels.contains(&8),
                                    move |_ev, win, app| {
                                        act_lvl8(SettingsAction::ToggleScreenshotLevel(8), win, app)
                                    },
                                )),
                        ))
                        .child(setting_row(
                            "截图保存位置",
                            None,
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .flex_1()
                                .child(self.f_input(
                                    3,
                                    "f-screenshot",
                                    "默认保存至系统图片目录",
                                    on_action.clone(),
                                ))
                                .child(action_button(
                                    "浏览文件夹",
                                    false,
                                    move |_ev, win, app| {
                                        act_shot_pick(
                                            SettingsAction::SelectScreenshotFolder,
                                            win,
                                            app,
                                )
                                    },
                                ))
                                .child(action_button(
                                    "打开截图文件夹",
                                    false,
                                    move |_ev, win, app| {
                                        act_shot_open(
                                            SettingsAction::OpenScreenshotFolder,
                                            win,
                                            app,
                                        )
                                    },
                                )),
                        ))
                    }),
            ))
    }

    // ─── 4. 日志维护组 (F-V05) ───
    fn render_log_group<F>(&self, config: &AppConfig, on_action: F) -> impl IntoElement
    where
        F: Fn(SettingsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let level = config.general.log_level;
        let act_open = on_action.clone();
        let act_toggle_dd = on_action.clone();
        let cur_label = match level {
            0 => "Debug",
            1 => "Info",
            2 => "Error",
            _ => "Info",
        };

        Card::new().rounded(px(12.0)).overflow_hidden().child(
            div()
                .flex()
                .flex_col()
                // 日志等级 (120px 下拉)
                .child(settings_card_item(
                    "日志等级",
                    Some("日志写入文件记录的等级（Debug, Info, Error）"),
                    true,
                    select_dropdown(
                        "dropdown-log-level".into(),
                        120.0,
                        cur_label,
                        self.open_log_level_dropdown,
                        div()
                            .flex()
                            .flex_col()
                            .child({
                                let act = on_action.clone();
                                select_dropdown_item("Debug", level == 0, move |_ev, win, app| {
                                    act(SettingsAction::SetLogLevel(0), win, app)
                                })
                            })
                            .child({
                                let act = on_action.clone();
                                select_dropdown_item("Info", level == 1, move |_ev, win, app| {
                                    act(SettingsAction::SetLogLevel(1), win, app)
                                })
                            })
                            .child({
                                let act = on_action.clone();
                                select_dropdown_item("Error", level == 2, move |_ev, win, app| {
                                    act(SettingsAction::SetLogLevel(2), win, app)
                                })
                            }),
                        move |_ev, win, app| {
                            act_toggle_dd(SettingsAction::ToggleDropdown("log_level"), win, app)
                        },
                    ),
                ))
                // 打开日志文件
                .child(settings_card_item(
                    "日志文件",
                    Some("[exe 目录]/log/"),
                    false,
                    action_button("打开文件夹", false, move |_ev, win, app| {
                        act_open(SettingsAction::OpenLogFolder, win, app)
                    }),
                )),
        )
    }

    // ─── 5. 个性化设置组 (F-V05 ~ F-V06) ───
    fn render_personalization_group<F>(&self, config: &AppConfig, on_action: F) -> impl IntoElement
    where
        F: Fn(SettingsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let p = &config.personalization;
        let act_mica = on_action.clone();
        let act_toggle_theme_dd = on_action.clone();
        let act_toggle_scale_dd = on_action.clone();
        let act_toggle_lang_dd = on_action.clone();

        let open_theme_col = self.open_sections.contains("themecolor");
        let open_card_col = self.open_sections.contains("cardcolors");
        let open_death_col = self.open_sections.contains("deathfontcolor");

        let act_toggle_tcol = on_action.clone();
        let act_toggle_ccol = on_action.clone();
        let act_toggle_dcol = on_action.clone();

        let act_reset_tcol = on_action.clone();
        let act_reset_ccol = on_action.clone();
        let act_reset_dcol = on_action.clone();

        let cur_theme_mode = match p.theme_mode.as_str() {
            "Light" => "浅色",
            "Dark" => "深色",
            _ => "跟随系统",
        };

        let cur_scale = match p.dpi_scale.as_str() {
            "100" => "100%",
            "125" => "125%",
            "150" => "150%",
            _ => "跟随系统",
        };

        let cur_lang = match p.language.as_str() {
            "zh_CN" => "简体中文",
            "zh_TW" => "繁體中文",
            "en_US" => "English",
            _ => "跟随系统",
        };

        div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            // Mica 开关与主题模式 (直显卡)
            .child(
                Card::new().rounded(px(12.0)).overflow_hidden().child(
                    div()
                        .flex()
                        .flex_col()
                        .child(settings_card_item(
                            "云母效果 (Windows 11)",
                            Some("启用 Windows 11 云母材质效果，使窗口背景模糊及跟随系统主题"),
                            true,
                            div()
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                    act_mica(SettingsAction::ToggleMica, win, app)
                                })
                                .child(Switch::new(p.mica_enabled)),
                        ))
                        .child(settings_card_item(
                            "主题模式",
                            Some("选择浅色/深色/跟随系统"),
                            false,
                            select_dropdown(
                                "dropdown-theme-mode".into(),
                                140.0,
                                cur_theme_mode,
                                self.open_theme_mode_dropdown,
                                div()
                                    .flex()
                                    .flex_col()
                                    .child({
                                        let act = on_action.clone();
                                        select_dropdown_item(
                                            "浅色",
                                            p.theme_mode == "Light",
                                            move |_ev, win, app| {
                                                act(
                                                    SettingsAction::SetThemeMode("Light".into()),
                                                    win,
                                                    app,
                                                )
                                            },
                                        )
                                    })
                                    .child({
                                        let act = on_action.clone();
                                        select_dropdown_item(
                                            "深色",
                                            p.theme_mode == "Dark",
                                            move |_ev, win, app| {
                                                act(
                                                    SettingsAction::SetThemeMode("Dark".into()),
                                                    win,
                                                    app,
                                                )
                                            },
                                        )
                                    })
                                    .child({
                                        let act = on_action.clone();
                                        select_dropdown_item(
                                            "跟随系统",
                                            p.theme_mode == "Auto",
                                            move |_ev, win, app| {
                                                act(
                                                    SettingsAction::SetThemeMode("Auto".into()),
                                                    win,
                                                    app,
                                                )
                                            },
                                        )
                                    }),
                                move |_ev, win, app| {
                                    act_toggle_theme_dd(
                                        SettingsAction::ToggleDropdown("theme_mode"),
                                        win,
                                        app,
                                    )
                                },
                            ),
                        )),
                ),
            )
            // 1. 主题颜色折叠项 (F-V06)
            .child(collapse_card(
                "settings-card-themecolor".into(),
                open_theme_col,
                None,
                "主题颜色",
                "自定义软件界面主色调 (点击色块选择)",
                Some(p.theme_color.to_uppercase()),
                move |_ev, win, app| {
                    act_toggle_tcol(SettingsAction::ToggleCollapse("themecolor"), win, app)
                },
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .child(setting_row(
                        "选择调色盘颜色:",
                        None,
                        render_color_picker(
                            "picker-themecolor".into(),
                            100.0,
                            &p.theme_color,
                            self.open_theme_color_picker,
                            &THEME_PALETTE,
                            {
                                let act = on_action.clone();
                                move |_ev, win, app| {
                                    act(SettingsAction::ToggleDropdown("theme_color"), win, app)
                                }
                            },
                            {
                                let act = on_action.clone();
                                move |hex, win, app| {
                                    act(SettingsAction::SetThemeColor(hex), win, app)
                                }
                            },
                        ),
                    ))
                    .child(div().flex().justify_end().child(action_button(
                        "重置默认颜色",
                        false,
                        move |_ev, win, app| {
                                act_reset_tcol(SettingsAction::ResetThemeColor, win, app)
                        },
                    ))),
            ))
            // 2. 战绩卡片颜色折叠项 (F-V06)
            .child(collapse_card(
                "settings-card-cardcolors".into(),
                open_card_col,
                None,
                "战绩卡片颜色",
                "自定义胜利、失败、重开对局的战绩卡片颜色",
                Some("已设置自定义颜色".to_string()),
                move |_ev, win, app| {
                    act_toggle_ccol(SettingsAction::ToggleCollapse("cardcolors"), win, app)
                },
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(setting_row(
                        "胜利",
                        None,
                        render_color_picker(
                            "picker-win-card".into(),
                            120.0,
                            &p.win_card_color,
                            self.open_win_color_picker,
                            &WIN_CARD_PALETTE,
                            {
                                let act = on_action.clone();
                                move |_ev, win, app| {
                                    act(SettingsAction::ToggleDropdown("win_color"), win, app)
                                }
                            },
                            {
                                let act = on_action.clone();
                                move |hex, win, app| {
                                    act(SettingsAction::SetWinCardColor(hex), win, app)
                                }
                            },
                        ),
                    ))
                    .child(setting_row(
                        "失败",
                        None,
                        render_color_picker(
                            "picker-lose-card".into(),
                            120.0,
                            &p.lose_card_color,
                            self.open_lose_color_picker,
                            &LOSE_CARD_PALETTE,
                            {
                                let act = on_action.clone();
                                move |_ev, win, app| {
                                    act(SettingsAction::ToggleDropdown("lose_color"), win, app)
                                }
                            },
                            {
                                let act = on_action.clone();
                                move |hex, win, app| {
                                    act(SettingsAction::SetLoseCardColor(hex), win, app)
                                }
                            },
                        ),
                    ))
                    .child(setting_row(
                        "重开",
                        None,
                        render_color_picker(
                            "picker-remake-card".into(),
                            120.0,
                            &p.remake_card_color,
                            self.open_remake_color_picker,
                            &REMAKE_CARD_PALETTE,
                            {
                                let act = on_action.clone();
                                move |_ev, win, app| {
                                    act(SettingsAction::ToggleDropdown("remake_color"), win, app)
                                }
                            },
                            {
                                let act = on_action.clone();
                                move |hex, win, app| {
                                    act(SettingsAction::SetRemakeCardColor(hex), win, app)
                                }
                            },
                        ),
                    ))
                    .child(div().flex().justify_end().child(action_button(
                        "重置默认颜色",
                        false,
                        move |_ev, win, app| {
                                act_reset_ccol(SettingsAction::ResetCardColors, win, app)
                        },
                    ))),
            ))
            // 3. 死亡数字颜色折叠项 (F-V06)
            .child(collapse_card(
                "settings-card-deathfontcolor".into(),
                open_death_col,
                None,
                "死亡数字颜色",
                "自定义对局详情中死亡数字的颜色",
                Some("已配置死亡高亮".to_string()),
                move |_ev, win, app| {
                    act_toggle_dcol(SettingsAction::ToggleCollapse("deathfontcolor"), win, app)
                },
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(setting_row(
                        "浅色模式高亮色:",
                        None,
                        render_color_picker(
                            "picker-light-death".into(),
                            100.0,
                            &p.light_deaths_number_color,
                            self.open_light_death_color_picker,
                            &LIGHT_DEATH_PALETTE,
                            {
                                let act = on_action.clone();
                                move |_ev, win, app| {
                                    act(SettingsAction::ToggleDropdown("light_death"), win, app)
                                }
                            },
                            {
                                let act = on_action.clone();
                                move |hex, win, app| {
                                    act(SettingsAction::SetLightDeathsColor(hex), win, app)
                                }
                            },
                        ),
                    ))
                    .child(setting_row(
                        "深色模式高亮色:",
                        None,
                        render_color_picker(
                            "picker-dark-death".into(),
                            100.0,
                            &p.dark_deaths_number_color,
                            self.open_dark_death_color_picker,
                            &DARK_DEATH_PALETTE,
                            {
                                let act = on_action.clone();
                                move |_ev, win, app| {
                                    act(SettingsAction::ToggleDropdown("dark_death"), win, app)
                                }
                            },
                            {
                                let act = on_action.clone();
                                move |hex, win, app| {
                                    act(SettingsAction::SetDarkDeathsColor(hex), win, app)
                                }
                            },
                        ),
                    ))
                    .child(div().flex().justify_end().child(action_button(
                        "重置默认颜色",
                        false,
                        move |_ev, win, app| {
                                act_reset_dcol(SettingsAction::ResetDeathsColors, win, app)
                        },
                    ))),
            ))
            // 界面缩放与语言选择 (直显卡 F-V05)
            .child(
                Card::new().rounded(px(12.0)).overflow_hidden().child(
                    div()
                        .flex()
                        .flex_col()
                        .child(settings_card_item(
                            "界面缩放",
                            Some("支持调整客户端视图的 DPI 缩放大小比例"),
                            true,
                            select_dropdown(
                                "dropdown-dpi-scale".into(),
                                140.0,
                                cur_scale,
                                self.open_scale_dropdown,
                                div()
                                    .flex()
                                    .flex_col()
                                    .child({
                                        let act = on_action.clone();
                                        select_dropdown_item(
                                            "跟随系统",
                                            p.dpi_scale == "Auto",
                                            move |_ev, win, app| {
                                                act(
                                                    SettingsAction::SetDpiScale("Auto".into()),
                                                    win,
                                                    app,
                                                )
                                            },
                                        )
                                    })
                                    .child({
                                        let act = on_action.clone();
                                        select_dropdown_item(
                                            "100%",
                                            p.dpi_scale == "100",
                                            move |_ev, win, app| {
                                                act(
                                                    SettingsAction::SetDpiScale("100".into()),
                                                    win,
                                                    app,
                                                )
                                            },
                                        )
                                    })
                                    .child({
                                        let act = on_action.clone();
                                        select_dropdown_item(
                                            "125%",
                                            p.dpi_scale == "125",
                                            move |_ev, win, app| {
                                                act(
                                                    SettingsAction::SetDpiScale("125".into()),
                                                    win,
                                                    app,
                                                )
                                            },
                                        )
                                    })
                                    .child({
                                        let act = on_action.clone();
                                        select_dropdown_item(
                                            "150%",
                                            p.dpi_scale == "150",
                                            move |_ev, win, app| {
                                                act(
                                                    SettingsAction::SetDpiScale("150".into()),
                                                    win,
                                                    app,
                                                )
                                            },
                                        )
                                    }),
                                move |_ev, win, app| {
                                    act_toggle_scale_dd(
                                        SettingsAction::ToggleDropdown("dpi_scale"),
                                        win,
                                        app,
                                    )
                                },
                            ),
                        ))
                        .child(settings_card_item(
                            "语言",
                            Some("选择软件界面语言选项"),
                            false,
                            select_dropdown(
                                "dropdown-language".into(),
                                140.0,
                                cur_lang,
                                self.open_lang_dropdown,
                                div()
                                    .flex()
                                    .flex_col()
                                    .child({
                                        let act = on_action.clone();
                                        select_dropdown_item(
                                            "跟随系统",
                                            p.language == "Auto",
                                            move |_ev, win, app| {
                                                act(
                                                    SettingsAction::SetLanguage("Auto".into()),
                                                    win,
                                                    app,
                                                )
                                            },
                                        )
                                    })
                                    .child({
                                        let act = on_action.clone();
                                        select_dropdown_item(
                                            "简体中文",
                                            p.language == "zh_CN",
                                            move |_ev, win, app| {
                                                act(
                                                    SettingsAction::SetLanguage("zh_CN".into()),
                                                    win,
                                                    app,
                                                )
                                            },
                                        )
                                    })
                                    .child({
                                        let act = on_action.clone();
                                        select_dropdown_item(
                                            "繁體中文",
                                            p.language == "zh_TW",
                                            move |_ev, win, app| {
                                                act(
                                                    SettingsAction::SetLanguage("zh_TW".into()),
                                                    win,
                                                    app,
                                                )
                                            },
                                        )
                                    })
                                    .child({
                                        let act = on_action.clone();
                                        select_dropdown_item(
                                            "English",
                                            p.language == "en_US",
                                            move |_ev, win, app| {
                                                act(
                                                    SettingsAction::SetLanguage("en_US".into()),
                                                    win,
                                                    app,
                                                )
                                            },
                                        )
                                    }),
                                move |_ev, win, app| {
                                    act_toggle_lang_dd(
                                        SettingsAction::ToggleDropdown("language"),
                                        win,
                                        app,
                                    )
                                },
                            ),
                        )),
                ),
            )
    }

    // ─── 6. 软件更新组 (F-V07) ───
    fn render_update_group<F>(&self, config: &AppConfig, on_action: F) -> impl IntoElement
    where
        F: Fn(SettingsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let is_port = yuumi_core::runtime::is_portable();
        let act_check = on_action.clone();
        let act_release = on_action.clone();
        let act_switch = on_action.clone();

        Card::new().rounded(px(12.0)).overflow_hidden().child(
            div().flex().flex_col().child(settings_card_item(
                    "检查更新",
                    Some(if is_port {
                        "便携版通过 zip 覆盖方式自动更新，更新后自动重启"
                    } else {
                        "在 Yuumi 启动时自动检查更新 — 开启后，检测到新版本将自动后台静默下载"
                    }),
                    false,
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                    .child(action_button(
                        "检查更新",
                        false,
                        move |_ev, win, app| act_check(SettingsAction::CheckUpdate, win, app),
                    ))
                        .when(is_port, |this| {
                        this.child(action_button(
                            "前往 GitHub 下载",
                            false,
                            move |_ev, win, app| {
                                act_release(SettingsAction::GoToReleases, win, app)
                            },
                        ))
                        })
                        .child(
                            div()
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                    act_switch(SettingsAction::ToggleCheckUpdate, win, app)
                                })
                                .child(Switch::new(config.general.enable_check_update)),
                        ),
                )),
        )
    }

    // ─── 7. 网络代理 (F-V08) ───
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
            None,
            "软件 HTTP 代理",
            "软件更新、TFT 数据、OP.GG 及 GitHub 服务访问走此 HTTP 代理通道",
            Some(
                if g.enable_http_proxy {
                "已启用"
            } else {
                "未启用"
                }
                .to_string(),
            ),
            move |_ev, win, app| act_toggle(SettingsAction::ToggleCollapse("httpproxy"), win, app),
            div()
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(16.0))
                .child(self.f_input(2, "f-proxy", "127.0.0.1:7897", on_action.clone()))
                .child(
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_switch(SettingsAction::ToggleHttpProxy, win, app)
                        })
                        .child(Switch::new(g.enable_http_proxy)),
                ),
        )
    }

    // ─── 8. 关于卡片 (F-V07) ───
    fn render_about_group<F>(&self, _config: &AppConfig, on_action: F) -> impl IntoElement
    where
        F: Fn(SettingsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let act_log = on_action.clone();
        let act_repo = on_action.clone();

        Card::new().rounded(px(12.0)).overflow_hidden().child(
            div()
                .w_full()
                .px(px(24.0))
                .py(px(28.0))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(14.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.0))
                        // Logo 徽标
                        .child(
                            div()
                                .size(px(32.0))
                                .rounded(px(8.0))
                                .bg(Theme::ACCENT_PRIMARY)
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(18.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(gpui::white())
                                .child("Y"),
                        )
                        .child(
                            div()
                                .text_size(px(18.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(Theme::TEXT_PRIMARY)
                                .child("Yuumi"),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(Theme::TEXT_MUTED)
                                .child("v2.5.0"),
                        ),
                )
                .child(
                    div()
                        .max_w(px(560.0))
                        .text_size(px(12.0))
                        .line_height(relative(1.5))
                        .text_color(Theme::TEXT_MUTED)
                        .text_align(TextAlign::Center)
                        .child("Yuumi 是一款基于 Tauri 与 GPUI 原生构建的英雄联盟辅助工具，提供战绩查询、对局信息、云顶之弈数据、战利品管理以及自动对局功能，让游戏体验更轻松。"),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        .child(action_button("更新日志", false, move |_ev, win, app| {
                            act_log(SettingsAction::ToggleChangelog, win, app)
                        }))
                        .child(action_button("开源仓库", false, move |_ev, win, app| {
                            act_repo(SettingsAction::OpenRepo, win, app)
                        })),
                ),
        )
    }

    // ─── 更新日志模态浮层 (F-V07) ───
    fn render_changelog_modal<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(SettingsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let act_close = on_action.clone();
        let act_refresh = on_action.clone();

        div()
            .id("changelog-modal-backdrop")
            .absolute()
            .inset_0()
            .bg(rgba(0x00000066))
            .flex()
            .items_center()
            .justify_center()
            .occlude()
            .child(
                div()
                    .w(px(640.0))
                    .max_w(px(760.0))
                    .max_h(px(520.0))
                    .bg(rgb(0xffffff))
                    .rounded_xl()
                    .shadow_xl()
                    .border_1()
                    .border_color(Theme::CARD_BORDER)
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    // 弹窗头部
                    .child(
                        div()
                            .w_full()
                            .px(px(20.0))
                            .py(px(14.0))
                            .border_b_1()
                            .border_color(Theme::CARD_BORDER.opacity(0.6))
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div().flex().items_center().gap(px(10.0)).child(
                                        div()
                                            .text_size(px(15.0))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(Theme::TEXT_PRIMARY)
                                            .child("更新日志"),
                                    ),
                            )
                            .child(
                                div()
                                    .size(px(24.0))
                                    .rounded_md()
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(14.0))
                                    .text_color(Theme::TEXT_MUTED)
                                    .hover(|s| {
                                        s.bg(Theme::HOVER_BG).text_color(Theme::TEXT_PRIMARY)
                                    })
                                    .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                        act_close(SettingsAction::ToggleChangelog, win, app)
                                    })
                                    .child("✕"),
                            ),
                    )
                    // 日志内容列表
                    .child(
                        div()
                            .id("changelog-body-scroll")
                            .flex_1()
                            .overflow_y_scroll()
                            .p(px(20.0))
                            .flex()
                            .flex_col()
                            .gap(px(14.0))
                            .child(action_button("刷新", false, move |_, win, app| {
                                act_refresh(SettingsAction::RefreshChangelog, win, app)
                            }))
                            .when(self.release_loading, |d| d.child("正在加载更新日志…"))
                            .when_some(self.release_error.clone(), |d, error| d.child(error))
                            .children(self.release_notes.iter().map(|release| {
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.0))
                                    .child(format!("{}  {}", release.tag, release.published_at))
                                    .child(release.body.clone())
                            })),
                    ),
            )
    }
}
