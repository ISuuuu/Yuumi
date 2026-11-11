//! 导航侧栏（1:1 对照 Vue `src/components/layout/NavigationSidebar.vue`）
//!
//! 复刻要点：
//! - 折叠 64px / 展开 210px，顶部 52px 汉堡按钮平滑切换；
//! - 主导航：首页、生涯、战绩查询、对局信息、云顶之弈(可选)、路人集(可选)、工具箱；
//! - 底部动作区（5 大入口）：OP.GG、修复重连、公告通知、召唤师名片(登录态)、设置(带更新小红点)；
//! - 选中态青透底 + 4px 内侧指示条，悬停与折叠对齐 Vue。

use crate::components::icon::{icon, opgg_logo};
use crate::components::lcu_image::LcuImage;
use crate::theme::Theme;
use crate::views::Route;
use gpui::prelude::FluentBuilder;
use gpui::*;
use yuumi_core::config::AppConfig;
use yuumi_core::parsers::summoner::SummonerInfo;

pub struct Sidebar;

impl Sidebar {
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        current_route: Route,
        expanded: bool,
        config: &AppConfig,
        summoner: Option<&SummonerInfo>,
        region_name: &str,
        has_update: bool,
        on_select: impl Fn(Route, &mut Window, &mut App) + 'static + Clone,
        on_toggle: impl Fn(&mut Window, &mut App) + 'static + Clone,
        on_open_opgg: impl Fn(&mut Window, &mut App) + 'static + Clone,
        on_reconnect: impl Fn(&mut Window, &mut App) + 'static + Clone,
        on_open_notice: impl Fn(&mut Window, &mut App) + 'static + Clone,
        on_select_self: impl Fn(&mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let mut main_routes = vec![
            Route::Home,
            Route::Career,
            Route::Search,
            Route::GameInfo,
        ];
        if !config.functions.hide_tft {
            main_routes.push(Route::Tft);
        }
        if !config.functions.hide_saved_players {
            main_routes.push(Route::SavedPlayers);
        }
        main_routes.push(Route::Tools);

        let width = if expanded {
            Theme::SIDEBAR_WIDTH_EXPANDED
        } else {
            Theme::SIDEBAR_WIDTH
        };

        let toggle_cb = on_toggle;

        div()
            .id("app-sidebar")
            .flex()
            .flex_col()
            .h_full()
            .w(width)
            .flex_shrink_0()
            .overflow_hidden()
            .bg(Theme::SIDEBAR_BG())
            .border_r_1()
            .border_color(Theme::CARD_BORDER())
            // 顶部：汉堡折叠按钮
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(Theme::SIDEBAR_HEADER_HEIGHT)
                    .px(px(10.0))
                    .gap(px(4.0))
                    .child(
                        div()
                            .id("sidebar-toggle")
                            .flex()
                            .flex_none()
                            .items_center()
                            .justify_center()
                            .w(px(36.0))
                            .h(px(36.0))
                            .rounded(px(8.0))
                            .text_color(Theme::TEXT_MUTED())
                            .hover(|s| s.bg(Theme::HOVER_BG()).text_color(Theme::TEXT_PRIMARY()))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                                toggle_cb(window, app);
                            })
                            .child(icon("icons/menu.svg", 20.0, Theme::TEXT_MUTED())),
                    ),
            )
            // 中部：主功能导航 (可根据配置隐藏 TFT / 路人集)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .p(px(8.0))
                    .flex_1()
                    .overflow_hidden()
                    .children(main_routes.into_iter().map(|route| {
                        Self::render_item(route, current_route, expanded, false, on_select.clone())
                    })),
            )
            // 底部：附加操作区 (OP.GG / 修复 / 公告 / 召唤师卡 / 设置)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .p(px(8.0))
                    .border_t_1()
                    .border_color(Theme::CARD_BORDER())
                    // 1. OP.GG
                    .child(Self::render_bottom_action(
                        "action-opgg",
                        crate::i18n::t("nav.opgg"),
                        opgg_logo(18.0),
                        expanded,
                        on_open_opgg,
                    ))
                    // 2. 修复无限加载 / 重新连接
                    .child(Self::render_bottom_action(
                        "action-reconnect",
                        crate::i18n::t("nav.reconnect"),
                        icon("icons/refresh.svg", 18.0, Theme::TEXT_MUTED()).into_any_element(),
                        expanded,
                        on_reconnect,
                    ))
                    // 3. 公告 / 更新日志
                    .child(Self::render_bottom_action(
                        "action-notice",
                        crate::i18n::t("nav.notice"),
                        icon("icons/bell.svg", 18.0, Theme::TEXT_MUTED()).into_any_element(),
                        expanded,
                        on_open_notice,
                    ))
                    // 4. 召唤师简短信息卡 (登录态展示)
                    .when_some(summoner, |el, sum| {
                        el.child(Self::render_user_card(
                            sum,
                            region_name,
                            expanded,
                            on_select_self,
                        ))
                    })
                    // 5. 设置 (有更新时展示红点)
                    .child(Self::render_item(
                        Route::Settings,
                        current_route,
                        expanded,
                        has_update,
                        on_select,
                    )),
            )
    }

    /// 渲染常规路由项
    fn render_item(
        route: Route,
        current_route: Route,
        expanded: bool,
        has_dot: bool,
        on_select: impl Fn(Route, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let is_active = current_route == route;
        let title = route.title();
        let icon_path = route.icon_path();
        let fg = if is_active {
            Theme::TEXT_PRIMARY()
        } else {
            Theme::TEXT_MUTED()
        };

        div()
            .id(SharedString::from(format!("nav-item-{}", title)))
            .relative()
            .flex()
            .items_center()
            .h(Theme::NAV_ITEM_HEIGHT)
            .rounded(Theme::RADIUS_MD)
            .border_1()
            .border_color(if is_active {
                Theme::SIDEBAR_ACTIVE_BORDER()
            } else {
                transparent_black()
            })
            .when(expanded, |el| el.px(px(14.0)))
            .when(!expanded, |el| el.justify_center())
            .when(is_active, |el| el.bg(Theme::SIDEBAR_ACTIVE()))
            .when(!is_active, |el| el.hover(|s| s.bg(Theme::HOVER_BG())))
            .cursor_pointer()
            // 左侧指示条 (4px，对照 Vue ::before)
            .when(is_active, |el| {
                el.child(
                    div()
                        .absolute()
                        .top(px(10.0))
                        .bottom(px(10.0))
                        .w(px(4.0))
                        .rounded_r(px(4.0))
                        .bg(Theme::ACCENT_PRIMARY())
                        .when(expanded, |el| el.left(px(-1.0)))
                        .when(!expanded, |el| el.left(px(3.0))),
                )
            })
            .child(
                div()
                    .relative()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(
                        icon_path,
                        18.0,
                        if is_active { Theme::ACCENT_PRIMARY() } else { fg },
                    ))
                    // 更新提示红点
                    .when(has_dot, |el| {
                        el.child(
                            div()
                                .absolute()
                                .top(px(-2.0))
                                .right(px(-2.0))
                                .size(px(6.0))
                                .rounded_full()
                                .bg(Theme::LOSS_BAR()),
                        )
                    }),
            )
            .when(expanded, |el| {
                el.child(
                    div()
                        .ml(px(12.0))
                        .flex_1()
                        .min_w(px(0.0))
                        .truncate()
                        .text_size(px(13.8))
                        .when(is_active, |el| el.font_weight(FontWeight::SEMIBOLD))
                        .text_color(fg)
                        .child(title),
                )
            })
            .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                on_select(route, window, app);
            })
    }

    /// 渲染底部独立动作按钮 (非路由页面跳转)
    fn render_bottom_action(
        id: &'static str,
        label: &'static str,
        icon_el: AnyElement,
        expanded: bool,
        on_click: impl Fn(&mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let cb = on_click;

        div()
            .id(id)
            .relative()
            .flex()
            .items_center()
            .h(Theme::NAV_ITEM_HEIGHT)
            .rounded(Theme::RADIUS_MD)
            .border_1()
            .border_color(transparent_black())
            .when(expanded, |el| el.px(px(14.0)))
            .when(!expanded, |el| el.justify_center())
            .hover(|s| s.bg(Theme::HOVER_BG()))
            .cursor_pointer()
            .child(icon_el)
            .when(expanded, |el| {
                el.child(
                    div()
                        .ml(px(12.0))
                        .flex_1()
                        .min_w(px(0.0))
                        .truncate()
                        .text_size(px(13.5))
                        .text_color(Theme::TEXT_MUTED())
                        .child(label),
                )
            })
            .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                cb(window, app);
            })
    }

    /// 渲染召唤师名片卡
    fn render_user_card(
        summoner: &SummonerInfo,
        region_name: &str,
        expanded: bool,
        on_select_self: impl Fn(&mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let name = summoner.full_name();
        let region = if region_name.is_empty() {
            format!("Lv. {}", summoner.summoner_level.unwrap_or(1))
        } else {
            region_name.to_string()
        };
        let avatar = format!(
            "/lol-game-data/assets/v1/profile-icons/{}.jpg",
            summoner.profile_icon_id.unwrap_or(29)
        );
        let click_cb = on_select_self;

        div()
            .id("sidebar-user-card")
            .flex()
            .items_center()
            .h(px(48.0))
            .when(expanded, |el| el.px(px(8.0)))
            .when(!expanded, |el| el.justify_center())
            .rounded(Theme::RADIUS_MD)
            .border_1()
            .border_color(transparent_black())
            .hover(|s| s.bg(Theme::HOVER_BG_STRONG()))
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                click_cb(window, app);
            })
            .child(
                LcuImage::new(avatar)
                    .size(px(30.0))
                    .circle()
                    .border_color(Theme::CARD_BORDER())
                    .placeholder("·"),
            )
            .when(expanded, |el| {
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .ml(px(10.0))
                        .min_w(px(0.0))
                        .child(
                            div()
                                .text_size(px(12.5))
                                .font_weight(FontWeight::BOLD)
                                .text_color(Theme::TEXT_PRIMARY())
                                .truncate()
                                .child(name),
                        )
                        .child(
                            div()
                                .text_size(px(10.5))
                                .text_color(Theme::TEXT_MUTED())
                                .truncate()
                                .child(region),
                        ),
                )
            })
    }
}

/// 透明占位边框：保持尺寸一致，避免 hover 跳动
fn transparent_black() -> Hsla {
    Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.0,
        a: 0.0,
    }
}
