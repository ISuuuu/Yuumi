//! 工具箱「客户端」组直显行：锁定游戏设置、修复客户端窗口、重启客户端。
//!
//! 归属工单 D03（锁定/解锁游戏设置、修复窗口、重启客户端）。
//! 自 `views/tools.rs` 按工单边界搬移，行为未改动。

use super::components::*;
use super::{ToolsAction, ToolsView};
use crate::components::switch::Switch;
use crate::theme::Theme;
use gpui::*;

impl ToolsView {
    // 行 12: 锁定游戏设置 (直显行 - T-V02)
    pub(super) fn render_lock_game_settings_row<F>(&self, on_action: F) -> impl IntoElement
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
    pub(super) fn render_fix_window_row<F>(&self, on_action: F) -> impl IntoElement
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
                .border_color(Theme::CARD_BORDER())
                .bg(Theme::CARD_BG())
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(Theme::TEXT_PRIMARY())
                .cursor_pointer()
                .hover(|s| {
                    s.bg(Theme::HOVER_BG())
                        .border_color(Theme::ACCENT_PRIMARY())
                })
                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                    act_fix(ToolsAction::FixClientWindow, win, app)
                })
                .child("修复"),
        )
    }

    // 行 14: 重启客户端 (直显行)
    pub(super) fn render_restart_client_row<F>(&self, on_action: F) -> impl IntoElement
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
                .border_color(Theme::CARD_BORDER())
                .bg(Theme::CARD_BG())
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(Theme::TEXT_PRIMARY())
                .cursor_pointer()
                .hover(|s| {
                    s.bg(Theme::HOVER_BG())
                        .border_color(Theme::ACCENT_PRIMARY())
                })
                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                    act_restart(ToolsAction::RestartClient, win, app)
                })
                .child("重启"),
        )
    }
}
