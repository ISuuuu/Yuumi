//! 启用后对接旧版 /lcuHub 协议，LCU 查询仅允许本地客户端的只读白名单。
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::{
    net::TcpStream,
    sync::{broadcast, watch, RwLock},
    task::JoinHandle,
    time::{sleep, timeout},
};
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::{protocol::WebSocketConfig, Message},
    MaybeTlsStream, WebSocketStream,
};
use url::Url;

use crate::{
    config::AppConfig,
    lcu::{client::LcuClient, ws::LcuWsEvent},
};

const SEPARATOR: char = '\x1e';
const MAX_FRAME: usize = 1024 * 1024;
const ALLOWED_PREFIXES: &[&str] = &[
    "/lol-summoner/",
    "/lol-match-history/",
    "/lol-ranked/",
    "/lol-champ-select/",
    "/lol-gameflow/",
    "/lol-game-queues/",
    "/lol-perks/",
    "/lol-game-data/",
];
type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;
type Client = Arc<RwLock<Option<LcuClient>>>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HubStatus {
    #[default]
    Disabled,
    Connecting,
    Connected,
    Retry,
    InvalidConfig,
}

impl HubStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Disabled => "云端未连接",
            Self::Connecting => "云端连接中",
            Self::Connected => "云端已连接",
            Self::Retry => "云端连接失败，等待重试",
            Self::InvalidConfig => "云端地址无效",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct HubConfig {
    hub: Url,
    user_id: String,
}

impl HubConfig {
    pub fn from_app(config: &AppConfig) -> Result<Option<Self>, String> {
        if !config.functions.lcu_realtime_enabled {
            return Ok(None);
        }
        let general = &config.general;
        let address = general.upload_api_url.trim();
        if address.is_empty() {
            return Err("未配置云端服务地址".into());
        }
        let user_id = [
            general.signalr_user_id.trim(),
            config.functions.lcu_user_id.trim(),
        ]
        .into_iter()
        .find(|id| !id.is_empty())
        .unwrap_or("lcu_user_001");
        Ok(Some(Self {
            hub: hub_url(address)?,
            user_id: user_id.into(),
        }))
    }
}

fn hub_url(address: &str) -> Result<Url, String> {
    let mut url = Url::parse(address).map_err(|_| "云端地址格式无效")?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("云端地址必须是无凭据和查询参数的 HTTP(S) 地址".into());
    }
    if url.scheme() != "https"
        && !matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))
    {
        return Err("云端服务必须使用 HTTPS".into());
    }
    let mut path = url.path().trim_end_matches('/');
    while let Some(base) = [
        "/api/lol/upload-batch",
        "/api/lol/upload",
        "/api/lol/lcu",
        "/lcuHub",
    ]
    .iter()
    .find_map(|suffix| path.strip_suffix(suffix))
    {
        path = base.trim_end_matches('/');
    }
    let path = format!("{path}/lcuHub");
    url.set_path(&path);
    Ok(url)
}

pub fn forwards_event(uri: &str) -> bool {
    matches!(
        uri,
        "/lol-summoner/v1/current-summoner"
            | "/lol-gameflow/v1/gameflow-phase"
            | "/lol-gameflow/v1/session"
            | "/lol-end-of-game/v1/eog-stats-block"
            | "/lol-gameflow/v1/watch"
            | "/lol-champ-select/v1/session"
    )
}

pub fn spawn(
    runtime: &tokio::runtime::Handle,
    config: HubConfig,
    client: Client,
    events: broadcast::Receiver<LcuWsEvent>,
    status: watch::Sender<HubStatus>,
) -> JoinHandle<()> {
    runtime.spawn(run(config, client, events, status))
}

async fn run(
    config: HubConfig,
    client: Client,
    events: broadcast::Receiver<LcuWsEvent>,
    status: watch::Sender<HubStatus>,
) {
    let Ok(http) = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
    else {
        status.send_replace(HubStatus::InvalidConfig);
        return;
    };
    loop {
        status.send_replace(HubStatus::Connecting);
        if let Ok(Ok(socket)) = timeout(Duration::from_secs(15), connect(&http, &config)).await {
            let _ = serve(
                socket,
                &config.user_id,
                &client,
                events.resubscribe(),
                &status,
            )
            .await;
        }
        status.send_replace(HubStatus::Retry);
        sleep(Duration::from_secs(5)).await;
    }
}

