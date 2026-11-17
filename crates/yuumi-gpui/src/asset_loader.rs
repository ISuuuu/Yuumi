//! GPUI 图片异步加载引擎与渲染格式适配器
//!
//! 基于 `yuumi_core::lcu::AssetCache`，使用 `image` crate 将原始图片解码为 GPUI `RenderImage` (BGRA)，
//! 并通过 `Arc<RwLock<HashMap<String, Arc<RenderImage>>>>` 在 UI 线程快速获取。

use gpui::{AppContext, RenderImage};
use image::Frame;
use smallvec::SmallVec;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use yuumi_core::lcu::AssetCache;

/// 默认解码图片内存预算：128 MiB（按 BGRA 像素字节计）
const DEFAULT_DECODED_CACHE_BYTES: usize = 128 * 1024 * 1024;
/// 单张图片加载失败后的自动重试冷却时间，防止渲染循环重复触发失败请求
const FAILED_RETRY_COOLDOWN: Duration = Duration::from_secs(10);

struct DecodedCacheEntry {
    image: Arc<RenderImage>,
    bytes: usize,
    seq: u64,
}

struct DecodedImageCache {
    max_bytes: usize,
    total_bytes: usize,
    next_seq: u64,
    entries: HashMap<String, DecodedCacheEntry>,
    lru_queue: VecDeque<(String, u64)>,
}

impl DecodedImageCache {
    fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            total_bytes: 0,
            next_seq: 1,
            entries: HashMap::new(),
            lru_queue: VecDeque::new(),
        }
    }

    fn get(&mut self, key: &str) -> Option<Arc<RenderImage>> {
        let entry = self.entries.get_mut(key)?;
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        entry.seq = seq;
        let image = entry.image.clone();
        self.lru_queue.push_back((key.to_string(), seq));
        Some(image)
    }

    fn insert(&mut self, key: String, image: Arc<RenderImage>) {
        let bytes = image.as_bytes(0).map(|b| b.len()).unwrap_or(0);
        if let Some(old) = self.entries.remove(&key) {
            self.total_bytes = self.total_bytes.saturating_sub(old.bytes);
        }
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        self.total_bytes = self.total_bytes.saturating_add(bytes);
        self.entries
            .insert(key.clone(), DecodedCacheEntry { image, bytes, seq });
        self.lru_queue.push_back((key.clone(), seq));

        // 超出字节预算时按 LRU 淘汰最久未使用的条目（保留刚插入的新条目）
        while self.total_bytes > self.max_bytes && self.entries.len() > 1 {
            let Some((oldest_key, oldest_seq)) = self.lru_queue.pop_front() else {
                break;
            };
            if oldest_key == key {
                continue;
            }
            if let Some(cur) = self.entries.get(&oldest_key) {
                if cur.seq == oldest_seq {
                    if let Some(removed) = self.entries.remove(&oldest_key) {
                        self.total_bytes = self.total_bytes.saturating_sub(removed.bytes);
                    }
                }
            }
        }
    }

    fn stats(&self) -> (usize, usize) {
        (self.entries.len(), self.total_bytes)
    }
}

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
    decoded_cache: Arc<Mutex<DecodedImageCache>>,
    /// 正在加载的资源路径集合：同一资源并发请求只发起一次实际加载（P0-2）
    inflight: Arc<Mutex<HashSet<String>>>,
    /// 最近加载失败的路径冷却时间，避免渲染周期内反复重放失败请求
    failed_recent: Arc<Mutex<HashMap<String, Instant>>>,
}

impl Default for AssetLoader {
    fn default() -> Self {
        Self::new(Arc::new(AssetCache::default()))
    }
}

impl AssetLoader {
    pub fn new(core_cache: Arc<AssetCache>) -> Self {
        Self::with_decoded_budget(core_cache, DEFAULT_DECODED_CACHE_BYTES)
    }

