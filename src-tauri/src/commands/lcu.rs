use crate::lcu::game_data::CherryAugmentDetail;
use crate::AppState;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LcuConnectionDetails {
    pub pid: u32,
    pub port: u16,
    pub server: Option<String>,
}

/// 获取当前 LCU 连接信息（PID、端口、大区）
#[tauri::command]
pub async fn get_lcu_connection_info(
    app_state: tauri::State<'_, AppState>,
) -> Result<Option<LcuConnectionDetails>, String> {
    let lock = app_state.lcu_client.read().await;
    match lock.as_ref() {
        Some(client) => Ok(Some(LcuConnectionDetails {
            pid: client.pid,
            port: client.port,
            server: client.server.clone(),
        })),
        None => Ok(None),
    }
}

/// 获取选人阶段所在队伍（蓝色方/红色方）
#[tauri::command]
pub async fn get_map_side(app_state: tauri::State<'_, AppState>) -> Result<Option<String>, String> {
    // 锁内只提取连接参数，立即释放读锁，避免跨最长约 2.4s 的重试循环持有锁阻塞 monitor 重连写锁
    let (port, token, http_client) = {
        let lock = app_state.lcu_client.read().await;
        let lcu = lock.as_ref().ok_or("LCU 未连接")?;
        (lcu.port, lcu.token.clone(), lcu.http_client.clone())
    };

    let auth = crate::build_auth_header(&token);
    let base = format!("https://127.0.0.1:{}", port);

    // 方法1: 从 pin-drop-notification 获取 mapSide
    // 重试最多 5 次因为选人会话初始化可能稍有延迟
    let map_side_url = format!("{}/lol-champ-select/v1/pin-drop-notification", base);
    for i in 0..5 {
        if i > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(600)).await;
        }
        match http_client
            .get(&map_side_url)
            .header("Authorization", &auth)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(data) = resp.json::<serde_json::Value>().await {
                    if let Some(side) = data.get("mapSide").and_then(|v| v.as_str()) {
                        if !side.is_empty() {
                            log::info!("获取队伍信息成功 (pin-drop): {}", side);
                            return Ok(Some(side.to_string()));
                        }
                    }
                }
            }
            Ok(resp) => log::warn!("pin-drop-notification 返回 HTTP {}", resp.status()),
            Err(e) => log::warn!("pin-drop-notification 请求失败: {}", e),
        }
    }

    // 方法2: 读取选人会话来推断队伍
    // 如果 myTeam 的 `cellId` 小的一方为蓝色方
    let session_url = format!("{}/lol-champ-select/v1/session", base);
    match http_client
        .get(&session_url)
        .header("Authorization", &auth)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(data) = resp.json::<serde_json::Value>().await {
                let _cell_id = data
                    .get("localPlayerCellId")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                if let Some(my_team) = data.get("myTeam").and_then(|v| v.as_array()) {
                    // 检查 myTeam 中最小 cellId 来判断哪一侧
                    let min_cell = my_team
                        .iter()
                        .filter_map(|p| p.get("cellId").and_then(|c| c.as_i64()))
                        .min()
                        .unwrap_or(0);
                    let max_cell = my_team
                        .iter()
                        .filter_map(|p| p.get("cellId").and_then(|c| c.as_i64()))
                        .max()
                        .unwrap_or(0);
                    if min_cell < 5 && max_cell < 5 {
                        log::info!(
                            "获取队伍信息成功 (session cellId): blue, min={}, max={}",
                            min_cell,
                            max_cell
                        );
                        return Ok(Some("blue".to_string()));
                    } else if min_cell >= 5 {
                        log::info!(
                            "获取队伍信息成功 (session cellId): red, min={}, max={}",
                            min_cell,
                            max_cell
                        );
                        return Ok(Some("red".to_string()));
                    }
                }
            }
        }
        _ => {}
    }

    // 方法3: 如果选人阶段已结束进入游戏 (GameStart / InProgress)，尝试从 /lol-gameflow/v1/session 推断
    let gameflow_url = format!("{}/lol-gameflow/v1/session", base);
    if let Ok(resp) = http_client
        .get(&gameflow_url)
        .header("Authorization", &auth)
        .send()
        .await
    {
        if resp.status().is_success() {
            if let Ok(data) = resp.json::<serde_json::Value>().await {
                let mut local_puuid = String::new();
                let mut local_id = 0i64;

                let me_url = format!("{}/lol-summoner/v1/current-summoner", base);
                if let Ok(me_resp) = http_client
                    .get(&me_url)
                    .header("Authorization", &auth)
                    .send()
                    .await
                {
                    if me_resp.status().is_success() {
                        if let Ok(me_val) = me_resp.json::<serde_json::Value>().await {
                            local_puuid = me_val
                                .get("puuid")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            local_id = me_val
                                .get("summonerId")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0);
                        }
                    }
                }

                let check_team = |team_key: &str| -> bool {
                    if let Some(arr) = data
                        .get("gameData")
                        .and_then(|gd| gd.get(team_key))
                        .and_then(|v| v.as_array())
                    {
                        return arr.iter().any(|p| {
                            let puuid_match = !local_puuid.is_empty()
                                && p.get("puuid").and_then(|v| v.as_str()) == Some(&local_puuid);
                            let id_match = local_id > 0
                                && p.get("summonerId").and_then(|v| v.as_i64()) == Some(local_id);
                            puuid_match || id_match
                        });
                    }
                    false
                };

                if check_team("teamOne") {
                    log::info!("获取队伍信息成功 (gameflow teamOne): blue");
                    return Ok(Some("blue".to_string()));
                } else if check_team("teamTwo") {
                    log::info!("获取队伍信息成功 (gameflow teamTwo): red");
                    return Ok(Some("red".to_string()));
                }
            }
        }
    }

    // 方法4: 尝试从游戏内 LiveClientData (2999 端口) 推断
    if let Ok(Some(active_name)) = get_liveclient_active_player_name().await {
        if let Ok(players) = get_liveclient_playerlist().await {
            if let Some(target) = players.iter().find(|p| {
                p.riot_id_game_name.as_deref() == Some(&active_name)
                    || p.summoner_name.as_deref() == Some(&active_name)
                    || p.riot_id.as_deref() == Some(&active_name)
            }) {
                if let Some(ref team) = target.team {
                    let side = if team.eq_ignore_ascii_case("ORDER") {
                        "blue"
                    } else {
                        "red"
                    };
                    log::info!("获取队伍信息成功 (liveclient team): {}", side);
                    return Ok(Some(side.to_string()));
                }
            }
        }
    }

    log::warn!("无法确定队伍信息");
    Ok(None)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDataAssetsDisplay {
    pub items: std::collections::HashMap<i32, String>,
    pub spells: std::collections::HashMap<i32, String>,
    pub runes: std::collections::HashMap<i32, String>,
    pub augments: std::collections::HashMap<i32, CherryAugmentDetail>,
}

