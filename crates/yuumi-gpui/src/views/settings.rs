//! 全局设置视图 (SettingsView)
//! 遵循纯白水晶极光设计语言，包含客户端路径、启动设置、个性化外观、自动化偏好、日志维护与关于面板

use crate::components::badge::Badge;
use crate::components::card::Card;
use crate::components::switch::Switch;
use crate::theme::Theme;
use gpui::*;

#[derive(Clone)]
pub struct SettingsView {
    pub client_path: String,
    pub wegame_path: String,
    pub close_to_tray: bool,
    pub start_with_windows: bool,
    pub minimize_on_game_start: bool,
    pub mica_enabled: bool,
    pub hide_tft: bool,
    pub hide_saved_players: bool,
    pub selected_theme_index: usize, // 0: 极光青, 1: 极光蓝, 2: 幻影紫, 3: 暖阳金
    pub api_concurrency: u32,
    pub auto_accept_delay_seconds: u32,
    pub auto_pick_delay_seconds: u32,
    pub log_level: String,
}

impl Default for SettingsView {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsView {
    pub fn new() -> Self {
        Self {
            client_path: "D:\\WeGameApps\\英雄联盟\\Game".to_string(),
            wegame_path: "D:\\WeGame\\wegame.exe".to_string(),
            close_to_tray: true,
            start_with_windows: false,
            minimize_on_game_start: false,
            mica_enabled: true,
            hide_tft: true,
            hide_saved_players: false,
            selected_theme_index: 0,
            api_concurrency: 8,
            auto_accept_delay_seconds: 2,
            auto_pick_delay_seconds: 3,
            log_level: "Info".to_string(),
        }
    }