    pub fn with_decoded_budget(core_cache: Arc<AssetCache>, max_decoded_bytes: usize) -> Self {
        Self {
            core_cache,
            decoded_cache: Arc::new(Mutex::new(DecodedImageCache::new(max_decoded_bytes))),
            inflight: Arc::new(Mutex::new(HashSet::new())),
            failed_recent: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 返回当前已解码缓存的 `(条目数, 像素总字节数)`
    #[allow(dead_code)]
    pub fn decoded_cache_stats(&self) -> (usize, usize) {
        self.decoded_cache
            .lock()
            .map(|c| c.stats())
            .unwrap_or((0, 0))
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
        for pixel in rgba.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }

        let frame = Frame::new(rgba);
        let render_img = RenderImage::new(SmallVec::from_elem(frame, 1));
        Ok(Arc::new(render_img))
    }

    pub fn get_builtin_image_bytes(path: &str) -> Option<&'static [u8]> {
        let p = path.trim_start_matches('/');
        match p {
            "images/kills.png" => Some(include_bytes!("../assets/images/kills.png")),
            "images/tower-100.png" => Some(include_bytes!("../assets/images/tower-100.png")),
            "images/tower-200.png" => Some(include_bytes!("../assets/images/tower-200.png")),
            "images/inhibitor-100.png" => {
                Some(include_bytes!("../assets/images/inhibitor-100.png"))
            }
            "images/inhibitor-200.png" => {
                Some(include_bytes!("../assets/images/inhibitor-200.png"))
            }
            "images/baron-100.png" => Some(include_bytes!("../assets/images/baron-100.png")),
            "images/baron-200.png" => Some(include_bytes!("../assets/images/baron-200.png")),
            "images/dragon-100.png" => Some(include_bytes!("../assets/images/dragon-100.png")),
            "images/dragon-200.png" => Some(include_bytes!("../assets/images/dragon-200.png")),
            "images/herald-100.png" => Some(include_bytes!("../assets/images/herald-100.png")),
            "images/herald-200.png" => Some(include_bytes!("../assets/images/herald-200.png")),
            "images/sr-victory.png" => Some(include_bytes!("../assets/images/sr-victory.png")),
            "images/sr-defeat.png" => Some(include_bytes!("../assets/images/sr-defeat.png")),
            "images/ha-victory.png" => Some(include_bytes!("../assets/images/ha-victory.png")),
            "images/ha-defeat.png" => Some(include_bytes!("../assets/images/ha-defeat.png")),
            "images/arena-victory.png" => {
                Some(include_bytes!("../assets/images/arena-victory.png"))
            }
            "images/arena-defeat.png" => Some(include_bytes!("../assets/images/arena-defeat.png")),
            "images/other-victory.png" => {
                Some(include_bytes!("../assets/images/other-victory.png"))
            }
            "images/other-defeat.png" => Some(include_bytes!("../assets/images/other-defeat.png")),
            "assets/ranked-icons/iron.png" => {
                Some(include_bytes!("../assets/ranked-icons/iron.png"))
            }
            "assets/ranked-icons/bronze.png" => {
                Some(include_bytes!("../assets/ranked-icons/bronze.png"))
            }
            "assets/ranked-icons/silver.png" => {
                Some(include_bytes!("../assets/ranked-icons/silver.png"))
            }
            "assets/ranked-icons/gold.png" => {
                Some(include_bytes!("../assets/ranked-icons/gold.png"))
            }
            "assets/ranked-icons/platinum.png" => {
                Some(include_bytes!("../assets/ranked-icons/platinum.png"))
            }
            "assets/ranked-icons/emerald.png" => {
                Some(include_bytes!("../assets/ranked-icons/emerald.png"))
            }
            "assets/ranked-icons/diamond.png" => {
                Some(include_bytes!("../assets/ranked-icons/diamond.png"))
            }
            "assets/ranked-icons/master.png" => {
                Some(include_bytes!("../assets/ranked-icons/master.png"))
            }
            "assets/ranked-icons/grandmaster.png" => {
                Some(include_bytes!("../assets/ranked-icons/grandmaster.png"))
            }
            "assets/ranked-icons/challenger.png" => {
                Some(include_bytes!("../assets/ranked-icons/challenger.png"))
            }
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

        // 纯内存已解码缓存命中，微秒级返回并刷新 LRU 热度
        if let Ok(mut lock) = self.decoded_cache.lock() {
            if let Some(img) = lock.get(trimmed) {
                return Some(img);
            }
        }

        // 内置静态图片在未命中内存缓存时做同步快速解码并写入
        if let Some(builtin) = Self::get_builtin_image_bytes(trimmed) {
            if let Ok(img) = Self::decode_bytes_to_render_image(builtin) {
                if let Ok(mut lock) = self.decoded_cache.lock() {
                    lock.insert(trimmed.to_string(), img.clone());
                }
                return Some(img);
            }
        }

        None
    }

    /// 供组件在缓存未命中（如曾被 LRU 淘汰）时按需触发后台加载，并在完成后刷新窗口
    pub fn schedule_load_if_needed(
        &self,
        path: &str,
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return;
        }
        if let Ok(lock) = self.inflight.lock() {
            if lock.contains(trimmed) {
                return;
            }
        }
        if let Ok(mut failed) = self.failed_recent.lock() {
            if let Some(at) = failed.get(trimmed) {
                if at.elapsed() < FAILED_RETRY_COOLDOWN {
                    return;
                }
                failed.remove(trimmed);
            }
        }
        let loader = self.clone();
        let path_owned = trimmed.to_string();
        let window_handle = window.window_handle();
        cx.spawn(async move |cx| {
            let loaded = if let Some(handle) = yuumi_core::runtime::tokio_handle() {
                let l = loader.clone();
                let p = path_owned.clone();
                handle
                    .spawn(async move { l.load_render_image(&p).await })
                    .await
                    .unwrap_or_else(|e| Err(format!("任务调度失败: {}", e)))
            } else {
                Err("Tokio 运行时未初始化".to_string())
            };
            if loaded.is_ok() {
                let _ = cx.update_window(window_handle, |_, w, _| {
                    w.refresh();
                });
            }
        })
        .detach();
    }

