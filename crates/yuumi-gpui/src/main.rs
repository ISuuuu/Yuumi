use gpui::prelude::*;
use gpui::{
    div, hsla, px, Bounds, Context, FontWeight, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Point, Render, Size, StatefulInteractiveElement, Styled, Window, WindowBounds,
    WindowHandle, WindowOptions, Application,
};
use std::path::Path;

mod agent_settings_view;
mod asset_loader;
mod bench_overlay;
mod career_view;
mod components;
mod game_info_view;
mod match_model;
pub mod platform;
mod theme;
mod tray;

use agent_settings_view::AgentSettingsView;
use bench_overlay::BenchOverlayView;
use career_view::{CareerHeaderStats, CareerView};
use components::input::SearchInput;
use components::titlebar::TitleBar;
use game_info_view::{load_mock_game_info, GameInfoData, GameInfoView};
use match_model::{load_real_fixtures, MatchItem};
use theme::Theme;
use tray::TrayManager;

pub const MAIN_WINDOW_TITLE: &str = "Yuumi · 英雄联盟对局助手 (GPUI 高性能原生内核)";
pub const BENCH_WINDOW_TITLE: &str = "Yuumi - 大乱斗板凳席悬浮窗";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabPage {
    GameInfo,
    Career,
    Settings,
}

pub struct MainWindow {
    titlebar: TitleBar,
    search_input: SearchInput,
    matches: Vec<MatchItem>,
    stats: CareerHeaderStats,
    game_info: GameInfoData,
    settings_view: AgentSettingsView,
    current_tab: TabPage,
    _tray: Option<TrayManager>,
    overlay_window: Option<WindowHandle<BenchOverlayView>>,
}

impl MainWindow {
    pub fn new(cx: &mut Context<Self>) -> Self {
        // 尝试从项目 testdata 目录加载 8 份真实 LCU 对局抓包快照
        let fixture_path = Path::new("../../src-tauri/testdata/match");
        let alt_fixture_path = Path::new("src-tauri/testdata/match");
        let matches = if fixture_path.exists() {
            load_real_fixtures(fixture_path)
        } else if alt_fixture_path.exists() {
            load_real_fixtures(alt_fixture_path)
        } else {
            Vec::new()
        };

        let stats = CareerHeaderStats::compute(&matches);
        let game_info = load_mock_game_info();
        let settings_view = AgentSettingsView::new();
        let tray = TrayManager::new().ok();
        let search_input = SearchInput::new("输入召唤师名 / 英雄过滤...");

        // 异步预热战绩卡片中的核心资源（英雄头像等），拉取后触发视图重绘
        let asset_paths: Vec<String> = matches
            .iter()
            .map(|m| m.display.champion_icon_url.clone())
            .filter(|u| !u.is_empty())
            .collect();

        cx.background_executor()
            .spawn(async move {
                let loader = asset_loader::global_asset_loader();
                for path in asset_paths {
                    let _ = loader.load_render_image(&path).await;
                }
            })
            .detach();

        Self {
            titlebar: TitleBar::new(MAIN_WINDOW_TITLE),
            search_input,
            matches,
            stats,
            game_info,
            settings_view,
            current_tab: TabPage::GameInfo,
            _tray: tray,
            overlay_window: None,
        }
    }

    pub fn toggle_card_expanded(&mut self, idx: usize, cx: &mut Context<Self>) {
        if let Some(m) = self.matches.get_mut(idx) {
            m.expanded = !m.expanded;
            cx.notify();
        }
    }

    pub fn clear_search(&mut self, cx: &mut Context<Self>) {
        self.search_input.clear();
        cx.notify();
    }

    pub fn switch_tab(&mut self, tab: TabPage, cx: &mut Context<Self>) {
        self.current_tab = tab;
        cx.notify();
    }

    pub fn toggle_auto_accept(&mut self, cx: &mut Context<Self>) {
        self.settings_view.match_config.auto_accept = !self.settings_view.match_config.auto_accept;
        cx.notify();
    }

