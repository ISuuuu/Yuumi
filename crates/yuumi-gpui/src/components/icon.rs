//! 线性图标渲染辅助
//!
//! 图标以 SVG 源码内嵌（见 `crate::assets`），GPUI 会把图形渲染成 alpha 蒙版
//! 再用 `text_color` 着色，因此同一份资源即可跟随主题在任意状态色下复用。

use gpui::prelude::*;
use gpui::*;

/// 渲染一枚线性图标
///
/// - `path`：`crate::assets::Assets` 中登记的路径，例如 `icons/home.svg`
/// - `size`：图标边长（Vue 侧为 18px 导航图标 / 14px 标题栏按钮图标）
/// - `color`：着色，通常取 `Theme::TEXT_MUTED` 或品牌青
pub fn icon(path: &'static str, size: f32, color: Hsla) -> impl IntoElement {
    svg()
        .path(path)
        .flex_none()
        .w(px(size))
        .h(px(size))
        .text_color(color)
}
