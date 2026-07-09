use base64::Engine;
use serde_json::Value;
use tauri::State;
use tokio::time::{sleep, Duration};

use crate::{build_auth_header, AppState};

/// 传输层错误的最大重试次数（对应 Python @retry(count=5)）。
/// 仅对连接拒绝/超时等传输层错误重试，HTTP 状态码错误不重试。
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY: Duration = Duration::from_millis(500);

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
    let _permit = app_state
        .api_semaphore
        .acquire()
        .await
        .map_err(|e| e.to_string())?;

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

/// LCU 资源路径白名单前缀
const ASSET_PATH_PREFIX: &str = "/lol-game-data/assets/";
const LOOT_ASSET_PATH_PREFIX: &str = "/fe/lol-loot/assets/";

/// 获取 LCU 静态资源（图片等），返回 data URL。
/// 前端可用于 <img :src="dataUrl">，绕过自签名证书问题。
/// 路径限制：必须以 `/lol-game-data/assets/` 或 `/fe/lol-loot/assets/` 开头。
#[tauri::command]
pub async fn get_lcu_asset(path: String, app_state: State<'_, AppState>) -> Result<String, String> {
    let is_valid = path.starts_with(ASSET_PATH_PREFIX) || path.starts_with(LOOT_ASSET_PATH_PREFIX);
    if !is_valid {
        return Err(format!(
            "不允许的资源路径，必须以 {} 或 {} 开头",
            ASSET_PATH_PREFIX, LOOT_ASSET_PATH_PREFIX
        ));
    }

    let lock = app_state.lcu().await?;
    let lcu = lock.as_ref().unwrap();

    let clean_path = if let Some(stripped) = path.strip_prefix(ASSET_PATH_PREFIX) {
        format!("{}{}", ASSET_PATH_PREFIX, stripped.to_lowercase())
    } else if let Some(stripped) = path.strip_prefix(LOOT_ASSET_PATH_PREFIX) {
        format!("{}{}", LOOT_ASSET_PATH_PREFIX, stripped.to_lowercase())
    } else {
        unreachable!() // validated above
    };
    let url = format!("https://127.0.0.1:{}{}", lcu.port, clean_path);
    let auth = build_auth_header(&lcu.token);

    let resp = lcu
        .http_client
        .get(&url)
        .header("Authorization", auth)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        log::warn!("LCU 资源加载失败 [{}]: {}", path, resp.status());
        return Err(format!("获取资源失败: HTTP {}", resp.status()));
    }

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/png")
        .to_string();

    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let data_url = format!("data:{};base64,{}", content_type, b64);
    log::debug!(
        "LCU 资源加载成功: {} ({} bytes, {} chars data-url)",
        path,
        bytes.len(),
        data_url.len()
    );
    Ok(data_url)
}
