use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::sync::RwLock;
use tokio_tungstenite::{connect_async_tls_with_config, tungstenite::protocol::Message, Connector};

/// SignalR 消息终止符
const RECORD_SEPARATOR: char = '\x1e';

/// 允许远程查询的 LCU Endpoint 白名单前缀
const ALLOWED_PREFIXES: &[&str] = &[
    "/lol-summoner/",
    "/lol-match-history/",
    "/lol-ranked/",
    "/lol-champ-select/",
    "/lol-gameflow/",
    "/lol-game-queues/",
    "/lol-perks/",
    "/lol-game-data/",
    "/lol-lobby/",
    "/lol-chat/",
    "/lol-matchmaking/",
];

/// 启动 SignalR Hub 连接。
/// 连接到远程服务器的 `/lcuHub`，支持远程 LCU 查询和状态上报。
pub fn start(app_handle: AppHandle, server_url: String, user_id: String) {
    tokio::spawn(async move {
        loop {
            log::info!(
                "正在连接 SignalR Hub: {}/lcuHub?userId={}",
                server_url,
                user_id
            );

            match try_connect(&server_url, &user_id).await {
                Ok(ws_stream) => {
                    log::info!("SignalR Hub 已连接");
                    handle_connection(ws_stream, &app_handle, &user_id).await;
                    log::warn!("SignalR Hub 断开，5 秒后重连");
                }
                Err(e) => {
                    log::warn!("SignalR Hub 连接失败: {}", e);
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    });
}

/// 尝试建立一次 SignalR WebSocket 连接
async fn try_connect(
    server_url: &str,
    user_id: &str,
) -> Result<
    tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    Box<dyn std::error::Error + Send + Sync>,
> {
    let ws_url = format!(
        "{}/lcuHub?userId={}&negotiateVersion=1",
        server_url
            .replace("http://", "ws://")
            .replace("https://", "wss://"),
        user_id
    );

    let native_tls_connector = native_tls::TlsConnector::builder()
        .danger_accept_invalid_certs(true)
        .build()?;
    let tls_connector = Connector::NativeTls(native_tls_connector);

    let request = http::Request::builder().uri(&ws_url).body(())?;

    let (ws_stream, _) =
        connect_async_tls_with_config(request, None, false, Some(tls_connector)).await?;

    Ok(ws_stream)
}

/// 处理 SignalR 连接：握手 → 消息循环
async fn handle_connection(
    ws_stream: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    app_handle: &AppHandle,
    user_id: &str,
) {
    let (mut write, mut read) = ws_stream.split();

    // SignalR JSON 握手
    let handshake = format!(r#"{{"protocol":"json","version":1}}{}"#, RECORD_SEPARATOR);
    if let Err(e) = write.send(Message::Text(handshake.into())).await {
        log::error!("SignalR 握手发送失败: {}", e);
        return;
    }

    // 消息循环
    let mut buffer = String::new();

    while let Some(msg_result) = read.next().await {
        match msg_result {
            Ok(Message::Text(text)) => {
                buffer.push_str(&text);

                // SignalR 消息以 \x1e 终止，可能一次收到多条
                while let Some(end_pos) = buffer.find(RECORD_SEPARATOR) {
                    let msg_str = buffer[..end_pos].to_string();
                    buffer = buffer[end_pos + RECORD_SEPARATOR.len_utf8()..].to_string();

                    if msg_str.trim().is_empty() {
                        continue;
                    }

                    process_signalr_message(&mut write, &msg_str, app_handle, user_id).await;
                }
            }
            Ok(Message::Close(_)) => {
                log::info!("SignalR 收到关闭帧");
                break;
            }
            Err(e) => {
                log::error!("SignalR 读取错误: {}", e);
                break;
            }
            _ => {}
        }
    }
}

/// 处理单条 SignalR 消息
async fn process_signalr_message(
    write: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    msg_str: &str,
    app_handle: &AppHandle,
    user_id: &str,
) {
    let msg: Value = match serde_json::from_str(msg_str) {
        Ok(v) => v,
        Err(e) => {
            log::debug!("SignalR JSON 解析失败: {}", e);
            return;
        }
    };

    let msg_type = msg.get("type").and_then(|v| v.as_i64()).unwrap_or(0);

    match msg_type {
        // Invocation (服务端调用客户端方法)
        1 => {
            let target = msg
                .get("target")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let arguments = msg
                .get("arguments")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let invocation_id = msg.get("invocationId").and_then(|v| v.as_str()).map(|s| s.to_string());

            match target {
                "ReceiveCommand" => {
                    handle_receive_command(write, &arguments, invocation_id, app_handle, user_id)
                        .await;
                }
                other => {
                    log::debug!("SignalR 未知方法: {}", other);
                }
            }
        }
        // Ping → 忽略（SignalR 自动回复）
        6 => {
            log::debug!("SignalR Ping");
        }
        // Close
        7 => {
            log::info!("SignalR 服务端关闭连接");
        }
        _ => {
            log::debug!("SignalR 消息类型: {}", msg_type);
        }
    }
}

/// 处理 ReceiveCommand：白名单校验 → LCU GET → ReportResult 回传
async fn handle_receive_command(
    write: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    arguments: &[Value],
    invocation_id: Option<String>,
    app_handle: &AppHandle,
    user_id: &str,
) {
    // 解析命令数据
    let command_data = match arguments.first() {
        Some(v) if v.is_string() => {
            serde_json::from_str::<Value>(v.as_str().unwrap_or("{}")).unwrap_or(Value::Null)
        }
        Some(v) => v.clone(),
        None => {
            log::warn!("ReceiveCommand: 缺少参数");
            return;
        }
    };

    let endpoint = command_data
        .get("Endpoint")
        .or_else(|| command_data.get("endpoint"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let query_id = arguments
        .get(1)
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // 安全校验
    if !is_endpoint_allowed(endpoint) {
        log::warn!("ReceiveCommand: endpoint 未授权: {}", endpoint);
        send_report(
            write,
            user_id,
            &serde_json::json!({"error": "endpoint 未授权"}).to_string(),
            query_id,
            invocation_id.as_deref(),
        )
        .await;
        return;
    }

    log::info!("ReceiveCommand: 查询 {}", endpoint);

    // 调用本地 LCU（8 秒超时）
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(8),
        lcu_get(app_handle, endpoint),
    )
    .await;

    let payload = match result {
        Ok(Ok(data)) => serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string()),
        Ok(Err(e)) => serde_json::json!({"error": e}).to_string(),
        Err(_) => serde_json::json!({"error": "LCU 请求超时 (8s)"}).to_string(),
    };

    send_report(write, user_id, &payload, query_id, invocation_id.as_deref()).await;
}

/// 路径白名单安全校验（防穿越攻击）
fn is_endpoint_allowed(endpoint: &str) -> bool {
    if endpoint.is_empty() || endpoint.contains("..") || endpoint.contains("//") {
        return false;
    }
    ALLOWED_PREFIXES
        .iter()
        .any(|prefix| endpoint.starts_with(prefix))
}

/// 向 SignalR Hub 发送 ReportResult
async fn send_report(
    write: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    user_id: &str,
    payload: &str,
    query_id: &str,
    invocation_id: Option<&str>,
) {
    let msg = serde_json::json!({
        "type": 1,
        "target": "ReportResult",
        "arguments": [user_id, payload, query_id],
        "invocationId": invocation_id.unwrap_or(""),
    });

    let text = format!("{}{}", msg, RECORD_SEPARATOR);
    if let Err(e) = write.send(Message::Text(text.into())).await {
        log::error!("ReportResult 发送失败: {}", e);
    }
}

/// 执行本地 LCU GET 请求
async fn lcu_get(app_handle: &AppHandle, endpoint: &str) -> Result<Value, String> {
    let state = app_handle.state::<crate::AppState>();
    let lock = state.lcu_client.read().await;
    let lcu = lock.as_ref().ok_or("LCU 未连接")?;

    let url = format!("https://127.0.0.1:{}{}", lcu.port, endpoint);
    let auth = crate::build_auth_header(&lcu.token);

    let resp = lcu
        .http_client
        .get(&url)
        .header("Authorization", auth)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if resp.status().is_success() {
        resp.json::<Value>().await.map_err(|e| e.to_string())
    } else {
        Err(format!("HTTP {}", resp.status()))
    }
}

// ─── 游戏状态上报 ───

/// 上报当前游戏状态给 SignalR Hub（供外部调用）
pub async fn report_game_phase(
    _write: &Arc<RwLock<Option<futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >>>>,
    _user_id: &str,
    phase: &str,
    _summoner_data: Option<&Value>,
) {
    let payload = serde_json::json!({
        "phase": phase,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    });

    log::debug!("上报游戏状态: {}", payload);
}
