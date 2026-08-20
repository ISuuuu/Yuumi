// ─── OP.GG 公共客户端与缓存 ───
// 数据代理（fetch_opgg_data / fetch_tft_meta_decks）共用的缓存、
// 客户端构建与重试逻辑，避免各处重复实现。

use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::AppState;

const OPGG_CACHE_MAX_ENTRIES: usize = 100;
const OPGG_CACHE_TTL: Duration = Duration::from_secs(600); // 10 分钟
const OPGG_DISK_CACHE_TTL: Duration = Duration::from_secs(86400); // 24 小时
const OPGG_MAX_RETRIES: u32 = 3;
const OPGG_RETRY_DELAY: Duration = Duration::from_millis(500);
/// OP.GG 单次请求超时（境外服务常走代理，避免连接挂起导致前端无限转圈）
const OPGG_TIMEOUT: Duration = Duration::from_secs(15);

struct OpggCacheEntry {
    data: Value,
    inserted_at: Instant,
}

static OPGG_CACHE: OnceLock<Mutex<HashMap<String, OpggCacheEntry>>> = OnceLock::new();

fn get_opgg_cache() -> &'static Mutex<HashMap<String, OpggCacheEntry>> {
    OPGG_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// FNV-1a 64位稳定哈希，生成磁盘缓存文件名（与 client.rs 的 stable_hash 同源）
fn cache_file_hash(key: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in key.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x00000100000001B3);
    }
    format!("{:016x}.json", hash)
}

/// OP.GG 磁盘缓存目录（跨启动持久化，app_data/cache/opgg/）
fn disk_cache_dir() -> Option<PathBuf> {
    let dir = crate::runtime::app_data_dir().join("cache").join("opgg");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// 读取磁盘缓存（未过期则返回数据），文件读写走 spawn_blocking 避免阻塞 tokio
pub(crate) async fn get_disk_cached(key: &str) -> Option<Value> {
    let key_owned = key.to_string();
    tokio::task::spawn_blocking(move || {
        let dir = disk_cache_dir()?;
        let file_path = dir.join(cache_file_hash(&key_owned));
        let meta = std::fs::metadata(&file_path).ok()?;
        if meta.modified().ok()?.elapsed().unwrap_or(Duration::MAX) > OPGG_DISK_CACHE_TTL {
            let _ = std::fs::remove_file(&file_path); // 物理删除过期文件
            return None;
        }
        let text = std::fs::read_to_string(&file_path).ok()?;
        serde_json::from_str(&text).ok()
    })
    .await
    .ok()
    .flatten()
}

/// 写入磁盘缓存（原子写：先写 tmp 再 rename）
fn put_disk_cached(key: &str, data: &Value) {
    let key_owned = key.to_string();
    let text = data.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let Some(dir) = disk_cache_dir() else { return };
        let hash = cache_file_hash(&key_owned);
        let target = dir.join(&hash);
        let temp = dir.join(format!("{}.tmp", hash));
        if std::fs::write(&temp, text).is_ok() {
            let _ = std::fs::rename(&temp, &target);
        }
    });
}

/// 读取缓存（内存未命中时回退磁盘缓存并回填内存）
pub(crate) async fn get_cached(key: &str) -> Option<Value> {
    if let Some(data) = get_mem_cached(key) {
        return Some(data);
    }
    if let Some(data) = get_disk_cached(key).await {
        log::debug!("OP.GG 磁盘缓存命中: {}", key);
        put_mem_cached(key.to_string(), data.clone());
        return Some(data);
    }
    None
}

/// 写入缓存（内存 + 磁盘双写）
pub(crate) fn put_cached(key: String, data: Value) {
    put_mem_cached(key.clone(), data.clone());
    put_disk_cached(&key, &data);
}

fn get_mem_cached(key: &str) -> Option<Value> {
    let cache = get_opgg_cache().lock().ok()?;
    let entry = cache.get(key)?;
    if entry.inserted_at.elapsed() < OPGG_CACHE_TTL {
        log::debug!("OP.GG 缓存命中: {}", key);
        Some(entry.data.clone())
    } else {
        None
    }
}

fn put_mem_cached(key: String, data: Value) {
    if let Ok(mut cache) = get_opgg_cache().lock() {
        if cache.len() >= OPGG_CACHE_MAX_ENTRIES {
            if let Some(oldest_key) = cache
                .iter()
                .min_by_key(|(_, e)| e.inserted_at)
                .map(|(k, _)| k.clone())
            {
                cache.remove(&oldest_key);
            }
        }
        log::debug!("OP.GG 缓存写入: {}", key);
        cache.insert(
            key,
            OpggCacheEntry {
                data,
                inserted_at: Instant::now(),
            },
        );
    }
}

