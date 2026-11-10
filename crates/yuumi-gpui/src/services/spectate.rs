//! 观战服务（工单 D01）
//!
//! 对照 Vue `src/components/tools/SpectateCard.vue` 与旧 Tauri `spectate_directly`：
//! - `LCU`：`/lol-summoner/v1/summoners` 取 puuid → 好友列表/自定义房间取对局 ID
//!   → `POST /lol-spectator/v1/spectate/launch`；失败时按旧版降级到 `CMD`
//! - `CMD`：`/entitlements/v1/token` 取 SGP accessToken → SGP 观战凭据
//!   → 直接以 `spectator ip:port key gameId server` 启动 `League of Legends.exe`
//!
//! 不可观战时返回带原因的 `Err`，不再只报「已发送」。

use std::path::{Path, PathBuf};
use std::process::Command;
use yuumi_core::lcu::client::LcuClient;

/// 腾讯国服大区白名单（SGP 仅在这些大区可用）
const TENCENT_SERVERS: &[&str] = &[
    "hn1", "hn10", "bgp2", "tj100", "cq100", "gz100", "nj100", "tj101",
];

/// 需要 k8s-sgp 子域名的特殊大区
const K8S_SGP_SERVERS: &[&str] = &["hn1", "hn10", "bgp2"];

/// SGP 请求使用的 UA（与旧版保持一致）
const SGP_USER_AGENT: &str = "RiotClient/78.0.1.1352 (Windows;10;co;red)";

/// 观战启动方式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpectateMethod {
    /// LCU 原生 `/lol-spectator/v1/spectate/launch`
    Lcu,
    /// SGP 凭据 + 直接拉起游戏本体
    Cmd,
}

impl SpectateMethod {
    /// 从 UI 下拉值解析，未知值按 LCU 处理
    pub fn from_ui(value: &str) -> Self {
        if value.eq_ignore_ascii_case("CMD") {
            Self::Cmd
        } else {
            Self::Lcu
        }
    }
}

/// 启动观战。`lol_path` 为客户端根目录（不含 WeGame 标记），CMD 方式必需。
pub async fn launch_spectate(
    client: &LcuClient,
    summoner_name: &str,
    method: SpectateMethod,
    lol_path: Option<&str>,
) -> Result<String, String> {
    let name = normalize_name(summoner_name);
    if name.is_empty() {
        return Err("请输入召唤师名称".to_string());
    }

    match method {
        SpectateMethod::Cmd => spectate_via_cmd(client, &name, lol_path).await,
        SpectateMethod::Lcu => spectate_via_lcu(client, &name, lol_path).await,
    }
}

/// 清理召唤师名：去掉首尾空白与游戏内复制带入的不可见方向标记
fn normalize_name(raw: &str) -> String {
    raw.trim().replace(['\u{2066}', '\u{2069}'], "")
}

/// LCU 方式：失败时按旧版降级到 CMD，并把两侧失败原因一起返回
async fn spectate_via_lcu(
    client: &LcuClient,
    name: &str,
    lol_path: Option<&str>,
) -> Result<String, String> {
    let summoner = yuumi_core::parsers::summoner::fetch_summoner_by_name(client, name)
        .await
        .map_err(|e| format!("未找到召唤师 \"{}\": {}", name, e))?;
    let puuid = summoner.puuid.clone();

    // 1. 好友列表中的当前对局 ID（好友正在游戏中时才有）
    let mut target_game_id: Option<String> = None;
    if let Ok(friends) = client.get("/lol-chat/v1/friends").await {
        target_game_id = friend_game_id(&friends, &puuid);
    }

    // 2. 自定义房间 / 大厅中的对局 ID
    if target_game_id.is_none() {
        if let Ok(lobby) = client.get("/lol-lobby/v2/lobby").await {
            target_game_id = lobby_game_id(&lobby);
        }
    }

    let drop_in_id = target_game_id.unwrap_or_else(|| name.to_string());
    let body = serde_json::json!({
        "allowObserveMode": "ALL",
        "dropInSpectateGameId": drop_in_id,
        "gameQueueType": "",
        "puuid": puuid,
    });

    match client
        .post("/lol-spectator/v1/spectate/launch", Some(body))
        .await
    {
        Ok(_) => Ok(format!("已启动观战: {}（对局 ID: {}）", name, drop_in_id)),
        Err(lcu_err) => {
            log::warn!("[Spectate] LCU 方式失败，尝试 CMD 兜底: {}", lcu_err);
            match spectate_via_cmd(client, name, lol_path).await {
                Ok(msg) => Ok(format!("{}（LCU 接口失败，已改用 CMD 方式）", msg)),
                Err(cmd_err) => Err(format!(
                    "观战失败: {}。CMD 兜底也失败: {}。该召唤师可能不在游戏中；\
                     自定义/新开局请先在官方客户端右键观战一次以同步密钥",
                    friendly_lcu_error(&lcu_err),
                    cmd_err
                )),
            }
        }
    }
}

