//! 首页（对照 YuumiX WinUI 版 `Pages/Home/HomePage.cs` 复刻）
//!
//! - H01: 已连接状态呈现统一页头（「主页」标题 + 「客户端连接成功」绿色徽章 + 副标题）、
//!   召唤师概览卡（头像 + 等级徽章 / 昵称#Tag / 段位行 / 「查看生涯」按钮）、
//!   游戏阶段卡（阶段图标 + 阶段名 + 「进行中」徽章 + 描述文案 / PID+Port / 「● LCU 在线」）、
//!   快捷入口 3 列网格（生涯 / 战绩查询 / 对局信息 / OP.GG / 其他功能），
//!   右下角绝对定位猫咪 Logo (120x120px) 带 hover 效果。
//! - H02: 未连接离线状态居中纵向排布：Logo 图 (96x96)、说明、带播放三角图标的「启动英雄联盟客户端」按钮、客户端路径列表（有路径才显示）、底部等待环与「等待英雄联盟客户端启动中...」。
//! - H03: 路径列表上方显示「客户端启动路径:」，单选 radio 与高亮边框；WeGame 标记（WEGAME_MARKER / "WEGAME"）默认选中，展示彩色 WeGame 徽章，次文案显示配置路径或斜体「未设置 WeGame 路径」；点击切换选中路径，点击启动调用 launch_lol_client。
//! - H04: 连接状态切换时立即响应，断开切为离线未连接状态，就绪立即展示真实数据。
//!   召唤师与段位数据由 MainWindow 经事件（ConnectionChanged / RankedStatsLoaded）被动注入，
//!   「客户端未登录完成」时召唤师卡显示加载占位，登录完成后由 live_lcu 的轮询兜底或
//!   LCU WS 推送自动补齐（先开软件后开客户端 / 先开客户端后开软件两种顺序均可用）。

use super::Route;
use crate::asset_loader::AssetLoader;
use crate::assets::logo_image;
use crate::components::icon::icon;
use crate::components::lcu_image::LcuImage;
use crate::components::offline_state::render_lcu_offline_state_with_tip;
use crate::components::titlebar::phase_label;
use crate::services::system_tools::{load_client_paths, WEGAME_MARKER};
use crate::services::ConnectionStatus;
use crate::theme::Theme;
use gpui::prelude::*;
use gpui::*;
use yuumi_core::parsers::game_info::{rank_tier_label, RankedQueueEntry, RankedStats};
use yuumi_core::parsers::summoner::SummonerInfo;