    /// 异步拉取并解码图片（磁盘读 / CPU 解码放 `spawn_blocking` 阻塞池，网络 I/O 留在 Tokio 异步线程）
    pub async fn load_render_image(&self, path: &str) -> Result<Arc<RenderImage>, String> {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return Err("图片路径为空".to_string());
        }

        if let Some(cached) = self.get_cached_render_image(trimmed) {
            return Ok(cached);
        }

        // P0-2 inflight 去重：同一资源并发预热只发起一次实际加载，跟随者等首个请求落缓存
        let is_leader = self.try_mark_inflight(trimmed);
        if !is_leader {
            return self.await_inflight_result(trimmed).await;
        }

        let result = self.load_uncached(trimmed).await;
        // 先落缓存再移出 inflight：跟随者轮询「缓存未命中且 inflight 已空」即判定失败，
        // 此顺序保证不会观察到「既不在缓存也不在加载中」的中间态
        match &result {
            Ok(render_img) => {
                if let Ok(mut lock) = self.decoded_cache.lock() {
                    lock.insert(trimmed.to_string(), render_img.clone());
                }
                if let Ok(mut failed) = self.failed_recent.lock() {
                    failed.remove(trimmed);
                }
            }
            Err(_) => {
                if let Ok(mut failed) = self.failed_recent.lock() {
                    failed.insert(trimmed.to_string(), Instant::now());
                }
            }
        }
        if let Ok(mut inflight) = self.inflight.lock() {
            inflight.remove(trimmed);
        }
        result
    }

    /// 标记资源进入加载队列；已在队列中（他人正在加载）返回 false
    fn try_mark_inflight(&self, path: &str) -> bool {
        self.inflight
            .lock()
            .map(|mut set| set.insert(path.to_string()))
            .unwrap_or(false)
    }

    /// 等待其他任务完成同一资源的加载并取其缓存结果
    async fn await_inflight_result(&self, path: &str) -> Result<Arc<RenderImage>, String> {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            if let Some(cached) = self.get_cached_render_image(path) {
                return Ok(cached);
            }
            let still_loading = self
                .inflight
                .lock()
                .map(|set| set.contains(path))
                .unwrap_or(false);
            if !still_loading {
                // 首个请求已结束但未产出缓存（加载失败）
                return Err(format!("图片加载失败（跟随任务）: {}", path));
            }
        }
    }

    /// 读取原始字节：磁盘缓存优先，未命中再走 LCU / CDragon
    async fn fetch_raw_bytes(&self, path: &str) -> Result<Vec<u8>, String> {
        let core_cache = Arc::clone(&self.core_cache);
        let path_owned = path.to_string();
        // 磁盘缓存读取是阻塞文件 I/O，放阻塞池避免占用 Tokio 异步线程
        let disk_hit =
            tokio::task::spawn_blocking(move || core_cache.try_read_disk_cache(&path_owned))
                .await
                .map_err(|e| format!("后台读取磁盘缓存失败: {}", e))?;

        match disk_hit {
            Some(raw) => Ok(raw),
            None => self.core_cache.fetch_asset_bytes(path).await,
        }
    }

    /// 实际执行磁盘缓存读取、网络拉取与 BGRA 解码（结果不落缓存的裸路径）
    async fn load_uncached(&self, path: &str) -> Result<Arc<RenderImage>, String> {
        let bytes = if let Some(builtin) = Self::get_builtin_image_bytes(path) {
            builtin.to_vec()
        } else {
            // 对照 Vue `LcuImage` 的「失败重试一次」：LCU 刚启动或 CDragon 瞬时 5xx 时可自愈
            let mut last_err = String::new();
            let mut fetched = None;
            for attempt in 0..2 {
                if attempt > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                }
                match self.fetch_raw_bytes(path).await {
                    Ok(raw) => {
                        fetched = Some(raw);
                        break;
                    }
                    Err(e) => last_err = e,
                }
            }
            match fetched {
                Some(raw) => raw,
                None => return Err(last_err),
            }
        };

        // CPU 图像解码放阻塞池，100+ 图标并发预热时不阻塞 Tokio I/O 线程
        tokio::task::spawn_blocking(move || Self::decode_bytes_to_render_image(&bytes))
            .await
            .map_err(|e| format!("后台解码任务失败: {}", e))?
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

    #[test]
    fn decoded_cache_enforces_byte_budget_with_lru_eviction() {
        let mut img_buf = image::RgbaImage::new(4, 4); // 16 * 4 = 64 bytes
        for pixel in img_buf.pixels_mut() {
            *pixel = image::Rgba([10, 20, 30, 255]);
        }
        let mut png_bytes = Vec::new();
        img_buf
            .write_to(
                &mut std::io::Cursor::new(&mut png_bytes),
                image::ImageFormat::Png,
            )
            .unwrap();

        let img = AssetLoader::decode_bytes_to_render_image(&png_bytes).unwrap();
        // 预算最多容纳 2 张（128 bytes）
        let mut cache = DecodedImageCache::new(128);
        cache.insert("a".into(), img.clone());
        cache.insert("b".into(), img.clone());
        assert_eq!(cache.stats(), (2, 128));

        // 命中 "a" 使其变热，再插入 "c" 时应淘汰最久未使用的 "b"
        assert!(cache.get("a").is_some());
        cache.insert("c".into(), img);
        assert_eq!(cache.stats(), (2, 128));
        assert!(cache.get("a").is_some());
        assert!(cache.get("c").is_some());
        assert!(cache.get("b").is_none());
    }
}