/// CMD 方式：SGP 观战凭据 + 直接启动游戏本体
async fn spectate_via_cmd(
    client: &LcuClient,
    name: &str,
    lol_path: Option<&str>,
) -> Result<String, String> {
    let server = client
        .server
        .clone()
        .ok_or_else(|| "无法获取大区信息（--rso_platform_id），请重启客户端后重试".to_string())?;
    let server_lower = server.to_lowercase();
    if !TENCENT_SERVERS.contains(&server_lower.as_str()) {
        return Err(format!(
            "CMD 观战仅支持腾讯大区，当前大区 {} 不支持",
            server
        ));
    }

    let puuid = yuumi_core::parsers::summoner::fetch_summoner_by_name(client, name)
        .await
        .map_err(|e| format!("未找到召唤师 \"{}\": {}", name, e))?
        .puuid;

    let access_token = fetch_sgp_token(client).await?;
    let credentials =
        fetch_spectator_credentials(client, &server, &server_lower, &puuid, &access_token).await?;

    let game_dir = resolve_game_dir(lol_path.ok_or_else(|| {
        "未配置英雄联盟客户端路径，请在设置中配置（CMD 观战需要游戏本体路径）".to_string()
    })?)?;
    let game_exe = game_dir.join("League of Legends.exe");

    log::info!(
        "[Spectate] CMD 启动 {:?} spectator {}:{} gameId={} server={}",
        game_exe,
        credentials.observer_ip,
        credentials.observer_port,
        credentials.game_id,
        server
    );

    Command::new(&game_exe)
        .arg("spectator")
        .arg(format!(
            "{}:{}",
            credentials.observer_ip, credentials.observer_port
        ))
        .arg(&credentials.encryption_key)
        .arg(credentials.game_id.to_string())
        .arg(&server)
        .current_dir(&game_dir)
        .spawn()
        .map_err(|e| format!("启动游戏客户端失败: {}", e))?;

    Ok(format!("观战启动成功（CMD 方式），目标: {}", name))
}

struct SpectatorCredentials {
    observer_ip: String,
    observer_port: i64,
    encryption_key: String,
    game_id: i64,
}

/// 通过 LCU 端口取 SGP accessToken（`/entitlements/v1/token`）
async fn fetch_sgp_token(client: &LcuClient) -> Result<String, String> {
    let url = format!("https://127.0.0.1:{}/entitlements/v1/token", client.port);
    let resp = client
        .http_client
        .get(&url)
        .header("Authorization", client.auth_header())
        .send()
        .await
        .map_err(|e| format!("获取 SGP token 失败: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!(
            "获取 SGP token 失败: HTTP {}",
            resp.status().as_u16()
        ));
    }
    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析 SGP token 失败: {}", e))?;
    data.get("accessToken")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| "SGP token 数据中缺少 accessToken".to_string())
}

