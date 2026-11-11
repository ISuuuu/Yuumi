//! GPUI 图片异步加载引擎与渲染格式适配器
//!
//! 基于 `yuumi_core::lcu::AssetCache`，使用 `image` crate 将原始图片解码为 GPUI `RenderImage` (BGRA)，
//! 并通过 `Arc<RwLock<HashMap<String, Arc<RenderImage>>>>` 在 UI 线程快速获取。

use gpui::RenderImage;
use image::Frame;
use smallvec::SmallVec;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};
use yuumi_core::lcu::AssetCache;

/// 全局单例 AssetLoader
static ASSET_LOADER: OnceLock<AssetLoader> = OnceLock::new();

/// 获取或初始化全局资源加载引擎
pub fn global_asset_loader() -> &'static AssetLoader {
    ASSET_LOADER.get_or_init(AssetLoader::default)
}

/// 用自定义 core cache 初始化全局资源加载引擎（必须在首次访问前调用，如已初始化则安全忽略）
pub fn init_global_asset_loader(core_cache: Arc<AssetCache>) {
    let _ = ASSET_LOADER.set(AssetLoader::new(core_cache));
}

#[derive(Clone)]
pub struct AssetLoader {
    core_cache: Arc<AssetCache>,
    decoded_cache: Arc<RwLock<HashMap<String, Arc<RenderImage>>>>,
}

impl Default for AssetLoader {
    fn default() -> Self {
        Self::new(Arc::new(AssetCache::default()))
    }
}

impl AssetLoader {
    pub fn new(core_cache: Arc<AssetCache>) -> Self {
        Self {
            core_cache,
            decoded_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 使用指定本地磁盘目录构造（便于隔离与测试）
    #[allow(dead_code)]
    pub fn with_cache_dir(cache_dir: PathBuf) -> Self {
        let core = Arc::new(AssetCache::with_cache_dir(None, cache_dir));
        Self::new(core)
    }

    /// 将图片原始字节（PNG / WebP / JPEG 等）解码并转换成 GPUI 原生所需的 BGRA `RenderImage`
    pub fn decode_bytes_to_render_image(bytes: &[u8]) -> Result<Arc<RenderImage>, String> {
        let raw_format =
            image::guess_format(bytes).map_err(|e| format!("无法识别图片格式: {}", e))?;

        let dyn_img = image::load_from_memory_with_format(bytes, raw_format)
            .map_err(|e| format!("图片解码失败: {}", e))?;

        let mut rgba = dyn_img.into_rgba8();
        // GPUI DirectX/Metal 渲染管线要求像素格式为 BGRA
        for pixel in rgba.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }

        let frame = Frame::new(rgba);
        let render_img = RenderImage::new(SmallVec::from_elem(frame, 1));
        Ok(Arc::new(render_img))
    }

    pub fn get_builtin_image_bytes(path: &str) -> Option<&'static [u8]> {
        let p = path.trim_start_matches('/');
        match p {
            "images/kills.png" => Some(include_bytes!("../../../public/images/kills.png")),
            "images/tower-100.png" => Some(include_bytes!("../../../public/images/tower-100.png")),
            "images/tower-200.png" => Some(include_bytes!("../../../public/images/tower-200.png")),
            "images/inhibitor-100.png" => {
                Some(include_bytes!("../../../public/images/inhibitor-100.png"))
            }
            "images/inhibitor-200.png" => {
                Some(include_bytes!("../../../public/images/inhibitor-200.png"))
            }
            "images/baron-100.png" => Some(include_bytes!("../../../public/images/baron-100.png")),
            "images/baron-200.png" => Some(include_bytes!("../../../public/images/baron-200.png")),
            "images/dragon-100.png" => {
                Some(include_bytes!("../../../public/images/dragon-100.png"))
            }
            "images/dragon-200.png" => {
                Some(include_bytes!("../../../public/images/dragon-200.png"))
            }
            "images/herald-100.png" => {
                Some(include_bytes!("../../../public/images/herald-100.png"))
            }
            "images/herald-200.png" => {
                Some(include_bytes!("../../../public/images/herald-200.png"))
            }
            "images/sr-victory.png" => {
                Some(include_bytes!("../../../public/images/sr-victory.png"))
            }
            "images/sr-defeat.png" => Some(include_bytes!("../../../public/images/sr-defeat.png")),
            "images/ha-victory.png" => {
                Some(include_bytes!("../../../public/images/ha-victory.png"))
            }
            "images/ha-defeat.png" => Some(include_bytes!("../../../public/images/ha-defeat.png")),
            "images/arena-victory.png" => {
                Some(include_bytes!("../../../public/images/arena-victory.png"))
            }
            "images/arena-defeat.png" => {
                Some(include_bytes!("../../../public/images/arena-defeat.png"))
            }
            "images/other-victory.png" => {
                Some(include_bytes!("../../../public/images/other-victory.png"))
            }
            "images/other-defeat.png" => {
                Some(include_bytes!("../../../public/images/other-defeat.png"))
            }
            "assets/ranked-icons/iron.png" => {
                Some(include_bytes!("../../../src/assets/ranked-icons/iron.png"))
            }
            "assets/ranked-icons/bronze.png" => Some(include_bytes!(
                "../../../src/assets/ranked-icons/bronze.png"
            )),
            "assets/ranked-icons/silver.png" => Some(include_bytes!(
                "../../../src/assets/ranked-icons/silver.png"
            )),
            "assets/ranked-icons/gold.png" => {
                Some(include_bytes!("../../../src/assets/ranked-icons/gold.png"))
            }
            "assets/ranked-icons/platinum.png" => Some(include_bytes!(
                "../../../src/assets/ranked-icons/platinum.png"
            )),
            "assets/ranked-icons/emerald.png" => Some(include_bytes!(
                "../../../src/assets/ranked-icons/emerald.png"
            )),
            "assets/ranked-icons/diamond.png" => Some(include_bytes!(
                "../../../src/assets/ranked-icons/diamond.png"
            )),
            "assets/ranked-icons/master.png" => Some(include_bytes!(
                "../../../src/assets/ranked-icons/master.png"
            )),
            "assets/ranked-icons/grandmaster.png" => Some(include_bytes!(
                "../../../src/assets/ranked-icons/grandmaster.png"
            )),
            "assets/ranked-icons/challenger.png" => Some(include_bytes!(
                "../../../src/assets/ranked-icons/challenger.png"
            )),
            _ => None,
        }
    }