/// 获取 LCU 预加载的静态资源映射 (ID -> iconPath)
#[tauri::command]
pub async fn get_game_data_assets(
    app_state: tauri::State<'_, AppState>,
) -> Result<GameDataAssetsDisplay, String> {
    let gd = app_state.game_data.read().await;
    Ok(GameDataAssetsDisplay {
        items: gd.items.clone(),
        spells: gd.spells.clone(),
        runes: gd.runes.clone(),
        augments: gd.augments.clone(),
    })
}

/// 获取本局大乱斗板凳席我曾拥有过的英雄列表（由 ws.rs 持续写入，悬浮窗挂载时主动拉取）
#[tauri::command]
pub fn get_bench_my_champions(app_state: tauri::State<'_, AppState>) -> Vec<i64> {
    app_state
        .bench_my_champions
        .lock()
        .map(|list| list.clone())
        .unwrap_or_default()
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LiveClientPlayer {
    pub summoner_name: Option<String>,
    pub riot_id: Option<String>,
    pub riot_id_game_name: Option<String>,
    pub riot_id_tag_line: Option<String>,
    pub champion_name: Option<String>,
    pub team: Option<String>,
    pub raw_champion_name: Option<String>,
    pub is_bot: Option<bool>,
}

fn create_liveclient_http_client(timeout_ms: u64) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .no_proxy()
        .timeout(std::time::Duration::from_millis(timeout_ms))
        .build()
        .map_err(|e| format!("创建 LiveClient HTTP 客户端失败: {}", e))
}

/// 从游戏客户端内网端口 (https://127.0.0.1:2999/liveclientdata/playerlist) 获取全部玩家数据
/// 在进入游戏加载(GameStart / InProgress)后直接由游戏引擎暴露，不受敌方隐藏生涯/脱敏限制
#[tauri::command]
pub async fn get_liveclient_playerlist() -> Result<Vec<LiveClientPlayer>, String> {
    let client = create_liveclient_http_client(1500)?;

    let resp = client
        .get("https://127.0.0.1:2999/liveclientdata/playerlist")
        .send()
        .await
        .map_err(|e| format!("请求 LiveClient playerlist 失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!(
            "LiveClient playerlist 返回状态码 {}",
            resp.status()
        ));
    }

    let players: Vec<LiveClientPlayer> = resp
        .json()
        .await
        .map_err(|e| format!("解析 LiveClient playerlist 响应失败: {}", e))?;

    log::debug!("获取 LiveClient playerlist 成功: {} 名玩家", players.len());
    Ok(players)
}

/// 从游戏客户端内网端口 (https://127.0.0.1:2999/liveclientdata/activeplayername) 获取当前玩家召唤师名称
#[tauri::command]
pub async fn get_liveclient_active_player_name() -> Result<Option<String>, String> {
    let client = create_liveclient_http_client(1500)?;

    let resp = client
        .get("https://127.0.0.1:2999/liveclientdata/activeplayername")
        .send()
        .await
        .map_err(|e| format!("请求 LiveClient activeplayername 失败: {}", e))?;

    if !resp.status().is_success() {
        return Ok(None);
    }

    let raw = resp.text().await.unwrap_or_default();
    let name = raw.trim_matches('"').trim().to_string();
    if name.is_empty() {
        Ok(None)
    } else {
        log::debug!("获取 LiveClient activeplayername 成功: {}", name);
        Ok(Some(name))
    }
}
