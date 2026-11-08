use crate::services::ConnectionStatus;
use crate::theme::Theme;
use crate::views::Route;
use gpui::prelude::*;
use gpui::*;

pub struct HomeView;

impl HomeView {
    pub fn render(
        status: &ConnectionStatus,
        on_navigate: impl Fn(Route, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let is_connected = matches!(status, ConnectionStatus::Connected { .. });
        let nav_search = on_navigate.clone();
        let nav_game = on_navigate.clone();
        let nav_career = on_navigate.clone();
        let nav_tools = on_navigate.clone();

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_4()
            // 欢迎横幅 / LCU 状态总览
            .child(
                div()
                    .w_full()
                    .p_5()
                    .rounded_xl()
                    .bg(Theme::CARD_BG)
                    .border_1()
                    .border_color(Theme::CARD_BORDER)
                    .shadow_sm()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_4()
                            .child(
                                div()
                                    .size(px(56.0))
                                    .rounded_full()
                                    .bg(if is_connected {
                                        Theme::ACCENT_LIGHT
                                    } else {
                                        Theme::BADGE_BG
                                    })
                                    .border_2()
                                    .border_color(if is_connected {
                                        Theme::AURORA_CYAN
                                    } else {
                                        Theme::CARD_BORDER
                                    })
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_2xl()
                                    .child(if is_connected { "🧙‍♂️" } else { "💤" }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_lg()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(Theme::TEXT_PRIMARY)
                                                    .child(match status {
                                                        ConnectionStatus::Connected { summoner, .. } => {
                                                            if let Some(s) = summoner.as_ref() {
                                                                s.full_name()
                                                            } else {
                                                                "召唤师已上线".to_string()
                                                            }
                                                        }
                                                        ConnectionStatus::Connecting => "正在连接英雄联盟客户端...".to_string(),
                                                        ConnectionStatus::Disconnected => "英雄联盟客户端未连接".to_string(),
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .px_2()
                                                    .py(px(2.0))
                                                    .rounded_md()
                                                    .text_xs()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .bg(if is_connected {
                                                        Theme::WIN_BG
                                                    } else {
                                                        Theme::BADGE_BG
                                                    })
                                                    .text_color(if is_connected {
                                                        Theme::WIN_TEXT
                                                    } else {
                                                        Theme::TEXT_MUTED
                                                    })
                                                    .child(if is_connected { "在线" } else { "离线" }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(Theme::TEXT_SECONDARY)
                                            .child(match status {
                                                ConnectionStatus::Connected { info, .. } => {
                                                    format!("LCU 通信正常 · 监听端口: {}", info.port)
                                                }
                                                ConnectionStatus::Connecting => {
                                                    "正在检索 LeagueClientUx 进程与 Lockfile...".to_string()
                                                }
                                                ConnectionStatus::Disconnected => {
                                                    "启动英雄联盟客户端后，Yuumi 将自动完成握手并同步对局数据".to_string()
                                                }
                                            }),
                                    ),
                            ),
                    ),
            )
            // 核心功能卡片网格
            .child(
                div()
                    .w_full()
                    .grid_cols(4)
                    .gap_4()
                    .child(Self::render_quick_entry(
                        "🔍 战绩查询",
                        "查询本区任意玩家详细历史与战绩走势",
                        Theme::AURORA_BLUE,
                        move |_ev, win, app| {
                            nav_search(Route::Search, win, app);
                        },
                    ))
                    .child(Self::render_quick_entry(
                        "⚔️ 对局信息",
                        "当前 10 人排位段位、组队关系与近期状态看板",
                        Theme::AURORA_CYAN,
                        move |_ev, win, app| {
                            nav_game(Route::GameInfo, win, app);
                        },
                    ))
                    .child(Self::render_quick_entry(
                        "📊 个人生涯",
                        "英雄熟练度、战利品开箱与历史胜率深度统计",
                        Theme::ACCENT_PRIMARY,
                        move |_ev, win, app| {
                            nav_career(Route::Career, win, app);
                        },
                    ))
                    .child(Self::render_quick_entry(
                        "🧰 自动流程",
                        "自动接受对局、秒选英雄与大乱斗摇号工具",
                        Theme::PREMADE_COLOR_3,
                        move |_ev, win, app| {
                            nav_tools(Route::Tools, win, app);
                        },
                    )),
            )
            // 系统状态与版本公告卡片
            .child(
                div()
                    .w_full()
                    .p_4()
                    .rounded_xl()
                    .bg(Theme::CARD_BG)
                    .border_1()
                    .border_color(Theme::CARD_BORDER)
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::TEXT_PRIMARY)
                            .child("✨ 极光原生内核 (GPUI + Direct3D 11) 运行中"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::TEXT_SECONDARY)
                            .child("彻底摆脱 WebView2 跨进程 IPC 与内存开销。DirectWrite 亚像素抗锯齿排版，启动毫秒级，帧率稳态 60-120fps。"),
                    ),
            )
    }

    fn render_quick_entry(
        title: &'static str,
        desc: &'static str,
        accent: Hsla,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        div()
            .p_4()
            .rounded_xl()
            .bg(Theme::CARD_BG)
            .border_1()
            .border_color(Theme::CARD_BORDER)
            .shadow_sm()
            .cursor_pointer()
            .hover(|s| s.bg(Theme::CARD_BG_HOVER).border_color(accent))
            .on_mouse_down(MouseButton::Left, on_click)
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(Theme::TEXT_PRIMARY)
                    .child(title),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Theme::TEXT_SECONDARY)
                    .child(desc),
            )
    }
}