    /// 获取段位图标的相对路径（可直接送给 LcuImage 渲染）
    pub fn get_ranked_icon_path(tier: &str) -> Option<&'static str> {
        match tier.trim().to_ascii_uppercase().as_str() {
            "IRON" => Some("assets/ranked-icons/iron.png"),
            "BRONZE" => Some("assets/ranked-icons/bronze.png"),
            "SILVER" => Some("assets/ranked-icons/silver.png"),
            "GOLD" => Some("assets/ranked-icons/gold.png"),
            "PLATINUM" => Some("assets/ranked-icons/platinum.png"),
            "EMERALD" => Some("assets/ranked-icons/emerald.png"),
            "DIAMOND" => Some("assets/ranked-icons/diamond.png"),
            "MASTER" => Some("assets/ranked-icons/master.png"),
            "GRANDMASTER" => Some("assets/ranked-icons/grandmaster.png"),
            "CHALLENGER" => Some("assets/ranked-icons/challenger.png"),
            _ => None,
        }
    }

    /// 同步纯内存快速查询已解码缓存（0 阻塞 UI 主线程，绝不在渲染主线程执行磁盘 I/O 或图像解码）
    pub fn get_cached_render_image(&self, path: &str) -> Option<Arc<RenderImage>> {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return None;
        }

        // 纯内存已解码缓存命中，微秒级返回
        if let Ok(lock) = self.decoded_cache.read() {
            if let Some(img) = lock.get(trimmed) {
                return Some(img.clone());
            }
        }

        // 内置静态图片在未命中内存缓存时做同步快速解码并写入
        if let Some(builtin) = Self::get_builtin_image_bytes(trimmed) {
            if let Ok(img) = Self::decode_bytes_to_render_image(builtin) {
                if let Ok(mut lock) = self.decoded_cache.write() {
                    lock.insert(trimmed.to_string(), img.clone());
                }
                return Some(img);
            }
        }

        None
    }

    /// 异步拉取并解码图片（在后台 Tokio 线程池执行磁盘/网络 I/O 与 CPU 图像解码）
    pub async fn load_render_image(&self, path: &str) -> Result<Arc<RenderImage>, String> {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return Err("图片路径为空".to_string());
        }

        if let Some(cached) = self.get_cached_render_image(trimmed) {
            return Ok(cached);
        }

        let bytes = if let Some(builtin) = Self::get_builtin_image_bytes(trimmed) {
            builtin.to_vec()
        } else if let Some(raw) = self.core_cache.try_read_disk_cache(trimmed) {
            raw
        } else {
            self.core_cache.fetch_asset_bytes(trimmed).await?
        };

        let render_img = Self::decode_bytes_to_render_image(&bytes)?;

        if let Ok(mut lock) = self.decoded_cache.write() {
            lock.insert(trimmed.to_string(), render_img.clone());
        }

        Ok(render_img)
    }

    /// 获取底层 core 缓存引用
    #[allow(dead_code)]
    pub fn core_cache(&self) -> &Arc<AssetCache> {
        &self.core_cache
    }

    /// 获取特定资源路径对应的本地磁盘缓存文件路径（如存在）
    #[allow(dead_code)]
    pub fn get_disk_cache_file_path(&self, path: &str) -> Option<PathBuf> {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return None;
        }
        let file_path = self.core_cache.cache_file_path(trimmed);
        if file_path.exists() {
            Some(file_path)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_png_to_render_image() {
        // 创建一个简单的 2x2 红色 PNG 图片
        let mut img_buf = image::RgbaImage::new(2, 2);
        for pixel in img_buf.pixels_mut() {
            *pixel = image::Rgba([255, 0, 0, 255]);
        }
        let mut png_bytes = Vec::new();
        img_buf
            .write_to(
                &mut std::io::Cursor::new(&mut png_bytes),
                image::ImageFormat::Png,
            )
            .unwrap();

        let render_img = AssetLoader::decode_bytes_to_render_image(&png_bytes).unwrap();
        assert_eq!(render_img.frame_count(), 1);
        let bytes = render_img.as_bytes(0).unwrap();
        // 原 RGBA [255, 0, 0, 255] 经 swap 转换为 BGRA [0, 0, 255, 255]
        assert_eq!(bytes[0], 0);
        assert_eq!(bytes[1], 0);
        assert_eq!(bytes[2], 255);
        assert_eq!(bytes[3], 255);
    }
}
