use futures_util::{SinkExt, StreamExt};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, Error as TlsError, SignatureScheme};
use serde_json::Value;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::{
    connect_async_tls_with_config,
    tungstenite::client::ClientRequestBuilder,
    tungstenite::protocol::Message,
    Connector,
};

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
        &self, _message: &[u8], _cert: &CertificateDer<'_>, _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self, _message: &[u8], _cert: &CertificateDer<'_>, _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

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

pub enum SignalrCommand {
    SendEvent {
        event_type: String,
        data: serde_json::Value,
    },
    SendRaw(Message),
}

static SIGNALR_TX: tokio::sync::Mutex<Option<mpsc::Sender<SignalrCommand>>> =
    tokio::sync::Mutex::const_new(None);

static SIGNALR_CANCEL_TX: tokio::sync::Mutex<Option<watch::Sender<bool>>> =
    tokio::sync::Mutex::const_new(None);

static CURRENT_SUMMONER_NAME: tokio::sync::Mutex<String> =
    tokio::sync::Mutex::const_new(String::new());

/// 停止 SignalR Hub 连接
pub async fn stop() {
    let mut cancel_lock = SIGNALR_CANCEL_TX.lock().await;
    if let Some(tx) = cancel_lock.take() {
        let _ = tx.send(true);
        log::info!("[SignalR] 已向后台任务发送停止信号");
    }
    let mut tx_lock = SIGNALR_TX.lock().await;
    *tx_lock = None;
}

/// 获取当前连接状态
#[tauri::command]
pub async fn get_signalr_status() -> Result<String, String> {
    let tx_lock = SIGNALR_TX.lock().await;
    if tx_lock.is_some() {
        Ok("connected".to_string())
    } else {
        let cancel_lock = SIGNALR_CANCEL_TX.lock().await;
        if cancel_lock.is_some() {
            Ok("connecting".to_string())
        } else {
            Ok("disconnected".to_string())
        }
    }
}

fn clean_server_url(url: &str) -> String {
    let mut clean = url.trim().to_string();
    loop {
        let before = clean.clone();
        clean = clean.trim_end_matches('/').to_string();
        if clean.ends_with("/api/lol/upload") {
            clean = clean[..clean.len() - "/api/lol/upload".len()].to_string();
        }
        if clean.ends_with("/api/lol/upload-batch") {
            clean = clean[..clean.len() - "/api/lol/upload-batch".len()].to_string();
        }
        if clean.ends_with("/api/lol/lcu") {
            clean = clean[..clean.len() - "/api/lol/lcu".len()].to_string();
        }
        if clean.ends_with("/lcuHub") {
            clean = clean[..clean.len() - "/lcuHub".len()].to_string();
        }
        if clean == before {
            break;
        }
    }
    clean
}

/// 启动 SignalR Hub 连接。
/// 连接到远程服务器 of `/lcuHub`，支持远程 LCU 查询和状态上报。
pub fn start(app_handle: AppHandle, server_url: String, user_id: String) {
    tauri::async_runtime::spawn(async move {
        stop().await;

        let server_url = clean_server_url(&server_url);

        let (cancel_tx, mut cancel_rx) = watch::channel(false);
        {
            let mut cancel_lock = SIGNALR_CANCEL_TX.lock().await;
            *cancel_lock = Some(cancel_tx);
        }

        loop {
            if *cancel_rx.borrow() {
                log::info!("[SignalR] 任务检测到停止信号，退出循环");
                break;
            }

            log::info!(
                "[SignalR] 正在尝试建立连接到: {}/lcuHub?userId={}",
                server_url,
                user_id
            );
            let _ = app_handle.emit("signalr-connecting", ());

            let connect_fut = try_connect(&server_url, &user_id);
            tokio::select! {
                res = connect_fut => {
                    match res {
                        Ok(ws_stream) => {
                            log::info!("[SignalR] WebSocket 连接成功！开始握手...");
                            let mut rx_clone = cancel_rx.clone();
                            tokio::select! {
                                _ = handle_connection(ws_stream, &app_handle, &user_id, &mut rx_clone) => {}
                                _ = cancel_rx.changed() => {
                                    log::info!("[SignalR] 连接运行期间收到停止信号，断开连接");
                                    break;
                                }
                            }
                            log::warn!("[SignalR] 与 Hub 的连接已断开，5 秒后将尝试重新连接");
                            let _ = app_handle.emit("signalr-disconnected", ());
                        }
                        Err(e) => {
                            let msg = e.to_string();
                            log::error!("[SignalR] 建立连接失败: {}，5 秒后重试", msg);
                            let _ = app_handle.emit("signalr-error", msg);
                        }
                    }
                }
                _ = cancel_rx.changed() => {
                    log::info!("[SignalR] 连接建立尝试被取消");
                    break;
                }
            }

            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {}
                _ = cancel_rx.changed() => {
                    log::info!("[SignalR] 等待重连期间收到停止信号");
                    break;
                }
            }
        }

        // 清理状态
        {
            let mut lock = SIGNALR_TX.lock().await;
            *lock = None;
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
    // 1. 进行 SignalR 协商 (Negotiate)
    let connection_token = match negotiate(server_url).await {
        Ok(token) => token,
        Err(e) => {
            log::error!("[SignalR] 协商失败: {}", e);
            return Err(e);
        }
    };

    // 2. 构造带 connectionToken 的 WSS URL
    let ws_url = format!(
        "{}/lcuHub?id={}&userId={}",
        server_url
            .replace("http://", "ws://")
            .replace("https://", "wss://"),
        connection_token,
        user_id
    );

    log::info!("[SignalR] 正在建立 WebSocket 连接到: {}", ws_url);

    let is_local = server_url.contains("127.0.0.1") || server_url.contains("localhost");

    let tls_connector = if is_local {
        // localhost: LCU 使用自签名证书，需跳过验证
        let tls_config = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerifier))
            .with_no_client_auth();
        Some(Connector::Rustls(Arc::new(tls_config)))
    } else {
        // 远程服务器: 使用正常 TLS 验证，不跳过证书检查
        None
    };

    let url: http::Uri = ws_url.parse()?;
    let request = ClientRequestBuilder::new(url);

    let (ws_stream, _) =
        connect_async_tls_with_config(request, None, false, tls_connector).await?;

    Ok(ws_stream)
}

