use base64::Engine;
use serde_json::Value;
use tauri::State;

use crate::{build_auth_header, AppState};

/// 统一的 LCU API 调用命令。
/// 前端通过 invoke("call_lcu_api", { method, path, body }) 调用。
#[tauri::command]
pub async fn call_lcu_api(
    method: String,
    path: String,
    body: Option<Value>,
    app_state: State<'_, AppState>,
) -> Result<Value, String> {
    let lock = app_state.lcu().await?;
    let lcu = lock.as_ref().unwrap();

    let url = format!("https://127.0.0.1:{}{}", lcu.port, path);

    // Basic Auth: base64("riot:<token>")
    let auth_value = build_auth_header(&lcu.token);

    let mut req = match method.to_uppercase().as_str() {
        "GET" => lcu.http_client.get(&url),
        "POST" => lcu.http_client.post(&url),
        "PUT" => lcu.http_client.put(&url),
        "PATCH" => lcu.http_client.patch(&url),
        "DELETE" => lcu.http_client.delete(&url),
        other => return Err(format!("不支持的 HTTP 方法: {}", other)),
    };

    req = req.header("Authorization", auth_value);

    if let Some(json_body) = body {
        req = req.json(&json_body);
    }

    let response = req.send().await.map_err(|e| e.to_string())?;
    let status = response.status();
    let text = response.text().await.map_err(|e| e.to_string())?;

    if status.is_success() {
        // 部分 LCU 接口成功时不返回 Body
        if text.is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text).map_err(|e| format!("JSON 解析失败: {}", e))
    } else {
        Err(format!(
            "LCU 返回错误 [{}]: {}",
            status.as_u16(),
            text
        ))
    }
}

/// 获取 LCU 静态资源（图片等），返回 data URL。
/// 前端可用于 <img :src="dataUrl">，绕过自签名证书问题。
#[tauri::command]
pub async fn get_lcu_asset(
    path: String,
    app_state: State<'_, AppState>,
) -> Result<String, String> {
    let lock = app_state.lcu().await?;
    let lcu = lock.as_ref().unwrap();

    let mut clean_path = path.clone();
    if let Some(pos) = clean_path.find("/lol-game-data/assets/") {
        let prefix_len = pos + "/lol-game-data/assets/".len();
        if prefix_len < clean_path.len() {
            let (prefix, suffix) = clean_path.split_at(prefix_len);
            clean_path = format!("{}{}", prefix, suffix.to_lowercase());
        }
    }

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
    log::debug!("LCU 资源加载成功: {} ({} bytes, {} chars data-url)", path, bytes.len(), data_url.len());
    Ok(data_url)
}