    pub fn toggle_auto_reconnect(&mut self, cx: &mut Context<Self>) {
        self.settings_view.match_config.auto_reconnect = !self.settings_view.match_config.auto_reconnect;
        cx.notify();
    }

    pub fn toggle_auto_play_again(&mut self, cx: &mut Context<Self>) {
        self.settings_view.match_config.auto_play_again = !self.settings_view.match_config.auto_play_again;
        cx.notify();
    }

    pub fn toggle_auto_pick(&mut self, cx: &mut Context<Self>) {
        self.settings_view.bp_config.auto_pick = !self.settings_view.bp_config.auto_pick;
        cx.notify();
    }

    pub fn toggle_auto_ban(&mut self, cx: &mut Context<Self>) {
        self.settings_view.bp_config.auto_ban = !self.settings_view.bp_config.auto_ban;
        cx.notify();
    }

    pub fn toggle_auto_spell(&mut self, cx: &mut Context<Self>) {
        self.settings_view.bp_config.auto_spell = !self.settings_view.bp_config.auto_spell;
        cx.notify();
    }

    pub fn toggle_bench_overlay(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(handle) = self.overlay_window.take() {
            let _ = handle.update(cx, |_view, _window, _cx| {
                // 关闭悬浮窗
            });
            cx.notify();
        } else {
            // 打开置顶半透明大乱斗悬浮窗
            let new_handle = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Point {
                            x: px(400.0),
                            y: px(120.0),
                        },
                        size: Size {
                            width: px(380.0),
                            height: px(180.0),
                        },
                    })),
                    titlebar: None,
                    focus: true,
                    is_movable: true,
                    ..Default::default()
                },
                |_window, cx| cx.new(|_cx| BenchOverlayView::new()),
            );

            if let Ok(h) = new_handle {
                self.overlay_window = Some(h);
                // 异步将板凳席小窗设置为 Windows 原生置顶与工具窗口（隐藏任务栏图标）
                cx.background_executor()
                    .spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        if let Some(hwnd) = platform::find_window_by_title(BENCH_WINDOW_TITLE) {
                            platform::set_window_topmost(hwnd, true);
                            platform::set_window_toolwindow(hwnd);
                        }
                    })
                    .detach();
            }
            cx.notify();
        }
    }
}