async fn connect(http: &reqwest::Client, config: &HubConfig) -> Result<Socket, String> {
    let mut negotiate = config.hub.clone();
    negotiate.set_path(&format!("{}/negotiate", config.hub.path()));
    negotiate
        .query_pairs_mut()
        .append_pair("negotiateVersion", "1");
    let response = http
        .post(negotiate)
        .send()
        .await
        .map_err(|_| "云端协商请求失败")?;
    let payload: Value = response
        .error_for_status()
        .map_err(|_| "云端协商失败")?
        .json()
        .await
        .map_err(|_| "云端协商响应无效")?;
    let token = payload
        .get("connectionToken")
        .or_else(|| payload.get("connectionId"))
        .and_then(Value::as_str)
        .filter(|token| !token.is_empty())
        .ok_or("协商响应缺少连接标识")?;

    let mut websocket = config.hub.clone();
    let scheme = if websocket.scheme() == "https" {
        "wss"
    } else {
        "ws"
    };
    websocket.set_scheme(scheme).map_err(|_| "云端协议无效")?;
    websocket
        .query_pairs_mut()
        .append_pair("id", token)
        .append_pair("userId", &config.user_id);
    let limits = WebSocketConfig::default().max_message_size(Some(MAX_FRAME));
    connect_async_with_config(websocket.as_str(), Some(limits), false)
        .await
        .map(|(socket, _)| socket)
        .map_err(|_| "云端 WebSocket 连接失败".into())
}