    pub fn render_panel<F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11>(
        &self,
        on_toggle_close_to_tray: F1,
        on_toggle_start_with_windows: F2,
        on_toggle_minimize_on_game: F3,
        on_toggle_mica: F4,
        on_toggle_hide_tft: F5,
        on_toggle_hide_saved: F6,
        on_select_theme: F7,
        on_detect_lol_path: F8,
        on_select_folder: F9,
        on_open_log_folder: F10,
        on_clear_cache: F11,
    ) -> impl IntoElement
    where
        F1: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
        F2: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
        F3: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
        F4: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
        F5: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
        F6: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
        F7: Fn(usize, &mut gpui::Window, &mut gpui::App) + 'static,
        F8: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
        F9: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
        F10: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
        F11: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    {
        let on_theme = std::rc::Rc::new(on_select_theme);
        let t0 = on_theme.clone();
        let t1 = on_theme.clone();
        let t2 = on_theme.clone();
        let t3 = on_theme.clone();

        div()
            .id("settings-scroll-view")
            .size_full()
            .flex()
            .flex_col()
            .gap_5()
            .p_4()
            .overflow_y_scroll()
            // 顶部标题
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child("设置 (Preferences & Configuration)"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::TEXT_MUTED)
                                    .child("管理客户端路径、窗口托盘行为、极光主题定制及本地缓存与日志"),
                            ),
                    )
                    .child(
                        Badge::new("v2.5.0 原生 GPUI 重构版")
                            .variant(crate::components::badge::BadgeVariant::Primary),
                    ),
            )
            // 模块 1：游戏客户端与启动设置
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(render_section_header("客户端与启动偏好 (Client & Startup)", Theme::AURORA_CYAN))
                    .child(
                        Card::new()
                            .p_4()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_4()
                                    // 路径 1: 英雄联盟根目录
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .gap_4()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .gap_1()
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(Theme::TEXT_PRIMARY)
                                                            .child("英雄联盟游戏安装路径"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(Theme::TEXT_MUTED)
                                                            .child(self.client_path.clone()),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .child(render_action_btn("自动检测", on_detect_lol_path))
                                                    .child(render_action_btn("浏览文件夹", on_select_folder)),
                                            ),
                                    )
                                    .child(render_divider())
                                    // 开关组: 启动与托盘
                                    .child(
                                        div()
                                            .grid_cols(2)
                                            .gap_3()
                                            .child(render_setting_switch(
                                                "📌 关闭主窗口时最小化到系统托盘",
                                                "点击窗口右上角关闭按钮时保留后台托盘运行，不退出应用",
                                                self.close_to_tray,
                                                on_toggle_close_to_tray,
                                            ))
                                            .child(render_setting_switch(
                                                "🚀 开机自启动 (Windows Startup)",
                                                "登录 Windows 时静默在系统托盘启动 Yuumi 助手",
                                                self.start_with_windows,
                                                on_toggle_start_with_windows,
                                            ))
                                            .child(render_setting_switch(
                                                "🎮 进入游戏对局时自动最小化",
                                                "检测到进入对局 (InProgress) 后自动将主窗口收起至托盘",
                                                self.minimize_on_game_start,
                                                on_toggle_minimize_on_game,
                                            ))
                                            .child(render_setting_switch(
                                                "✨ 启用 Windows 原生 Mica (云母) 背景特效",
                                                "在 Win11 系统下呈现原生微磨砂亚克力玻璃视觉材质",
                                                self.mica_enabled,
                                                on_toggle_mica,
                                            )),
                                    ),
                            ),
                    ),
            )
            // 模块 2：个性化与主题外观
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(render_section_header("个性化与极光外观 (Theme & Personalization)", Theme::AURORA_BLUE))
                    .child(
                        Card::new()
                            .p_4()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_4()
                                    // 主题色选择器
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .gap_1()
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(Theme::TEXT_PRIMARY)
                                                            .child("主题配色方案 (Accent Colors)"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(Theme::TEXT_MUTED)
                                                            .child("设置主色调、导航选中光效及卡片悬浮高亮颜色"),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .child(render_color_dot("极光青", Theme::AURORA_CYAN, self.selected_theme_index == 0, move |_, w, a| (t0)(0, w, a)))
                                                    .child(render_color_dot("极光蓝", Theme::AURORA_BLUE, self.selected_theme_index == 1, move |_, w, a| (t1)(1, w, a)))
                                                    .child(render_color_dot("幻影紫", hsla(270.0 / 360.0, 0.85, 0.60, 1.0), self.selected_theme_index == 2, move |_, w, a| (t2)(2, w, a)))
                                                    .child(render_color_dot("暖阳金", Theme::GOLD, self.selected_theme_index == 3, move |_, w, a| (t3)(3, w, a))),
                                            ),
                                    )
                                    .child(render_divider())
                                    // 侧边栏菜单显示控制
                                    .child(
                                        div()
                                            .grid_cols(2)
                                            .gap_3()
                                            .child(render_setting_switch(
                                                "♟️ 隐藏侧边栏「云顶之弈」入口",
                                                "专注峡谷与大乱斗模式，开启后从导航侧边栏隐藏云顶入口",
                                                self.hide_tft,
                                                on_toggle_hide_tft,
                                            ))
                                            .child(render_setting_switch(
                                                "📒 隐藏侧边栏「路人集」入口",
                                                "隐藏同局玩家相遇历史与标记管理入口，保持侧边栏极简",
                                                self.hide_saved_players,
                                                on_toggle_hide_saved,
                                            )),
                                    ),
                            ),
                    ),
            )
            // 模块 3：自动化性能与高级参数
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(render_section_header("高级参数与性能偏好 (Advanced Preferences)", Theme::GOLD))
                    .child(
                        Card::new()
                            .p_4()
                            .child(
                                div()
                                    .grid_cols(3)
                                    .gap_4()
                                    // 参数 1: LCU 并发请求数
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(Theme::TEXT_PRIMARY)
                                                    .child("LCU API 并发数"),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(Theme::TEXT_MUTED)
                                                    .child("对局信息批量拉取并发上限"),
                                            )
                                            .child(
                                                div()
                                                    .mt_1()
                                                    .child(Badge::new(format!("{} 线程", self.api_concurrency)).variant(crate::components::badge::BadgeVariant::Primary)),
                                            ),
                                    )
                                    // 参数 2: 自动接受延迟
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(Theme::TEXT_PRIMARY)
                                                    .child("自动接受等待延时"),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(Theme::TEXT_MUTED)
                                                    .child("匹配就绪弹出后的安全确认间隔"),
                                            )
                                            .child(
                                                div()
                                                    .mt_1()
                                                    .child(Badge::new(format!("{} 秒", self.auto_accept_delay_seconds)).variant(crate::components::badge::BadgeVariant::Default)),
                                            ),
                                    )
                                    // 参数 3: 选人锁定延迟
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(Theme::TEXT_PRIMARY)
                                                    .child("自动秒选锁定延迟"),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(Theme::TEXT_MUTED)
                                                    .child("轮到选人后防手抖延迟秒数"),
                                            )
                                            .child(
                                                div()
                                                    .mt_1()
                                                    .child(Badge::new(format!("{} 秒", self.auto_pick_delay_seconds)).variant(crate::components::badge::BadgeVariant::Default)),
                                            ),
                                    ),
                            ),
                    ),
            )
            // 模块 4：维护与关于
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(render_section_header("维护与系统信息 (Maintenance & About)", Theme::PREMADE_COLOR_3))
                    .child(
                        Card::new()
                            .p_4()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_4()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .gap_1()
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(Theme::TEXT_PRIMARY)
                                                            .child("客户端缓存与本地日志"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(Theme::TEXT_MUTED)
                                                            .child(format!("当前日志等级: {} · 缓存已加载", self.log_level)),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .child(render_action_btn("打开日志文件夹", on_open_log_folder))
                                                    .child(render_action_btn("清空静态资源缓存", on_clear_cache)),
                                            ),
                                    )
                                    .child(render_divider())
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .gap_1()
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(Theme::TEXT_PRIMARY)
                                                            .child("Yuumi 原生客户端引擎"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(Theme::TEXT_MUTED)
                                                            .child("Rust + GPUI 原生驱动 · Direct3D 11 / DirectWrite 硬件加速 · 零 Chromium 开销"),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .child(Badge::new("检查更新").ghost())
                                                    .child(Badge::new("GitHub 仓库").ghost()),
                                            ),
                                    ),
                            ),
                    ),
            )
    }
}

