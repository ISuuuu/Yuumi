// ─── 腾讯国服 SGP 公共工具 ───
// 战绩查询、观战、TFT 战绩等模块共用的大区白名单、SGP base 构造，
// 以及进程级复用的 SGP HTTP 客户端与带 30 分钟 TTL 的 accessToken 缓存。

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// 腾讯大区白名单（SGP 仅在这些大区可用）
const TENCENT_SERVERS: &[&str] = &[
    "hn1", "hn10", "bgp2", "tj100", "cq100", "gz100", "nj100", "tj101",
];

/// 需要 k8s-sgp 子域名的特殊大区
const K8S_SGP_SERVERS: &[&str] = &["hn1", "hn10", "bgp2"];

/// SGP 单次请求超时
const SGP_TIMEOUT: Duration = Duration::from_secs(15);

/// accessToken 缓存有效期：30 分钟（token 实际约 1 小时过期，留出安全裕量）
const SGP_TOKEN_TTL: Duration = Duration::from_secs(30 * 60);

/// 判断大区是否为腾讯国服（SGP 可用），入参为小写的大区标识
pub fn is_tencent_server(server_lower: &str) -> bool {
    TENCENT_SERVERS.contains(&server_lower)
}

/// 构建 SGP base URL，入参为小写的大区标识
pub fn sgp_base_url(server_lower: &str) -> String {
    if K8S_SGP_SERVERS.contains(&server_lower) {
        format!("https://{}-k8s-sgp.lol.qq.com:21019", server_lower)
    } else {
        format!("https://{}-sgp.lol.qq.com:21019", server_lower)
    }
}

/// 获取进程级复用的 SGP HTTP 客户端（自签证书 + 不走代理 + 超时）
pub fn get_sgp_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .no_proxy()
            .user_agent("RiotClient/78.0.1.1352 (Windows;10;co;red)")
            .timeout(SGP_TIMEOUT)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

/// accessToken 缓存条目（绑定 LCU 端口，LCU 重启端口变化即失效）
struct SgpTokenCache {
    port: u16,
    token: String,
    fetched_at: Instant,
}

static SGP_TOKEN_CACHE: OnceLock<Mutex<Option<SgpTokenCache>>> = OnceLock::new();

/// 获取 SGP 鉴权用 accessToken（/entitlements/v1/token 的 accessToken 字段）。
/// 结果按 LCU 端口缓存 30 分钟，避免每次 SGP 请求都重复获取。
pub async fn get_sgp_token(port: u16, auth: &str) -> Result<String, String> {
    let cache = SGP_TOKEN_CACHE.get_or_init(|| Mutex::new(None));

    // 命中缓存则直接复用（锁仅短暂持有，不在 await 期间持有）
    {
        let guard = cache
            .lock()
            .map_err(|e| format!("SGP token 缓存锁异常: {}", e))?;
        if let Some(entry) = guard.as_ref() {
            if entry.port == port && entry.fetched_at.elapsed() < SGP_TOKEN_TTL {
                return Ok(entry.token.clone());
            }
        }
    }

    let token_url = format!("https://127.0.0.1:{}/entitlements/v1/token", port);
    let token_resp = get_sgp_client()
        .get(&token_url)
        .header("Authorization", auth)
        .send()
        .await
        .map_err(|e| format!("获取 SGP token 失败: {}", e))?;

    if !token_resp.status().is_success() {
        return Err(format!(
            "获取 SGP token 失败: HTTP {}",
            token_resp.status().as_u16()
        ));
    }

    let token_data: serde_json::Value = token_resp
        .json()
        .await
        .map_err(|e| format!("解析 SGP token 失败: {}", e))?;
    let token = token_data
        .get("accessToken")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "SGP token 数据中缺少 accessToken".to_string())?
        .to_string();

    if let Ok(mut guard) = cache.lock() {
        *guard = Some(SgpTokenCache {
            port,
            token: token.clone(),
            fetched_at: Instant::now(),
        });
    }

    Ok(token)
}

/// 清空 SGP token 缓存（LCU 连接/断开时调用，防止使用失效 token）
pub fn clear_sgp_token_cache() {
    if let Some(cache) = SGP_TOKEN_CACHE.get() {
        if let Ok(mut guard) = cache.lock() {
            *guard = None;
        }
    }
}
