//! 内嵌静态资源（线性图标与应用 Logo）
//!
//! 图标以 SVG 源码编译进二进制，通过实现 `gpui::AssetSource` 交给 `svg()` 元素渲染；
//! GPUI 的 SVG 渲染器会把图形转成 alpha 蒙版并用 `text_color` 着色，
//! 因此图标颜色完全跟随主题语义色，无需为每种状态维护多份资源。

use gpui::{AssetSource, RenderImage, Result, SharedString};
use std::borrow::Cow;
use std::sync::{Arc, OnceLock};

/// 图标资源集合（路径形如 `icons/home.svg`）
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let bytes: Option<&'static [u8]> = match path {
            "icons/back.svg" => Some(include_bytes!("../assets/icons/back.svg")),
            "icons/home.svg" => Some(include_bytes!("../assets/icons/home.svg")),
            "icons/user.svg" => Some(include_bytes!("../assets/icons/user.svg")),
            "icons/search.svg" => Some(include_bytes!("../assets/icons/search.svg")),
            "icons/gamepad.svg" => Some(include_bytes!("../assets/icons/gamepad.svg")),
            "icons/bookmark.svg" => Some(include_bytes!("../assets/icons/bookmark.svg")),
            "icons/wrench.svg" => Some(include_bytes!("../assets/icons/wrench.svg")),
            "icons/settings.svg" => Some(include_bytes!("../assets/icons/settings.svg")),
            "icons/menu.svg" => Some(include_bytes!("../assets/icons/menu.svg")),
            "icons/minimize.svg" => Some(include_bytes!("../assets/icons/minimize.svg")),
            "icons/maximize.svg" => Some(include_bytes!("../assets/icons/maximize.svg")),
            "icons/restore.svg" => Some(include_bytes!("../assets/icons/restore.svg")),
            "icons/chevron-down.svg" => Some(include_bytes!("../assets/icons/chevron-down.svg")),
            "icons/chevron-up.svg" => Some(include_bytes!("../assets/icons/chevron-up.svg")),
            "icons/close.svg" => Some(include_bytes!("../assets/icons/close.svg")),
            "icons/copy.svg" => Some(include_bytes!("../assets/icons/copy.svg")),
            "icons/lock.svg" => Some(include_bytes!("../assets/icons/lock.svg")),
            "icons/play.svg" => Some(include_bytes!("../assets/icons/play.svg")),
            "icons/refresh.svg" => Some(include_bytes!("../assets/icons/refresh.svg")),
            "icons/bell.svg" => Some(include_bytes!("../assets/icons/bell.svg")),
            "icons/tft.svg" => Some(include_bytes!("../assets/icons/tft.svg")),
            "icons/opgg.svg" => Some(include_bytes!("../assets/icons/opgg.svg")),
            _ => None,
        };
        Ok(bytes.map(Cow::Borrowed))
    }

    fn list(&self, _path: &str) -> Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}

/// 应用 Logo（复用仓库内的 `public/logo.png`，避免二进制资源重复入库）
const LOGO_PNG: &[u8] = include_bytes!("../../../public/logo.png");

static LOGO_IMAGE: OnceLock<Option<Arc<RenderImage>>> = OnceLock::new();

/// 同步获取已解码的 Logo 图（首次调用解码，后续命中缓存）
pub fn logo_image() -> Option<Arc<RenderImage>> {
    LOGO_IMAGE
        .get_or_init(|| {
            crate::asset_loader::AssetLoader::decode_bytes_to_render_image(LOGO_PNG)
                .map_err(|e| log::warn!("[Assets] Logo 解码失败: {}", e))
                .ok()
        })
        .clone()
}
