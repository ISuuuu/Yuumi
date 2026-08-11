use base64::Engine;
use futures_util::stream::{self, StreamExt};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;
use tauri::State;
use tokio::time::sleep;

use crate::{build_auth_header, AppState};

/// 传输层错误的最大重试次数（对应 Python @retry(count=5)）。
/// 仅对连接拒绝/超时等传输层错误重试，HTTP 状态码错误不重试。
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY: Duration = Duration::from_millis(500);

/// 批量资源请求的最大并发数（限制同时进行的 LCU/CDN 请求，避免滑枕大量慢请求）
const ASSET_BATCH_CONCURRENCY: usize = 8;

/// 资源缓存有效期：7 天
const CACHE_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// CDN 兜底下载共用的 HTTP 客户端（进程级复用，避免每次请求重建 TLS 连接）
fn cdn_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

/// 返回资源缓存目录，不存在时自动创建
fn get_asset_cache_dir() -> Option<PathBuf> {
    let dir = dirs::config_dir()?
        .join("Yuumi")
        .join("cache")
        .join("assets");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// FNV-1a 64位稳定哈希算法，保证相同路径在跨平台、跨编译器版本下生成的哈希名一致
fn stable_hash(s: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in s.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x00000100000001B3);
    }
    format!("{:016x}", hash)
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

/// 获取 TFT 本地持久化缓存的绝对路径（兼容并复用 Seraphine 已下载的图片）
fn get_tft_local_cache_path(path: &str) -> Option<PathBuf> {
    let lower_path = path.to_lowercase();

    // TFT 资源路径特征检测：兼容 LCU 和 CDragon 两套命名体系
    let is_tft = lower_path.contains("/tft/")
        || lower_path.contains("tft_champion_icons")
        || lower_path.contains("tft_item_icons")
        || lower_path.contains("tft_trait_icons")
        || lower_path.contains("tft13_")
        || lower_path.contains("tft14_")
        || lower_path.contains("tft15_")
        || lower_path.contains("tft16_")
        || lower_path.contains("tft17_")
        || lower_path.contains("tft9_")
        || lower_path.contains("tft8_")
        || lower_path.contains("/traiticons/")
        || lower_path.contains("trait_icon_");

    if !is_tft {
        return None;
    }

    let file_name = lower_path.split('/').next_back()?.replace(".tex", ".png");
    if file_name.is_empty() {
        return None;
    }

    let sub_folder = if lower_path.contains("champion") || lower_path.contains("championsplashes") {
        "tft_champion_icons"
    } else if lower_path.contains("trait") || lower_path.contains("traiticons") {
        "tft_trait_icons"
    } else {
        // items or augments
        "tft_item_icons"
    };

    let config_dir = dirs::config_dir()?;

    // 1. 优先尝试从 Seraphine 的缓存目录读取已有的图片文件
    let seraphine_path = config_dir
        .join("Seraphine")
        .join("game")
        .join(sub_folder)
        .join(&file_name);
    if seraphine_path.exists() {
        return Some(seraphine_path);
    }

    // 2. 否则，使用 Yuumi 自己的持久化缓存路径
    let yuumi_path = config_dir
        .join("Yuumi")
        .join("game")
        .join(sub_folder)
        .join(&file_name);
    Some(yuumi_path)
}

