//! 首页（对照 Vue `src/views/Home.vue`）
//!
//! 1:1 核心复刻要点 (H01~H04)：
//! - H01: 已连接状态大标题「已连接至英雄联盟客户端」，详情卡片展示 PID / --app-port，未读到显示「正在获取客户端连接详情...」，右下角绝对定位猫咪 Logo (120x120px) 带 hover / active 效果。
//! - H02: 未连接离线状态居中纵向排布：Logo 图 (96x96)、说明、带播放三角图标的「启动英雄联盟客户端」按钮、客户端路径列表（有路径才显示）、底部等待环与「等待英雄联盟客户端启动中...」。
//! - H03: 路径列表上方显示「客户端启动路径:」，单选 radio 与高亮边框；WeGame 标记（WEGAME_MARKER / "WEGAME"）默认选中，展示彩色 WeGame 徽章，次文案显示配置路径或斜体「未设置 WeGame 路径」；点击切换选中路径，点击启动调用 launch_lol_client。
//! - H04: 连接状态切换时立即响应，断开切为离线未连接状态，就绪立即展示真实 PID / port。

use crate::assets::logo_image;
use crate::components::icon::icon;
use crate::components::offline_state::render_lcu_offline_state_with_tip;
use crate::services::system_tools::{load_client_paths, WEGAME_MARKER};
use crate::services::ConnectionStatus;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;

pub fn is_wegame(path: &str) -> bool {
    path == WEGAME_MARKER || path.eq_ignore_ascii_case("wegame")
}

#[derive(Clone)]
pub struct HomeView {
    /// 客户端路径列表（对应配置中的 LolPath）
    pub lol_paths: Vec<String>,
    /// WeGame 路径（对应配置中的 WegamePath）
    pub wegame_path: String,
    /// 当前选中的客户端路径
    pub selected_path: Option<String>,
    /// 启动状态或错误信息（message, is_error）
    pub launch_message: Option<(String, bool)>,
}

impl Default for HomeView {
    fn default() -> Self {
        Self::new()
    }
}

impl HomeView {
    pub fn new() -> Self {
        let (paths, wegame) = load_client_paths();
        let selected = if paths.iter().any(|p| is_wegame(p)) {
            paths.iter().find(|p| is_wegame(p)).cloned()
        } else {
            paths.first().cloned()
        };

        Self {
            lol_paths: paths,
            wegame_path: wegame,
            selected_path: selected,
            launch_message: None,
        }
    }

    /// 重新载入客户端路径（当从设置页修改或重新检测后调用）
    pub fn reload_paths(&mut self) {
        let (paths, wegame) = load_client_paths();
        if self.selected_path.is_none()
            || !self
                .selected_path
                .as_ref()
                .map(|s| paths.contains(s))
                .unwrap_or(false)
        {
            self.selected_path = if paths.iter().any(|p| is_wegame(p)) {
                paths.iter().find(|p| is_wegame(p)).cloned()
            } else {
                paths.first().cloned()
            };
        }
        self.lol_paths = paths;
        self.wegame_path = wegame;
    }

    pub fn render<F1, F2>(
        &self,
        status: &ConnectionStatus,
        on_select_path: F1,
        on_launch: F2,
    ) -> impl IntoElement
    where
        F1: Fn(String, &mut Window, &mut App) + 'static + Clone,
        F2: Fn(&mut Window, &mut App) + 'static + Clone,
    {
        if matches!(status, ConnectionStatus::Connected { .. }) {
            div()
                .relative()
                .size_full()
                .px(px(24.0))
                .py(px(40.0))
                .flex()
                .items_center()
                .justify_center()
                .child(self.render_connected(status))
                .child(
                    // 连接成功后猫咪移到窗体右下角（bottom: 40px, right: 40px, 120x120px，保持比例，悬停/激活效果）
                    div()
                        .absolute()
                        .bottom(px(40.0))
                        .right(px(40.0))
                        .cursor_pointer()
                        .opacity(0.85)
                        .hover(|s| s.opacity(1.0).shadow_lg())
                        .when_some(logo_image(), |el, logo| {
                            el.child(
                                img(logo)
                                    .w(px(120.0))
                                    .h(px(120.0))
                                    .object_fit(ObjectFit::Contain),
                            )
                        }),
                )
                .into_any_element()
        } else {
            render_lcu_offline_state_with_tip(
                "请先启动英雄联盟客户端，或者通过下方按钮直接启动游戏",
                Some(self.render_offline_actions(on_select_path, on_launch).into_any_element()),
            )
        }
    }

