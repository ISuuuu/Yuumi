use base64::Engine;
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;
use tauri::State;
use tokio::time::sleep;

use crate::{build_auth_header, AppState};

/// 传输层错误的最大重试次数（对应 Python @retry(count=5)）。
/// 仅对连接拒绝/超时等传输层错误重试，HTTP 状态码错误不重试。
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY: Duration = Duration::from_millis(500);

/// 资源缓存有效期：7 天
const CACHE_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// 返回资源缓存目录，不存在时自动创建
fn get_asset_cache_dir() -> Option<PathBuf> {
    let dir = dirs::config_dir()?
        .join("Yuumi")
        .join("cache")
        .join("assets");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// 根据 URL 或路径中的扩展名猜测 content-type
fn guess_content_type(path: &str) -> String {
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

/// 尝试从文件缓存读取，返回 (data_url, content_type)，过期或不存在则返回 None
fn try_read_asset_cache(path: &str) -> Option<(String, String)> {
    let dir = get_asset_cache_dir()?;
    // 使用路径的 hash 作为文件名，避免路径分隔符问题
    let hash = {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    };
    let file_path = dir.join(&hash);

    let meta = std::fs::metadata(&file_path).ok()?;
    let modified = meta.modified().ok()?;
    if modified.elapsed().unwrap_or(Duration::MAX) > CACHE_TTL {
        return None;
    }

    let data_url = std::fs::read_to_string(&file_path).ok()?;
    if data_url.starts_with("data:") {
        let content_type = data_url
            .split(';')
            .next()
            .unwrap_or("data:image/png")
            .trim_start_matches("data:")
            .to_string();
        Some((data_url, content_type))
    } else {
        None
    }
}

/// 将 data URL 写入文件缓存
fn write_asset_cache(path: &str, data_url: &str) {
    let Some(dir) = get_asset_cache_dir() else {
        return;
    };
    let hash = {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    };
    let _ = std::fs::write(dir.join(&hash), data_url);
}

/// 允许前端调用的 LCU API 路径前缀白名单
const ALLOWED_API_PREFIXES: &[&str] = &[
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
    "/lol-spectator/",
    "/riotclient/",
    "/entitlements/",
    "/system/",
];

/// 统一的 LCU API 调用命令。
/// 前端通过 invoke("call_lcu_api", { method, path, body }) 调用。
#[tauri::command]
pub async fn call_lcu_api(
    method: String,
    path: String,
    body: Option<Value>,
    app_state: State<'_, AppState>,
) -> Result<Value, String> {
    // 路径前缀白名单校验
    let path_allowed = ALLOWED_API_PREFIXES.iter().any(|p| path.starts_with(p));
    if !path_allowed {
        return Err(format!("不允许的 API 路径: {}", path));
    }

    // 获取并发许可
    let semaphore = {
        let lock = app_state.api_semaphore.read().await;
        lock.clone()
    };
    let _permit = semaphore.acquire().await.map_err(|e| e.to_string())?;

    let lock = app_state.lcu().await?;
    let lcu = lock.as_ref().unwrap();

    let url = format!("https://127.0.0.1:{}{}", lcu.port, path);

    // Basic Auth: base64("riot:<token>")
    let auth_value = build_auth_header(&lcu.token);

    let mut last_err = String::new();

    for attempt in 1..=MAX_RETRIES {
        let mut req = match method.to_uppercase().as_str() {
            "GET" => lcu.http_client.get(&url),
            "POST" => lcu.http_client.post(&url),
            "PUT" => lcu.http_client.put(&url),
            "PATCH" => lcu.http_client.patch(&url),
            "DELETE" => lcu.http_client.delete(&url),
            other => return Err(format!("不支持的 HTTP 方法: {}", other)),
        };

        req = req.header("Authorization", &auth_value);

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
                    // 对于非选人阶段请求选人接口产生的常规 404 返回，降级为 debug 日志防止刷屏爆红
                    if status.as_u16() == 404
                        && (path.contains("pickable-champion-ids")
                            || path.contains("/lol-champ-select/v1/session"))
                    {
                        log::debug!(
                            "LCU API 常规未激活提示: {} {}, 状态码: {}, 响应: {}",
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
                    sleep(RETRY_DELAY).await;
                }
            }
        }
    }

    Err(last_err)
}

/// 获取 LCU 静态资源（图片等），返回 data URL。
/// 前端可用于 <img :src="dataUrl">，绕过自签名证书问题。
/// 路径限制：必须以 `/lol-game-data/assets/` 或 `/fe/lol-loot/assets/` 开头。
/// 支持 7 天文件缓存，相同资源在缓存有效期内直接返回，无需重复请求。
#[tauri::command]
pub async fn get_lcu_asset(path: String, app_state: State<'_, AppState>) -> Result<String, String> {
    let is_lcu_asset = path.starts_with("/lol-game-data/assets/");
    let is_loot_asset = path.starts_with("/fe/lol-loot/assets/");
    if !is_lcu_asset && !is_loot_asset {
        return Err(
            "不允许的资源路径，必须以 /lol-game-data/assets/ 或 /fe/lol-loot/assets/ 开头"
                .to_string(),
        );
    }

    // 优先读取文件缓存
    if let Some((data_url, _)) = try_read_asset_cache(&path) {
        log::debug!("LCU 资源缓存命中: {}", path);
        return Ok(data_url);
    }

    let lock = app_state.lcu().await?;
    let lcu = lock.as_ref().unwrap();

    // 根据路径类型决定请求 URL：LCU 资源走本地代理，战利品资源走 CommunityDragon CDN
    let url = if is_lcu_asset {
        let clean_path = path
            .strip_prefix("/lol-game-data/assets/")
            .map(|s| format!("/lol-game-data/assets/{}", s.to_lowercase()))
            .unwrap_or(path.clone());
        format!("https://127.0.0.1:{}{}", lcu.port, clean_path)
    } else {
        // /fe/lol-loot/assets/... → CommunityDragon CDN
        let sub_path = path.strip_prefix("/fe/lol-loot/").unwrap_or(&path);
        format!(
            "https://raw.communitydragon.org/latest/plugins/rcp-fe-lol-loot/global/default/{}",
            sub_path
        )
    };

    let resp = if is_lcu_asset {
        let auth = build_auth_header(&lcu.token);
        lcu.http_client
            .get(&url)
            .header("Authorization", auth)
            .send()
            .await
            .map_err(|e| e.to_string())?
    } else {
        // CommunityDragon CDN 无需认证
        lcu.http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?
    };

    if !resp.status().is_success() {
        log::warn!("LCU 资源加载失败 [{}]: {}", path, resp.status());
        return Err(format!("获取资源失败: HTTP {}", resp.status()));
    }

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .filter(|s| s.starts_with("image/"))
        .unwrap_or_else(|| guess_content_type(&path));

    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let data_url = format!("data:{};base64,{}", content_type, b64);
    log::debug!(
        "LCU 资源加载成功: {} ({} bytes, {} chars data-url)",
        path,
        bytes.len(),
        data_url.len()
    );

    // 异步写入缓存（不阻塞返回）
    let cache_path = path.clone();
    let cache_data = data_url.clone();
    tokio::spawn(async move {
        write_asset_cache(&cache_path, &cache_data);
    });

    Ok(data_url)
}
