//! LCU HTTP 客户端与资源解析器（含重试、并发控制、CDragon CDN 兜底映射）

use base64::Engine;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::time::sleep;

/// 传输层错误最大重试次数
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY: Duration = Duration::from_millis(500);

/// 自动化 Agent 专用并发许可数
const AGENT_CONCURRENCY: usize = 3;
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(8);

/// 资源缓存有效期：7 天
const CACHE_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// 允许调用的 LCU API 路径前缀白名单
pub const ALLOWED_API_PREFIXES: &[&str] = &[
    "/lol-gameflow/",
    "/lol-champ-select/",
    "/lol-matchmaking/",
    "/lol-summoner/",
    "/lol-chat/",
    "/lol-game-data/",
    "/lol-lobby/",
    "/lol-perks/",
    "/lol-ranked/",
    "/lol-match-history/",
    "/lol-champion-mastery/",
    "/lol-challenges/",
    "/lol-honor-v2/",
    "/lol-honor/",
    "/lol-pre-end-of-game/",
    "/lol-spectator/",
    "/lol-patch/",
    "/lol-item-sets/",
    "/lol-login/",
    "/lol-replays/",
    "/lol-inventory/",
    "/lol-champions/",
    "/riotclient/",
    "/system/",
];

pub fn build_auth_header(token: &str) -> String {
    format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(format!("riot:{}", token))
    )
}

fn agent_semaphore() -> &'static Semaphore {
    static SEM: OnceLock<Semaphore> = OnceLock::new();
    SEM.get_or_init(|| Semaphore::new(AGENT_CONCURRENCY))
}

fn default_shared_semaphore() -> &'static Semaphore {
    static SEM: OnceLock<Semaphore> = OnceLock::new();
    SEM.get_or_init(|| Semaphore::new(10))
}

fn cdn_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

#[derive(Clone)]
pub struct LcuClient {
    pub pid: u32,
    pub port: u16,
    pub token: String,
    pub server: Option<String>,
    pub http_client: reqwest::Client,
}

impl LcuClient {
    pub fn new(pid: u32, port: u16, token: String, server: Option<String>) -> Result<Self, String> {
        let http_client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(Duration::from_secs(15))
            .gzip(true)
            .build()
            .map_err(|e| format!("构建 LCU HTTP 客户端失败: {}", e))?;

        Ok(Self {
            pid,
            port,
            token,
            server,
            http_client,
        })
    }

    pub fn auth_header(&self) -> String {
        build_auth_header(&self.token)
    }