pub fn is_wegame(path: &str) -> bool {
    path == WEGAME_MARKER
        || path.eq_ignore_ascii_case("wegame")
        || path.to_ascii_lowercase().ends_with("wegame.exe")
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
    /// 辅助方法：计算默认选中路径（若有 WeGame 优先选中 WeGame，否则选中首项）
    fn pick_default_selected(paths: &[String]) -> Option<String> {
        if let Some(wegame) = paths.iter().find(|p| is_wegame(p)) {
            Some(wegame.clone())
        } else {
            paths.first().cloned()
        }
    }

    pub fn new() -> Self {
        // [P0-6] 构造期不做路径探测：主窗构造后紧接 reload_paths_from(配置) 会整体覆盖，
        // 首次启动（配置尚无 LolPath）时这里的探测结果必被丢弃，却要在 UI 线程付出数秒
        // （find_wegame_exe 会枚举注册表卸载项并逐个扫盘）。
        Self {
            lol_paths: Vec::new(),
            wegame_path: String::new(),
            selected_path: None,
            launch_message: None,
        }
    }

    /// 根据传入的客户端路径与 WeGame 路径更新，并确保优先默认选中 WeGame
    pub fn reload_paths_from(&mut self, paths: Vec<String>, wegame: String) {
        let selected = if paths.iter().any(|p| is_wegame(p))
            && !self
                .selected_path
                .as_deref()
                .map(is_wegame)
                .unwrap_or(false)
        {
            paths.iter().find(|p| is_wegame(p)).cloned()
        } else if let Some(ref cur) = self.selected_path {
            if paths.contains(cur) {
                Some(cur.clone())
            } else {
                Self::pick_default_selected(&paths)
            }
        } else {
            Self::pick_default_selected(&paths)
        };

        self.selected_path = selected;
        self.lol_paths = paths;
        self.wegame_path = wegame;
    }

    /// 重新载入客户端路径（从磁盘兜底重读）
    pub fn reload_paths(&mut self) {
        let (paths, wegame) = load_client_paths();
        self.reload_paths_from(paths, wegame);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render<F1, F2, F3, F4>(
        &self,
        status: &ConnectionStatus,
        summoner: Option<&SummonerInfo>,
        ranked: Option<&RankedStats>,
        phase: &str,
        on_select_path: F1,
        on_launch: F2,
        on_navigate: F3,
        on_open_opgg: F4,
    ) -> impl IntoElement
    where
        F1: Fn(String, &mut Window, &mut App) + 'static + Clone,
        F2: Fn(&mut Window, &mut App) + 'static + Clone,
        F3: Fn(Route, &mut Window, &mut App) + 'static + Clone,
        F4: Fn(&mut Window, &mut App) + 'static + Clone,
    {
        if matches!(status, ConnectionStatus::Connected { .. }) {
            div()
                .relative()
                .size_full()
                .child(
                    div()
                        .id("home-scroll-view")
                        .size_full()
                        .overflow_y_scroll()
                        .child(
                            div()
                                .px(px(32.0))
                                .py(px(24.0))
                                .pb(px(32.0))
                                .flex()
                                .flex_col()
                                .gap(px(20.0))
                                .child(self.render_page_header())
                                .child(self.render_summoner_card(
                                    summoner,
                                    ranked,
                                    on_navigate.clone(),
                                ))
                                .child(self.render_phase_card(status, phase))
                                .child(self.render_quick_access(on_navigate, on_open_opgg)),
                        ),
                )
                .child(
                    // 右下角猫咪 Logo（bottom: 40px, right: 40px, 120x120px，悬停/激活效果）
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
                Some(
                    self.render_offline_actions(on_select_path, on_launch)
                        .into_any_element(),
                ),
            )
        }
    }

    /// H01 页头：标题 + 连接成功徽章 + 副标题
    fn render_page_header(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(28.0))
                            .font_weight(FontWeight::EXTRA_BOLD)
                            .text_color(Theme::TEXT_PRIMARY())
                            .child("主页"),
                    )
                    .child(
                        pill_base(Theme::WIN_TEXT(), Theme::over_card(Theme::BADGE_BG()))
                            .child("客户端连接成功"),
                    ),
            )
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(Theme::TEXT_SECONDARY())
                    .child("查看当前召唤师状态、游戏阶段与常用功能快捷直达"),
            )
    }

    /// H01 召唤师概览卡：头像 + 等级徽章 / 昵称#Tag / 段位行 / 「查看生涯」按钮
    fn render_summoner_card<F3>(
        &self,
        summoner: Option<&SummonerInfo>,
        ranked: Option<&RankedStats>,
        on_navigate: F3,
    ) -> impl IntoElement
    where
        F3: Fn(Route, &mut Window, &mut App) + 'static + Clone,
    {
        let card = home_card().p(px(20.0)).flex().items_center();

        let Some(s) = summoner else {
            // 客户端已连接但尚未登录完成（先开软件后登录）：显示加载占位
            return card
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(16.0))
                        .py(px(8.0))
                        .child(
                            div()
                                .flex_none()
                                .size(px(32.0))
                                .rounded_full()
                                .border(px(2.5))
                                .border_color(Theme::ACCENT_PRIMARY()),
                        )
                        .child(
                            div()
                                .text_size(px(14.0))
                                .text_color(Theme::TEXT_SECONDARY())
                                .child("正在获取召唤师信息..."),
                        ),
                )
                .into_any_element();
        };

        // 头像栈：64px 圆形头像 + 下方 Lv 等级胶囊
        let avatar_stack = div()
            .flex_none()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(4.0))
            .mr(px(20.0))
            .child(
                LcuImage::new(s.icon_path())
                    .size(px(64.0))
                    .circle()
                    .border_color(Theme::CARD_BORDER())
                    .placeholder("·"),
            )
            .when_some(s.summoner_level, |el, level| {
                el.child(
                    pill_base(Theme::TEXT_PRIMARY(), Theme::over_card(Theme::CARD_BG()))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("Lv.{}", level)),
                )
            });

        // 名字行 + 段位行
        let name = s
            .game_name
            .as_deref()
            .or(s.display_name.as_deref())
            .unwrap_or("未知召唤师");
        let info_col = div()
            .flex_1()
            .min_w(px(0.0))
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .overflow_hidden()
                    .child(
                        div()
                            .text_size(px(20.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(Theme::TEXT_PRIMARY())
                            .truncate()
                            .child(name.to_string()),
                    )
                    .children(s.tag_line.as_deref().map(|tag| {
                        div()
                            .flex_none()
                            .text_size(px(14.0))
                            .text_color(Theme::TEXT_SECONDARY())
                            .child(format!("#{}", tag))
                    })),
            )
            .child(render_ranked_row(ranked));

        let view_career = div()
            .id("home-view-career")
            .flex_none()
            .flex()
            .items_center()
            .gap(px(4.0))
            .px(px(16.0))
            .py(px(8.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(Theme::CARD_BORDER())
            .bg(Theme::CARD_BG())
            .cursor_pointer()
            .text_size(px(13.0))
            .text_color(Theme::TEXT_PRIMARY())
            .hover(|s| {
                s.bg(Theme::CARD_BG_HOVER())
                    .border_color(Theme::ACCENT_PRIMARY())
            })
            .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
                on_navigate(Route::Career, win, app);
            })
            .child("查看生涯")
            .child(icon(
                "icons/chevron-right.svg",
                12.0,
                Theme::TEXT_SECONDARY(),
            ));

        card.child(avatar_stack)
            .child(info_col)
            .child(view_career)
            .into_any_element()
    }

    /// H01 游戏阶段卡：阶段图标盒 + 阶段名 + 「进行中」徽章 + 描述 / PID + Port + 「● LCU 在线」
    fn render_phase_card(&self, status: &ConnectionStatus, phase: &str) -> impl IntoElement {
        let (pid, port) = match status {
            ConnectionStatus::Connected { info, .. } => (info.pid, info.port),
            _ => (0, 0),
        };

        home_card()
            .p(px(20.0))
            .flex()
            .items_center()
            .gap(px(16.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(16.0))
                    .flex_1()
                    .min_w(px(0.0))
                    .child(
                        div()
                            .flex_none()
                            .size(px(44.0))
                            .rounded(Theme::RADIUS_MD)
                            .bg(Theme::over_card(Theme::ITEM_SLOT_BG()))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icon(phase_icon(phase), 20.0, Theme::ACCENT_PRIMARY())),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .min_w(px(0.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .text_size(px(20.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(Theme::TEXT_PRIMARY())
                                            .truncate()
                                            .child(format!(
                                                "游戏阶段：{}",
                                                home_phase_title(phase)
                                            )),
                                    )
                                    .child(
                                        pill_base(
                                            Theme::ACCENT_PRIMARY(),
                                            Theme::over_card(Theme::ACCENT_ALPHA_10()),
                                        )
                                        .child("进行中"),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .text_color(Theme::TEXT_SECONDARY())
                                    .truncate()
                                    .child(phase_description(phase)),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .items_end()
                    .gap(px(6.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.0))
                            .font_family(mono_font_family())
                            .text_size(px(12.0))
                            .text_color(Theme::TEXT_SECONDARY())
                            .child(format!("PID: {}", pid))
                            .child(format!("Port: {}", port)),
                    )
                    .child(
                        pill_base(Theme::TEXT_PRIMARY(), Theme::over_card(Theme::BADGE_BG()))
                            .child(div().text_color(Theme::WIN_TEXT()).child("●"))
                            .child("LCU 在线"),
                    ),
            )
    }

    /// H01 快捷入口：3 列网格（第二行 2 张磁贴 + 1 个空占位保持列宽一致）
    fn render_quick_access<F3, F4>(&self, on_navigate: F3, on_open_opgg: F4) -> impl IntoElement
    where
        F3: Fn(Route, &mut Window, &mut App) + 'static + Clone,
        F4: Fn(&mut Window, &mut App) + 'static + Clone,
    {
        let nav_career = on_navigate.clone();
        let nav_search = on_navigate.clone();
        let nav_game_info = on_navigate.clone();
        let nav_tools = on_navigate.clone();
        div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .child(
                div()
                    .text_size(px(20.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(Theme::TEXT_PRIMARY())
                    .child("快捷入口"),
            )
            .child(
                div()
                    .flex()
                    .gap(px(12.0))
                    .child(quick_tile(
                        "quick-career",
                        "icons/user.svg",
                        "生涯",
                        "查看历史战绩、战利品与熟练度",
                        move |win, app| nav_career(Route::Career, win, app),
                    ))
                    .child(quick_tile(
                        "quick-search",
                        "icons/search.svg",
                        "战绩查询",
                        "搜索召唤师并分析对局详情",
                        move |win, app| nav_search(Route::Search, win, app),
                    ))
                    .child(quick_tile(
                        "quick-game-info",
                        "icons/gamepad.svg",
                        "对局信息",
                        "选人与对局中实时看板",
                        move |win, app| nav_game_info(Route::GameInfo, win, app),
                    )),
            )
            .child(
                div()
                    .flex()
                    .gap(px(12.0))
                    .child(quick_tile(
                        "quick-opgg",
                        "icons/opgg.svg",
                        "OP.GG",
                        "英雄出装、符文与克制排行",
                        move |win, app| on_open_opgg(win, app),
                    ))
                    .child(quick_tile(
                        "quick-tools",
                        "icons/wrench.svg",
                        "其他功能",
                        "自动接受、自动选人与实用工具",
                        move |win, app| nav_tools(Route::Tools, win, app),
                    ))
                    // 第三列空占位：保持磁贴宽度与第一行一致（对齐 YuumiX 网格布局）
                    .child(div().flex_1()),
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
                .hover(|s| s.bg(Theme::ACCENT_LIGHT()))
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
                        .bg(Theme::CARD_BG())
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

/// 卡片基座：实色卡底 + 1px 描边 + 大圆角 + 轻阴影（与生涯页头部卡一致）
fn home_card() -> Div {
    div()
        .w_full()
        .bg(Theme::over_page(Theme::CARD_BG()))
        .border_1()
        .border_color(Theme::CARD_BORDER())
        .rounded_xl()
        .shadow_sm()
}

/// 胶囊徽章基座（对齐 YuumiX Badge：1px 描边 + 全圆角 + 浅底色，内容由调用方填充）
fn pill_base(text_color: Hsla, bg: Hsla) -> Div {
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(4.0))
        .px(px(8.0))
        .py(px(2.0))
        .rounded_full()
        .bg(bg)
        .border_1()
        .border_color(Theme::CARD_BORDER())
        .text_size(px(12.0))
        .text_color(text_color)
}

/// 段位行：段位徽标 + 段位名 + LP + 胜率；未定级时显示「单双排位：未定级」
///
/// 与生涯页「单双排位」行同源同语义（严格取 `RANKED_SOLO_5x5`，不做同族/任意队列兜底）：
/// 国服「召唤师之旅」JADE 队列的 WOOD 等是进度体系而非段位，不得混入展示。
fn render_ranked_row(ranked: Option<&RankedStats>) -> AnyElement {
    let solo = ranked.and_then(|stats| stats.queue_map.get("RANKED_SOLO_5x5"));

    match home_solo_entry(ranked) {
        Some(entry) => div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(rank_medal(&entry.tier))
            .child(
                div()
                    .text_size(px(14.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(Theme::TEXT_PRIMARY())
                    .child(rank_tier_label(&entry.tier, &entry.division)),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(Theme::TEXT_SECONDARY())
                    .child(format!("{} LP", entry.league_points)),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(Theme::TEXT_SECONDARY())
                    .child(format!(
                        "胜率 {:.0}% ({}胜 {}负)",
                        entry.win_rate(),
                        entry.wins,
                        entry.losses
                    )),
            )
            .into_any_element(),
        None => {
            // 定级赛未完成（tier 为空且 is_provisional）按生涯页语义显示「定级中」
            let text = if solo.is_some_and(|entry| entry.is_provisional) {
                "单双排位：定级中"
            } else {
                "单双排位：未定级"
            };
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(rank_medal(""))
                .child(
                    div()
                        .text_size(px(14.0))
                        .text_color(Theme::TEXT_SECONDARY())
                        .child(text),
                )
                .into_any_element()
        }
    }
}

/// 主页段位数据源：生涯页「单双排位」行的已定级条目（严格单双排队列，未定级返回 None）
fn home_solo_entry(ranked: Option<&RankedStats>) -> Option<&RankedQueueEntry> {
    ranked?
        .queue_map
        .get("RANKED_SOLO_5x5")
        .filter(|entry| entry.is_placed())
}

/// 段位徽标：已知段位走内置段位图（无需预热），未知段位显示灰色圆形占位
fn rank_medal(tier: &str) -> AnyElement {
    match AssetLoader::get_ranked_icon_path(tier) {
        Some(path) => LcuImage::new(path)
            .size(px(22.0))
            .border_color(Theme::CARD_BORDER())
            .into_any_element(),
        None => div()
            .flex_none()
            .size(px(22.0))
            .rounded_full()
            .bg(Theme::over_card(Theme::ITEM_SLOT_BG()))
            .border_1()
            .border_color(Theme::CARD_BORDER())
            .into_any_element(),
    }
}

/// 阶段显示名（复用标题栏 i18n 映射，空闲态单独兜底）
fn home_phase_title(phase: &str) -> &'static str {
    match phase {
        "None" | "Unknown" => "空闲",
        _ => phase_label(phase).unwrap_or("未知"),
    }
}

/// 阶段描述文案（对齐 YuumiX HomePage 游戏阶段卡）
fn phase_description(phase: &str) -> &'static str {
    match phase {
        "None" | "Unknown" => "当前客户端处于空闲等待状态",
        "Lobby" => "已进入游戏房间，可准备对局或组队匹配",
        "Matchmaking" => "正在寻找对局，找到后将自动弹出确认",
        "ReadyCheck" => "对局已就绪，请在规定时间内完成确认",
        "ChampSelect" => "英雄选择阶段，请留意预选阵容与出装符文",
        "GameStart" | "InProgress" => "游戏对局正在进行中",
        "Reconnect" => "游戏意外中断，请尝试重新连接游戏服务器",
        "PreEndOfGame" | "WaitingForStats" | "EndOfGame" => "本场对局已结束，正在同步结算统计数据",
        _ => "客户端运行中",
    }
}

/// 阶段图标：对局中→对勾，选人→手柄，重连→警告，其余→主页
fn phase_icon(phase: &str) -> &'static str {
    match phase {
        "GameStart" | "InProgress" => "icons/checkmark.svg",
        "ChampSelect" => "icons/gamepad.svg",
        "Reconnect" => "icons/warning.svg",
        _ => "icons/home.svg",
    }
}

/// 详情卡使用等宽字体
fn mono_font_family() -> SharedString {
    "Consolas".into()
}

/// 快捷入口磁贴（对齐 YuumiX：图标 / 标题 / 描述纵向排布，横向拉伸铺满格子）
fn quick_tile<F>(
    id: &'static str,
    icon_path: &'static str,
    title: &'static str,
    desc: &'static str,
    on_click: F,
) -> impl IntoElement
where
    F: Fn(&mut Window, &mut App) + 'static,
{
    div()
        .id(ElementId::Name(id.into()))
        .flex_1()
        .min_w(px(0.0))
        .flex()
        .flex_col()
        .items_start()
        .gap(px(6.0))
        .px(px(16.0))
        .py(px(12.0))
        .rounded_xl()
        .bg(Theme::over_page(Theme::CARD_BG()))
        .border_1()
        .border_color(Theme::CARD_BORDER())
        .shadow_sm()
        .cursor_pointer()
        .text_color(Theme::TEXT_PRIMARY())
        .hover(|s| {
            s.bg(Theme::over_page(Theme::CARD_BG_HOVER()))
                .border_color(Theme::ACCENT_PRIMARY().opacity(0.3))
                .shadow_md()
        })
        .on_mouse_down(MouseButton::Left, move |_ev, win, app| {
            on_click(win, app);
        })
        .child(icon(icon_path, 18.0, Theme::ACCENT_PRIMARY()))
        .child(
            div()
                .text_size(px(14.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(Theme::TEXT_PRIMARY())
                .child(title),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(Theme::TEXT_SECONDARY())
                .child(desc),
        )
}

#[cfg(test)]
mod tests {
    use super::{home_solo_entry, is_wegame, phase_description, HomeView};
    use yuumi_core::parsers::game_info::{rank_tier_label, RankedQueueEntry, RankedStats};

    #[test]
    fn test_is_wegame_matches_marker_and_exe() {
        assert!(is_wegame("WeGame"));
        assert!(is_wegame("wegame"));
        assert!(is_wegame("WEGAME"));
        assert!(is_wegame(r"D:\Program Files\Tencent\WeGame\wegame.exe"));
        assert!(!is_wegame(r"D:\WeGameApps\League of Legends\TCLS"));
    }

    #[test]
    fn test_pick_default_selected_prefers_wegame() {
        let paths = vec![
            r"D:\WeGameApps\英雄联盟（含经典模式）\TCLS".to_string(),
            "WeGame".to_string(),
        ];
        let selected = HomeView::pick_default_selected(&paths);
        assert_eq!(selected.as_deref(), Some("WeGame"));

        let paths_without_wegame = vec![
            r"D:\WeGameApps\英雄联盟\TCLS".to_string(),
            r"E:\Riot Games\League of Legends".to_string(),
        ];
        let selected_fallback = HomeView::pick_default_selected(&paths_without_wegame);
        assert_eq!(
            selected_fallback.as_deref(),
            Some(r"D:\WeGameApps\英雄联盟\TCLS")
        );
    }

    #[test]
    fn test_reload_paths_from_prefers_wegame() {
        let mut home = HomeView {
            lol_paths: vec![r"D:\Games\LOL".to_string()],
            wegame_path: "".to_string(),
            selected_path: Some(r"D:\Games\LOL".to_string()),
            launch_message: None,
        };

        // 当新增了 WeGame 时，应自动优先选中 WeGame
        home.reload_paths_from(
            vec![r"D:\Games\LOL".to_string(), "WeGame".to_string()],
            r"D:\WeGame".to_string(),
        );
        assert_eq!(home.selected_path.as_deref(), Some("WeGame"));
    }

    #[test]
    fn test_phase_description_mapping() {
        assert_eq!(
            phase_description("Lobby"),
            "已进入游戏房间，可准备对局或组队匹配"
        );
        assert_eq!(phase_description("None"), "当前客户端处于空闲等待状态");
        assert_eq!(phase_description("Unknown"), "当前客户端处于空闲等待状态");
        assert_eq!(phase_description("InProgress"), "游戏对局正在进行中");
        assert_eq!(
            phase_description("EndOfGame"),
            "本场对局已结束，正在同步结算统计数据"
        );
        assert_eq!(phase_description("WatchInProgress"), "客户端运行中");
    }

    #[test]
    fn test_home_solo_entry_follows_career_solo_queue() {
        // 国服「召唤师之旅」JADE 队列的 WOOD 不是段位，不得兜底混入
        let mut stats = RankedStats::default();
        stats.queue_map.insert(
            "JADE_RANKED_SOLO_5x5".to_string(),
            RankedQueueEntry {
                tier: "WOOD".to_string(),
                division: "III".to_string(),
                league_points: 20,
                wins: 1,
                ..Default::default()
            },
        );
        assert!(home_solo_entry(Some(&stats)).is_none());

        // 严格取单双排位：已定级时显示中文段位（与生涯页 rank_tier_label 一致）
        stats.queue_map.insert(
            "RANKED_SOLO_5x5".to_string(),
            RankedQueueEntry {
                tier: "GOLD".to_string(),
                division: "I".to_string(),
                league_points: 45,
                wins: 9,
                losses: 3,
                ..Default::default()
            },
        );
        let entry = home_solo_entry(Some(&stats)).expect("单双排位已定级应返回条目");
        assert_eq!(rank_tier_label(&entry.tier, &entry.division), "黄金 I");

        // 单双排位定级赛中（tier 为空）不算已定级
        stats.queue_map.insert(
            "RANKED_SOLO_5x5".to_string(),
            RankedQueueEntry {
                is_provisional: true,
                ..Default::default()
            },
        );
        assert!(home_solo_entry(Some(&stats)).is_none());

        // 无排位数据
        assert!(home_solo_entry(None).is_none());
        assert!(home_solo_entry(Some(&RankedStats::default())).is_none());
    }
}
