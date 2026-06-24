use serde_json::Value;
use tauri::State;
use tokio::sync::RwLock;

use crate::{build_auth_header, LcuClient};
use std::sync::Arc;

/// 统一的 LCU API 调用命令。
/// 前端通过 invoke("call_lcu_api", { method, path, body }) 调用。
#[tauri::command]
pub async fn call_lcu_api(
    method: String,
    path: String,
    body: Option<Value>,
    lcu_state: State<'_, Arc<RwLock<Option<LcuClient>>>>,
) -> Result<Value, String> {
    let lock = lcu_state.read().await;
    let lcu = lock
        .as_ref()
        .ok_or("LCU 未连接，请先启动英雄联盟客户端")?;

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