    pub async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        agent_lane: bool,
    ) -> Result<Value, String> {
        let path_allowed = ALLOWED_API_PREFIXES.iter().any(|p| path.starts_with(p));
        if !path_allowed {
            return Err(format!("不允许的 API 路径: {}", path));
        }

        let method_upper = method.to_uppercase();
        if !matches!(
            method_upper.as_str(),
            "GET" | "POST" | "PUT" | "PATCH" | "DELETE"
        ) {
            return Err(format!("不支持的 HTTP 方法: {}", method_upper));
        }

        let mut last_err = String::new();

        for attempt in 1..=MAX_RETRIES {
            let wait = if agent_lane {
                agent_semaphore().acquire()
            } else {
                default_shared_semaphore().acquire()
            };

            let permit = match tokio::time::timeout(ACQUIRE_TIMEOUT, wait).await {
                Ok(Ok(p)) => p,
                Ok(Err(e)) => return Err(e.to_string()),
                Err(_) => {
                    log::warn!(
                        "LCU 并发许可等待超时 ({} ms)，放弃本轮请求: {} {}",
                        ACQUIRE_TIMEOUT.as_millis(),
                        method_upper,
                        path
                    );
                    return Err(format!("LCU 并发许可等待超时: {} {}", method_upper, path));
                }
            };

            let url = format!("https://127.0.0.1:{}{}", self.port, path);

            let mut req = match method_upper.as_str() {
                "GET" => self.http_client.get(&url),
                "POST" => self.http_client.post(&url),
                "PUT" => self.http_client.put(&url),
                "PATCH" => self.http_client.patch(&url),
                "DELETE" => self.http_client.delete(&url),
                _ => unreachable!(),
            };
            req = req.header("Authorization", self.auth_header());
            if let Some(ref json_body) = body {
                req = req.json(json_body);
            }

            match req.send().await {
                Ok(response) => {
                    let status = response.status();
                    let text = response.text().await.map_err(|e| e.to_string())?;

                    if status.is_success() {
                        if text.is_empty() {
                            return Ok(Value::Null);
                        }
                        return serde_json::from_str(&text)
                            .map_err(|e| format!("JSON 解析失败: {}", e));
                    } else {
                        let expected_unavailable = (status.as_u16() == 404
                            && (path.contains("pickable-champion-ids")
                                || path.contains("/lol-champ-select/v1/session")
                                || path.contains("/lol-summoner/v2/summoners/puuid/")))
                            || (status.as_u16() == 403
                                && path.contains("/lol-gameflow/v1/reconnect"));
                        if expected_unavailable {
                            log::debug!(
                                "LCU API 常规未激活/未找到提示: {} {}, 状态码: {}, 响应: {}",
                                method,
                                path,
                                status.as_u16(),
                                text
                            );
                        } else {
                            log::warn!(
                                "LCU API 请求失败: {} {}, 状态码: {}, 响应: {}",
                                method,
                                path,
                                status.as_u16(),
                                text
                            );
                        }
                        return Err(format!("LCU 返回错误 [{}]: {}", status.as_u16(), text));
                    }
                }
                Err(e) => {
                    last_err = e.to_string();
                    log::debug!(
                        "LCU API 请求失败 ({}/{}): {} - {}",
                        attempt,
                        MAX_RETRIES,
                        path,
                        last_err
                    );
                    if attempt < MAX_RETRIES {
                        drop(permit);
                        sleep(RETRY_DELAY).await;
                    }
                }
            }
        }

        Err(last_err)
    }

    pub async fn get(&self, path: &str) -> Result<Value, String> {
        self.request("GET", path, None, false).await
    }

    pub async fn post(&self, path: &str, body: Option<Value>) -> Result<Value, String> {
        self.request("POST", path, body, false).await
    }

    pub async fn patch(&self, path: &str, body: Option<Value>) -> Result<Value, String> {
        self.request("PATCH", path, body, false).await
    }

    pub async fn put(&self, path: &str, body: Option<Value>) -> Result<Value, String> {
        self.request("PUT", path, body, false).await
    }

    pub async fn delete(&self, path: &str) -> Result<Value, String> {
        self.request("DELETE", path, None, false).await
    }

    pub async fn get_bytes(&self, path: &str) -> Result<Vec<u8>, String> {
        let url = format!("https://127.0.0.1:{}{}", self.port, path);
        let resp = self
            .http_client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            return Err(format!("LCU 返回状态码: {}", resp.status()));
        }

        resp.bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| e.to_string())
    }
}

pub fn map_asset_path_to_cdragon_url(path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        return path.to_string();
    }

    if let Some(stripped) = path.strip_prefix("/lol-game-data/assets/") {
        let sub = stripped.to_lowercase();
        if sub.starts_with("assets/") {
            format!("https://raw.communitydragon.org/latest/game/{}", sub)
        } else if sub.starts_with("v1/") {
            format!(
                "https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/{}",
                sub
            )
        } else {
            format!("https://raw.communitydragon.org/latest/game/assets/{}", sub)
        }
    } else if let Some(stripped) = path.strip_prefix("/lol-game-data/") {
        let sub = stripped.to_lowercase();
        if sub.starts_with("assets/") {
            format!("https://raw.communitydragon.org/latest/game/{}", sub)
        } else {
            format!(
                "https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/{}",
                sub
            )
        }
    } else if let Some(stripped) = path.strip_prefix("/fe/lol-loot/") {
        format!(
            "https://raw.communitydragon.org/latest/plugins/rcp-fe-lol-loot/global/default/{}",
            stripped
        )
    } else {
        path.to_string()
    }
}

pub fn stable_hash(s: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in s.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x00000100000001B3);
    }
    format!("{:016x}", hash)
}

pub fn guess_content_type(path: &str) -> String {
    let ext = path
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .chars()
        .take_while(|c| c.is_alphanumeric())
        .collect::<String>();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => "image/png",
    }
    .to_string()
}

