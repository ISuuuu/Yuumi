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
use std::cell::Cell;
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
    SampleSv(&'static str, f32, f32),
    SampleHue(&'static str, f32),
    SampleAlpha(&'static str, f32),
    SelectColorPreset(&'static str, String),
    EditColorPickerHex(String),
    ApplyColorPickerHex(&'static str),
    CloseColorPicker,
    SetDpiScale(String),
    SetLanguage(String),

    // ─── 6. 更新与代理 (Update & Proxy) ───
    CheckUpdate,
    ToggleCheckUpdate,
    GoToReleases,
    ToggleHttpProxy,

    // ─── 7. 关于 ───
    ToggleChangelog,
    RefreshChangelog,
    OpenRepo,
}

/// 24 色全色谱调色盘（覆盖暖色、冷色、粉紫与灰阶全光谱）
pub const FULL_SPECTRUM_PALETTE: [&str; 24] = [
    // 暖色 / 红橙黄
    "#F43F5E", "#EF4444", "#EA580C", "#F97316", "#F59E0B", "#EAB308", "#84CC16", "#22C55E",
    // 冷色 / 绿青蓝
    "#10B981", "#14B8A6", "#009FAA", "#06B6D4", "#0EA5E9", "#3B82F6", "#1890FF", "#6366F1",
    // 紫色 / 粉色 / 中性色
    "#8B5CF6", "#722ED1", "#A855F7", "#D946EF", "#EC4899", "#0F172A", "#64748B", "#CBD5E1",
];

/// 推荐主题色预设
pub const THEME_PALETTE: [(&str, &str); 12] = [
    ("#009FAA", "极光青 (默认)"),
    ("#1890FF", "极光蓝"),
    ("#722ED1", "幻影紫"),
    ("#FA8C16", "暖阳金"),
    ("#52C41A", "极光绿"),
    ("#F5222D", "烈焰红"),
    ("#EB2F96", "樱花粉"),
    ("#13C2C2", "青碧色"),
    ("#0EA5E9", "天蓝色"),
    ("#8B5CF6", "星灵紫"),
    ("#F97316", "暮霞橙"),
    ("#10B981", "翡翠绿"),
];

/// 胜利卡片推荐预设
pub const WIN_CARD_PALETTE: [(&str, &str); 6] = [
    ("#3339B01B", "默认绿 (20%)"),
    ("#4039B01B", "加深绿 (25%)"),
    ("#2539B01B", "淡雅绿 (15%)"),
    ("#3352C41A", "翠青绿 (20%)"),
    ("#3310B981", "薄荷绿 (20%)"),
    ("#331890FF", "胜利蓝 (20%)"),
];

/// 失败卡片推荐预设
pub const LOSE_CARD_PALETTE: [(&str, &str); 6] = [
    ("#33D3190C", "默认红 (20%)"),
    ("#40D3190C", "加深红 (25%)"),
    ("#25D3190C", "淡雅红 (15%)"),
    ("#33F5222D", "绯红 (20%)"),
    ("#33E11D48", "玫瑰红 (20%)"),
    ("#33722ED1", "败北紫 (20%)"),
];

/// 重开卡片推荐预设
pub const REMAKE_CARD_PALETTE: [(&str, &str); 6] = [
    ("#33A2A2A2", "默认灰 (20%)"),
    ("#40A2A2A2", "加深灰 (25%)"),
    ("#25A2A2A2", "淡雅灰 (15%)"),
    ("#338C8C8C", "冷月灰 (20%)"),
    ("#3364748B", "蓝灰 (20%)"),
    ("#33D97706", "重开金 (20%)"),
];

/// 浅色主题死亡数字字体推荐预设
pub const LIGHT_DEATH_PALETTE: [(&str, &str); 6] = [
    ("#FFB60000", "默认深红"),
    ("#FFD3190C", "猩红"),
    ("#FFCF1322", "暗赤"),
    ("#FFF5222D", "鲜红"),
    ("#FFE11D48", "玫瑰红"),
    ("#FF991B1B", "酒红"),
];

/// 深色主题死亡数字字体推荐预设
pub const DARK_DEATH_PALETTE: [(&str, &str); 6] = [
    ("#FFFF8D8D", "默认浅红"),
    ("#FFFF7875", "柔粉红"),
    ("#FFFFA39E", "浅桃红"),
    ("#FFFFBB96", "浅珊瑚"),
    ("#FFFB7185", "亮粉红"),
    ("#FFF87171", "亮珊瑚"),
];

/// 预设主题色（兼容外部引用）
pub const THEME_COLOR_PRESETS: [(&str, &str); 4] = [
    ("#009faa", "极光青"),
    ("#1890ff", "极光蓝"),
    ("#722ed1", "幻影紫"),
    ("#fa8c16", "暖阳金"),
];

pub use crate::theme::parse_hex_color;

/// 从 #AARRGGBB 或 #RRGGBB 中提取纯 RGB 部分（如 "#39B01B"）
pub fn extract_pure_rgb(hex: &str) -> String {
    let s = hex.trim().trim_start_matches('#');
    if s.len() == 8 {
        format!("#{}", &s[2..8]).to_uppercase()
    } else if s.len() == 6 {
        format!("#{}", s).to_uppercase()
    } else {
        "#000000".to_string()
    }
}

/// 从 #AARRGGBB 提取透明度百分比 (0..=100)；若为 6 位则默认 100
pub fn extract_alpha_percent(hex: &str) -> u32 {
    let s = hex.trim().trim_start_matches('#');
    if s.len() == 8 {
        if let Ok(a) = u8::from_str_radix(&s[0..2], 16) {
            return ((a as f32 / 255.0) * 100.0).round() as u32;
        }
    }
    100
}

/// 合成带透明度的颜色：将纯 RGB (如 "#39B01B") 与百分比 (0..=100) 合成为 "#AARRGGBB"
pub fn build_color_with_alpha(pure_rgb: &str, percent: u32) -> String {
    let clean = pure_rgb.trim().trim_start_matches('#');
    let rgb6 = if clean.len() >= 6 {
        &clean[clean.len() - 6..]
    } else {
        "000000"
    };
    let clamped = percent.clamp(0, 100);
    let alpha_byte = ((clamped as f32 / 100.0) * 255.0).round() as u8;
    format!("#{alpha_byte:02X}{rgb6}").to_uppercase()
}

/// 校验并规范化用户输入的颜色 Hex
pub fn normalize_hex_input(
    input: &str,
    supports_alpha: bool,
    current_alpha_percent: u32,
) -> Option<String> {
    let clean = input.trim().trim_start_matches('#');
    if clean.len() == 6 {
        if u32::from_str_radix(clean, 16).is_ok() {
            if supports_alpha {
                Some(build_color_with_alpha(&format!("#{clean}"), current_alpha_percent))
            } else {
                Some(format!("#{clean}").to_uppercase())
            }
        } else {
            None
        }
    } else if clean.len() == 8 {
        if u32::from_str_radix(clean, 16).is_ok() {
            if supports_alpha {
                Some(format!("#{clean}").to_uppercase())
            } else {
                Some(format!("#{}", &clean[2..8]).to_uppercase())
            }
        } else {
            None
        }
    } else {
        None
    }
}

/// 将 RGB [0.0..=1.0] 转换为 HSV (h: 0..360, s: 0..1, v: 0..1)
pub fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let h = if delta > 0.00001 {
        let raw = if (max - r).abs() < 0.00001 {
            ((g - b) / delta).rem_euclid(6.0)
        } else if (max - g).abs() < 0.00001 {
            (b - r) / delta + 2.0
        } else {
            (r - g) / delta + 4.0
        };
        (raw * 60.0).rem_euclid(360.0)
    } else {
        0.0
    };
    let s = if max > 0.00001 { delta / max } else { 0.0 };
    let v = max;
    (h, s, v)
}

/// 将 HSV (h: 0..360, s: 0..1, v: 0..1) 转换为 RGB [0.0..=1.0]
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let c = v * s;
    let h_prime = (h.rem_euclid(360.0)) / 60.0;
    let x = c * (1.0 - (h_prime.rem_euclid(2.0) - 1.0).abs());
    let m = v - c;
    let (r1, g1, b1) = match h_prime.floor() as i32 % 6 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (r1 + m, g1 + m, b1 + m)
}

/// 解析 Hex 色值为 (h, s, v, a)
pub fn hex_to_hsva(hex: &str) -> (f32, f32, f32, f32) {
    let s = hex.trim().trim_start_matches('#');
    let mut a = 1.0;
    let (r, g, b) = if s.len() == 8 {
        let a_byte = u8::from_str_radix(&s[0..2], 16).unwrap_or(255);
        let r_byte = u8::from_str_radix(&s[2..4], 16).unwrap_or(0);
        let g_byte = u8::from_str_radix(&s[4..6], 16).unwrap_or(0);
        let b_byte = u8::from_str_radix(&s[6..8], 16).unwrap_or(0);
        a = a_byte as f32 / 255.0;
        (
            r_byte as f32 / 255.0,
            g_byte as f32 / 255.0,
            b_byte as f32 / 255.0,
        )
    } else if s.len() == 6 {
        let r_byte = u8::from_str_radix(&s[0..2], 16).unwrap_or(0);
        let g_byte = u8::from_str_radix(&s[2..4], 16).unwrap_or(0);
        let b_byte = u8::from_str_radix(&s[4..6], 16).unwrap_or(0);
        (
            r_byte as f32 / 255.0,
            g_byte as f32 / 255.0,
            b_byte as f32 / 255.0,
        )
    } else {
        (0.0, 0.62, 0.67)
    };
    let (h, s_val, v) = rgb_to_hsv(r, g, b);
    (h, s_val, v, a)
}

/// 将 HSVA 合成为十六进制字符串（6 位 #RRGGBB 或 8 位 #AARRGGBB）
pub fn hsva_to_hex(h: f32, s: f32, v: f32, a: f32, include_alpha: bool) -> String {
    let (r, g, b) = hsv_to_rgb(h, s, v);
    let r_byte = (r * 255.0).round().clamp(0.0, 255.0) as u8;
    let g_byte = (g * 255.0).round().clamp(0.0, 255.0) as u8;
    let b_byte = (b * 255.0).round().clamp(0.0, 255.0) as u8;
    let a_byte = (a * 255.0).round().clamp(0.0, 255.0) as u8;
    if include_alpha {
        format!("#{a_byte:02X}{r_byte:02X}{g_byte:02X}{b_byte:02X}")
    } else {
        format!("#{r_byte:02X}{g_byte:02X}{b_byte:02X}")
    }
}

thread_local! {
    static SV_BOUNDS: Cell<Option<Bounds<Pixels>>> = const { Cell::new(None) };
    static HUE_BOUNDS: Cell<Option<Bounds<Pixels>>> = const { Cell::new(None) };
    static ALPHA_BOUNDS: Cell<Option<Bounds<Pixels>>> = const { Cell::new(None) };
}

/// HSV 连续取样调色浮层控件（1:1 复刻 Vue 原版二维画板取样 + 色相彩虹条取样 + 透明度取样体验）
#[allow(clippy::too_many_arguments)]
fn render_color_picker(
    id: ElementId,
    picker_key: &'static str,
    width_px: f32,
    color_hex: &str,
    supports_alpha: bool,
    open: bool,
    h: f32,
    s: f32,
    v: f32,
    a: f32,
    palette: &[(&'static str, &'static str)],
    hex_input: &str,
    hex_focus: Option<&FocusHandle>,
    window: &Window,
    on_toggle: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static + Clone,
    on_sample_sv: impl Fn(f32, f32, &mut Window, &mut App) + 'static + Clone,
    on_sample_hue: impl Fn(f32, &mut Window, &mut App) + 'static + Clone,
    on_sample_alpha: impl Fn(f32, &mut Window, &mut App) + 'static + Clone,
    on_select_preset: impl Fn(String, &mut Window, &mut App) + 'static + Clone,
    on_edit_hex: impl Fn(String, &mut Window, &mut App) + 'static + Clone,
    on_apply_hex: impl Fn(&'static str, &mut Window, &mut App) + 'static + Clone,
    on_close: impl Fn(&mut Window, &mut App) + 'static + Clone,
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
                    Theme::SIDEBAR_ACTIVE_BORDER()
                } else {
                    Theme::CARD_BORDER()
                })
                .bg(Theme::CARD_BG())
                .flex()
                .items_center()
                .justify_between()
                .gap(px(6.0))
                .cursor_pointer()
                .hover(|s| s.border_color(Theme::ACCENT_PRIMARY()))
                .on_mouse_down(MouseButton::Left, on_toggle)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(
                            div()
                                .w(px(18.0))
                                .h(px(16.0))
                                .rounded(px(3.0))
                                .border_1()
                                .border_color(Theme::CARD_BORDER())
                                .bg(swatch_color),
                        )
                        .child(
                            div()
                                .text_size(px(11.5))
                                .text_color(Theme::TEXT_SECONDARY())
                                .child(hex_display.clone()),
                        ),
                )
                .when(supports_alpha, |this| {
                    this.child(
                        div()
                            .text_size(px(10.5))
                            .text_color(Theme::TEXT_MUTED())
                            .child(format!("{:.0}%", a * 100.0)),
                    )
                }),
        )
        .when(open, |this| {
            // 当前色相的纯色底色 (Hue, S=1, V=1)
            let pure_hue = hsv_to_rgb(h, 1.0, 1.0);
            let pure_hue_rgba = Rgba {
                r: pure_hue.0,
                g: pure_hue.1,
                b: pure_hue.2,
                a: 1.0,
            };
            // 当前实际纯 RGB 色
            let cur_rgb = hsv_to_rgb(h, s, v);

            this.child(
                deferred(
                    div()
                        .absolute()
                        .top_full()
                        .right_0()
                        .mt(px(4.0))
                        .p(px(12.0))
                        .rounded_xl()
                        .border_1()
                        .border_color(Theme::CARD_BORDER())
                        .bg(rgb(0xffffff))
                        .shadow_xl()
                        .occlude()
                        .flex()
                        .flex_col()
                        .gap(px(10.0))
                        .w(px(280.0))
                        // 1. 顶部取样状态预览与关闭
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .pb(px(8.0))
                                .border_b_1()
                                .border_color(Theme::CARD_BORDER())
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(8.0))
                                        .child(
                                            div()
                                                .w(px(26.0))
                                                .h(px(22.0))
                                                .rounded_md()
                                                .border_1()
                                                .border_color(Theme::CARD_BORDER())
                                                .bg(swatch_color),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(12.5))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(Theme::TEXT_PRIMARY())
                                                .child(hex_display.clone()),
                                        )
                                        .when(supports_alpha, |t| {
                                            t.child(
                                                div()
                                                    .px(px(6.0))
                                                    .py(px(1.0))
                                                    .rounded_md()
                                                    .bg(Theme::HOVER_BG())
                                                    .text_size(px(10.5))
                                                    .text_color(Theme::TEXT_MUTED())
                                                    .child(format!("透明度: {:.0}%", a * 100.0)),
                                            )
                                        }),
                                )
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .text_color(Theme::TEXT_MUTED())
                                        .cursor_pointer()
                                        .hover(|s| s.text_color(Theme::TEXT_PRIMARY()))
                                        .on_mouse_down(MouseButton::Left, {
                                            let close = on_close.clone();
                                            move |_ev, win, app| close(win, app)
                                        })
                                        .child("✕"),
                                ),
                        )
                        // 2. 二维饱和度 / 明度取样画板 (S-V 采样板)
                        .child(
                            div()
                                .id("cp-sv-board")
                                .relative()
                                .w(px(256.0))
                                .h(px(120.0))
                                .rounded_md()
                                .overflow_hidden()
                                .cursor(CursorStyle::Crosshair)
                                // 底色为当前色相对应的纯光谱色
                                .bg(pure_hue_rgba)
                                // 水平白渐变：白色 -> 透明 (S: 0 -> 1)
                                .child(
                                    div()
                                        .absolute()
                                        .inset_0()
                                        .bg(linear_gradient(
                                            90.0,
                                            linear_color_stop(rgba(0xffffffff), 0.0),
                                            linear_color_stop(rgba(0xffffff00), 1.0),
                                        )),
                                )
                                // 垂直黑渐变：透明 -> 黑色 (V: 1 -> 0)
                                .child(
                                    div()
                                        .absolute()
                                        .inset_0()
                                        .bg(linear_gradient(
                                            180.0,
                                            linear_color_stop(rgba(0x00000000), 0.0),
                                            linear_color_stop(rgba(0x000000ff), 1.0),
                                        )),
                                )
                                .child(
                                    canvas(
                                        |bounds, _win, _app| {
                                            SV_BOUNDS.with(|c| c.set(Some(bounds)));
                                        },
                                        |_bounds, _, _win, _app| {},
                                    )
                                    .size_full()
                                    .absolute()
                                    .inset_0(),
                                )
                                // 取样指针十字环
                                .child(
                                    div()
                                        .absolute()
                                        .left(px((s * 256.0 - 6.0).clamp(0.0, 244.0)))
                                        .top(px(((1.0 - v) * 120.0 - 6.0).clamp(0.0, 108.0)))
                                        .size(px(12.0))
                                        .rounded_full()
                                        .border_2()
                                        .border_color(gpui::white())
                                        .shadow_sm(),
                                )
                                .on_mouse_down(MouseButton::Left, {
                                    let on_sv = on_sample_sv.clone();
                                    move |ev, win, app| {
                                        if let Some(bounds) = SV_BOUNDS.with(|c| c.get()) {
                                            let rx = ((f32::from(ev.position.x) - f32::from(bounds.origin.x)) / f32::from(bounds.size.width)).clamp(0.0, 1.0);
                                            let ry = ((f32::from(ev.position.y) - f32::from(bounds.origin.y)) / f32::from(bounds.size.height)).clamp(0.0, 1.0);
                                            on_sv(rx, 1.0 - ry, win, app);
                                        }
                                    }
                                })
                                .on_mouse_move({
                                    let on_sv = on_sample_sv.clone();
                                    move |ev, win, app| {
                                        if ev.dragging() {
                                            if let Some(bounds) = SV_BOUNDS.with(|c| c.get()) {
                                                let rx = ((f32::from(ev.position.x) - f32::from(bounds.origin.x)) / f32::from(bounds.size.width)).clamp(0.0, 1.0);
                                                let ry = ((f32::from(ev.position.y) - f32::from(bounds.origin.y)) / f32::from(bounds.size.height)).clamp(0.0, 1.0);
                                                on_sv(rx, 1.0 - ry, win, app);
                                            }
                                        }
                                    }
                                }),
                        )
                        // 3. 一维色相彩虹取样条 (Hue 渐变滑块)
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(3.0))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .text_size(px(10.5))
                                                .text_color(Theme::TEXT_MUTED())
                                                .child("色相 (Hue)"),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(10.5))
                                                .text_color(Theme::TEXT_MUTED())
                                                .child(format!("{:.0}°", h)),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("cp-hue-bar")
                                        .relative()
                                        .w(px(256.0))
                                        .h(px(12.0))
                                        .rounded_md()
                                        .overflow_hidden()
                                        .cursor_pointer()
                                        .flex()
                                        .child(div().flex_1().h_full().bg(linear_gradient(90.0, linear_color_stop(rgba(0xff0000ff), 0.0), linear_color_stop(rgba(0xffff00ff), 1.0))))
                                        .child(div().flex_1().h_full().bg(linear_gradient(90.0, linear_color_stop(rgba(0xffff00ff), 0.0), linear_color_stop(rgba(0x00ff00ff), 1.0))))
                                        .child(div().flex_1().h_full().bg(linear_gradient(90.0, linear_color_stop(rgba(0x00ff00ff), 0.0), linear_color_stop(rgba(0x00ffffff), 1.0))))
                                        .child(div().flex_1().h_full().bg(linear_gradient(90.0, linear_color_stop(rgba(0x00ffffff), 0.0), linear_color_stop(rgba(0x0000ffff), 1.0))))
                                        .child(div().flex_1().h_full().bg(linear_gradient(90.0, linear_color_stop(rgba(0x0000ffff), 0.0), linear_color_stop(rgba(0xff00ffff), 1.0))))
                                        .child(div().flex_1().h_full().bg(linear_gradient(90.0, linear_color_stop(rgba(0xff00ffff), 0.0), linear_color_stop(rgba(0xff0000ff), 1.0))))
                                        .child(
                                            canvas(
                                                |bounds, _win, _app| {
                                                    HUE_BOUNDS.with(|c| c.set(Some(bounds)));
                                                },
                                                |_bounds, _, _win, _app| {},
                                            )
                                            .size_full()
                                            .absolute()
                                            .inset_0(),
                                        )
                                        .child(
                                            div()
                                                .absolute()
                                                .left(px(((h / 360.0) * 256.0 - 6.0).clamp(0.0, 244.0)))
                                                .top(px(0.0))
                                                .size(px(12.0))
                                                .rounded_full()
                                                .border_2()
                                                .border_color(gpui::white())
                                                .shadow_sm(),
                                        )
                                        .on_mouse_down(MouseButton::Left, {
                                            let on_h = on_sample_hue.clone();
                                            move |ev, win, app| {
                                                if let Some(bounds) = HUE_BOUNDS.with(|c| c.get()) {
                                                    let rx = ((f32::from(ev.position.x) - f32::from(bounds.origin.x)) / f32::from(bounds.size.width)).clamp(0.0, 1.0);
                                                    on_h((rx * 360.0).min(359.9), win, app);
                                                }
                                            }
                                        })
                                        .on_mouse_move({
                                            let on_h = on_sample_hue.clone();
                                            move |ev, win, app| {
                                                if ev.dragging() {
                                                    if let Some(bounds) = HUE_BOUNDS.with(|c| c.get()) {
                                                        let rx = ((f32::from(ev.position.x) - f32::from(bounds.origin.x)) / f32::from(bounds.size.width)).clamp(0.0, 1.0);
                                                        on_h((rx * 360.0).min(359.9), win, app);
                                                    }
                                                }
                                            }
                                        }),
                                ),
                        )
                        // 4. 透明度滑条 (针对支持透明度的对局卡片)
                        .when(supports_alpha, |this| {
                            this.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(3.0))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .text_size(px(10.5))
                                                    .text_color(Theme::TEXT_MUTED())
                                                    .child("透明度 (Alpha)"),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(10.5))
                                                    .text_color(Theme::TEXT_MUTED())
                                                    .child(format!("{:.0}%", a * 100.0)),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id("cp-alpha-bar")
                                            .relative()
                                            .w(px(256.0))
                                            .h(px(12.0))
                                            .rounded_md()
                                            .overflow_hidden()
                                            .cursor_pointer()
                                            .bg(linear_gradient(
                                                90.0,
                                                linear_color_stop(Rgba { r: cur_rgb.0, g: cur_rgb.1, b: cur_rgb.2, a: 0.0 }, 0.0),
                                                linear_color_stop(Rgba { r: cur_rgb.0, g: cur_rgb.1, b: cur_rgb.2, a: 1.0 }, 1.0),
                                            ))
                                            .child(
                                                canvas(
                                                    |bounds, _win, _app| {
                                                        ALPHA_BOUNDS.with(|c| c.set(Some(bounds)));
                                                    },
                                                    |_bounds, _, _win, _app| {},
                                                )
                                                .size_full()
                                                .absolute()
                                                .inset_0(),
                                            )
                                            .child(
                                                div()
                                                    .absolute()
                                                    .left(px((a * 256.0 - 6.0).clamp(0.0, 244.0)))
                                                    .top(px(0.0))
                                                    .size(px(12.0))
                                                    .rounded_full()
                                                    .border_2()
                                                    .border_color(gpui::white())
                                                    .shadow_sm(),
                                            )
                                            .on_mouse_down(MouseButton::Left, {
                                                let on_a = on_sample_alpha.clone();
                                                move |ev, win, app| {
                                                    if let Some(bounds) = ALPHA_BOUNDS.with(|c| c.get()) {
                                                        let rx = ((f32::from(ev.position.x) - f32::from(bounds.origin.x)) / f32::from(bounds.size.width)).clamp(0.0, 1.0);
                                                        on_a(rx, win, app);
                                                    }
                                                }
                                            })
                                            .on_mouse_move({
                                                let on_a = on_sample_alpha.clone();
                                                move |ev, win, app| {
                                                    if ev.dragging() {
                                                        if let Some(bounds) = ALPHA_BOUNDS.with(|c| c.get()) {
                                                            let rx = ((f32::from(ev.position.x) - f32::from(bounds.origin.x)) / f32::from(bounds.size.width)).clamp(0.0, 1.0);
                                                            on_a(rx, win, app);
                                                        }
                                                    }
                                                }
                                            }),
                                    ),
                            )
                        })
                        // 5. 十六进制直接输入行
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(
                                    if let Some(focus) = hex_focus {
                                        let is_focused = focus.is_focused(window);
                                        let on_edit = on_edit_hex.clone();
                                        let on_app = on_apply_hex.clone();
                                        text_field(
                                            "color-hex-field",
                                            hex_input,
                                            if supports_alpha { "#AARRGGBB 或 #RRGGBB" } else { "#RRGGBB" },
                                            px(196.0),
                                            focus,
                                            is_focused,
                                            TextFieldOptions::default(),
                                            move |val, win, app| on_edit(val, win, app),
                                            move |_win, _app| {},
                                            move |_val, win, app| on_app(picker_key, win, app),
                                        ).into_any_element()
                                    } else {
                                        div().into_any_element()
                                    }
                                )
                                .child(
                                    action_button("应用", false, {
                                        let on_app = on_apply_hex.clone();
                                        move |_ev, win, app| on_app(picker_key, win, app)
                                    })
                                ),
                        )
                        // 6. 快捷推荐预设
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .text_size(px(10.5))
                                        .text_color(Theme::TEXT_MUTED())
                                        .child("常用预设"),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_wrap()
                                        .gap(px(5.0))
                                        .children(palette.iter().map(|(hex, _name)| {
                                            let h_str = hex.to_string();
                                            let c = parse_hex_color(hex);
                                            let is_cur = color_hex.eq_ignore_ascii_case(hex);
                                            let sel = on_select_preset.clone();
                                            div()
                                                .size(px(20.0))
                                                .rounded_md()
                                                .border_1()
                                                .border_color(if is_cur {
                                                    Theme::ACCENT_PRIMARY()
                                                } else {
                                                    Theme::CARD_BORDER()
                                                })
                                                .bg(c)
                                                .cursor_pointer()
                                                .hover(|s| s.opacity(0.8))
                                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                                    sel(h_str.clone(), win, app)
                                                })
                                        })),
                                ),
                        )
                        // 7. 底部完成按钮
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .pt(px(2.0))
                                .border_t_1()
                                .border_color(Theme::CARD_BORDER())
                                .child(
                                    action_button("完成", false, {
                                        let close = on_close.clone();
                                        move |_ev, win, app| close(win, app)
                                    })
                                ),
                        ),
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

    // 取样器交互状态 (HSV 取样)
    pub color_picker_key: &'static str,
    pub color_picker_h: f32,
    pub color_picker_s: f32,
    pub color_picker_v: f32,
    pub color_picker_a: f32,
    pub color_hex_input: String,
    pub color_hex_focus: Option<FocusHandle>,

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

            color_picker_key: "theme_color",
            color_picker_h: 184.0,
            color_picker_s: 1.0,
            color_picker_v: 0.67,
            color_picker_a: 1.0,
            color_hex_input: String::new(),
            color_hex_focus: None,

            show_changelog: false,
            release_loading: false,
            release_notes: Vec::new(),
            release_error: None,
            f_inputs: Default::default(),
            f_focus: None,
            status_feedback: None,
        }
    }

    /// 关闭所有打开的调色浮层
    pub fn close_all_color_pickers(&mut self) {
        self.open_theme_color_picker = false;
        self.open_win_color_picker = false;
        self.open_lose_color_picker = false;
        self.open_remake_color_picker = false;
        self.open_light_death_color_picker = false;
        self.open_dark_death_color_picker = false;
    }

    pub fn bind_f_inputs(&mut self, config: &AppConfig, cx: &mut App) {
        self.f_inputs = [
            config.general.upload_api_url.clone(),
            config.general.signalr_user_id.clone(),
            config.general.http_proxy_addr.clone(),
            config.functions.screenshot_save_path.clone(),
        ];
        self.f_focus = Some(std::array::from_fn(|_| cx.focus_handle()));
        self.color_hex_focus = Some(cx.focus_handle());
    }

    fn f_input<F>(
        &self,
        index: usize,
        key: &'static str,
        placeholder: &'static str,
        window: &Window,
        on_action: F,
    ) -> Div
    where
        F: Fn(SettingsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let Some(focus) = self.f_focus.as_ref().map(|f| &f[index]) else {
            return div();
        };
        let is_focused = focus.is_focused(window);
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
                is_focused,
                TextFieldOptions::default(),
                move |value, win, app| edit(SettingsAction::EditFInput(key, value), win, app),
                move |win, app| {
                    clear(SettingsAction::EditFInput(key, String::new()), win, app);
                    clear(SettingsAction::SaveFInput(key), win, app);
                },
                move |_, win, app| submit(SettingsAction::SaveFInput(key), win, app),
            ))
            .child(action_button(crate::i18n::t("common.save"), false, move |_, win, app| {
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

    /// 云端连接状态文案（yuumi-core 的 `HubStatus::label` 为硬编码中文，
    /// 视图层在本方法内改查 i18n 键，避免改动 yuumi-core）
    fn cloud_status_label(&self) -> &'static str {
        match self.cloud_status {
            yuumi_core::signalr::HubStatus::Disabled => crate::i18n::t("settings.cloudDisconnected"),
            yuumi_core::signalr::HubStatus::Connecting => crate::i18n::t("settings.cloudConnecting"),
            yuumi_core::signalr::HubStatus::Connected => crate::i18n::t("settings.cloudConnected"),
            yuumi_core::signalr::HubStatus::Retry => crate::i18n::t("settings.cloudConnectFailed"),
            yuumi_core::signalr::HubStatus::InvalidConfig => crate::i18n::t("settings.cloudInvalidAddr"),
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
        window: &mut Window,
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
                        div().flex().items_center().mb(px(8.0)).child(
                            div()
                                .text_size(px(22.0))
                                .font_weight(FontWeight::EXTRA_BOLD)
                                .text_color(Theme::TEXT_PRIMARY())
                                .child(crate::i18n::t("settings.title")),
                        ),
                    )
                    // ─── 1. 功能 ───
                    .child(self.render_group_header(crate::i18n::t("settings.groupFeatures")))
                    .child(self.render_features_group(config, on_action.clone()))
                    // ─── 2. OP.GG ───
                    .child(self.render_group_header("OP.GG"))
                    .child(self.render_opgg_group(config, on_action.clone()))
                    // ─── 3. 通用设置 ───
                    .child(self.render_group_header(crate::i18n::t("settings.generalGroup")))
                    .child(self.render_general_group(config, window, on_action.clone()))
                    // ─── 4. 日志 ───
                    .child(self.render_group_header(crate::i18n::t("settings.logGroup")))
                    .child(self.render_log_group(config, on_action.clone()))
                    // ─── 5. 个性化 ───
                    .child(self.render_group_header(crate::i18n::t("settings.personalizationGroup")))
                    .child(self.render_personalization_group(config, window, on_action.clone()))
                    // ─── 6. 软件更新 ───
                    .child(self.render_group_header(crate::i18n::t("settings.softwareUpdateGroup")))
                    .child(self.render_update_group(config, on_action.clone()))
                    // ─── 7. 网络代理 ───
                    .child(self.render_proxy_group(config, window, on_action.clone()))
                    // ─── 8. 关于 ───
                    .child(self.render_group_header(crate::i18n::t("settings.aboutGroup")))
                    .child(self.render_about_group(config, on_action.clone()))
                    .child(div().h(px(32.0))),
            )
    }

    /// 分组小标题（全中文、统一上边距与粗体）
    fn render_group_header(&self, title: &'static str) -> Div {
        div()
            .text_size(px(12.5))
            .font_weight(FontWeight::BOLD)
            .text_color(Theme::TEXT_MUTED())
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
                    crate::i18n::t("settings.lcuConcurrencyTitle"),
                    Some(crate::i18n::t("settings.lcuConcurrencyDesc")),
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
                    crate::i18n::t("settings.defaultGamesTitle"),
                    Some(crate::i18n::t("settings.defaultGamesDesc")),
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
                    crate::i18n::t("settings.gameInfoFilterTitle"),
                    Some(crate::i18n::t("settings.gameInfoFilterDesc")),
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
                    crate::i18n::t("settings.reserveGameInfoTitle"),
                    Some(crate::i18n::t("settings.reserveGameInfoDesc")),
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
                    crate::i18n::t("settings.showTierInGameInfoTitle"),
                    Some(crate::i18n::t("settings.showTierInGameInfoDesc")),
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
                    crate::i18n::t("settings.autoShowOpggTitle"),
                    Some(crate::i18n::t("settings.autoShowOpggDesc")),
                    true,
                    div()
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            act_show(SettingsAction::ToggleAutoShowOpgg, win, app)
                        })
                        .child(Switch::new(f.auto_show_opgg)),
                ))
                .child(settings_card_item(
                    crate::i18n::t("settings.pinOpggTitle"),
                    Some(crate::i18n::t("settings.pinOpggDesc")),
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
    fn render_general_group<F>(&self, config: &AppConfig, window: &Window, on_action: F) -> impl IntoElement
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
                crate::i18n::t("settings.lolPathGroup"),
                crate::i18n::t("settings.lolPathDesc"),
                Some(if g.lol_path.is_empty() {
                    crate::i18n::t("settings.pathNotSet").to_string()
                } else {
                    crate::i18n::t("settings.pathSetCount").replace("{count}", &g.lol_path.len().to_string())
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
                                .text_color(Theme::TEXT_MUTED())
                                .text_align(TextAlign::Center)
                                .child(crate::i18n::t("settings.pathEmpty")),
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
                            .border_color(Theme::CARD_BORDER())
                            .bg(Theme::CARD_BG())
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
                                                .bg(Theme::ACCENT_LIGHT())
                                                .text_size(px(11.0))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(Theme::ACCENT_PRIMARY())
                                                .child("WeGame"),
                                        )
                                    })
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(if is_wegame && g.wegame_path.is_none() {
                                                Theme::TEXT_MUTED()
                                            } else {
                                                Theme::TEXT_PRIMARY()
                                            })
                                            .child(if is_wegame {
                                                g.wegame_path
                                                    .as_deref()
                                                    .unwrap_or(crate::i18n::t("settings.wegamePathPlaceholder"))
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
                                    .text_color(Theme::TEXT_MUTED())
                                    .hover(|s| s.bg(Theme::HOVER_BG()).text_color(Theme::LOSS_TEXT()))
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
                                crate::i18n::t("settings.detectBtn"),
                                false,
                                move |_ev, win, app| {
                                    act_detect(SettingsAction::DetectLolPath, win, app)
                                },
                            ))
                            .child(action_button(
                                crate::i18n::t("settings.browseBtn"),
                                false,
                                move |_ev, win, app| {
                                    act_select_folder(SettingsAction::SelectLolFolder, win, app)
                                },
                            ))
                            .child(action_button(
                                crate::i18n::t("settings.wegameAddBtn"),
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
                            crate::i18n::t("settings.autoStartLolTitle"),
                            Some(crate::i18n::t("settings.autoStartLolDesc")),
                            true,
                            div()
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                    act_start_lol(SettingsAction::ToggleStartLolWithApp, win, app)
                                })
                                .child(Switch::new(g.enable_start_lol_with_app)),
                        ))
                        .child(settings_card_item(
                            crate::i18n::t("settings.clearCacheTitle"),
                            Some(crate::i18n::t("settings.clearCacheDesc")),
                            true,
                            action_button(crate::i18n::t("settings.deleteBtn"), false, move |_ev, win, app| {
                                act_clear_cache(SettingsAction::ClearCache, win, app)
                            }),
                        ))
                        .child(settings_card_item(
                            crate::i18n::t("settings.closeToTrayTitle"),
                            Some(crate::i18n::t("settings.closeToTrayDesc")),
                            true,
                            div()
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                    act_tray(SettingsAction::ToggleCloseToTray, win, app)
                                })
                                .child(Switch::new(g.enable_close_to_tray.unwrap_or(true))),
                        ))
                        .child(settings_card_item(
                            crate::i18n::t("settings.startMinimizedTitle"),
                            Some(crate::i18n::t("settings.startMinimizedDesc")),
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
                crate::i18n::t("settings.cloudServiceTitle"),
                crate::i18n::t("settings.cloudServiceDesc"),
                Some({
                    let upload_st = if g.upload_api_url.is_empty() {
                        crate::i18n::t("settings.uploadNotConfigured")
                    } else {
                        crate::i18n::t("settings.uploadConfigured")
                    };
                    if f.lcu_realtime_enabled {
                        format!("{} / {}", upload_st, self.cloud_status_label())
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
                        crate::i18n::t("settings.apiServerAddrLabel"),
                        None,
                        self.f_input(0, "f-upload", "http://example.com", window, on_action.clone()),
                    ))
                    // 2. LCU 实时服务开关
                    .child(setting_row(
                        crate::i18n::t("settings.realtimeLcuLabel"),
                        Some(crate::i18n::t("settings.realtimeLcuHint")),
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
                        self.f_input(1, "f-user", crate::i18n::t("settings.useridPlaceholder"), window, on_action.clone()),
                    )),
            ))
            // 隐藏菜单折叠卡片 (F-V04)
            .child(collapse_card(
                "settings-card-hidemenu".into(),
                open_hide,
                None,
                crate::i18n::t("settings.hideMenuTitle"),
                crate::i18n::t("settings.hideMenuDesc"),
                Some(
                    if f.hide_tft || f.hide_saved_players {
                        crate::i18n::t("settings.enabled")
                    } else {
                        crate::i18n::t("settings.disabled")
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
                        crate::i18n::t("settings.hideMenuOptionTft"),
                        None,
                        div()
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                act_tft(SettingsAction::ToggleHideTft, win, app)
                            })
                            .child(Switch::new(f.hide_tft)),
                    ))
                    .child(setting_row(
                        crate::i18n::t("settings.hideMenuOptionSavedPlayers"),
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
            .child(self.render_group_header(crate::i18n::t("settings.screenshotGroup")))
            .child(collapse_card(
                "settings-card-screenshot".into(),
                open_shot,
                None,
                crate::i18n::t("settings.enableScreenshotOnMultikillTitle"),
                crate::i18n::t("settings.enableScreenshotOnMultikillDesc"),
                Some(
                    if f.enable_screenshot_on_multikill {
                        crate::i18n::t("settings.enabled")
                    } else {
                        crate::i18n::t("settings.disabled")
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
                        crate::i18n::t("settings.enableScreenshotOnMultikillTitle"),
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
                            crate::i18n::t("settings.screenshotLevelsTitle"),
                            None,
                            div()
                                .flex()
                                .items_center()
                                .gap(px(14.0))
                                .child(checkbox(
                                    "cb-triple".into(),
                                    crate::i18n::t("settings.tripleKill"),
                                    f.screenshot_on_multikill_levels.contains(&3),
                                    move |_ev, win, app| {
                                        act_lvl3(SettingsAction::ToggleScreenshotLevel(3), win, app)
                                    },
                                ))
                                .child(checkbox(
                                    "cb-quadra".into(),
                                    crate::i18n::t("settings.quadraKill"),
                                    f.screenshot_on_multikill_levels.contains(&4),
                                    move |_ev, win, app| {
                                        act_lvl4(SettingsAction::ToggleScreenshotLevel(4), win, app)
                                    },
                                ))
                                .child(checkbox(
                                    "cb-penta".into(),
                                    crate::i18n::t("settings.pentaKill"),
                                    f.screenshot_on_multikill_levels.contains(&5),
                                    move |_ev, win, app| {
                                        act_lvl5(SettingsAction::ToggleScreenshotLevel(5), win, app)
                                    },
                                ))
                                .child(checkbox(
                                    "cb-godlike".into(),
                                    crate::i18n::t("settings.legendary"),
                                    f.screenshot_on_multikill_levels.contains(&8),
                                    move |_ev, win, app| {
                                        act_lvl8(SettingsAction::ToggleScreenshotLevel(8), win, app)
                                    },
                                )),
                        ))
                        .child(setting_row(
                            crate::i18n::t("settings.screenshotSavePathTitle"),
                            None,
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .flex_1()
                                .child(self.f_input(
                                    3,
                                    "f-screenshot",
                                    crate::i18n::t("settings.screenshotSavePathPlaceholder"),
                                    window,
                                    on_action.clone(),
                                ))
                                .child(action_button(
                                    crate::i18n::t("settings.browseBtn"),
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
                                    crate::i18n::t("settings.openScreenshotFolderBtn"),
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
                    crate::i18n::t("settings.logLevelTitle"),
                    Some(crate::i18n::t("settings.logLevelDesc")),
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
                    crate::i18n::t("settings.logFileTitle"),
                    Some(crate::i18n::t("settings.logFileDesc")),
                    false,
                    action_button(crate::i18n::t("settings.openFolderBtn"), false, move |_ev, win, app| {
                        act_open(SettingsAction::OpenLogFolder, win, app)
                    }),
                )),
        )
    }

    // ─── 5. 个性化设置组 (F-V05 ~ F-V06) ───
    fn render_personalization_group<F>(&self, config: &AppConfig, window: &Window, on_action: F) -> impl IntoElement
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
            "Light" => crate::i18n::t("settings.themeModeLight"),
            "Dark" => crate::i18n::t("settings.themeModeDark"),
            _ => crate::i18n::t("settings.themeModeAuto"),
        };

        let cur_scale = match p.dpi_scale.as_str() {
            "100" => "100%",
            "125" => "125%",
            "150" => "150%",
            _ => crate::i18n::t("settings.dpiScaleAuto"),
        };

        let cur_lang = match p.language.as_str() {
            "zh_CN" => "简体中文",
            "zh_TW" => "繁體中文",
            "en_US" => "English",
            _ => crate::i18n::t("settings.langAuto"),
        };

        let make_color_picker = |picker_id: ElementId,
                                 picker_key: &'static str,
                                 width_px: f32,
                                 color_hex: &str,
                                 supports_alpha: bool,
                                 open: bool,
                                 palette: &'static [(&'static str, &'static str)]| {
            let act_toggle = on_action.clone();
            let act_sv = on_action.clone();
            let act_h = on_action.clone();
            let act_a = on_action.clone();
            let act_preset = on_action.clone();
            let act_edit = on_action.clone();
            let act_apply = on_action.clone();
            let act_close = on_action.clone();

            render_color_picker(
                picker_id,
                picker_key,
                width_px,
                color_hex,
                supports_alpha,
                open,
                self.color_picker_h,
                self.color_picker_s,
                self.color_picker_v,
                self.color_picker_a,
                palette,
                &self.color_hex_input,
                self.color_hex_focus.as_ref(),
                window,
                move |_ev, win, app| act_toggle(SettingsAction::ToggleDropdown(picker_key), win, app),
                move |s, v, win, app| act_sv(SettingsAction::SampleSv(picker_key, s, v), win, app),
                move |h, win, app| act_h(SettingsAction::SampleHue(picker_key, h), win, app),
                move |a, win, app| act_a(SettingsAction::SampleAlpha(picker_key, a), win, app),
                move |hex, win, app| act_preset(SettingsAction::SelectColorPreset(picker_key, hex), win, app),
                move |val, win, app| act_edit(SettingsAction::EditColorPickerHex(val), win, app),
                move |key, win, app| act_apply(SettingsAction::ApplyColorPickerHex(key), win, app),
                move |win, app| act_close(SettingsAction::CloseColorPicker, win, app),
            )
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
                            crate::i18n::t("settings.micaTitle"),
                            Some(crate::i18n::t("settings.micaDesc")),
                            true,
                            div()
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                                    act_mica(SettingsAction::ToggleMica, win, app)
                                })
                                .child(Switch::new(p.mica_enabled)),
                        ))
                        .child(settings_card_item(
                            crate::i18n::t("settings.themeModeTitle"),
                            Some(crate::i18n::t("settings.themeModeDesc")),
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
                                            crate::i18n::t("settings.themeModeLight"),
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
                                            crate::i18n::t("settings.themeModeDark"),
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
                                            crate::i18n::t("settings.themeModeAuto"),
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
                crate::i18n::t("settings.themeColorTitle"),
                crate::i18n::t("settings.themeColorDesc"),
                Some(p.theme_color.to_uppercase()),
                move |_ev, win, app| {
                    act_toggle_tcol(SettingsAction::ToggleCollapse("themecolor"), win, app)
                },
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .child(setting_row(
                        crate::i18n::t("settings.themeColorPickerLabel"),
                        None,
                        make_color_picker(
                            "picker-themecolor".into(),
                            "theme_color",
                            100.0,
                            &p.theme_color,
                            false,
                            self.open_theme_color_picker,
                            &THEME_PALETTE,
                        ),
                    ))
                    .child(div().flex().justify_end().child(action_button(
                        crate::i18n::t("settings.resetColors"),
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
                crate::i18n::t("settings.cardColorTitle"),
                crate::i18n::t("settings.cardColorDesc"),
                Some(crate::i18n::t("settings.cardColorStatusSet").to_string()),
                move |_ev, win, app| {
                    act_toggle_ccol(SettingsAction::ToggleCollapse("cardcolors"), win, app)
                },
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(setting_row(
                        crate::i18n::t("settings.winCard"),
                        None,
                        make_color_picker(
                            "picker-win-card".into(),
                            "win_color",
                            120.0,
                            &p.win_card_color,
                            true,
                            self.open_win_color_picker,
                            &WIN_CARD_PALETTE,
                        ),
                    ))
                    .child(setting_row(
                        crate::i18n::t("settings.loseCard"),
                        None,
                        make_color_picker(
                            "picker-lose-card".into(),
                            "lose_color",
                            120.0,
                            &p.lose_card_color,
                            true,
                            self.open_lose_color_picker,
                            &LOSE_CARD_PALETTE,
                        ),
                    ))
                    .child(setting_row(
                        crate::i18n::t("settings.remakeCard"),
                        None,
                        make_color_picker(
                            "picker-remake-card".into(),
                            "remake_color",
                            120.0,
                            &p.remake_card_color,
                            true,
                            self.open_remake_color_picker,
                            &REMAKE_CARD_PALETTE,
                        ),
                    ))
                    .child(div().flex().justify_end().child(action_button(
                        crate::i18n::t("settings.resetColors"),
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
                crate::i18n::t("settings.deathColorTitle"),
                crate::i18n::t("settings.deathColorDesc"),
                Some(crate::i18n::t("settings.deathColorStatusSet").to_string()),
                move |_ev, win, app| {
                    act_toggle_dcol(SettingsAction::ToggleCollapse("deathfontcolor"), win, app)
                },
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(setting_row(
                        crate::i18n::t("settings.lightDeathLabel"),
                        None,
                        make_color_picker(
                            "picker-light-death".into(),
                            "light_death",
                            100.0,
                            &p.light_deaths_number_color,
                            false,
                            self.open_light_death_color_picker,
                            &LIGHT_DEATH_PALETTE,
                        ),
                    ))
                    .child(setting_row(
                        crate::i18n::t("settings.darkDeathLabel"),
                        None,
                        make_color_picker(
                            "picker-dark-death".into(),
                            "dark_death",
                            100.0,
                            &p.dark_deaths_number_color,
                            false,
                            self.open_dark_death_color_picker,
                            &DARK_DEATH_PALETTE,
                        ),
                    ))
                    .child(div().flex().justify_end().child(action_button(
                        crate::i18n::t("settings.resetColors"),
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
                            crate::i18n::t("settings.dpiScaleTitle"),
                            Some(crate::i18n::t("settings.dpiScaleDesc")),
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
                                            crate::i18n::t("settings.dpiScaleAuto"),
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
                            crate::i18n::t("settings.langTitle"),
                            Some(crate::i18n::t("settings.langDesc")),
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
                                            crate::i18n::t("settings.langAuto"),
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
                crate::i18n::t("settings.checkUpdateTitle"),
                Some(if is_port {
                    crate::i18n::t("settings.checkUpdateDescPortable")
                } else {
                    crate::i18n::t("settings.checkUpdateDescGpui")
                }),
                false,
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .child(action_button(
                        crate::i18n::t("settings.checkUpdateBtn"),
                        false,
                        move |_ev, win, app| act_check(SettingsAction::CheckUpdate, win, app),
                    ))
                    .when(is_port, |this| {
                        this.child(action_button(
                            crate::i18n::t("settings.goToReleases"),
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
    fn render_proxy_group<F>(&self, config: &AppConfig, window: &Window, on_action: F) -> impl IntoElement
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
            crate::i18n::t("settings.httpProxyGroup"),
            crate::i18n::t("settings.httpProxyDesc"),
            Some(
                if g.enable_http_proxy {
                    crate::i18n::t("settings.enabled")
                } else {
                    crate::i18n::t("settings.disabled")
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
                .child(self.f_input(2, "f-proxy", "127.0.0.1:7897", window, on_action.clone()))
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
                                .bg(Theme::ACCENT_PRIMARY())
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
                                .text_color(Theme::TEXT_PRIMARY())
                                .child("Yuumi"),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(Theme::TEXT_MUTED())
                                .child("v2.5.0"),
                        ),
                )
                .child(
                    div()
                        .max_w(px(560.0))
                        .text_size(px(12.0))
                        .line_height(relative(1.5))
                        .text_color(Theme::TEXT_MUTED())
                        .text_align(TextAlign::Center)
                        .child(crate::i18n::t("settings.aboutIntroGpui")),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        .child(action_button(crate::i18n::t("settings.aboutHistoryBtn"), false, move |_ev, win, app| {
                            act_log(SettingsAction::ToggleChangelog, win, app)
                        }))
                        .child(action_button(crate::i18n::t("settings.aboutRepository"), false, move |_ev, win, app| {
                            act_repo(SettingsAction::OpenRepo, win, app)
                        })),
                ),
        )
    }

    // ─── 更新日志模态浮层 (F-V07) ───
    pub fn render_changelog_modal<F>(&self, on_action: F) -> impl IntoElement
    where
        F: Fn(SettingsAction, &mut Window, &mut App) + 'static + Clone,
    {
        let act_close = on_action.clone();
        let act_refresh = on_action.clone();

        deferred(
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
                        .border_color(Theme::CARD_BORDER())
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
                                .border_color(Theme::CARD_BORDER().opacity(0.6))
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div().flex().items_center().gap(px(10.0)).child(
                                        div()
                                            .text_size(px(15.0))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(Theme::TEXT_PRIMARY())
                                            .child(crate::i18n::t("settings.aboutHistoryTitle")),
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
                                        .text_color(Theme::TEXT_MUTED())
                                        .hover(|s| {
                                            s.bg(Theme::HOVER_BG()).text_color(Theme::TEXT_PRIMARY())
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
                                .child(action_button(crate::i18n::t("common.refresh"), false, move |_, win, app| {
                                    act_refresh(SettingsAction::RefreshChangelog, win, app)
                                }))
                                .when(self.release_loading, |d| d.child(crate::i18n::t("settings.aboutHistoryLoading")))
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
                ),
        )
        .with_priority(10)
    }
}
