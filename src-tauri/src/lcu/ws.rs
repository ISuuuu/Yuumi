use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, Error as TlsError, SignatureScheme};
use serde::Deserialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::watch;
use tokio_tungstenite::{
    connect_async_tls_with_config,
    tungstenite::{client::ClientRequestBuilder, http, Message},
    Connector,
};

// ── 订阅消息（对齐 Python: [5, "OnJsonApiEvent"]，2 个元素）────────────────
const SUBSCRIBE_MSG: &str = r#"[5, "OnJsonApiEvent"]"#;

/// champ-select session 事件节流间隔（毫秒）：
/// 选人阶段 LCU 每秒推送多次（倒计时/悬停/动作变更），前端与各 Agent 只需要最新状态，
/// 300ms 合并一次可显著降低前端 watcher / Agent / SignalR 的全链路处理压力
const SESSION_THROTTLE_MS: u64 = 300;
static LAST_SESSION_TS: AtomicU64 = AtomicU64::new(0);

/// 判断 session 事件是否放行（处于节流窗口内则丢弃中间帧）
fn session_throttle_allowed() -> bool {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let last = LAST_SESSION_TS.load(Ordering::Relaxed);
    if now.saturating_sub(last) < SESSION_THROTTLE_MS {
        return false;
    }
    LAST_SESSION_TS
        .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
        .is_ok()
}

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
/// 每次调用会取消前一个仍在运行的连接循环，避免旧 port/token 的僵尸重试。
pub fn connect(app_handle: AppHandle, port: u16, token: String) {
    // 取消上一次 WS 循环
    let mut cancel_rx = {
        let state = app_handle.state::<crate::AppState>();
        let mut old_tx = state.ws_cancel_tx.lock().unwrap();
        // 发送取消信号给旧循环
        if let Some(tx) = old_tx.take() {
            let _ = tx.send(true);
        }
        let (tx, rx) = watch::channel(false);
        *old_tx = Some(tx);
        rx
    };

    crate::spawn_log_panic(async move {
        loop {
            // 在尝试连接前检查是否已被取消
            if *cancel_rx.borrow() {
                log::info!("[WS] 循环已被取消（新的连接已启动）");
                return;
            }

            log::info!("[WS] 正在连接 LCU WebSocket (port={})...", port);

            tokio::select! {
                result = try_connect(port, &token) => {
                    match result {
                        Ok(ws_stream) => {
                            log::info!("[WS] LCU WebSocket 已连接");
                            let _ = app_handle.emit("lcu-ws-connected", ());

                            // 初始主动获取一次当前游戏阶段以触发自动化状态对齐
                            let app_clone = app_handle.clone();
                            crate::spawn_log_panic(async move {
                                let state = app_clone.state::<crate::AppState>();
                                // 锁内仅提取连接参数，随后释放再发起 HTTP，避免跨 await 持有读锁阻塞 monitor 写锁
                                let conn = {
                                    let lcu_lock = state.lcu_client.read().await;
                                    lcu_lock
                                        .as_ref()
                                        .map(|lcu| (lcu.port, lcu.token.clone(), lcu.http_client.clone()))
                                };
                                if let Some((port, token, http_client)) = conn {
                                    let url = format!(
                                        "https://127.0.0.1:{}/lol-gameflow/v1/gameflow-phase",
                                        port
                                    );
                                    let auth = crate::build_auth_header(&token);
                                    if let Ok(resp) = http_client
                                        .get(&url)
                                        .header("Authorization", auth)
                                        .send()
                                        .await
                                    {
                                        if let Ok(phase) = resp.text().await {
                                            let phase = phase.trim_matches('"');
                                            log::info!("[WS] 获取到初始游戏阶段: {}", phase);
                                            let _ = state.gameflow_tx.try_send(
                                                crate::agents::auto_match::GameflowEvent::PhaseChanged(
                                                    phase.to_string(),
                                                ),
                                            );
                                        }
                                    }
                                }
                            });

                            tokio::select! {
                                _ = handle_messages(ws_stream, &app_handle) => {}
                                _ = cancel_rx.changed() => {
                                    log::info!("[WS] 连接中收到取消信号，断开");
                                    return;
                                }
                            }
                            log::warn!("[WS] LCU WebSocket 断开，2 秒后重连");
                            let _ = app_handle.emit("lcu-ws-disconnected", ());
                        }
                        Err(e) => {
                            let msg = e.to_string();
                            log::warn!("[WS] 连接失败: {}，2 秒后重试", msg);
                            let _ = app_handle.emit("lcu-ws-error", &msg);
                        }
                    }
                }
                _ = cancel_rx.changed() => {
                    log::info!("[WS] 循环已被取消（新的连接已启动）");
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
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Box<dyn std::error::Error + Send + Sync>,
> {
    // rustls ClientConfig，NoVerifier = Python ssl=False（进程级复用，避免每次重建）
    static TLS_CONFIG: OnceLock<Arc<ClientConfig>> = OnceLock::new();
    let tls_config = TLS_CONFIG
        .get_or_init(|| {
            Arc::new(
                ClientConfig::builder()
                    .dangerous()
                    .with_custom_certificate_verifier(Arc::new(NoVerifier))
                    .with_no_client_auth(),
            )
        })
        .clone();

    let connector = Connector::Rustls(tls_config);

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

    // 选人会话事件每秒推送多次，300ms 节流合并（置于 emit 之前），
    // 前端 emit / Agent / bench / SignalR 全链路均针对最新状态即可，丢弃过密中间帧
    if uri.starts_with("/lol-champ-select/v1/session") && !session_throttle_allowed() {
        return;
    }

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
            match crate::agents::auto_bp::ChampSelectSession::deserialize(data) {
                Ok(session) => {
                    if let Err(e) = state.bp_session_tx.try_send(session) {
                        match e {
                            tokio::sync::mpsc::error::TrySendError::Full(_) => {
                                log::warn!(
                                    "[WS] BP Session 消息发送频繁，通道已满，丢弃过密中间帧"
                                );
                            }
                            tokio::sync::mpsc::error::TrySendError::Closed(_) => {
                                log::warn!("[WS] 推送 BP Session 失败: 通道已关闭");
                            }
                        }
                    }
                }
                Err(e) => {
                    log::warn!(
                        "[WS] 选人会话反序列化失败: {}, data keys: {:?}",
                        e,
                        data.as_object().map(|o| o.keys().collect::<Vec<_>>())
                    );
                }
            }
        }
    }

    // 向悬浮窗直接推送"我的当前英雄ID"，解决多窗口localStorage隔离问题
    if uri.starts_with("/lol-champ-select/v1/current-champion") {
        if let Some(cid) = event_data.get("data").and_then(|v| v.as_i64()) {
            if cid > 0 {
                log::info!("[WS] 推送我的英雄到悬浮窗: {}", cid);
                // 写入 AppState 缓存（悬浮窗挂载时可主动拉取）
                {
                    let state = app_handle.state::<crate::AppState>();
                    if let Ok(mut list) = state.bench_my_champions.lock() {
                        if !list.contains(&cid) {
                            list.push(cid);
                        }
                    };
                }
                let _ = app_handle.emit_to(
                    tauri::EventTarget::WebviewWindow {
                        label: "bench-overlay".to_string(),
                    },
                    "bench-my-champion",
                    cid,
                );
            }
        }
    }

    if uri.starts_with("/lol-champ-select/v1/session") {
        if let Some(data) = event_data.get("data") {
            if let (Some(my_team), Some(local_cell_id)) = (
                data.get("myTeam").and_then(|v| v.as_array()),
                data.get("localPlayerCellId").and_then(|v| v.as_i64()),
            ) {
                for player in my_team {
                    if player.get("cellId").and_then(|v| v.as_i64()) == Some(local_cell_id) {
                        let cid = player
                            .get("championId")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0)
                            .max(
                                player
                                    .get("championPickIntent")
                                    .and_then(|v| v.as_i64())
                                    .unwrap_or(0),
                            );
                        if cid > 0 {
                            // 写入 Rust AppState 历史缓存
                            let state = app_handle.state::<crate::AppState>();
                            if let Ok(mut list) = state.bench_my_champions.lock() {
                                if !list.contains(&cid) {
                                    list.push(cid);
                                    log::info!("[WS] 从 session 记录我的英雄到缓存: {}", cid);
                                }
                            }
                            let _ = app_handle.emit_to(
                                tauri::EventTarget::WebviewWindow {
                                    label: "bench-overlay".to_string(),
                                },
                                "bench-my-champion",
                                cid,
                            );
                        }
                        break;
                    }
                }
            }
        }
    }

    if uri.starts_with("/lol-gameflow/v1/gameflow-phase") {
        if let Some(phase) = event_data.get("data").and_then(|v| v.as_str()) {
            if let Err(e) =
                state
                    .gameflow_tx
                    .try_send(crate::agents::auto_match::GameflowEvent::PhaseChanged(
                        phase.to_string(),
                    ))
            {
                log::warn!("[WS] 推送 Gameflow PhaseChanged 失败: {}", e);
            }
            // 只有当真正从非 ChampSelect 阶段跨阶段进入 ChampSelect 时，才清空上局历史英雄缓存
            if let Ok(mut last_phase) = state.last_gameflow_phase.lock() {
                if *last_phase != "ChampSelect" && phase == "ChampSelect" {
                    if let Ok(mut list) = state.bench_my_champions.lock() {
                        list.clear();
                        log::info!("[WS] 跨阶段进入选人阶段，已清空板凳席历史英雄缓存");
                    }
                }
                *last_phase = phase.to_string();
            } else {
                log::warn!("[WS] last_gameflow_phase 锁中毒，跳过板凳席缓存清空");
            }
        }
    }

    if uri.starts_with("/lol-matchmaking/v1/ready-check") {
        if let Some(data) = event_data.get("data") {
            if let Ok(ready_check) = crate::agents::auto_match::ReadyCheckData::deserialize(data) {
                if let Err(e) = state.gameflow_tx.try_send(
                    crate::agents::auto_match::GameflowEvent::ReadyCheck(ready_check),
                ) {
                    log::warn!("[WS] 推送 Gameflow ReadyCheck 失败: {}", e);
                }
            }
        }
    }

    if uri.starts_with("/lol-honor-v2/v1/ballot") {
        if let Some(data) = event_data.get("data") {
            if let Ok(ballot) = crate::agents::auto_match::HonorBallot::deserialize(data) {
                if let Err(e) = state.gameflow_tx.try_send(
                    crate::agents::auto_match::GameflowEvent::HonorBallot(ballot),
                ) {
                    log::warn!("[WS] 推送 Gameflow HonorBallot 失败: {}", e);
                }
            }
        }
    }

    if uri.starts_with("/lol-lobby/v2/received-invitations") {
        if let Some(data) = event_data.get("data") {
            if let Ok(invitations) =
                Vec::<crate::agents::auto_match::ReceivedInvitation>::deserialize(data)
            {
                if let Err(e) = state.gameflow_tx.try_send(
                    crate::agents::auto_match::GameflowEvent::ReceivedInvitations(invitations),
                ) {
                    log::warn!("[WS] 推送 Gameflow ReceivedInvitations 失败: {}", e);
                }
            }
        }
    }

    // ── SignalR 转发 ──
    if uri == "/lol-gameflow/v1/gameflow-phase"
        || uri == "/lol-gameflow/v1/session"
        || uri == "/lol-end-of-game/v1/eog-stats-block"
        || uri == "/lol-gameflow/v1/watch"
        || uri == "/lol-champ-select/v1/session"
        || uri == "/lol-summoner/v1/current-summoner"
    {
        // 仅提取一次 data 字段，避免对整个事件体克隆后再克隆 data 的双重深拷贝
        let data = event_data
            .get("data")
            .unwrap_or(&serde_json::Value::Null)
            .clone();
        let uri = uri.to_string();
        crate::spawn_log_panic(async move {
            if uri == "/lol-gameflow/v1/gameflow-phase" {
                if let Some(phase) = data.as_str() {
                    let phase_name = match phase {
                        "None" => "无",
                        "Lobby" => "大厅",
                        "Matchmaking" => "匹配中",
                        "ReadyCheck" => "等待确认",
                        "ChampSelect" => "英雄选择",
                        "GameStart" => "游戏开始",
                        "InProgress" => "游戏中",
                        "EndOfGame" => "结算界面",
                        "Reconnect" => "断线重连",
                        _ => phase,
                    };
                    let name = crate::signalr::get_current_summoner_name().await;
                    let _ = crate::signalr::send_event(
                        "game_phase_changed",
                        serde_json::json!({
                            "phase": phase,
                            "phaseName": phase_name,
                            "summonerName": name,
                        }),
                    )
                    .await;
                }
            } else if uri == "/lol-gameflow/v1/session" {
                let name = crate::signalr::get_current_summoner_name().await;
                let _ = crate::signalr::send_event(
                    "game_session",
                    serde_json::json!({
                        "data": data,
                        "summonerName": name,
                    }),
                )
                .await;
            } else if uri == "/lol-end-of-game/v1/eog-stats-block" {
                let name = crate::signalr::get_current_summoner_name().await;
                let _ = crate::signalr::send_event(
                    "eog_stats",
                    serde_json::json!({
                        "data": data,
                        "summonerName": name,
                    }),
                )
                .await;
            } else if uri == "/lol-gameflow/v1/watch" {
                let name = crate::signalr::get_current_summoner_name().await;
                let _ = crate::signalr::send_event(
                    "watch_event",
                    serde_json::json!({
                        "data": data,
                        "summonerName": name,
                    }),
                )
                .await;
            } else if uri == "/lol-champ-select/v1/session" {
                let phase = data
                    .pointer("/timer/phase")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let actions = data
                    .get("actions")
                    .cloned()
                    .unwrap_or(serde_json::Value::Array(vec![]));
                let name = crate::signalr::get_current_summoner_name().await;
                let _ = crate::signalr::send_event(
                    "champ_select",
                    serde_json::json!({
                        "phase": phase,
                        "actions": actions,
                        "summonerName": name,
                    }),
                )
                .await;
            } else if uri == "/lol-summoner/v1/current-summoner" && data.is_object() {
                crate::signalr::update_summoner_info(data).await;
            }
        });
    }
}
