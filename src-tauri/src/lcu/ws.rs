use std::sync::Arc;

use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, Error as TlsError, SignatureScheme};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use tokio_tungstenite::{
    connect_async_tls_with_config,
    tungstenite::{client::ClientRequestBuilder, http, Message},
    Connector,
};

// ── 订阅消息（对齐 Python: [5, "OnJsonApiEvent"]，2 个元素）────────────────
const SUBSCRIBE_MSG: &str = r#"[5, "OnJsonApiEvent"]"#;

/// 前端关心的事件 URI 前缀列表。
/// 等同于 Python LcuWebSocket 里 subscribes 的 uri 过滤。
const WATCHED_URIS: &[&str] = &[
    "/lol-gameflow/v1/gameflow-phase",
    "/lol-champ-select/v1/session",
    "/lol-champ-select/v1/current-champion",
    "/lol-matchmaking/v1/ready-check",
];

// ── 等价于 Python ssl=False：完全不验证任何证书 ───────────────────────────
#[derive(Debug)]
struct NoVerifier;

impl ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        // 接受所有签名算法
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// 启动 LCU WebSocket 连接并在后台持续监听。
/// 对齐 Python: aiohttp.ClientSession.ws_connect(address, ssl=False) → [5, event] 订阅
pub fn connect(app_handle: AppHandle, port: u16, token: String) {
    tauri::async_runtime::spawn(async move {
        loop {
            log::info!("[WS] 正在连接 LCU WebSocket (port={})...", port);

            match try_connect(port, &token).await {
                Ok(ws_stream) => {
                    log::info!("[WS] LCU WebSocket 已连接");
                    let _ = app_handle.emit("lcu-ws-connected", ());
                    handle_messages(ws_stream, &app_handle).await;
                    log::warn!("[WS] LCU WebSocket 断开，2 秒后重连");
                    let _ = app_handle.emit("lcu-ws-disconnected", ());
                }
                Err(e) => {
                    let msg = e.to_string();
                    log::warn!("[WS] 连接失败: {}，2 秒后重试", msg);
                    // 把错误发到前端，方便调试
                    let _ = app_handle.emit("lcu-ws-error", &msg);
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    });
}

/// 建立 WSS 连接。
/// 对齐 Python:
///   session = aiohttp.ClientSession(auth=BasicAuth('riot', token), headers={...})
///   ws = await session.ws_connect(address, ssl=False)
async fn try_connect(
    port: u16,
    token: &str,
) -> Result<
    tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    Box<dyn std::error::Error + Send + Sync>,
> {
    // rustls ClientConfig，NoVerifier = Python ssl=False
    let tls_config = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoVerifier))
        .with_no_client_auth();

    let connector = Connector::Rustls(Arc::new(tls_config));

    // Basic Auth header（对齐 Python: BasicAuth('riot', token)）
    let credentials = format!("riot:{}", token);
    let encoded = base64::engine::general_purpose::STANDARD.encode(&credentials);
    let auth_value = format!("Basic {}", encoded);

    // 使用 ClientRequestBuilder 构造 WebSocket 握手请求，
    // 它会自动生成 Sec-WebSocket-Key / Version / Connection / Upgrade 等必要头，
    // 同时支持添加 Authorization 自定义头。
    // （直接用 Request::builder() 不会自动添加 WebSocket 头，会导致握手失败）
    let url: http::Uri = format!("wss://127.0.0.1:{}/", port).parse()?;
    let request = ClientRequestBuilder::new(url)
        .with_header("Authorization", auth_value)
        .with_header("Content-Type", "application/json")
        .with_header("Accept", "application/json");

    let (ws_stream, _) =
        connect_async_tls_with_config(request, None, false, Some(connector)).await?;

    Ok(ws_stream)
}

/// 处理 WebSocket 消息流。
/// 对齐 Python:
///   await ws.send_json([5, event])   ← 订阅
///   data = json.loads(msg.data)[2]   ← 取第 3 个元素
///   self.matchUri(data)              ← URI 过滤
async fn handle_messages(
    ws_stream: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    app_handle: &AppHandle,
) {
    let (mut write, mut read) = ws_stream.split();

    // 发送订阅（对齐 Python: await ws.send_json([5, event])）
    if let Err(e) = write
        .send(Message::Text(SUBSCRIBE_MSG.to_string().into()))
        .await
    {
        log::error!("[WS] 发送订阅消息失败: {}", e);
        return;
    }
    log::info!("[WS] 已发送订阅: {}", SUBSCRIBE_MSG);

    while let Some(msg_result) = read.next().await {
        match msg_result {
            Ok(Message::Text(text)) => {
                // 对齐 Python: json.loads(msg.data)[2]
                process_event(text.as_str(), app_handle);
            }
            Ok(Message::Close(_)) => {
                log::info!("[WS] 收到关闭帧");
                break;
            }
            Err(e) => {
                log::error!("[WS] 读取错误: {}", e);
                break;
            }
            _ => {}
        }
    }
}

/// 解析 LCU 事件并广播给前端。
/// LCU WAMP 格式: [8, "OnJsonApiEvent_xxx", { "uri": "...", "eventType": "...", "data": ... }]
/// 对齐 Python matchUri：检查 uri 和 eventType
fn process_event(text: &str, app_handle: &AppHandle) {
    let value: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return,
    };

    // 取 arr[2]（对齐 Python: json.loads(msg.data)[2]）
    let arr = match value.as_array() {
        Some(a) if a.len() >= 3 && a[0].as_u64() == Some(8) => a,
        _ => return,
    };

    let event_data = &arr[2];

    let uri = match event_data.get("uri").and_then(|v| v.as_str()) {
        Some(u) => u,
        None => return,
    };

    // 只广播前端关心的 URI（对齐 Python matchUri 的 uri 过滤）
    let should_emit = WATCHED_URIS.iter().any(|prefix| uri.starts_with(prefix));
    if should_emit {
        log::debug!("[WS] 事件: {}", uri);
        let _ = app_handle.emit("lcu-ws-event", event_data.clone());
    }

    // ── 内部 Agent 转发 ──────────────────────────────────────────────────
    let state = app_handle.state::<crate::AppState>();

    if uri.starts_with("/lol-champ-select/v1/session") {
        if let Some(data) = event_data.get("data") {
            if let Ok(session) =
                serde_json::from_value::<crate::agents::auto_bp::ChampSelectSession>(data.clone())
            {
                let _ = state.bp_session_tx.try_send(session);
            }
        }
    }

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
            if let Ok(ready_check) =
                serde_json::from_value::<crate::agents::auto_match::ReadyCheckData>(data.clone())
            {
                let _ = state
                    .gameflow_tx
                    .try_send(crate::agents::auto_match::GameflowEvent::ReadyCheck(
                        ready_check,
                    ));
            }
        }
    }
}
