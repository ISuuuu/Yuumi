// ─── OP.GG 公共客户端与缓存 ───
// 数据代理（fetch_opgg_data / fetch_tft_meta_decks）共用的缓存、
// 客户端构建与重试逻辑，避免各处重复实现。

use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::AppState;

const OPGG_CACHE_MAX_ENTRIES: usize = 100;
const OPGG_CACHE_TTL: Duration = Duration::from_secs(600); // 10 分钟
const OPGG_MAX_RETRIES: u32 = 3;
const OPGG_RETRY_DELAY: Duration = Duration::from_millis(500);

struct OpggCacheEntry {
    data: Value,
    inserted_at: Instant,
}

static OPGG_CACHE: OnceLock<Mutex<HashMap<String, OpggCacheEntry>>> = OnceLock::new();

fn get_opgg_cache() -> &'static Mutex<HashMap<String, OpggCacheEntry>> {
    OPGG_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 读取缓存（未过期则返回数据）
pub(crate) fn get_cached(key: &str) -> Option<Value> {
    let cache = get_opgg_cache().lock().ok()?;
    let entry = cache.get(key)?;
    if entry.inserted_at.elapsed() < OPGG_CACHE_TTL {
        log::info!("OP.GG 缓存命中: {}", key);
        Some(entry.data.clone())
    } else {
        None
    }
}

/// 写入缓存（超限时淘汰最旧条目）
pub(crate) fn put_cached(key: String, data: Value) {
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
        log::info!("OP.GG 缓存写入: {}", key);
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
pub fn build_opgg_client(enable_proxy: bool, proxy_addr: &str) -> reqwest::Client {
    let mut builder = reqwest::Client::builder().user_agent(
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

/// 读取 OP.GG 代理配置
async fn opgg_proxy(app_state: &AppState) -> (bool, String) {
    let cfg = app_state.config.read().await;
    (
        cfg.general.enable_opgg_proxy,
        cfg.general.opgg_proxy_addr.clone(),
    )
}

/// GET 请求：带内存缓存与传输层重试（缓存的是原始响应体）
pub(crate) async fn get_json(
    app_state: &AppState,
    url: &str,
    query: &[(&str, &str)],
    cache_key: &str,
) -> Result<Value, String> {
    if let Some(data) = get_cached(cache_key) {
        return Ok(data);
    }

    let (enable_proxy, proxy_addr) = opgg_proxy(app_state).await;
    let client = build_opgg_client(enable_proxy, &proxy_addr);

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
    let (enable_proxy, proxy_addr) = opgg_proxy(app_state).await;
    let client = build_opgg_client(enable_proxy, &proxy_addr);

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