async fn negotiate(server_url: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let is_local = server_url.contains("127.0.0.1") || server_url.contains("localhost");
    
    let http_client = reqwest::Client::builder()
        .danger_accept_invalid_certs(is_local)
        .build()?;
        
    let negotiate_url = format!("{}/lcuHub/negotiate?negotiateVersion=1", server_url);
    log::info!("[SignalR] 正在发送协商 (Negotiate) 请求到: {}", negotiate_url);
    
    let resp = http_client.post(&negotiate_url).send().await?;
    if !resp.status().is_success() {
        return Err(format!("协商请求失败: HTTP {}", resp.status()).into());
    }
    
    let val: serde_json::Value = resp.json().await?;
    log::debug!("[SignalR] 协商返回数据: {:?}", val);
    
    let token = val.get("connectionToken")
        .or_else(|| val.get("connectionId"))
        .and_then(|v| v.as_str())
        .ok_or("协商响应中缺少 connectionToken 或 connectionId")?;
        
    Ok(token.to_string())
}

/// 处理 SignalR 连接：握手 → 消息循环
async fn handle_connection(
    ws_stream: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    app_handle: &AppHandle,
    user_id: &str,
    cancel_rx: &mut watch::Receiver<bool>,
) {
    let (mut write, mut read) = ws_stream.split();

    // SignalR JSON 握手
    let handshake = format!(r#"{{"protocol":"json","version":1}}{}"#, RECORD_SEPARATOR);
    if let Err(e) = write.send(Message::Text(handshake.into())).await {
        log::error!("[SignalR] 握手请求发送失败: {}", e);
        return;
    }

    let (cmd_tx, mut cmd_rx) = mpsc::channel::<SignalrCommand>(64);
    {
        let mut lock = SIGNALR_TX.lock().await;
        *lock = Some(cmd_tx);
    }

    log::info!("[SignalR] 握手包发送成功！协议切换就绪。");
    let _ = app_handle.emit("signalr-connected", ());

    // 握手成功后，查询当前玩家信息并推送 summoner_info
    let app_handle_clone = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        log::info!("[SignalR] 正在向 LCU 获取当前登录玩家信息用于首次云端对齐...");
        if let Ok(summoner) = query_current_summoner(&app_handle_clone).await {
            update_summoner_info(summoner).await;
        } else {
            log::warn!("[SignalR] 未能在 LCU 中获取到当前玩家数据（游戏可能尚未启动或未登录）");
        }
    });

    // 启动心跳定时器
    let user_id_heartbeat = user_id.to_string();
    let (heartbeat_cancel_tx, mut heartbeat_cancel_rx) = tokio::sync::oneshot::channel::<()>();
    let heartbeat_cmd_tx = SIGNALR_TX.lock().await.clone();
    if let Some(h_tx) = heartbeat_cmd_tx {
        tauri::async_runtime::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            interval.tick().await; // skip first immediate tick
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let msg = serde_json::json!({
                            "type": 1,
                            "target": "Heartbeat",
                            "arguments": [user_id_heartbeat],
                        });
                        let text = format!("{}{}", msg, RECORD_SEPARATOR);
                        let _ = h_tx.send(SignalrCommand::SendRaw(Message::Text(text.into()))).await;
                        log::debug!("[SignalR] 心跳已发送");
                    }
                    _ = &mut heartbeat_cancel_rx => {
                        log::info!("[SignalR] 心跳维持任务已停止");
                        break;
                    }
                }
            }
        });
    }

    let mut buffer = String::new();

    loop {
        tokio::select! {
            // 1. 处理写入端发送的命令
            Some(cmd) = cmd_rx.recv() => {
                match cmd {
                    SignalrCommand::SendEvent { event_type, data } => {
                        let query_id = uuid::Uuid::new_v4().to_string();
                        let message = serde_json::json!({
                            "eventType": event_type,
                            "data": data,
                            "timestamp": std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs_f64(),
                        });
                        let payload = message.to_string();
                        log::info!(
                            "[SignalR] >>> 发送云端事件: target=ReportResult, type={}, queryId={}, 长度: {} 字节",
                            event_type,
                            query_id,
                            payload.len()
                        );
                        let msg = serde_json::json!({
                            "type": 1,
                            "target": "ReportResult",
                            "arguments": [user_id, payload, query_id],
                        });
                        let text = format!("{}{}", msg, RECORD_SEPARATOR);
                        if let Err(e) = write.send(Message::Text(text.into())).await {
                            log::error!("[SignalR] 事件发送出错: {}", e);
                            break;
                        }
                    }
                    SignalrCommand::SendRaw(msg) => {
                        if let Err(e) = write.send(msg).await {
                            log::error!("[SignalR] 原始帧发送出错: {}", e);
                            break;
                        }
                    }
                }
            }
            // 2. 处理接收端的消息
            msg_result = read.next() => {
                match msg_result {
                    Some(Ok(Message::Text(text))) => {
                        buffer.push_str(&text);
                        while let Some(end_pos) = buffer.find(RECORD_SEPARATOR) {
                            let msg_str = buffer[..end_pos].to_string();
                            buffer = buffer[end_pos + RECORD_SEPARATOR.len_utf8()..].to_string();

                            if msg_str.trim().is_empty() {
                                continue;
                            }

                            process_signalr_message(&mut write, &msg_str, app_handle, user_id).await;
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        log::info!("[SignalR] 收到服务端的 WebSocket 关闭帧");
                        break;
                    }
                    Some(Err(e)) => {
                        log::error!("[SignalR] 读通道读取失败: {}", e);
                        break;
                    }
                    None => {
                        log::info!("[SignalR] WebSocket 通道已关闭（服务端断开连接）");
                        break;
                    }
                    _ => {}
                }
            }
            // 3. 接收外部取消信号
            _ = cancel_rx.changed() => {
                log::info!("[SignalR] 收到取消指令，正在关闭连接并退出处理线程...");
                break;
            }
        }
    }

    let _ = heartbeat_cancel_tx.send(());
    {
        let mut lock = SIGNALR_TX.lock().await;
        *lock = None;
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
            log::debug!("[SignalR] 忽略非法 JSON 消息: {}", e);
            return;
        }
    };

    let msg_type = msg.get("type").and_then(|v| v.as_i64()).unwrap_or(0);

    match msg_type {
        // Invocation (远程命令触发)
        1 => {
            let target = msg.get("target").and_then(|v| v.as_str()).unwrap_or("");
            let arguments = msg
                .get("arguments")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let invocation_id = msg
                .get("invocationId")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            match target {
                "ReceiveCommand" => {
                    handle_receive_command(write, &arguments, invocation_id, app_handle, user_id)
                        .await;
                }
                other => {
                    log::debug!("[SignalR] 忽略未知调用目标: {}", other);
                }
            }
        }
        6 => {
            log::debug!("[SignalR] 收到心跳 Ping 信号");
        }
        7 => {
            log::info!("[SignalR] 收到服务端下发的 Close 指令");
        }
        _ => {
            log::debug!("[SignalR] 未处理的消息类型: {}", msg_type);
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
    let command_data = match arguments.first() {
        Some(v) if v.is_string() => {
            serde_json::from_str::<Value>(v.as_str().unwrap_or("{}")).unwrap_or(Value::Null)
        }
        Some(v) => v.clone(),
        None => {
            log::warn!("[SignalR] ReceiveCommand 缺少参数");
            return;
        }
    };

    let endpoint = command_data
        .get("Endpoint")
        .or_else(|| command_data.get("endpoint"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let query_id = arguments.get(1).and_then(|v| v.as_str()).unwrap_or("");

    log::info!("[SignalR] <<< 收到后端远程命令: endpoint={}, queryId={}", endpoint, query_id);

    if !is_endpoint_allowed(endpoint) {
        log::warn!("[SignalR] ReceiveCommand 拒绝执行未授权路径: {}", endpoint);
        send_report(
            write,
            user_id,
            &serde_json::json!({ "error": "endpoint 未授权" }).to_string(),
            query_id,
            invocation_id.as_deref(),
        )
        .await;
        return;
    }

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(8),
        lcu_get(app_handle, endpoint),
    )
    .await;

    let payload = match result {
        Ok(Ok(data)) => {
            log::info!("[SignalR] 成功执行 LCU 远程命令: endpoint={}", endpoint);
            serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string())
        }
        Ok(Err(e)) => {
            log::error!("[SignalR] LCU 接口请求失败: {}, endpoint={}", e, endpoint);
            serde_json::json!({ "error": e }).to_string()
        }
        Err(_) => {
            log::error!("[SignalR] LCU 接口请求超时(8秒): endpoint={}", endpoint);
            serde_json::json!({ "error": "LCU 请求超时 (8s)" }).to_string()
        }
    };

    send_report(write, user_id, &payload, query_id, invocation_id.as_deref()).await;
}

fn is_endpoint_allowed(endpoint: &str) -> bool {
    if endpoint.is_empty() || endpoint.contains("..") || endpoint.contains("//") {
        return false;
    }
    ALLOWED_PREFIXES
        .iter()
        .any(|prefix| endpoint.starts_with(prefix))
}

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
        log::error!("[SignalR] ReportResult 回执包发送失败: {}", e);
    } else {
        log::info!("[SignalR] >>> 成功发送结果回执给云端, queryId={}", query_id);
    }
}

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

