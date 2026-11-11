use crate::assets::logo_image;
use crate::theme::Theme;
use gpui::prelude::FluentBuilder;
use gpui::*;

/// 统一猫咪头像尺寸（120x120，大气居中）
pub const OFFLINE_CAT_SIZE: Pixels = px(120.0);

/// 离线页面统一的猫咪头像（96x96，居中，带阴影与轻微悬浮效果）
pub fn render_offline_cat() -> impl IntoElement {
    div()
        .mb(px(24.0))
        .cursor_default()
        .hover(|s| s.opacity(0.92))
        .when_some(logo_image(), |el, logo| {
            el.child(
                img(logo)
                    .w(OFFLINE_CAT_SIZE)
                    .h(OFFLINE_CAT_SIZE)
                    .object_fit(ObjectFit::Contain)
                    .shadow_lg(),
            )
        })
}

/// 统一的离线提示文案
pub fn render_offline_tip(text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .text_size(px(14.1))
        .line_height(px(22.0))
        .text_color(Theme::TEXT_SECONDARY())
        .child(text.into())
}

/// 统一的离线状态渲染（猫咪头像在垂直正中 + 提示文案 + 可选扩展按钮/操作面板）
pub fn render_lcu_offline_state_with_tip(
    tip: impl Into<SharedString>,
    extra_action: Option<AnyElement>,
) -> AnyElement {
    div()
        .relative()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .px(px(24.0))
        // 顶部弹性占位，将核心内容自然推至垂直正中
        .child(div().flex_1())
        .child(
            div()
                .relative()
                .flex()
                .flex_col()
                .items_center()
                .text_center()
                .max_w(px(520.0))
                .w_full()
                .child(render_offline_cat())
                .child(render_offline_tip(tip))
                .when_some(extra_action, |el, action| {
                    el.child(
                        div()
                            .absolute()
                            .top(px(166.0))
                            .left_0()
                            .right_0()
                            .flex()
                            .flex_col()
                            .items_center()
                            .child(action),
                    )
                }),
        )
        // 底部弹性占位，保证核心内容上下完全对称居中
        .child(div().flex_1())
        .into_any_element()
}

/// 统一的离线状态渲染（默认文案 "请先启动英雄联盟客户端"）
pub fn render_lcu_offline_state(extra_action: Option<AnyElement>) -> AnyElement {
    render_lcu_offline_state_with_tip("请先启动英雄联盟客户端", extra_action)
}