async fn fetch_spectator_credentials(
    client: &LcuClient,
    server: &str,
    server_lower: &str,
    puuid: &str,
    access_token: &str,
) -> Result<SpectatorCredentials, String> {
    let sgp_base = if K8S_SGP_SERVERS.contains(&server_lower) {
        format!("https://{}-k8s-sgp.lol.qq.com:21019", server_lower)
    } else {
        format!("https://{}-sgp.lol.qq.com:21019", server_lower)
    };
    let url = format!(
        "{}/gsm/v1/ledge/spectator/region/{}/puuid/{}",
        sgp_base, server, puuid
    );

    let resp = client
        .http_client
        .get(&url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("User-Agent", SGP_USER_AGENT)
        .send()
        .await
        .map_err(|e| format!("SGP 请求失败: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        log::warn!(
            "[Spectate] SGP 观战请求失败: HTTP {}, body: {}",
            status,
            body
        );
        if status.as_u16() == 404 || body.contains("NOT_IN_GAME") || body.contains("not found") {
            return Err("该召唤师当前不在游戏中".to_string());
        }
        return Err(format!("获取观战数据失败 (HTTP {})", status.as_u16()));
    }

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析 SGP 响应失败: {}", e))?;
    let credentials = data
        .get("playerCredentials")
        .ok_or_else(|| "该召唤师当前不在游戏中".to_string())?;

    let observer_ip = credentials
        .get("observerServerIp")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "观战凭据缺少 observerServerIp".to_string())?
        .to_string();
    let observer_port = credentials
        .get("observerServerPort")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| "观战凭据缺少 observerServerPort".to_string())?;
    let encryption_key = credentials
        .get("observerEncryptionKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "观战凭据缺少 observerEncryptionKey".to_string())?
        .to_string();
    let game_id = credentials
        .get("gameId")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| "观战凭据缺少 gameId".to_string())?;

    Ok(SpectatorCredentials {
        observer_ip,
        observer_port,
        encryption_key,
        game_id,
    })
}

fn friend_game_id(friends: &serde_json::Value, puuid: &str) -> Option<String> {
    let friend = friends.as_array()?.iter().find(|entry| {
        entry.get("puuid").and_then(|v| v.as_str()) == Some(puuid)
            || entry.get("id").and_then(|v| v.as_str()) == Some(puuid)
    })?;
    valid_game_id(friend.get("lol").and_then(|lol| lol.get("gameId")))
}

fn lobby_game_id(lobby: &serde_json::Value) -> Option<String> {
    valid_game_id(lobby.get("gameConfig").and_then(|config| config.get("id")))
}

fn valid_game_id(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value?;
    let raw = match value {
        serde_json::Value::Number(n) => {
            let id = n.as_i64().unwrap_or(0);
            if id <= 0 {
                return None;
            }
            id.to_string()
        }
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() || trimmed == "0" || trimmed.eq_ignore_ascii_case("null") {
                return None;
            }
            trimmed.to_string()
        }
        _ => return None,
    };
    Some(raw)
}

fn friendly_lcu_error(raw: &str) -> String {
    if raw.contains("404") {
        "客户端未找到可观战的对局（404）".to_string()
    } else if raw.to_uppercase().contains("ERROR_IN_GAME") {
        "请先退出当前对局/房间再发起观战".to_string()
    } else {
        raw.to_string()
    }
}

fn resolve_game_dir(lol_path: &str) -> Result<PathBuf, String> {
    let primary = Path::new(lol_path).join("Game");
    if primary.join("League of Legends.exe").exists() {
        return Ok(primary);
    }
    if let Some(parent) = Path::new(lol_path).parent() {
        let fallback = parent.join("Game");
        if fallback.join("League of Legends.exe").exists() {
            return Ok(fallback);
        }
    }
    Err(format!(
        "未找到游戏可执行文件 League of Legends.exe（已尝试 {} 及其上级 Game 目录），请在设置中确认客户端安装路径",
        primary.display()
    ))
}
