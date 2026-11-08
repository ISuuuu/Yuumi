use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, Error as TlsError, SignatureScheme};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{broadcast, watch};
use tokio_tungstenite::{
    connect_async_tls_with_config,
    tungstenite::{client::ClientRequestBuilder, http, Message},
    Connector,
};

use super::event_projection::{self, EventDedup};

/// 订阅消息：对齐 Python [5, "OnJsonApiEvent"]
pub const SUBSCRIBE_MSG: &str = r#"[5, "OnJsonApiEvent"]"#;

/// 默认 session 事件节流间隔（毫秒）
const SESSION_THROTTLE_MS: u64 = 50;
static SESSION_EPOCH: OnceLock<std::time::Instant> = OnceLock::new();
static LAST_SESSION_TS: AtomicU64 = AtomicU64::new(0);

fn session_throttle_allowed() -> bool {
    let epoch = SESSION_EPOCH.get_or_init(std::time::Instant::now);
    let now = epoch.elapsed().as_micros() as u64;
    let last = LAST_SESSION_TS.load(Ordering::Relaxed);
    if now.saturating_sub(last) < SESSION_THROTTLE_MS * 1000 {
        return false;
    }
    LAST_SESSION_TS
        .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
        .is_ok()
}

/// 默认关心并分发的事件 URI 前缀
pub const WATCHED_URIS: &[&str] = &[
    "/lol-gameflow/v1/gameflow-phase",
    "/lol-gameflow/v1/session",
    "/lol-champ-select/v1/session",
    "/lol-champ-select/v1/current-champion",
    "/lol-matchmaking/v1/ready-check",
    "/lol-honor-v2/v1/ballot",
    "/lol-lobby/v2/received-invitations",
    "/lol-end-of-game/v1/eog-stats-block",
    "/lol-gameflow/v1/watch",
    "/lol-summoner/v1/current-summoner",
];

/// 经过投影与格式化后的 LCU WebSocket 事件数据
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LcuWsEvent {
    pub uri: String,
    pub event_type: String,
    pub data: Value,
}

/// WebSocket 状态机事件 / 生命周期通知
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsLifecycleEvent {
    Connected,
    Disconnected,
    Error(String),
}

/// 完全不验证证书（等价于 Python ssl=False）
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
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// LCU WebSocket 客户端 handle
#[derive(Clone)]
pub struct LcuWsHandle {
    pub event_tx: broadcast::Sender<LcuWsEvent>,
    pub lifecycle_tx: broadcast::Sender<WsLifecycleEvent>,
    cancel_tx: watch::Sender<bool>,
}

impl LcuWsHandle {
    pub fn cancel(&self) {
        let _ = self.cancel_tx.send(true);
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<LcuWsEvent> {
        self.event_tx.subscribe()
    }

    pub fn subscribe_lifecycle(&self) -> broadcast::Receiver<WsLifecycleEvent> {
        self.lifecycle_tx.subscribe()
    }
}

/// 启动 LCU WebSocket 监听并在后台持续重连。
/// 返回 LcuWsHandle 供外部订阅事件或取消连接。
pub fn start_ws(port: u16, token: String) -> LcuWsHandle {
    let (event_tx, _) = broadcast::channel(128);
    let (lifecycle_tx, _) = broadcast::channel(32);
    let (cancel_tx, cancel_rx) = watch::channel(false);

    let handle = LcuWsHandle {
        event_tx: event_tx.clone(),
        lifecycle_tx: lifecycle_tx.clone(),
        cancel_tx,
    };

    tokio::spawn(async move {
        run_ws_loop(port, token, event_tx, lifecycle_tx, cancel_rx).await;
    });

    handle
}

/// 核心重连循环
pub async fn run_ws_loop(
    port: u16,
    token: String,
    event_tx: broadcast::Sender<LcuWsEvent>,
    lifecycle_tx: broadcast::Sender<WsLifecycleEvent>,
    mut cancel_rx: watch::Receiver<bool>,
) {
    loop {
        if *cancel_rx.borrow() {
            log::info!("[WS] 循环已被外部取消");
            return;
        }

        log::info!("[WS] 正在连接 LCU WebSocket (port={})...", port);

        tokio::select! {
            result = try_connect(port, &token) => {
                match result {
                    Ok(ws_stream) => {
                        log::info!("[WS] LCU WebSocket 已连接");
                        let _ = lifecycle_tx.send(WsLifecycleEvent::Connected);

                        tokio::select! {
                            _ = handle_messages(ws_stream, &event_tx) => {}
                            _ = cancel_rx.changed() => {
                                log::info!("[WS] 连接中收到取消信号，断开退出");
                                return;
                            }
                        }

                        log::warn!("[WS] LCU WebSocket 断开，准备重连");
                        let _ = lifecycle_tx.send(WsLifecycleEvent::Disconnected);
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        log::warn!("[WS] 连接失败: {}，2 秒后重试", msg);
                        let _ = lifecycle_tx.send(WsLifecycleEvent::Error(msg));
                    }
                }
            }
            _ = cancel_rx.changed() => {
                log::info!("[WS] 连接前收到取消信号");
                return;
            }
        }

        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {}
            _ = cancel_rx.changed() => {
                log::info!("[WS] 等待重连期间收到取消信号");
                return;
            }
        }
    }
}

