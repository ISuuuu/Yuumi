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
/// - `color`：着色，通常取 `Theme::TEXT_MUTED()` 或品牌青
pub fn icon(path: &'static str, size: f32, color: Hsla) -> impl IntoElement {
    svg()
        .path(path)
        .flex_none()
        .w(px(size))
        .h(px(size))
        .text_color(color)
}

/// 渲染 OP.GG 品牌徽标（原色位图）。
///
/// 侧栏入口（18px）与 OP.GG 窗口头部（22px）共用。Vue 侧这两处都是原色
/// `<img :src="opggIcon">`，若改用 `icon()` 走 SVG 蒙版着色会丢掉渐变与白色字形。
pub fn opgg_logo(size: f32) -> AnyElement {
    // Vue 侧 18px 图标配 3px 圆角，同比换算其它尺寸
    let radius = px(size * 0.17);
    match crate::assets::opgg_logo_image() {
        Some(image) => div()
            .flex_none()
            .size(px(size))
            .rounded(radius)
            .overflow_hidden()
            .child(img(image).size_full().rounded(radius))
            .into_any_element(),
        // 解码失败时退回品牌主色块，保证布局不塌
        None => div()
            .flex_none()
            .size(px(size))
            .rounded(radius)
            .bg(rgb(0x636ff9))
            .into_any_element(),
    }
}
