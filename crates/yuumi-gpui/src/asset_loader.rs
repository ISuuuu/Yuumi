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

    /// 同步快速查询已解码缓存（若已解码命中则直接返回供渲染）
    pub fn get_cached_render_image(&self, path: &str) -> Option<Arc<RenderImage>> {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return None;
        }

        // 1. 已解码缓存命中
        if let Ok(lock) = self.decoded_cache.read() {
            if let Some(img) = lock.get(trimmed) {
                return Some(img.clone());
            }
        }

        // 2. 磁盘或内存有原始字节，同步尝试快速解码
        if let Some(raw_bytes) = self.core_cache.try_read_disk_cache(trimmed) {
            if let Ok(render_img) = Self::decode_bytes_to_render_image(&raw_bytes) {
                if let Ok(mut lock) = self.decoded_cache.write() {
                    lock.insert(trimmed.to_string(), render_img.clone());
                }
                return Some(render_img);
            }
        }

        None
    }

    /// 异步拉取并解码图片
    pub async fn load_render_image(&self, path: &str) -> Result<Arc<RenderImage>, String> {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return Err("图片路径为空".to_string());
        }

        if let Some(cached) = self.get_cached_render_image(trimmed) {
            return Ok(cached);
        }

        let bytes = self.core_cache.fetch_asset_bytes(trimmed).await?;
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
