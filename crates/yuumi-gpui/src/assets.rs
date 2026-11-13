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
            "icons/chevron-right.svg" => Some(include_bytes!("../assets/icons/chevron-right.svg")),
            "icons/close.svg" => Some(include_bytes!("../assets/icons/close.svg")),
            "icons/copy.svg" => Some(include_bytes!("../assets/icons/copy.svg")),
            "icons/lock.svg" => Some(include_bytes!("../assets/icons/lock.svg")),
            "icons/play.svg" => Some(include_bytes!("../assets/icons/play.svg")),
            "icons/refresh.svg" => Some(include_bytes!("../assets/icons/refresh.svg")),
            "icons/bell.svg" => Some(include_bytes!("../assets/icons/bell.svg")),
            "icons/tft.svg" => Some(include_bytes!("../assets/icons/tft.svg")),
            "icons/opgg.svg" => Some(include_bytes!("../assets/icons/opgg.svg")),
            "icons/tier-1.svg" => Some(include_bytes!("../assets/icons/tier-1.svg")),
            "icons/tier-2.svg" => Some(include_bytes!("../assets/icons/tier-2.svg")),
            "icons/tier-3.svg" => Some(include_bytes!("../assets/icons/tier-3.svg")),
            "icons/tier-4.svg" => Some(include_bytes!("../assets/icons/tier-4.svg")),
            "icons/drag.svg" => Some(include_bytes!("../assets/icons/drag.svg")),
            "icons/cs.svg" => Some(include_bytes!("../assets/icons/cs.svg")),
            "icons/gold.svg" => Some(include_bytes!("../assets/icons/gold.svg")),
            _ => None,
        };
        Ok(bytes.map(Cow::Borrowed))
    }

    fn list(&self, _path: &str) -> Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}

/// 应用 Logo
const LOGO_PNG: &[u8] = include_bytes!("../assets/logo.png");

/// OP.GG 品牌徽标（原色 PNG）
///
/// GPUI 的 `svg()` 会把图形压成 alpha 蒙版并统一着色，官方 logo 的渐变底与
/// 白色字形会被一并涂成单色而丢失识别度，所以这里内嵌一张由同一份
/// `assets/icons/opgg.svg` 渲染出的 96×96 原色位图。
const OPGG_LOGO_PNG: &[u8] = include_bytes!("../assets/icons/opgg-logo.png");

/// 分路位置图标（沿用旧版 Vue 的 5 张彩色小图：浅色小地图 + 彩色分路）
///
/// 这些是小尺寸彩色位图，走 SVG 的 alpha 蒙版会被涂成单色而丢掉分路配色，
/// 因此与 OP.GG 徽标一样内嵌 PNG 原色渲染。
const POSITION_ICON_FILES: [(&str, &[u8]); 5] = [
    ("TOP", include_bytes!("../assets/images/positions/top.png")),
    (
        "JUNGLE",
        include_bytes!("../assets/images/positions/jungle.png"),
    ),
    (
        "MIDDLE",
        include_bytes!("../assets/images/positions/middle.png"),
    ),
    (
        "BOTTOM",
        include_bytes!("../assets/images/positions/bottom.png"),
    ),
    (
        "UTILITY",
        include_bytes!("../assets/images/positions/utility.png"),
    ),
];

static POSITION_ICONS: OnceLock<std::collections::HashMap<&'static str, Arc<RenderImage>>> =
    OnceLock::new();

/// 同步获取分路位置图标，键为 PositionKey::as_str()（TOP / JUNGLE / MIDDLE / BOTTOM / UTILITY）
pub fn position_icon_image(key: &str) -> Option<Arc<RenderImage>> {
    let icons = POSITION_ICONS.get_or_init(|| {
        POSITION_ICON_FILES
            .iter()
            .filter_map(|(name, bytes)| {
                crate::asset_loader::AssetLoader::decode_bytes_to_render_image(bytes)
                    .map_err(|error| log::warn!("[Assets] 分路图标 {name} 解码失败: {error}"))
                    .ok()
                    .map(|image| (*name, image))
            })
            .collect()
    });
    icons.get(key).cloned()
}

static LOGO_IMAGE: OnceLock<Option<Arc<RenderImage>>> = OnceLock::new();
static OPGG_LOGO_IMAGE: OnceLock<Option<Arc<RenderImage>>> = OnceLock::new();

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

/// 同步获取已解码的 OP.GG 品牌徽标
pub fn opgg_logo_image() -> Option<Arc<RenderImage>> {
    OPGG_LOGO_IMAGE
        .get_or_init(|| {
            crate::asset_loader::AssetLoader::decode_bytes_to_render_image(OPGG_LOGO_PNG)
                .map_err(|e| log::warn!("[Assets] OP.GG logo 解码失败: {}", e))
                .ok()
        })
        .clone()
}
