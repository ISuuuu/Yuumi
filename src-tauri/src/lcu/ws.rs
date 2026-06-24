use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async_tls_with_config, tungstenite::protocol::Message, Connector, MaybeTlsStream, WebSocketStream};

const WS_SUBSCRIBE_MSG: &str = r#"[5, "OnJsonApiEvent"]"#;

/// 前端关心的核心事件 URI 前缀列表。
/// 仅这些路径会被推送给前端，避免高频无关事件淹没。
const WATCHED_URIS: &[&str] = &[
    "/lol-gameflow/v1/gameflow-phase",
    "/lol-champ-select/v1/session",
    "/lol-champ-select/v1/current-champion",
    "/lol-matchmaking/v1/ready-check",
];

/// 建立 LCU WebSocket 连接并在后台监听事件。
/// 每当收到匹配的事件，通过 Tauri emit 推送给前端。
pub fn connect(app_handle: AppHandle, port: u16, token: String) {
    tokio::spawn(async move {
        loop {
            log::info!("正在连接 LCU WebSocket (port={})...", port);

            match try_connect(port, &token).await {
                Ok(ws_stream) => {
                    log::info!("LCU WebSocket 已连接");
                    let _ = app_handle.emit("lcu-ws-connected", ());
                    handle_messages(ws_stream, &app_handle).await;
                    log::warn!("LCU WebSocket 断开，将在 2 秒后重连");
                    let _ = app_handle.emit("lcu-ws-disconnected", ());
                }
                Err(e) => {
                    log::warn!("LCU WebSocket 连接失败: {}，将在 2 秒后重连", e);
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    });
}

/// 尝试建立一次 WebSocket 连接
async fn try_connect(
    port: u16,
    token: &str,
) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, Box<dyn std::error::Error + Send + Sync>>
{
    let ws_url = format!("wss://127.0.0.1:{}/", port);

    // 忽略 SSL 校验的 TLS connector
    let native_tls_connector = native_tls::TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .build()?;
    let tls_connector = Connector::NativeTls(native_tls_connector);

    // Basic Auth header
    let credentials = format!("riot:{}", token);
    let encoded = base64::engine::general_purpose::STANDARD.encode(&credentials);
    let auth_value = format!("Basic {}", encoded);

    let request = http::Request::builder()
        .uri(&ws_url)
        .header("Authorization", auth_value)
        .body(())?;

    let (ws_stream, _) =
        connect_async_tls_with_config(request, None, false, Some(tls_connector)).await?;

    Ok(ws_stream)
}

/// 处理 WebSocket 消息流：订阅事件、过滤并广播
async fn handle_messages(
    ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    app_handle: &AppHandle,
) {
    let (mut write, mut read) = ws_stream.split();

    // 订阅 OnJsonApiEvent
    if let Err(e) = write.send(Message::Text(WS_SUBSCRIBE_MSG.into())).await {
        log::error!("发送订阅消息失败: {}", e);
        return;
    }

    while let Some(msg_result) = read.next().await {
        match msg_result {
            Ok(Message::Text(text)) => {
                process_event(&text, app_handle);
            }
            Ok(Message::Close(_)) => {
                log::info!("WebSocket 收到关闭帧");
                break;
            }
            Err(e) => {
                log::error!("WebSocket 读取错误: {}", e);
                break;
            }
            _ => {}
        }
    }
}

/// 解析 LCU WebSocket 事件，过滤后广播给前端。
/// LCU WAMP 格式: [8, { "uri": "/...", "data": ... }, ...]
fn process_event(text: &str, app_handle: &AppHandle) {
    let value: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return,
    };

    // WAMP 事件格式: [8, { "uri": "...", "data": ... }]
    let arr = match value.as_array() {
        Some(a) if a.len() >= 3 && a[0].as_u64() == Some(8) => a,
        _ => return,
    };

    let event_data = &arr[2];

    let uri = match event_data.get("uri").and_then(|v| v.as_str()) {
        Some(u) => u,
        None => return,
    };

    // 只广播前端关心的事件
    let should_emit = WATCHED_URIS.iter().any(|prefix| uri.starts_with(prefix));

    if should_emit {
        log::debug!("LCU 事件: {}", uri);
        let _ = app_handle.emit("lcu-ws-event", event_data.clone());
    }

    // 选人会话事件：解析并转发给 BP Agent
    if uri.starts_with("/lol-champ-select/v1/session") {
        if let Some(data) = event_data.get("data") {
            match serde_json::from_value::<crate::agents::auto_bp::ChampSelectSession>(data.clone())
            {
                Ok(session) => {
                    let state = app_handle.state::<crate::AppState>();
                    // 非阻塞发送，channel 满时丢弃旧数据
                    let _ = state.bp_session_tx.try_send(session);
                }
                Err(e) => {
                    log::debug!("解析选人会话数据失败: {}", e);
                }
            }
        }
    }

    // 游戏流程事件：转发给 auto_match Agent
    let state = app_handle.state::<crate::AppState>();

    if uri.starts_with("/lol-gameflow/v1/gameflow-phase") {
        if let Some(phase) = event_data.get("data").and_then(|v| v.as_str()) {
            let _ = state
                .gameflow_tx
                .try_send(crate::agents::auto_match::GameflowEvent::PhaseChanged(
                    phase.to_string(),
                ));
        }
    }

    if uri.starts_with("/lol-matchmaking/v1/ready-check") {
        if let Some(data) = event_data.get("data") {
            match serde_json::from_value::<crate::agents::auto_match::ReadyCheckData>(data.clone())
            {
                Ok(ready_check) => {
                    let _ = state
                        .gameflow_tx
                        .try_send(crate::agents::auto_match::GameflowEvent::ReadyCheck(
                            ready_check,
                        ));
                }
                Err(e) => {
                    log::debug!("解析 ReadyCheck 数据失败: {}", e);
                }
            }
        }
    }
}