/// 构建访问 OP.GG 的 HTTP 客户端（支持可选代理）
fn build_opgg_client(enable_proxy: bool, proxy_addr: &str) -> reqwest::Client {
    let mut builder = reqwest::Client::builder()
        .timeout(OPGG_TIMEOUT)
        .user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        );

    if enable_proxy && !proxy_addr.is_empty() {
        let proxy_url = if proxy_addr.contains("://") {
            proxy_addr.to_string()
        } else {
            format!("http://{}", proxy_addr)
        };
        if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
            builder = builder.proxy(proxy);
            log::info!("OP.GG 请求已配置代理: {}", proxy_url);
        } else {
            log::warn!("无效的 OP.GG 代理地址: {}", proxy_addr);
        }
    }

    builder.build().unwrap_or_else(|_| reqwest::Client::new())
}

/// 已缓存的 OP.GG 客户端（代理配置不变时进程级复用，变更时才重建）
struct OpggClientEntry {
    enable_proxy: bool,
    proxy_addr: String,
    client: reqwest::Client,
}

static OPGG_CLIENT: OnceLock<Mutex<Option<OpggClientEntry>>> = OnceLock::new();

fn get_opgg_client(enable_proxy: bool, proxy_addr: &str) -> reqwest::Client {
    let cache = OPGG_CLIENT.get_or_init(|| Mutex::new(None));
    let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(entry) = guard.as_ref() {
        if entry.enable_proxy == enable_proxy && entry.proxy_addr == proxy_addr {
            return entry.client.clone();
        }
    }
    let client = build_opgg_client(enable_proxy, proxy_addr);
    *guard = Some(OpggClientEntry {
        enable_proxy,
        proxy_addr: proxy_addr.to_string(),
        client: client.clone(),
    });
    client
}

/// 读取全局代理配置
async fn proxy_config(app_state: &AppState) -> (bool, String) {
    let cfg = app_state.config.read().await;
    (
        cfg.general.enable_http_proxy,
        cfg.general.http_proxy_addr.clone(),
    )
}

/// GET 请求：带内存/磁盘缓存与传输层重试（缓存的是原始响应体）
pub(crate) async fn get_json(
    app_state: &AppState,
    url: &str,
    query: &[(&str, &str)],
    cache_key: &str,
) -> Result<Value, String> {
    if let Some(data) = get_cached(cache_key).await {
        return Ok(data);
    }

    let (enable_proxy, proxy_addr) = proxy_config(app_state).await;
    let client = get_opgg_client(enable_proxy, &proxy_addr);

    let mut last_err = String::new();
    for attempt in 1..=OPGG_MAX_RETRIES {
        match client.get(url).query(query).send().await {
            Ok(resp) => {
                let data: Value = resp
                    .json()
                    .await
                    .map_err(|e| format!("解析 OP.GG 响应失败: {}", e))?;
                put_cached(cache_key.to_string(), data.clone());
                return Ok(data);
            }
            Err(e) => {
                last_err = format!("OP.GG 请求失败: {}", e);
                if attempt < OPGG_MAX_RETRIES {
                    log::warn!(
                        "{}，{:.0}s 后重试 ({}/{})",
                        last_err,
                        OPGG_RETRY_DELAY.as_secs_f64(),
                        attempt,
                        OPGG_MAX_RETRIES
                    );
                    tokio::time::sleep(OPGG_RETRY_DELAY).await;
                }
            }
        }
    }
    Err(last_err)
}

/// POST 请求：带传输层重试，不管理缓存（调用方需处理后数据再手动缓存）
pub(crate) async fn post_json(
    app_state: &AppState,
    url: &str,
    body: &Value,
) -> Result<Value, String> {
    let (enable_proxy, proxy_addr) = proxy_config(app_state).await;
    let client = get_opgg_client(enable_proxy, &proxy_addr);

    let mut last_err = String::new();
    for attempt in 1..=OPGG_MAX_RETRIES {
        match client.post(url).json(body).send().await {
            Ok(resp) => {
                return resp
                    .json()
                    .await
                    .map_err(|e| format!("解析 OP.GG 响应失败: {}", e));
            }
            Err(e) => {
                last_err = format!("OP.GG MCP 请求失败: {}", e);
                if attempt < OPGG_MAX_RETRIES {
                    log::warn!(
                        "{}，{:.0}s 后重试 ({}/{})",
                        last_err,
                        OPGG_RETRY_DELAY.as_secs_f64(),
                        attempt,
                        OPGG_MAX_RETRIES
                    );
                    tokio::time::sleep(OPGG_RETRY_DELAY).await;
                }
            }
        }
    }
    Err(last_err)
}