    /// H01 已连接状态
    fn render_connected(&self, status: &ConnectionStatus) -> impl IntoElement {
        let details = match status {
            ConnectionStatus::Connected { info, .. } => {
                if info.pid > 0 || info.port > 0 {
                    Some((info.pid, info.port))
                } else {
                    None
                }
            }
            _ => None,
        };

        let card_body = if let Some((pid, port)) = details {
            let mut rows = div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(10.0))
                .text_size(px(13.1))
                .font_family(mono_font_family())
                .text_color(Theme::TEXT_MUTED());

            for (label, value) in [
                ("PID =", pid.to_string()),
                ("--app-port =", port.to_string()),
            ] {
                rows = rows.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(px(8.0))
                        .whitespace_nowrap()
                        .child(div().text_color(Theme::TEXT_MUTED()).child(label))
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(Theme::TEXT_PRIMARY())
                                .child(value),
                        ),
                );
            }
            rows.into_any_element()
        } else {
            div()
                .text_size(px(13.6))
                .text_color(Theme::TEXT_MUTED())
                .child("正在获取客户端连接详情...")
                .into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .items_center()
            .w_full()
            .max_w(px(600.0))
            .child(
                div()
                    .text_size(px(28.8))
                    .font_weight(FontWeight::EXTRA_BOLD)
                    .text_color(Theme::TEXT_PRIMARY())
                    .mb(px(20.0))
                    .child("已连接客户端"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .w_full()
                    .max_w(px(500.0))
                    .p(px(24.0))
                    .rounded(Theme::RADIUS_LG)
                    .bg(Theme::over_page(Theme::CARD_BG()))
                    .border_1()
                    .border_color(Theme::CARD_BORDER())
                    .shadow_sm()
                    .hover(|s| {
                        s.border_color(Theme::ACCENT_PRIMARY().opacity(0.3))
                            .shadow_md()
                    })
                    .child(card_body),
            )
    }

    /// H02 未连接离线操作区（启动按钮、路径列表与等待环）
    fn render_offline_actions<F1, F2>(&self, on_select_path: F1, on_launch: F2) -> impl IntoElement
    where
        F1: Fn(String, &mut Window, &mut App) + 'static + Clone,
        F2: Fn(&mut Window, &mut App) + 'static + Clone,
    {
        let mut offline_container = div()
            .flex()
            .flex_col()
            .items_center()
            .text_center()
            .max_w(px(520.0))
            .w_full()
            .pt(px(24.0));

        // 1) 带播放三角图标的「启动英雄联盟客户端」按钮（launch-btn）
        offline_container = offline_container.child(
            div()
                .id("launch-btn")
                .flex()
                .items_center()
                .gap(px(8.0))
                .px(px(24.0))
                .py(px(10.0))
                .rounded(px(8.0))
                .border(px(1.5))
                .border_color(Theme::ACCENT_PRIMARY())
                .text_size(px(14.1))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(Theme::ACCENT_PRIMARY())
                .cursor_pointer()
                .hover(|s| {
                    s.bg(Theme::ACCENT_HOVER())
                        .text_color(Theme::TEXT_ON_PRIMARY())
                })
                .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                    on_launch(win, app);
                })
                .child(icon("icons/play.svg", 14.0, Theme::ACCENT_PRIMARY()))
                .child("启动英雄联盟客户端"),
        );

        // 启动消息提示（如果有）
        if let Some((msg, is_err)) = &self.launch_message {
            offline_container = offline_container.child(
                div()
                    .mt(px(8.0))
                    .mb(px(12.0))
                    .text_size(px(12.5))
                    .text_color(if *is_err {
                        Theme::DEATH_RED()
                    } else {
                        Theme::ACCENT_PRIMARY()
                    })
                    .child(msg.clone()),
            );
        } else {
            offline_container = offline_container.child(div().mb(px(16.0)));
        }

        // 5) 客户端路径列表（当有 lol_paths 时显示，无路径时隐藏）
        if !self.lol_paths.is_empty() {
            let mut path_list = div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .mb(px(24.0))
                .w_full()
                .max_w(px(460.0));

            path_list = path_list.child(
                div()
                    .text_size(px(12.5))
                    .text_color(Theme::TEXT_MUTED())
                    .mb(px(2.0))
                    .text_center()
                    .child("客户端启动路径:"),
            );

            for (idx, path) in self.lol_paths.iter().enumerate() {
                let is_selected = self.selected_path.as_deref() == Some(path.as_str());

                // 单选 radio 圆圈
                let radio = if is_selected {
                    div()
                        .flex_none()
                        .size(px(14.0))
                        .rounded_full()
                        .border(px(2.0))
                        .border_color(Theme::ACCENT_PRIMARY())
                        .bg(Theme::ACCENT_PRIMARY())
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(div().size(px(4.0)).rounded_full().bg(rgb(0xffffff)))
                } else {
                    div()
                        .flex_none()
                        .size(px(14.0))
                        .rounded_full()
                        .border(px(2.0))
                        .border_color(Theme::CARD_BORDER())
                };

                let text_node: AnyElement = if is_wegame(path) {
                    let badge = div()
                        .flex_none()
                        .px(px(8.0))
                        .py(px(2.0))
                        .rounded(px(4.0))
                        .bg(Theme::ACCENT_PRIMARY())
                        .text_color(rgb(0xffffff))
                        .text_size(px(12.0))
                        .font_weight(FontWeight::BOLD)
                        .child("WeGame");

                    let sub = if !self.wegame_path.is_empty() {
                        div()
                            .text_size(px(12.8))
                            .text_color(Theme::TEXT_PRIMARY())
                            .truncate()
                            .child(self.wegame_path.clone())
                    } else {
                        div()
                            .text_size(px(12.8))
                            .text_color(Theme::TEXT_MUTED())
                            .italic()
                            .child("未设置 WeGame 路径")
                    };

                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .overflow_hidden()
                        .child(badge)
                        .child(sub)
                        .into_any_element()
                } else {
                    div()
                        .text_size(px(12.8))
                        .font_family(mono_font_family())
                        .text_color(if is_selected {
                            Theme::TEXT_PRIMARY()
                        } else {
                            Theme::TEXT_MUTED()
                        })
                        .truncate()
                        .child(path.clone())
                        .into_any_element()
                };

                let on_select = on_select_path.clone();
                let p_str = path.clone();

                path_list = path_list.child(
                    div()
                        .id(ElementId::Name(format!("path-item-{}", idx).into()))
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .px(px(12.0))
                        .py(px(8.0))
                        .rounded(px(6.0))
                        .border_1()
                        .border_color(if is_selected {
                            Theme::ACCENT_PRIMARY()
                        } else {
                            Theme::CARD_BORDER()
                        })
                        .bg(if is_selected {
                            Theme::ACCENT_HOVER()
                        } else {
                            Theme::CARD_BG()
                        })
                        .cursor_pointer()
                        .hover(|s| {
                            if !is_selected {
                                s.border_color(Theme::ACCENT_PRIMARY().opacity(0.3))
                            } else {
                                s
                            }
                        })
                        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                            on_select(p_str.clone(), win, app);
                        })
                        .child(radio)
                        .child(text_node),
                );
            }

            offline_container = offline_container.child(path_list);
        }

        // 6) 底部等待环及文案「等待英雄联盟客户端启动中...」（loading-ring 带旋转环与文字）
        offline_container = offline_container.child(
            div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .px(px(18.0))
                .py(px(10.0))
                .rounded(px(20.0))
                .bg(Theme::over_page(Theme::CARD_BG()))
                .border_1()
                .border_color(Theme::CARD_BORDER())
                .shadow_sm()
                .child(
                    div()
                        .flex_none()
                        .size(px(14.0))
                        .rounded_full()
                        .border(px(2.5))
                        .border_color(Theme::ACCENT_PRIMARY()),
                )
                .child(
                    div()
                        .text_size(px(12.8))
                        .text_color(Theme::TEXT_SECONDARY())
                        .child("等待英雄联盟客户端启动中..."),
                ),
        );

        offline_container
    }
}

/// 详情卡使用等宽字体
fn mono_font_family() -> SharedString {
    "Consolas".into()
}