async fn serve(
    mut socket: Socket,
    user: &str,
    lcu: &Client,
    mut events: broadcast::Receiver<LcuWsEvent>,
    status: &watch::Sender<HubStatus>,
) -> Result<(), String> {
    send(&mut socket, json!({"protocol": "json", "version": 1})).await?;
    let mut buffer = String::new();
    let handshake = timeout(Duration::from_secs(10), async {
        loop {
            receive(&mut socket, &mut buffer).await?;
            if let Some(frame) = pop_frame(&mut buffer) {
                let value: Value = serde_json::from_str(&frame).map_err(|_| "云端握手响应无效")?;
                if !value.is_object() || value.get("error").is_some() {
                    return Err("云端拒绝握手".to_string());
                }
                return Ok(());
            }
        }
    })
    .await
    .map_err(|_| "云端握手超时")?;
    handshake?;
    status.send_replace(HubStatus::Connected);
    let mut heartbeat = tokio::time::interval(Duration::from_secs(30));
    heartbeat.tick().await;
    let mut summoner_name = String::new();
    if let Ok(player) = lcu_get(lcu, "/lol-summoner/v1/current-summoner").await {
        let (name, payload) = summoner_event(&player);
        summoner_name = name;
        report_event(&mut socket, user, "summoner_info", payload).await?;
    }

    loop {
        while let Some(frame) = pop_frame(&mut buffer) {
            let Ok(msg) = serde_json::from_str::<Value>(&frame) else {
                continue;
            };
            if msg["type"] == 7 {
                return Ok(());
            }
            if msg["type"] == 1 && msg["target"] == "ReceiveCommand" {
                handle_query(&mut socket, user, lcu, &msg).await?;
            }
        }
        tokio::select! {
            result = receive(&mut socket, &mut buffer) => result?,
            _ = heartbeat.tick() => {
                send(&mut socket, json!({"type": 1, "target": "Heartbeat", "arguments": [user]})).await?;
            }
            event = events.recv() => {
                match event {
                    Ok(event) => {
                        if let Some((kind, payload)) = format_event(event, &mut summoner_name) {
                            report_event(&mut socket, user, kind, payload).await?;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => return Ok(()),
                }
            }
        }
    }
}

async fn receive(socket: &mut Socket, buffer: &mut String) -> Result<(), String> {
    match socket.next().await {
        Some(Ok(Message::Text(text))) => {
            if buffer.len().saturating_add(text.len()) > MAX_FRAME {
                return Err("云端消息超过大小限制".into());
            }
            buffer.push_str(&text);
        }
        Some(Ok(Message::Close(_))) | Some(Err(_)) | None => return Err("云端连接断开".into()),
        Some(Ok(Message::Ping(_))) => socket.flush().await.map_err(|_| "云端连接断开")?,
        _ => {}
    }
    Ok(())
}

fn pop_frame(buffer: &mut String) -> Option<String> {
    let end = buffer.find(SEPARATOR)?;
    Some(
        buffer
            .drain(..=end)
            .collect::<String>()
            .trim_end_matches(SEPARATOR)
            .to_owned(),
    )
}

async fn send(socket: &mut Socket, value: Value) -> Result<(), String> {
    timeout(
        Duration::from_secs(8),
        socket.send(Message::Text(format!("{value}{SEPARATOR}").into())),
    )
    .await
    .map_err(|_| "云端发送超时")?
    .map_err(|_| "云端发送失败".into())
}

fn endpoint_allowed(endpoint: &str) -> bool {
    let path = endpoint.split_once('?').map_or(endpoint, |(path, _)| path);
    !endpoint
        .chars()
        .any(|c| c.is_control() || c == '\\' || c == '#')
        && !path.contains('%')
        && !path.contains("..")
        && !path.contains("//")
        && ALLOWED_PREFIXES
            .iter()
            .any(|prefix| path.starts_with(prefix))
}

async fn lcu_get(lcu: &Client, endpoint: &str) -> Result<Value, &'static str> {
    let client = lcu.read().await.clone().ok_or("LCU 未连接")?;
    timeout(Duration::from_secs(8), client.get(endpoint))
        .await
        .map_err(|_| "LCU 请求超时")?
        .map_err(|_| "LCU 请求失败")
}

async fn handle_query(
    socket: &mut Socket,
    user: &str,
    lcu: &Client,
    msg: &Value,
) -> Result<(), String> {
    let arg = &msg["arguments"][0];
    let parsed;
    let command = if let Some(text) = arg.as_str() {
        parsed = serde_json::from_str::<Value>(text).unwrap_or(Value::Null);
        &parsed
    } else {
        arg
    };
    let endpoint = command
        .get("Endpoint")
        .or_else(|| command.get("endpoint"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let result = if endpoint_allowed(endpoint) {
        lcu_get(lcu, endpoint).await
    } else {
        Err("endpoint 未授权")
    };
    let payload = result.unwrap_or_else(|error| json!({ "error": error }));
    send(
        socket,
        json!({
            "type": 1, "target": "ReportResult",
            "arguments": [user, payload.to_string(), msg["arguments"][1].as_str().unwrap_or("")],
            "invocationId": msg["invocationId"].as_str().unwrap_or("")
        }),
    )
    .await
}

async fn report_event(
    socket: &mut Socket,
    user: &str,
    kind: &str,
    data: Value,
) -> Result<(), String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();
    let payload = json!({"eventType": kind, "data": data, "timestamp": timestamp}).to_string();
    send(
        socket,
        json!({
            "type": 1, "target": "ReportResult",
            "arguments": [user, payload, uuid::Uuid::new_v4().to_string()]
        }),
    )
    .await
}

fn summoner_event(data: &Value) -> (String, Value) {
    let name = data["displayName"]
        .as_str()
        .filter(|name| !name.is_empty())
        .or_else(|| data["gameName"].as_str())
        .unwrap_or("Unknown");
    (
        name.into(),
        json!({
            "summonerName": name, "summonerId": data["summonerId"], "puuid": data["puuid"],
            "platformId": data["platformId"], "level": data.get("summonerLevel").unwrap_or(&data["level"]),
            "profileIconId": data["profileIconId"],
        }),
    )
}

fn format_event(event: LcuWsEvent, name: &mut String) -> Option<(&'static str, Value)> {
    let data = event.data;
    let kind = match event.uri.as_str() {
        "/lol-summoner/v1/current-summoner" if data.is_object() => {
            let (player, payload) = summoner_event(&data);
            *name = player;
            return Some(("summoner_info", payload));
        }
        "/lol-gameflow/v1/gameflow-phase" => {
            let phase = data.as_str()?;
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
            return Some((
                "game_phase_changed",
                json!({"phase": phase, "phaseName": phase_name, "summonerName": name}),
            ));
        }
        "/lol-champ-select/v1/session" => {
            return Some((
                "champ_select",
                json!({
                    "phase": data.pointer("/timer/phase").and_then(Value::as_str).unwrap_or(""),
                    "actions": data["actions"].as_array().cloned().unwrap_or_default(), "summonerName": name,
                }),
            ))
        }
        "/lol-gameflow/v1/session" => "game_session",
        "/lol-end-of-game/v1/eog-stats-block" => "eog_stats",
        "/lol-gameflow/v1/watch" => "watch_event",
        _ => return None,
    };
    Some((kind, json!({"data": data, "summonerName": name})))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_shared_server_address_and_legacy_user_fallback() {
        let mut config = AppConfig::default();
        config.general.upload_api_url = "https://example.test/path/api/lol/upload-batch".into();
        assert!(HubConfig::from_app(&config).unwrap().is_none());
        config.functions.lcu_realtime_enabled = true;
        config.functions.lcu_user_id = "player + /".into();
        config.general.signalr_server_url = "https://old.example.test".into();
        let hub = HubConfig::from_app(&config).unwrap().unwrap();
        assert_eq!(hub.hub.as_str(), "https://example.test/path/lcuHub");
        assert_eq!(hub.user_id, "player + /");
        config.general.upload_api_url.clear();
        assert!(HubConfig::from_app(&config).is_err());
        config.general.upload_api_url = "http://example.test".into();
        assert!(HubConfig::from_app(&config).is_err());
        config.general.upload_api_url = "http://127.0.0.1:1234".into();
        assert!(HubConfig::from_app(&config).is_ok());
        config.general.upload_api_url = "file:///test".into();
        assert!(HubConfig::from_app(&config).is_err());
    }

    #[test]
    fn allows_only_local_read_paths_from_remote_command() {
        assert!(endpoint_allowed("/lol-match-history/v1/games?begIndex=0"));
        for path in [
            "https://example.test/lol-summoner/",
            "/lol-summoner/../lol-chat/",
            "/lol-gameflow/%2e%2e/lol-chat/",
            "/lol-summoner/\\lol-chat/",
            "/lol-chat/v1/me",
            "/lol-summoner/\r\n",
        ] {
            assert!(!endpoint_allowed(path), "{path:?}");
        }
    }

    #[tokio::test]
    async fn local_server_negotiates_connects_and_disconnects() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut http, _) = listener.accept().await.unwrap();
            let mut buffer = [0; 4096];
            let _ = http.read(&mut buffer).await.unwrap();
            let body = r#"{"connectionToken":"test-token"}"#;
            http.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).as_bytes()).await.unwrap();
            drop(http);
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
            let handshake = socket.next().await.unwrap().unwrap().into_text().unwrap();
            assert!(handshake.contains("protocol"));
            socket
                .send(Message::Text(format!("{{}}{SEPARATOR}").into()))
                .await
                .unwrap();
            tokio::time::sleep(Duration::from_millis(300)).await;
        });
        let mut config = AppConfig::default();
        config.functions.lcu_realtime_enabled = true;
        config.general.upload_api_url = format!("http://{address}");
        let hub = HubConfig::from_app(&config).unwrap().unwrap();
        let (events, receiver) = broadcast::channel(8);
        let _events = events;
        let (status, mut changes) = watch::channel(HubStatus::Disabled);
        let task = spawn(
            &tokio::runtime::Handle::current(),
            hub,
            Arc::new(RwLock::new(None)),
            receiver,
            status,
        );
        timeout(Duration::from_secs(5), async {
            loop {
                if *changes.borrow_and_update() == HubStatus::Connected {
                    break;
                }
                changes.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
        server.await.unwrap();
        timeout(Duration::from_secs(5), async {
            loop {
                if *changes.borrow_and_update() == HubStatus::Retry {
                    break;
                }
                changes.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
        task.abort();
    }
    #[test]
    fn preserves_data_after_handshake_and_partial_frames() {
        let mut input = format!("{{}}{SEPARATOR}{{\"type\":6}}{SEPARATOR}{{");
        assert_eq!(pop_frame(&mut input).as_deref(), Some("{}"));
        assert_eq!(pop_frame(&mut input).as_deref(), Some("{\"type\":6}"));
        assert!(pop_frame(&mut input).is_none());
        assert_eq!(input, "{");
    }
}
