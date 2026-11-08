//! 导航侧栏（对照 Vue `src/components/layout/NavigationSidebar.vue`）
//!
//! 复刻要点：折叠 64 / 展开 210、头部 52 高的折叠按钮行、菜单行 42 高配 18px 线性图标、
//! 选中态青透底 + 4px 内侧指示条，底部动作组与主路由用分隔线分开。

use crate::components::icon::icon;
use crate::components::lcu_image::LcuImage;
use crate::theme::Theme;
use crate::views::Route;
use gpui::prelude::FluentBuilder;
use gpui::*;
use yuumi_core::parsers::summoner::SummonerInfo;

pub struct Sidebar;

impl Sidebar {
    pub fn render(
        current_route: Route,
        expanded: bool,
        summoner: Option<&SummonerInfo>,
        on_select: impl Fn(Route, &mut Window, &mut App) + 'static + Clone,
        on_toggle: impl Fn(&mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let main_routes = [
            Route::Home,
            Route::Career,
            Route::Search,
            Route::GameInfo,
            Route::SavedPlayers,
            Route::Tools,
        ];

        let width = if expanded {
            Theme::SIDEBAR_WIDTH_EXPANDED
        } else {
            Theme::SIDEBAR_WIDTH
        };

        let toggle_cb = on_toggle;

        div()
            .flex()
            .flex_col()
            .h_full()
            .w(width)
            .flex_shrink_0()
            .overflow_hidden()
            .bg(Theme::SIDEBAR_BG)
            .border_r_1()
            .border_color(Theme::CARD_BORDER)
            // 顶部：折叠按钮
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
                            .text_color(Theme::TEXT_MUTED)
                            .hover(|s| s.bg(Theme::HOVER_BG).text_color(Theme::TEXT_PRIMARY))
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                                toggle_cb(window, app);
                            })
                            .child(icon("icons/menu.svg", 20.0, Theme::TEXT_MUTED)),
                    ),
            )
            // 中部：一级导航
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .p(px(8.0))
                    .flex_1()
                    .children(main_routes.into_iter().map(|route| {
                        Self::render_item(route, current_route, expanded, on_select.clone())
                    })),
            )
            // 底部：动作组（召唤师信息 + 设置）
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .p(px(8.0))
                    .border_t_1()
                    .border_color(Theme::CARD_BORDER)
                    .when_some(summoner, |el, summoner| {
                        let name = summoner.full_name();
                        let level = summoner.summoner_level;
                        let avatar = format!(
                            "/lol-game-data/assets/v1/profile-icons/{}.jpg",
                            summoner.profile_icon_id.unwrap_or(29)
                        );
                        let nav_career = on_select.clone();
                        el.child(
                            div()
                                .id("sidebar-user-card")
                                .flex()
                                .items_center()
                                .h(px(52.0))
                                .when(expanded, |el| el.px(px(10.0)))
                                .when(!expanded, |el| el.justify_center())
                                .my(px(8.0))
                                .rounded(Theme::RADIUS_MD)
                                .border_1()
                                .border_color(transparent_black())
                                .hover(|s| s.bg(Theme::HOVER_BG_STRONG))
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                                    nav_career(Route::Career, window, app);
                                })
                                .child(
                                    LcuImage::new(avatar)
                                        .size(px(32.0))
                                        .circle()
                                        .border_color(Theme::CARD_BORDER)
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
                                                    .text_size(px(13.1))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(Theme::TEXT_PRIMARY)
                                                    .truncate()
                                                    .child(name),
                                            )
                                            .when_some(level, |el, level| {
                                                el.child(
                                                    div()
                                                        .text_size(px(10.9))
                                                        .text_color(Theme::TEXT_MUTED)
                                                        .child(format!("Lv. {}", level)),
                                                )
                                            }),
                                    )
                                }),
                        )
                    })
                    .child(Self::render_item(
                        Route::Settings,
                        current_route,
                        expanded,
                        on_select,
                    )),
            )
    }

    fn render_item(
        route: Route,
        current_route: Route,
        expanded: bool,
        on_select: impl Fn(Route, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let is_active = current_route == route;
        let title = route.title();
        let icon_path = route.icon_path();
        let fg = if is_active {
            Theme::TEXT_PRIMARY
        } else {
            Theme::TEXT_MUTED
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
                Theme::SIDEBAR_ACTIVE_BORDER
            } else {
                transparent_black()
            })
            .when(expanded, |el| el.px(px(14.0)))
            .when(!expanded, |el| el.justify_center())
            .when(is_active, |el| el.bg(Theme::SIDEBAR_ACTIVE))
            .when(!is_active, |el| el.hover(|s| s.bg(Theme::HOVER_BG)))
            .cursor_pointer()
            // 左侧选中指示条（4px，仅圆右侧，对照 Vue ::before）
            .when(is_active, |el| {
                el.child(
                    div()
                        .absolute()
                        .top(px(10.0))
                        .bottom(px(10.0))
                        .w(px(4.0))
                        .rounded_r(px(4.0))
                        .bg(Theme::ACCENT_PRIMARY)
                        .when(expanded, |el| el.left(px(-1.0)))
                        .when(!expanded, |el| el.left(px(3.0))),
                )
            })
            .child(icon(
                icon_path,
                18.0,
                if is_active { Theme::ACCENT_PRIMARY } else { fg },
            ))
            .when(expanded, |el| {
                el.child(
                    div()
                        .ml(px(12.0))
                        .flex_1()
                        .min_w(px(0.0))
                        .truncate()
                        .text_size(px(14.1))
                        .when(is_active, |el| el.font_weight(FontWeight::SEMIBOLD))
                        .text_color(fg)
                        .child(title),
                )
            })
            .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                on_select(route, window, app);
            })
    }
}

/// 透明占位边框：保持未激活项与激活项尺寸一致，避免 hover/选中时行高跳动
fn transparent_black() -> Hsla {
    Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.0,
        a: 0.0,
    }
}
