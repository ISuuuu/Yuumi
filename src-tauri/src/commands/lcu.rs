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
    let lock = app_state.lcu_client.read().await;
    let lcu = lock.as_ref().ok_or("LCU 未连接")?;

    let auth = crate::build_auth_header(&lcu.token);
    let base = format!("https://127.0.0.1:{}", lcu.port);

    // 方法1: 从 pin-drop-notification 获取 mapSide
    // 重试最多 5 次因为选人会话初始化可能稍有延迟
    let map_side_url = format!("{}/lol-champ-select/v1/pin-drop-notification", base);
    for i in 0..5 {
        if i > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(600)).await;
        }
        match lcu
            .http_client
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
    match lcu
        .http_client
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
                    // 在 5v5 中，cellId 范围 0-4 = 蓝色方，5-9 = 红色方
                    let side = if min_cell < 5 && max_cell < 5 {
                        "blue"
                    } else if min_cell >= 5 {
                        "red"
                    } else {
                        // 无法从 cellId 确定，尝试从已用的英雄 ID 推断
                        return Ok(None);
                    };
                    log::info!(
                        "获取队伍信息成功 (session cellId): {}, min={}, max={}",
                        side,
                        min_cell,
                        max_cell
                    );
                    return Ok(Some(side.to_string()));
                }
            }
        }
        _ => {}
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
    })
}