// ─── 辅助方法及外部推送 ───

pub async fn get_current_summoner_name() -> String {
    let lock = CURRENT_SUMMONER_NAME.lock().await;
    if lock.is_empty() {
        "Unknown".to_string()
    } else {
        lock.clone()
    }
}

async fn query_current_summoner(app_handle: &AppHandle) -> Result<serde_json::Value, String> {
    let state = app_handle.state::<crate::AppState>();
    let lock = state.lcu_client.read().await;
    let lcu = lock.as_ref().ok_or("LCU 未连接")?;

    let url = format!(
        "https://127.0.0.1:{}/lol-summoner/v1/current-summoner",
        lcu.port
    );
    let auth = crate::build_auth_header(&lcu.token);

    let resp = lcu
        .http_client
        .get(&url)
        .header("Authorization", auth)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if resp.status().is_success() {
        resp.json::<serde_json::Value>().await.map_err(|e| e.to_string())
    } else {
        Err(format!("HTTP {}", resp.status()))
    }
}

pub async fn update_summoner_info(summoner: serde_json::Value) {
    let name = summoner
        .get("displayName")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();
    {
        let mut name_lock = CURRENT_SUMMONER_NAME.lock().await;
        *name_lock = name.clone();
    }
    let info = serde_json::json!({
        "summonerName": summoner.get("displayName").unwrap_or(&serde_json::Value::Null),
        "summonerId": summoner.get("summonerId").unwrap_or(&serde_json::Value::Null),
        "puuid": summoner.get("puuid").unwrap_or(&serde_json::Value::Null),
        "platformId": summoner.get("platformId").unwrap_or(&serde_json::Value::Null),
        "level": summoner.get("summonerLevel").or_else(|| summoner.get("level")).unwrap_or(&serde_json::Value::Null),
        "profileIconId": summoner.get("profileIconId").unwrap_or(&serde_json::Value::Null),
    });
    log::info!("[SignalR] 推送召唤师对齐信息: PUUID={}, 名字={}", summoner.get("puuid").and_then(|v| v.as_str()).unwrap_or(""), name);
    let _ = send_event("summoner_info", info).await;
}

pub async fn send_event(event_type: &str, data: serde_json::Value) -> Result<(), String> {
    let tx_lock = SIGNALR_TX.lock().await;
    if let Some(tx) = tx_lock.as_ref() {
        tx.send(SignalrCommand::SendEvent {
            event_type: event_type.to_string(),
            data,
        })
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("SignalR 未连接".to_string())
    }
}