fn render_section_header(title: &'static str, dot_color: Hsla) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(div().size(px(8.0)).rounded_full().bg(dot_color))
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(Theme::TEXT_PRIMARY)
                .child(title),
        )
}

fn render_divider() -> impl IntoElement {
    div()
        .w_full()
        .h(px(1.0))
        .bg(Theme::CARD_BORDER.opacity(0.5))
}

fn render_setting_switch<F>(
    title: &'static str,
    desc: &'static str,
    checked: bool,
    on_toggle: F,
) -> impl IntoElement
where
    F: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
{
    div()
        .flex()
        .items_start()
        .justify_between()
        .gap_3()
        .p_2()
        .rounded_md()
        .hover(|this| this.bg(Theme::HOVER_BG))
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(Theme::TEXT_PRIMARY)
                        .child(title),
                )
                .child(div().text_xs().text_color(Theme::TEXT_MUTED).child(desc)),
        )
        .child(
            div()
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, on_toggle)
                .child(Switch::new(checked)),
        )
}

fn render_action_btn<F>(text: &'static str, on_click: F) -> impl IntoElement
where
    F: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
{
    div()
        .px_3()
        .py_1()
        .rounded(px(6.0))
        .bg(Theme::BADGE_BG)
        .border_1()
        .border_color(Theme::CARD_BORDER)
        .hover(|this| {
            this.bg(Theme::HOVER_BG)
                .border_color(Theme::ACCENT_PRIMARY.opacity(0.4))
        })
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, on_click)
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(Theme::TEXT_PRIMARY)
        .child(text)
}

fn render_color_dot<F>(
    _name: &'static str,
    color: Hsla,
    is_selected: bool,
    on_click: F,
) -> impl IntoElement
where
    F: Fn(&gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static,
{
    div()
        .size(px(20.0))
        .rounded_full()
        .bg(color)
        .border_2()
        .border_color(if is_selected {
            Theme::TEXT_PRIMARY
        } else {
            gpui::transparent_black()
        })
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, on_click)
}