/// 尝试从文件缓存读取，返回 (data_url, content_type)，过期或不存在则返回 None
fn try_read_asset_cache(path: &str) -> Option<(String, String)> {
    let dir = get_asset_cache_dir()?;
    let hash = stable_hash(path);
    let file_path = dir.join(&hash);

    let meta = std::fs::metadata(&file_path).ok()?;
    let modified = meta.modified().ok()?;
    if modified.elapsed().unwrap_or(Duration::MAX) > CACHE_TTL {
        let _ = std::fs::remove_file(&file_path); // 物理删除过期文件，避免磁盘垃圾无限膨胀
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
    let hash = stable_hash(path);
    let target_path = dir.join(&hash);
    let temp_path = dir.join(format!("{}.tmp", &hash));

    // 先写临时文件，成功后再原子重命名覆盖，规避并发写锁定和文件内容截断损坏风险
    if std::fs::write(&temp_path, data_url).is_ok() {
        let _ = std::fs::rename(&temp_path, target_path);
    }
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
    "/lol-honor-v2/",
    "/lol-honor/",
    "/lol-pre-end-of-game/",
    "/lol-spectator/",
    "/lol-patch/",
    "/riotclient/",
    "/entitlements/",
    "/system/",
];

/// 统一的 LCU API 请求入口（带并发信号量、传输层重试与路径白名单）。
/// 供 Tauri command `call_lcu_api` 与内部 agent（auto_bp/auto_match）复用，
/// 避免各模块各写一套 HTTP 调用。
pub async fn lcu_request(
    app_state: &AppState,
    method: &str,
    path: &str,
    body: Option<Value>,
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

    // 在锁内只提取连接参数（http_client 克隆是 Arc 浅拷贝，代价极低），
    // 尽早释放读锁，避免重试循环期间阻塞 monitor 的重连写锁
    let (port, token, http_client) = {
        let lock = app_state.lcu().await?;
        let lcu = lock.as_ref().unwrap();
        (lcu.port, lcu.token.clone(), lcu.http_client.clone())
    };

    let url = format!("https://127.0.0.1:{}{}", port, path);

    // Basic Auth: base64("riot:<token>")
    let auth_value = build_auth_header(&token);

    let mut last_err = String::new();

    for attempt in 1..=MAX_RETRIES {
        let mut req = match method.to_uppercase().as_str() {
            "GET" => http_client.get(&url),
            "POST" => http_client.post(&url),
            "PUT" => http_client.put(&url),
            "PATCH" => http_client.patch(&url),
            "DELETE" => http_client.delete(&url),
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

/// 等待游戏静态资源（技能/符文/装备图标等）加载完成。
/// 战绩解析依赖资源表拼接图标 URL，资源未就绪时最多轮询等待 2 秒，
/// 供战绩查询、对局分析等解析命令复用。
pub async fn wait_for_game_data(app_state: &AppState) {
    let mut check_count = 0;
    while check_count < 20 {
        {
            let assets = app_state.game_data.read().await;
            if !assets.spells.is_empty() {
                break;
            }
        }
        if app_state.lcu().await.is_err() {
            break;
        }
        sleep(Duration::from_millis(100)).await;
        check_count += 1;
    }
}

/// 统一的 LCU API 调用命令。
/// 前端通过 invoke("call_lcu_api", { method, path, body }) 调用。
#[tauri::command]
pub async fn call_lcu_api(
    method: String,
    path: String,
    body: Option<Value>,
    app_state: State<'_, AppState>,
) -> Result<Value, String> {
    lcu_request(app_state.inner(), &method, &path, body).await
}

/// 获取 LCU 静态资源（图片等），返回 data URL。
/// 前端可用于 <img :src="dataUrl">，绕过自签名证书问题。
/// 路径限制：必须以 `/lol-game-data/assets/`、`/fe/lol-loot/assets/` 或 `http(s)://` CDN 绝对路径开头。
/// 支持 7 天文件缓存，相同资源在缓存有效期内直接返回，无需重复请求。
/// 当 LCU 未开启或资源 404 时，参考 Seraphine 自动降级从 CommunityDragon CDN 下载。
///
/// 这也是批量命令 `get_lcu_assets` 复用的单个资源解析核心，保证取值链路完全一致。
async fn resolve_asset(app_state: &AppState, path: &str) -> Result<String, String> {
    let is_http_cdn = path.starts_with("http://") || path.starts_with("https://");
    let is_lcu_asset = path.starts_with("/lol-game-data/assets/");
    let is_loot_asset = path.starts_with("/fe/lol-loot/assets/");
    if !is_http_cdn && !is_lcu_asset && !is_loot_asset {
        return Err(
            "不允许的资源路径，必须以 /lol-game-data/assets/、/fe/lol-loot/assets/ 或 http(s):// 开头"
                .to_string(),
        );
    }

    // 优先读取 TFT 本地持久化缓存（复用 Seraphine 已缓存的图片文件）。
    // 文件读取移入 spawn_blocking，避免阻塞 tokio 工作线程。
    if let Some(tft_local_path) = get_tft_local_cache_path(path) {
        let tft_owned = tft_local_path.clone();
        let content_type = guess_content_type(path);
        if let Ok(Some(bytes)) = tokio::task::spawn_blocking(move || {
            if tft_owned.exists() {
                std::fs::read(&tft_owned).ok()
            } else {
                None
            }
        })
        .await
        {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            let data_url = format!("data:{};base64,{}", content_type, b64);
            log::debug!("TFT 持久化缓存命中: {:?}", tft_local_path);
            return Ok(data_url);
        }
    }

    // 优先读取文件缓存（spawn_blocking 避免阻塞 tokio 工作线程）
    {
        let path_owned = path.to_string();
        if let Ok(Some((data_url, _))) =
            tokio::task::spawn_blocking(move || try_read_asset_cache(&path_owned)).await
        {
            log::debug!("资源缓存命中: {}", path);
            return Ok(data_url);
        }
    }

    // 1. 尝试优先从本地 LCU 获取
    if is_lcu_asset || is_loot_asset {
        // 锁内只提取连接参数（http_client 克隆是 Arc 浅拷贝），立即释放读锁，
        // 避免跨 HTTP await 持有锁阻塞 monitor 重连写锁
        let (port, token, http_client) = {
            let lcu_guard = app_state.lcu_client.read().await;
            match lcu_guard.as_ref() {
                Some(lcu) => (lcu.port, lcu.token.clone(), lcu.http_client.clone()),
                None => return Err("LCU 未连接".to_string()),
            }
        };
        // 战利品资源（/fe/lol-loot/...）路径区分大小写，直接透传；游戏资源则统一小写
        let clean_path = if is_lcu_asset {
            path.strip_prefix("/lol-game-data/assets/")
                .map(|s| format!("/lol-game-data/assets/{}", s.to_lowercase()))
                .unwrap_or_else(|| path.to_string())
        } else {
            path.to_string()
        };
        let lcu_url = format!("https://127.0.0.1:{}{}", port, clean_path);
        let auth = build_auth_header(&token);

        if let Ok(resp) = http_client
            .get(&lcu_url)
            .header("Authorization", auth)
            .send()
            .await
        {
            if resp.status().is_success() {
                let content_type = resp
                    .headers()
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string())
                    .filter(|s| s.starts_with("image/"))
                    .unwrap_or_else(|| guess_content_type(path));

                if let Ok(bytes) = resp.bytes().await {
                    return Ok(save_asset_and_build_url(path, &content_type, &bytes));
                }
            } else if resp.status().as_u16() == 404
                && is_lcu_asset
                && clean_path.contains("/assets/assets/")
            {
                // 如果双重 assets/assets/ 404，尝试降级为单重 assets/ 再发一次请求
                let retry_path = clean_path.replace("/assets/assets/", "/assets/");
                let retry_url = format!("https://127.0.0.1:{}{}", port, retry_path);
                let auth_retry = build_auth_header(&token);
                if let Ok(retry_resp) = http_client
                    .get(&retry_url)
                    .header("Authorization", auth_retry)
                    .send()
                    .await
                {
                    if retry_resp.status().is_success() {
                        let content_type = retry_resp
                            .headers()
                            .get("content-type")
                            .and_then(|v| v.to_str().ok())
                            .map(|s| s.to_string())
                            .filter(|s| s.starts_with("image/"))
                            .unwrap_or_else(|| guess_content_type(path));

                        if let Ok(bytes) = retry_resp.bytes().await {
                            return Ok(save_asset_and_build_url(path, &content_type, &bytes));
                        }
                    }
                }
            }
        }
    }

    // 2. 备用：LCU 不可用、未连上或返回 404 时，向 CDragon CDN 发起请求（参照 Seraphine 逻辑）
    let client = cdn_client();

    let url = if is_http_cdn {
        path.to_string()
    } else if is_lcu_asset {
        let mut sub_path = path
            .strip_prefix("/lol-game-data/")
            .unwrap_or(path)
            .to_lowercase();
        if sub_path.starts_with("assets/assets/") {
            sub_path = sub_path
                .strip_prefix("assets/")
                .unwrap_or(&sub_path)
                .to_string();
        }
        if sub_path.starts_with("assets/") {
            format!("https://raw.communitydragon.org/latest/game/{}", sub_path)
        } else {
            format!(
                "https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/{}",
                sub_path
            )
        }
    } else {
        // /fe/lol-loot/assets/... → CommunityDragon CDN
        let sub_path = path.strip_prefix("/fe/lol-loot/").unwrap_or(path);
        format!(
            "https://raw.communitydragon.org/latest/plugins/rcp-fe-lol-loot/global/default/{}",
            sub_path
        )
    };

    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        log::warn!("资源加载失败 [{}]: HTTP {}", url, resp.status());
        return Err(format!("获取资源失败: HTTP {}", resp.status()));
    }

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .filter(|s| s.starts_with("image/"))
        .unwrap_or_else(|| guess_content_type(path));

    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    log::debug!("资源从 CDN 加载成功: {} ({} bytes)", path, bytes.len());

    Ok(save_asset_and_build_url(path, &content_type, &bytes))
}

/// 单个资源的 Tauri 命令：调用核心解析逻辑，保持原有取值链路不变。
#[tauri::command]
pub async fn get_lcu_asset(path: String, app_state: State<'_, AppState>) -> Result<String, String> {
    resolve_asset(app_state.inner(), &path).await
}

/// 批量资源请求的单项结果
#[derive(serde::Serialize, serde::Deserialize)]
pub struct AssetItem {
    pub path: String,
    pub data_url: Option<String>,
    pub error: Option<String>,
}

/// 批量获取 LCU 静态资源（图片等），返回每个资源对应的 data URL。
/// 每个路径均复用 `get_lcu_asset` 相同的取值链路（TFT 缓存 → 文件缓存 → LCU → CDragon 兜底），
/// 单个资源的失败不会影响其他资源，可显著减少前端 IPC 往返次数。
#[tauri::command]
pub async fn get_lcu_assets(
    paths: Vec<String>,
    app_state: State<'_, AppState>,
) -> Result<Vec<AssetItem>, String> {
    let app = app_state.inner();
    let results: Vec<AssetItem> = stream::iter(paths)
        .map(|path| async move {
            match resolve_asset(app, &path).await {
                Ok(data_url) => AssetItem {
                    path,
                    data_url: Some(data_url),
                    error: None,
                },
                Err(e) => AssetItem {
                    path,
                    data_url: None,
                    error: Some(e),
                },
            }
        })
        .buffer_unordered(ASSET_BATCH_CONCURRENCY)
        .collect()
        .await;
    Ok(results)
}

fn save_asset_and_build_url(path: &str, content_type: &str, bytes: &[u8]) -> String {
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    let data_url = format!("data:{};base64,{}", content_type, b64);

    let cache_path = path.to_string();
    let cache_data = data_url.clone();
    tauri::async_runtime::spawn_blocking(move || {
        write_asset_cache(&cache_path, &cache_data);
    });

    if let Some(tft_local_path) = get_tft_local_cache_path(path) {
        let bytes_vec = bytes.to_vec();
        tauri::async_runtime::spawn_blocking(move || {
            if let Some(parent) = tft_local_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&tft_local_path, &bytes_vec);
        });
    }

    data_url
}
