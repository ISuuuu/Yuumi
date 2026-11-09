//! 纯白水晶极光风格 LCU 图片组件 (LcuImage)
//!
//! 提供圆角卡片、圆形头像、装备格等形态的原生图片渲染；
//! 自动对接 `global_asset_loader()`，支持磁盘本地缓存、内存 LRU 以及异步拉取 fallback。

use crate::asset_loader::global_asset_loader;
use crate::theme::Theme;
use gpui::*;
use std::sync::Arc;

/// 图片形态变体
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LcuImageShape {
    /// 矩形圆角 (默认 md, 6px)
    Rounded,
    /// 装备小格子 (3px 圆角)
    ItemSlot,
    /// 圆形头像 (用于召唤师头像、英雄圆环等)
    Circle,
}

/// 纯白水晶极光风格 LCU 原生图片组件
pub struct LcuImage {
    /// 原始 LCU 资源路径或 CDN 相对路径 (例如 `/lol-game-data/assets/v1/champion-icons/1.png`)
    src: String,
    /// 尺寸 (宽和高相同)
    size: Pixels,
    /// 外观形态
    shape: LcuImageShape,
    /// 边框高亮颜色
    border_color: Option<Hsla>,
    /// 边框粗细
    border_width: Pixels,
    /// 占位文本或字符
    placeholder_text: Option<SharedString>,
    /// 占位背景底色
    placeholder_bg: Option<Hsla>,
    /// 缩放适应方式
    fit: ObjectFit,
}

impl LcuImage {
    /// 创建一个新的 LcuImage
    pub fn new(src: impl Into<String>) -> Self {
        Self {
            src: src.into(),
            size: px(32.0),
            shape: LcuImageShape::Rounded,
            border_color: None,
            border_width: px(1.0),
            placeholder_text: None,
            placeholder_bg: None,
            fit: ObjectFit::Cover,
        }
    }

    /// 设置尺寸大小
    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    /// 设置形状形态
    pub fn shape(mut self, shape: LcuImageShape) -> Self {
        self.shape = shape;
        self
    }

    /// 圆形头像预设
    pub fn circle(mut self) -> Self {
        self.shape = LcuImageShape::Circle;
        self
    }

    /// 装备槽位预设
    #[allow(dead_code)]
    pub fn item_slot(mut self) -> Self {
        self.shape = LcuImageShape::ItemSlot;
        self
    }

    /// 设置边框颜色
    pub fn border_color(mut self, color: impl Into<Hsla>) -> Self {
        self.border_color = Some(color.into());
        self
    }

    /// 设置边框粗细
    #[allow(dead_code)]
    pub fn border_width(mut self, width: Pixels) -> Self {
        self.border_width = width;
        self
    }

    /// 设置兜底/未加载时的占位符文本
    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder_text = Some(text.into());
        self
    }

    /// 设置占位背景颜色
    #[allow(dead_code)]
    pub fn placeholder_bg(mut self, bg: impl Into<Hsla>) -> Self {
        self.placeholder_bg = Some(bg.into());
        self
    }

    /// 设置对象填充模式
    #[allow(dead_code)]
    pub fn object_fit(mut self, fit: ObjectFit) -> Self {
        self.fit = fit;
        self
    }

    /// 尝试从当前全局加载器同步获取已解码的 RenderImage
    #[allow(dead_code)]
    pub fn get_render_image(&self) -> Option<Arc<RenderImage>> {
        if self.src.trim().is_empty() {
            return None;
        }
        global_asset_loader().get_cached_render_image(&self.src)
    }
}

impl IntoElement for LcuImage {
    type Element = Div;

    fn into_element(self) -> Self::Element {
        let loader = global_asset_loader();
        let trimmed_src = self.src.trim();
        let cached_img = if trimmed_src.is_empty() {
            None
        } else {
            loader.get_cached_render_image(trimmed_src)
        };

        let default_border = Theme::CARD_BORDER;
        let border_c = self.border_color.unwrap_or(default_border);
        let bg_c = self.placeholder_bg.unwrap_or(Theme::BADGE_BG);

        let mut container = div()
            .size(self.size)
            .bg(bg_c)
            .border(self.border_width)
            .border_color(border_c)
            .overflow_hidden()
            .flex()
            .items_center()
            .justify_center();

        match self.shape {
            LcuImageShape::Circle => {
                container = container.rounded_full();
            }
            LcuImageShape::ItemSlot => {
                container = container.rounded(px(3.0));
            }
            LcuImageShape::Rounded => {
                container = container.rounded_lg();
            }
        }

        if let Some(render_img) = cached_img {
            // 图片元素自带圆角：GPUI 的 img 不随父级 overflow_hidden 的圆角裁切，
            // 必须把圆角落到 img 自身的 corner_radii 上才会生效
            let image = match self.shape {
                LcuImageShape::Circle => img(render_img)
                    .size_full()
                    .object_fit(self.fit)
                    .rounded_full(),
                LcuImageShape::ItemSlot => img(render_img)
                    .size_full()
                    .object_fit(self.fit)
                    .rounded(px(3.0)),
                LcuImageShape::Rounded => img(render_img)
                    .size_full()
                    .object_fit(self.fit)
                    .rounded_lg(),
            };
            container.child(image)
        } else {
            // 占位显示：纯白水晶极光风格柔和微光 + 占位文本
            if let Some(txt) = self.placeholder_text {
                container.child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(Theme::TEXT_MUTED)
                        .child(txt),
                )
            } else {
                container.child(div().size_full().bg(Theme::BADGE_BG).opacity(0.6))
            }
        }
    }
}