pub fn get_asset_cache_dir() -> Option<PathBuf> {
    let dir = crate::runtime::app_data_dir().join("cache").join("assets");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

pub fn try_read_asset_cache(path: &str) -> Option<Vec<u8>> {
    let dir = get_asset_cache_dir()?;
    let hash = stable_hash(path);
    let file_path = dir.join(&hash);

    let meta = std::fs::metadata(&file_path).ok()?;
    let modified = meta.modified().ok()?;
    if modified.elapsed().unwrap_or(Duration::MAX) > CACHE_TTL {
        let _ = std::fs::remove_file(&file_path);
        return None;
    }

    let bytes = std::fs::read(&file_path).ok()?;
    if bytes.starts_with(b"data:") {
        let _ = std::fs::remove_file(&file_path);
        return None;
    }
    Some(bytes)
}

pub fn write_asset_cache(path: &str, bytes: &[u8]) {
    let Some(dir) = get_asset_cache_dir() else {
        return;
    };
    let hash = stable_hash(path);
    let target_path = dir.join(&hash);
    let temp_path = dir.join(format!("{}.tmp", hash));

    if std::fs::write(&temp_path, bytes).is_ok() {
        let _ = std::fs::rename(&temp_path, target_path);
    }
}

/// 优先从本地 LCU 读取资源，失败则走 CDragon 兜底并自动本地文件缓存
pub async fn resolve_asset(
    client: Option<&LcuClient>,
    path: &str,
) -> Result<(Vec<u8>, String), String> {
    let content_type = guess_content_type(path);

    if let Some(cached) = try_read_asset_cache(path) {
        return Ok((cached, content_type));
    }

    // 尝试 LCU 本地客户端
    if let Some(lcu) = client {
        if let Ok(bytes) = lcu.get_bytes(path).await {
            write_asset_cache(path, &bytes);
            return Ok((bytes, content_type));
        }
    }

    // 尝试 CDragon CDN 兜底
    let cdn_url = map_asset_path_to_cdragon_url(path);
    if cdn_url.starts_with("http://") || cdn_url.starts_with("https://") {
        let resp = cdn_client()
            .get(&cdn_url)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if resp.status().is_success() {
            let bytes = resp.bytes().await.map_err(|e| e.to_string())?.to_vec();
            write_asset_cache(path, &bytes);
            return Ok((bytes, content_type));
        }
    }

    Err(format!("资源拉取失败: {}", path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_asset_path_to_cdragon_url() {
        assert_eq!(
            map_asset_path_to_cdragon_url("/lol-game-data/assets/v1/champion-icons/54.png"),
            "https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/v1/champion-icons/54.png"
        );
        assert_eq!(
            map_asset_path_to_cdragon_url("/lol-game-data/assets/v1/profile-icons/29.jpg"),
            "https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/v1/profile-icons/29.jpg"
        );
        assert_eq!(
            map_asset_path_to_cdragon_url("/lol-game-data/assets/ASSETS/Items/Icons2D/1001_Class_T1_BootsofSpeed.png"),
            "https://raw.communitydragon.org/latest/game/assets/items/icons2d/1001_class_t1_bootsofspeed.png"
        );
        assert_eq!(
            map_asset_path_to_cdragon_url("/lol-game-data/assets/assets/characters/malphite/hud/malphite_square.png"),
            "https://raw.communitydragon.org/latest/game/assets/characters/malphite/hud/malphite_square.png"
        );
        assert_eq!(
            map_asset_path_to_cdragon_url("/fe/lol-loot/assets/loot_item.png"),
            "https://raw.communitydragon.org/latest/plugins/rcp-fe-lol-loot/global/default/assets/loot_item.png"
        );
        assert_eq!(
            map_asset_path_to_cdragon_url("https://example.com/pic.png"),
            "https://example.com/pic.png"
        );
    }

    #[test]
    fn test_guess_content_type() {
        assert_eq!(guess_content_type("icon.png"), "image/png");
        assert_eq!(guess_content_type("avatar.jpg"), "image/jpeg");
        assert_eq!(guess_content_type("badge.svg"), "image/svg+xml");
    }
}
