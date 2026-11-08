//! LCU 静态图片资源纯 Rust 异步加载与多级缓存引擎
//!
//! 提供内存缓存 + 本地磁盘缓存 + LCU 本地优先 / CDragon CDN 兜底映射。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use crate::lcu::client::{map_asset_path_to_cdragon_url, stable_hash, LcuClient};
use crate::runtime;

/// 默认最大内存缓存项数
const DEFAULT_MEMORY_CACHE_CAPACITY: usize = 256;

#[derive(Clone)]
pub struct AssetCache {
    /// 可选的活跃 LCU 客户端引用
    lcu_client: Option<LcuClient>,
    /// 自定义磁盘缓存根目录（若未指定则使用系统默认 data/cache/assets）
    custom_cache_dir: Option<PathBuf>,
    /// 内存缓存：asset path -> raw bytes
    memory_cache: Arc<RwLock<HashMap<String, Arc<Vec<u8>>>>>,
    /// 内存缓存容量上限
    memory_capacity: usize,
    /// 共享的 HTTP CDN 客户端
    http_client: reqwest::Client,
}

impl Default for AssetCache {
    fn default() -> Self {
        Self::new(None)
    }
}

impl AssetCache {
    /// 创建一个新的资源缓存引擎
    pub fn new(lcu_client: Option<LcuClient>) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .gzip(true)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            lcu_client,
            custom_cache_dir: None,
            memory_cache: Arc::new(RwLock::new(HashMap::new())),
            memory_capacity: DEFAULT_MEMORY_CACHE_CAPACITY,
            http_client,
        }
    }

    /// 使用指定磁盘缓存目录构造（便于测试与隔离）
    pub fn with_cache_dir(lcu_client: Option<LcuClient>, cache_dir: PathBuf) -> Self {
        let mut cache = Self::new(lcu_client);
        cache.custom_cache_dir = Some(cache_dir);
        cache
    }

    /// 获取磁盘缓存目录
    pub fn cache_dir(&self) -> PathBuf {
        if let Some(dir) = &self.custom_cache_dir {
            dir.clone()
        } else {
            runtime::app_data_dir().join("cache").join("assets")
        }
    }

    /// 获取特定路径对应的磁盘缓存文件路径
    pub fn cache_file_path(&self, path: &str) -> PathBuf {
        let hash = stable_hash(path);
        self.cache_dir().join(hash)
    }

    /// 尝试从内存读取
    pub fn get_from_memory(&self, path: &str) -> Option<Arc<Vec<u8>>> {
        self.memory_cache.read().ok()?.get(path).cloned()
    }

    /// 写入内存缓存
    pub fn put_to_memory(&self, path: &str, bytes: Arc<Vec<u8>>) {
        if let Ok(mut lock) = self.memory_cache.write() {
            if lock.len() >= self.memory_capacity && !lock.contains_key(path) {
                // 简单剔除第一个键防止内存无限制增长
                if let Some(first_key) = lock.keys().next().cloned() {
                    lock.remove(&first_key);
                }
            }
            lock.insert(path.to_string(), bytes);
        }
    }

    /// 尝试从磁盘缓存读取原始字节
    pub fn try_read_disk_cache(&self, path: &str) -> Option<Vec<u8>> {
        let file_path = self.cache_file_path(path);
        let bytes = std::fs::read(&file_path).ok()?;
        // 校验非损坏的临时或旧 data: 脏数据
        if bytes.is_empty() || bytes.starts_with(b"data:") {
            let _ = std::fs::remove_file(&file_path);
            return None;
        }
        Some(bytes)
    }

    /// 写入磁盘缓存（原子写入防截断）
    pub fn write_disk_cache(&self, path: &str, bytes: &[u8]) -> std::io::Result<PathBuf> {
        let dir = self.cache_dir();
        std::fs::create_dir_all(&dir)?;

        let hash = stable_hash(path);
        let target_path = dir.join(&hash);
        let temp_path = dir.join(format!("{}.{}.tmp", hash, std::process::id()));

        std::fs::write(&temp_path, bytes)?;
        std::fs::rename(&temp_path, &target_path)?;

        Ok(target_path)
    }

    /// 异步加载资源原始字节
    /// 优先级：内存 LRU -> 本地磁盘缓存 -> LCU 本地 HTTP -> CDragon CDN 兜底映射
    pub async fn fetch_asset_bytes(&self, path: &str) -> Result<Vec<u8>, String> {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return Err("资源路径为空".to_string());
        }

        // 1. 内存缓存检查
        if let Some(mem_bytes) = self.get_from_memory(trimmed) {
            return Ok((*mem_bytes).clone());
        }

        // 2. 本地磁盘缓存检查
        if let Some(disk_bytes) = self.try_read_disk_cache(trimmed) {
            let arc_bytes = Arc::new(disk_bytes.clone());
            self.put_to_memory(trimmed, arc_bytes);
            return Ok(disk_bytes);
        }

        // 3. 尝试从活动 LCU 客户端拉取
        if let Some(lcu) = &self.lcu_client {
            if trimmed.starts_with('/') {
                if let Ok(bytes) = lcu.get_bytes(trimmed).await {
                    let _ = self.write_disk_cache(trimmed, &bytes);
                    let arc_bytes = Arc::new(bytes.clone());
                    self.put_to_memory(trimmed, arc_bytes);
                    return Ok(bytes);
                }
            }
        }

        // 4. CDragon CDN 兜底映射拉取
        let cdn_url = map_asset_path_to_cdragon_url(trimmed);
        if cdn_url.starts_with("http://") || cdn_url.starts_with("https://") {
            let resp = self
                .http_client
                .get(&cdn_url)
                .send()
                .await
                .map_err(|e| format!("CDN 网络请求失败 ({}): {}", cdn_url, e))?;

            if resp.status().is_success() {
                let bytes = resp
                    .bytes()
                    .await
                    .map_err(|e| format!("CDN 读取流失败: {}", e))?
                    .to_vec();

                let _ = self.write_disk_cache(trimmed, &bytes);
                let arc_bytes = Arc::new(bytes.clone());
                self.put_to_memory(trimmed, arc_bytes);
                return Ok(bytes);
            } else {
                return Err(format!(
                    "CDN 响应状态码异常 ({}): {}",
                    resp.status(),
                    cdn_url
                ));
            }
        }

        Err(format!("无法解析并加载资源: {}", path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_cache_disk_write_and_read() {
        let temp_dir = std::env::temp_dir().join(format!("yuumi_asset_test_{}", std::process::id()));
        let cache = AssetCache::with_cache_dir(None, temp_dir.clone());

        let test_path = "/lol-game-data/assets/v1/champion-icons/1.png";
        let test_data = b"FAKE_PNG_BINARY_BYTES_123456";

        // 初始不存在
        assert!(cache.try_read_disk_cache(test_path).is_none());

        // 写入磁盘
        let saved_path = cache.write_disk_cache(test_path, test_data).unwrap();
        assert!(saved_path.exists());

        // 读取磁盘验证
        let read_bytes = cache.try_read_disk_cache(test_path).expect("应成功读取");
        assert_eq!(read_bytes.as_slice(), test_data);

        // 验证内存缓存写入与读取
        cache.put_to_memory(test_path, Arc::new(read_bytes));
        let mem = cache.get_from_memory(test_path).expect("内存应命中");
        assert_eq!(mem.as_slice(), test_data);

        // 清理测试目录
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_asset_cache_fetch_disk_hit() {
        let temp_dir =
            std::env::temp_dir().join(format!("yuumi_asset_test_fetch_{}", std::process::id()));
        let cache = AssetCache::with_cache_dir(None, temp_dir.clone());

        let test_path = "/lol-game-data/assets/v1/profile-icons/29.jpg";
        let test_data = b"FAKE_JPG_DATA";

        cache.write_disk_cache(test_path, test_data).unwrap();

        let loaded = cache.fetch_asset_bytes(test_path).await.unwrap();
        assert_eq!(loaded.as_slice(), test_data);

        // 清理
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_asset_cache_empty_path_error() {
        let cache = AssetCache::default();
        let err = cache.fetch_asset_bytes("   ").await.unwrap_err();
        assert!(err.contains("为空"));
    }
}