/// 尝试建立 TLS WebSocket 连接
pub async fn try_connect(
    port: u16,
    token: &str,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Box<dyn std::error::Error + Send + Sync>,
> {
    static TLS_CONFIG: OnceLock<Arc<ClientConfig>> = OnceLock::new();
    let tls_config = TLS_CONFIG
        .get_or_init(|| {
            let _ = rustls::crypto::ring::default_provider().install_default();
            Arc::new(
                ClientConfig::builder_with_provider(Arc::new(
                    rustls::crypto::ring::default_provider(),
                ))
                .with_safe_default_protocol_versions()
                .expect("初始化安全协议版本失败")
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(NoVerifier))
                .with_no_client_auth(),
            )
        })
        .clone();

    let connector = Connector::Rustls(tls_config);

    let credentials = format!("riot:{}", token);
    let encoded = base64::engine::general_purpose::STANDARD.encode(&credentials);
    let auth_value = format!("Basic {}", encoded);

    let url: http::Uri = format!("wss://127.0.0.1:{}/", port).parse()?;
    let request = ClientRequestBuilder::new(url)
        .with_header("Authorization", auth_value)
        .with_header("Content-Type", "application/json")
        .with_header("Accept", "application/json");

    let (ws_stream, _) =
        connect_async_tls_with_config(request, None, false, Some(connector)).await?;

    Ok(ws_stream)
}

/// 处理 WebSocket 连接读写
async fn handle_messages<S>(mut ws_stream: S, event_tx: &broadcast::Sender<LcuWsEvent>)
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
        + SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error>
        + Unpin,
{
    if let Err(e) = ws_stream
        .send(Message::Text(SUBSCRIBE_MSG.to_string().into()))
        .await
    {
        log::error!("[WS] 发送订阅消息失败: {}", e);
        return;
    }
    log::info!("[WS] 已发送订阅: {}", SUBSCRIBE_MSG);

    let mut dedup = EventDedup::default();

    while let Some(msg_result) = ws_stream.next().await {
        match msg_result {
            Ok(Message::Text(text)) => {
                if let Some(event) = parse_and_project_event(text.as_str(), &mut dedup) {
                    let _ = event_tx.send(event);
                }
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

/// 解析单条 WAMP 文本消息，并进行白名单投影与去重
pub fn parse_and_project_event(text: &str, dedup: &mut EventDedup) -> Option<LcuWsEvent> {
    let value: Value = serde_json::from_str(text).ok()?;
    let arr = value.as_array()?;
    if arr.len() < 3 || arr[0].as_u64() != Some(8) {
        return None;
    }

    let event_data = &arr[2];
    let uri = event_data.get("uri")?.as_str()?;

    // 选人会话节流
    if uri.starts_with("/lol-champ-select/v1/session") && !session_throttle_allowed() {
        return None;
    }

    // 寻找匹配的 URI 前缀
    let matched_prefix = WATCHED_URIS
        .iter()
        .find(|prefix| uri.starts_with(**prefix))?;

    let raw_data = event_data.get("data").unwrap_or(&Value::Null);
    let projected = event_projection::project(matched_prefix, raw_data);

    if !dedup.accept(matched_prefix, &projected) {
        return None;
    }

    let event_type = event_data
        .get("eventType")
        .and_then(|v| v.as_str())
        .unwrap_or("Update")
        .to_string();

    Some(LcuWsEvent {
        uri: uri.to_string(),
        event_type,
        data: projected,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_and_project_event_gameflow_phase() {
        let mut dedup = EventDedup::default();
        let payload = r#"[8, "OnJsonApiEvent_lol_gameflow", {"uri": "/lol-gameflow/v1/gameflow-phase", "eventType": "Update", "data": "ChampSelect"}]"#;

        let event = parse_and_project_event(payload, &mut dedup);
        assert!(event.is_some());
        let ev = event.unwrap();
        assert_eq!(ev.uri, "/lol-gameflow/v1/gameflow-phase");
        assert_eq!(ev.data, json!("ChampSelect"));

        // 重复内容去重
        let event_dup = parse_and_project_event(payload, &mut dedup);
        assert!(event_dup.is_none());

        // 新内容放行
        let payload_new = r#"[8, "OnJsonApiEvent_lol_gameflow", {"uri": "/lol-gameflow/v1/gameflow-phase", "eventType": "Update", "data": "InProgress"}]"#;
        let event_new = parse_and_project_event(payload_new, &mut dedup);
        assert!(event_new.is_some());
    }

    #[test]
    fn test_unwatched_uri_filtered() {
        let mut dedup = EventDedup::default();
        let payload = r#"[8, "OnJsonApiEvent_random", {"uri": "/lol-unrelated/v1/something", "eventType": "Update", "data": 123}]"#;
        let event = parse_and_project_event(payload, &mut dedup);
        assert!(event.is_none());
    }

    #[test]
    fn test_invalid_wamp_format() {
        let mut dedup = EventDedup::default();
        let payload = r#"[5, "Subscribed"]"#;
        assert!(parse_and_project_event(payload, &mut dedup).is_none());
    }
}
