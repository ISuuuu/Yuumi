//! 对局回放服务（LCU 回放元数据查询、下载 ROFL、唤起客户端观战播放）

use crate::lcu::client::LcuClient;

/// 查询指定对局回放当前状态（如 "checking", "downloading", "watch", "download", "unsupported", "error" 等）
pub async fn get_replay_status(client: &LcuClient, game_id: u64) -> Result<String, String> {
    let path = format!("/lol-replays/v1/metadata/{}", game_id);
    let val = match client.get(&path).await {
        Ok(v) => v,
        Err(_) => {
            // 若尚未存在本地元数据（LCU 返回 404），先触发元数据创建
            let create_path = format!("/lol-replays/v2/metadata/{}/create", game_id);
            let _ = client.post(&create_path, Some(serde_json::json!({}))).await;
            client.get(&path).await?
        }
    };
    let state = val
        .get("state")
        .and_then(|s| s.as_str())
        .or_else(|| val.as_str())
        .unwrap_or("unknown")
        .to_string();
    Ok(state)
}

/// 触发下载指定对局回放文件
pub async fn download_game_replay(client: &LcuClient, game_id: u64) -> Result<(), String> {
    // 确保本地元数据已初始化注册
    let create_path = format!("/lol-replays/v2/metadata/{}/create", game_id);
    let _ = client.post(&create_path, Some(serde_json::json!({}))).await;

    let path = format!("/lol-replays/v1/rofls/{}/download", game_id);
    let body = serde_json::json!({ "componentType": "replay-button_match-history" });
    let _ = client.post(&path, Some(body)).await?;
    Ok(())
}

/// 播放指定对局回放，唤起客户端观战
pub async fn watch_game_replay(client: &LcuClient, game_id: u64) -> Result<(), String> {
    let path = format!("/lol-replays/v1/rofls/{}/watch", game_id);
    let body = serde_json::json!({ "componentType": "replay-button_match-history" });
    let _ = client.post(&path, Some(body)).await?;
    Ok(())
}
