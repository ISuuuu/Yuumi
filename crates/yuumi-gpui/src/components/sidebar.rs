use gpui::prelude::FluentBuilder;
use gpui::*;
use crate::theme;
use crate::views::Route;

/// 48px 极光水晶风格左侧导航栏
pub struct Sidebar;

impl Sidebar {
    pub fn render(
        current_route: Route,
        on_select: impl Fn(Route, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let main_routes = [
            Route::Home,
            Route::Career,
            Route::Search,
            Route::GameInfo,
            Route::SavedPlayers,
            Route::Tools,
        ];

        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_between()
            .w(theme::SIDEBAR_WIDTH)
            .h_full()
            .bg(theme::SIDEBAR_BG)
            .border_r_1()
            .border_color(theme::BORDER_SECONDARY)
            .py_2()
            // 顶部 Logo + 一级路由
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .w_full()
                    .gap_1()
                    // Logo
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(32.0))
                            .h(px(32.0))
                            .mb_2()
                            .rounded(px(8.0))
                            .bg(theme::PRIMARY.opacity(0.15))
                            .border_1()
                            .border_color(theme::PRIMARY.opacity(0.3))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::EXTRA_BOLD)
                                    .text_color(theme::PRIMARY)
                                    .child("Y"),
                            ),
                    )
                    // 主导航列表
                    .children(main_routes.into_iter().map(|route| {
                        let is_active = current_route == route;
                        Self::render_item(route, is_active, on_select.clone())
                    })),
            )
            // 底部设置
            .child(
                div()
                    .w_full()
                    .child(Self::render_item(Route::Settings, current_route == Route::Settings, on_select)),
            )
    }

    fn render_item(
        route: Route,
        is_active: bool,
        on_select: impl Fn(Route, &mut Window, &mut App) + 'static + Clone,
    ) -> impl IntoElement {
        let title = route.title();
        let icon = route.icon();

        div()
            .id(SharedString::from(format!("nav-item-{}", title)))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .w_full()
            .h(px(46.0))
            .relative()
            .cursor_pointer()
            .when(is_active, |this| {
                this.bg(theme::SIDEBAR_ACTIVE)
            })
            .hover(|this| {
                this.bg(theme::SIDEBAR_HOVER)
            })
            // 左侧选中极光强调线
            .when(is_active, |this| {
                this.child(
                    div()
                        .absolute()
                        .left_0()
                        .top(px(8.0))
                        .bottom(px(8.0))
                        .w(px(3.0))
                        .rounded_r(px(2.0))
                        .bg(theme::PRIMARY),
                )
            })
            // 路由图标/文字
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_0p5()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(if is_active {
                                theme::PRIMARY
                            } else {
                                theme::TEXT_SECONDARY
                            })
                            .child(icon),
                    )
                    .child(
                        div()
                            .text_color(if is_active {
                                theme::PRIMARY
                            } else {
                                theme::TEXT_MUTED
                            })
                            .text_size(px(10.0))
                            .child(title),
                    ),
            )
            .on_mouse_down(MouseButton::Left, move |_ev, window, app| {
                on_select(route, window, app);
            })
    }
}