impl Render for MainWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let matches_count = self.matches.len();
        let stats = &self.stats;
        let is_overlay_open = self.overlay_window.is_some();
        let view_entity = cx.entity().clone();
        let search_view = cx.entity().clone();

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(Theme::BG_APP)
            .font_family(Theme::font_family())
            // 1. 无边框自定义标题栏（接入 Win32 最小化与安全退出）
            .child(self.titlebar.render(
                window,
                |_w, _app| {
                    if let Some(hwnd) = platform::find_window_by_title(MAIN_WINDOW_TITLE) {
                        platform::minimize_window(hwnd);
                    }
                },
                |_w, app| {
                    app.quit();
                },
            ))
            // 2. 顶部快捷操作栏（搜索框、选项卡切换、悬浮窗开关、状态指标）
            .child(
                div()
                    .w_full()
                    .px_4()
                    .py_2()
                    .bg(Theme::CARD_BG)
                    .border_b_1()
                    .border_color(Theme::CARD_BORDER)
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::TEXT_PRIMARY)
                                    .child("GPUI Direct3D 11 硬件加速渲染"),
                            )
                            .child(
                                div()
                                    .px_2()
                                    .py(px(2.0))
                                    .rounded_md()
                                    .bg(Theme::BADGE_BG)
                                    .text_xs()
                                    .text_color(Theme::TEXT_SECONDARY)
                                    .child(format!("已装载 {} 场真实对局样本", matches_count)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            // 选项卡切换：对局信息 (GameInfo) / 个人生涯 (Career) / 自动化设置 (Settings)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .p(px(2.0))
                                    .rounded_lg()
                                    .bg(Theme::BADGE_BG)
                                    .child(
                                        div()
                                            .id("tab-game-info")
                                            .px_3()
                                            .py(px(3.0))
                                            .rounded_md()
                                            .bg(if self.current_tab == TabPage::GameInfo {
                                                Theme::CARD_BG_HOVER
                                            } else {
                                                hsla(0.0, 0.0, 0.0, 0.0)
                                            })
                                            .border_1()
                                            .border_color(if self.current_tab == TabPage::GameInfo {
                                                Theme::CARD_BORDER
                                            } else {
                                                hsla(0.0, 0.0, 0.0, 0.0)
                                            })
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(if self.current_tab == TabPage::GameInfo {
                                                Theme::ACCENT_PRIMARY
                                            } else {
                                                Theme::TEXT_SECONDARY
                                            })
                                            .cursor_pointer()
                                            .on_mouse_down(MouseButton::Left, cx.listener(
                                                |this, _ev, _window, cx| {
                                                    this.switch_tab(TabPage::GameInfo, cx);
                                                },
                                            ))
                                            .child("⚔️ 10人对局看板"),
                                    )
                                    .child(
                                        div()
                                            .id("tab-career")
                                            .px_3()
                                            .py(px(3.0))
                                            .rounded_md()
                                            .bg(if self.current_tab == TabPage::Career {
                                                Theme::CARD_BG_HOVER
                                            } else {
                                                hsla(0.0, 0.0, 0.0, 0.0)
                                            })
                                            .border_1()
                                            .border_color(if self.current_tab == TabPage::Career {
                                                Theme::CARD_BORDER
                                            } else {
                                                hsla(0.0, 0.0, 0.0, 0.0)
                                            })
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(if self.current_tab == TabPage::Career {
                                                Theme::ACCENT_PRIMARY
                                            } else {
                                                Theme::TEXT_SECONDARY
                                            })
                                            .cursor_pointer()
                                            .on_mouse_down(MouseButton::Left, cx.listener(
                                                |this, _ev, _window, cx| {
                                                    this.switch_tab(TabPage::Career, cx);
                                                },
                                            ))
                                            .child("📊 生涯历史"),
                                    )
                                    .child(
                                        div()
                                            .id("tab-settings")
                                            .px_3()
                                            .py(px(3.0))
                                            .rounded_md()
                                            .bg(if self.current_tab == TabPage::Settings {
                                                Theme::CARD_BG_HOVER
                                            } else {
                                                hsla(0.0, 0.0, 0.0, 0.0)
                                            })
                                            .border_1()
                                            .border_color(if self.current_tab == TabPage::Settings {
                                                Theme::CARD_BORDER
                                            } else {
                                                hsla(0.0, 0.0, 0.0, 0.0)
                                            })
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(if self.current_tab == TabPage::Settings {
                                                Theme::ACCENT_PRIMARY
                                            } else {
                                                Theme::TEXT_SECONDARY
                                            })
                                            .cursor_pointer()
                                            .on_mouse_down(MouseButton::Left, cx.listener(
                                                |this, _ev, _window, cx| {
                                                    this.switch_tab(TabPage::Settings, cx);
                                                },
                                            ))
                                            .child("⚙️ 自动流程"),
                                    ),
                            )
                            // 搜索输入组件
                            .child(self.search_input.render(move |_w, app| {
                                search_view.update(app, |this, cx| {
                                    this.clear_search(cx);
                                });
                            }))
                            // 悬浮窗开关
                            .child(
                                div()
                                    .id("btn-toggle-overlay")
                                    .px_3()
                                    .py(px(4.0))
                                    .rounded_md()
                                    .bg(if is_overlay_open {
                                        Theme::ACCENT_LIGHT
                                    } else {
                                        Theme::BADGE_BG
                                    })
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(if is_overlay_open {
                                        Theme::ACCENT_PRIMARY
                                    } else {
                                        Theme::TEXT_PRIMARY
                                    })
                                    .cursor_pointer()
                                    .hover(|s| s.bg(Theme::ACCENT_LIGHT))
                                    .on_mouse_down(MouseButton::Left, cx.listener(
                                        |this, _ev, window, cx| {
                                            this.toggle_bench_overlay(window, cx);
                                        },
                                    ))
                                    .child(if is_overlay_open {
                                        "关闭板凳席悬浮窗"
                                    } else {
                                        "🪟 打开大乱斗板凳席小窗"
                                    }),
                            ),
                    ),
            )
            // 3. 主内容区域：垂直滚动容器
            .child(
                div()
                    .id("main-scroll-view")
                    .flex_1()
                    .overflow_y_scroll()
                    .p_4()
                    .when(self.current_tab == TabPage::GameInfo, |el| {
                        el.child(GameInfoView::render(&self.game_info))
                    })
                    .when(self.current_tab == TabPage::Career, |el| {
                        el.child(CareerView::render_header(stats))
                            .children(self.matches.iter().enumerate().map(|(idx, m)| {
                                let view = view_entity.clone();
                                CareerView::render_match_card(
                                    m,
                                    idx,
                                    move |clicked_idx, _window, app| {
                                        view.update(app, |this, cx| {
                                            this.toggle_card_expanded(clicked_idx, cx);
                                        });
                                    },
                                )
                            }))
                    })
                    .when(self.current_tab == TabPage::Settings, |el| {
                        let view = view_entity.clone();
                        let p_accept = {
                            let v = view.clone();
                            move |_ev: &_, _win: &mut _, app: &mut _| {
                                v.update(app, |this, cx| this.toggle_auto_accept(cx));
                            }
                        };
                        let p_reconn = {
                            let v = view.clone();
                            move |_ev: &_, _win: &mut _, app: &mut _| {
                                v.update(app, |this, cx| this.toggle_auto_reconnect(cx));
                            }
                        };
                        let p_again = {
                            let v = view.clone();
                            move |_ev: &_, _win: &mut _, app: &mut _| {
                                v.update(app, |this, cx| this.toggle_auto_play_again(cx));
                            }
                        };
                        let p_pick = {
                            let v = view.clone();
                            move |_ev: &_, _win: &mut _, app: &mut _| {
                                v.update(app, |this, cx| this.toggle_auto_pick(cx));
                            }
                        };
                        let p_ban = {
                            let v = view.clone();
                            move |_ev: &_, _win: &mut _, app: &mut _| {
                                v.update(app, |this, cx| this.toggle_auto_ban(cx));
                            }
                        };
                        let p_spell = {
                            let v = view.clone();
                            move |_ev: &_, _win: &mut _, app: &mut _| {
                                v.update(app, |this, cx| this.toggle_auto_spell(cx));
                            }
                        };
                        el.child(self.settings_view.render_panel(
                            p_accept, p_reconn, p_again, p_pick, p_ban, p_spell,
                        ))
                    }),
            )
    }
}

