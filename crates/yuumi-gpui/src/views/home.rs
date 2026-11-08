//! 首页（对照 Vue `src/views/Home.vue` 与 `File/pic/首页-已连接.png`）
//!
//! 构图要点：内容区唯一的视觉中心——居中的连接标题 + 最大宽 500 的浅透详情卡，
//! 右下角固定 120 的猫图；离线态沿用旧版连接提示与等待提示，不与加载态互相替代。

use crate::assets::logo_image;
use crate::services::ConnectionStatus;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;

pub struct HomeView;

impl HomeView {
    pub fn render(status: &ConnectionStatus) -> impl IntoElement {
        div()
            .relative()
            .size_full()
            // 旧版首页内边距：2.5rem / 1.5rem
            .px(px(24.0))
            .py(px(40.0))
            .flex()
            .items_center()
            .justify_center()
            .when(matches!(status, ConnectionStatus::Connected { .. }), |el| {
                el.child(Self::render_connected(status)).child(
                    // 连接成功后猫咪移到窗体右下角（距右 / 距底各 40）
                    div().absolute().bottom(px(40.0)).right(px(40.0)).when_some(
                        logo_image(),
                        |el, logo| {
                            el.child(
                                img(logo)
                                    .w(px(120.0))
                                    .h(px(120.0))
                                    .object_fit(ObjectFit::Contain)
                                    .opacity(0.85),
                            )
                        },
                    ),
                )
            })
            .when(
                !matches!(status, ConnectionStatus::Connected { .. }),
                |el| el.child(Self::render_offline(status)),
            )
    }

    fn render_connected(status: &ConnectionStatus) -> impl IntoElement {
        let (pid, port) = match status {
            ConnectionStatus::Connected { info, .. } => {
                (info.pid.to_string(), info.port.to_string())
            }
            _ => ("-".to_string(), "-".to_string()),
        };

        let mut rows = div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(10.0))
            .text_size(px(13.1))
            .font_family(mono_font_family())
            .text_color(Theme::TEXT_MUTED);

        for (label, value) in [("PID =", pid), ("--app-port =", port)] {
            rows = rows.child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(8.0))
                    .whitespace_nowrap()
                    .child(div().text_color(Theme::TEXT_MUTED).child(label))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(Theme::TEXT_PRIMARY)
                            .child(value),
                    ),
            );
        }

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
                    .text_color(Theme::TEXT_PRIMARY)
                    .mb(px(20.0))
                    .child("客户端连接成功 🎉"),
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
                    .bg(Theme::CARD_BG)
                    .border_1()
                    .border_color(Theme::CARD_BORDER)
                    .shadow_sm()
                    .child(rows),
            )
    }

    fn render_offline(status: &ConnectionStatus) -> impl IntoElement {
        let (title, desc) = match status {
            ConnectionStatus::Connecting => (
                "正在连接英雄联盟客户端…",
                "正在检索 LeagueClientUx 进程与 Lockfile",
            ),
            _ => ("客户端未连接", "请先启动国服或外服的《英雄联盟》客户端。"),
        };

        div()
            .flex()
            .flex_col()
            .items_center()
            .text_center()
            .max_w(px(500.0))
            .when_some(logo_image(), |el, logo| {
                el.child(
                    div().mb(px(24.0)).child(
                        img(logo)
                            .w(px(96.0))
                            .h(px(96.0))
                            .object_fit(ObjectFit::Contain),
                    ),
                )
            })
            .child(
                div()
                    .text_size(px(28.8))
                    .font_weight(FontWeight::EXTRA_BOLD)
                    .text_color(Theme::TEXT_PRIMARY)
                    .mb(px(20.0))
                    .child(title),
            )
            .child(
                div()
                    .text_size(px(14.1))
                    .line_height(px(22.0))
                    .text_color(Theme::TEXT_SECONDARY)
                    .mb(px(32.0))
                    .child(desc),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .px(px(18.0))
                    .py(px(10.0))
                    .rounded(px(20.0))
                    .bg(Theme::CARD_BG)
                    .border_1()
                    .border_color(Theme::CARD_BORDER)
                    .shadow_sm()
                    .child(
                        // 等待环：静态双环近似原版旋转指示器
                        div()
                            .flex_none()
                            .size(px(14.0))
                            .rounded_full()
                            .border(px(2.5))
                            .border_color(Theme::ACCENT_PRIMARY),
                    )
                    .child(
                        div()
                            .text_size(px(12.8))
                            .text_color(Theme::TEXT_SECONDARY)
                            .child("正在等待 LeagueClient 启动..."),
                    ),
            )
    }
}

/// 详情卡使用等宽字体
fn mono_font_family() -> SharedString {
    "Consolas".into()
}