fn main() {
    // 1. Windows 单实例互斥量检测（Named Mutex）
    let mutex_name = "Global\\Yuumi_GPUI_SingleInstance_Mutex";
    let _instance_guard = match platform::acquire_single_instance(mutex_name) {
        Ok(guard) => guard,
        Err(platform::SingleInstanceError::AlreadyRunning) => {
            eprintln!("检测到 Yuumi 已在运行中，激活现有窗口...");
            if let Some(hwnd) = platform::find_window_by_title(MAIN_WINDOW_TITLE) {
                platform::restore_and_foreground(hwnd);
            }
            return;
        }
        Err(e) => {
            eprintln!("互斥量检测失败: {}", e);
            return;
        }
    };

    let app = Application::new();

    app.run(move |cx| {
        let window_options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: Point {
                    x: px(150.0),
                    y: px(80.0),
                },
                size: Size {
                    width: px(1120.0),
                    height: px(780.0),
                },
            })),
            titlebar: None,
            focus: true,
            is_movable: true,
            ..Default::default()
        };

        let window_res = cx.open_window(window_options, |_window, cx| {
            cx.new(MainWindow::new)
        });

        if let Err(e) = window_res {
            eprintln!("打开 GPUI 窗口失败: {:?}", e);
        }
    });
}
