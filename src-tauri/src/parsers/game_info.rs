use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use tauri::State;

use crate::{build_auth_header, AppState};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerFateInfo {
    pub fate_flag: Option<String>,
    pub recently_champion_name: Option<String>,
    pub game_id: Option<u64>,
    pub game_creation: Option<u64>,
    #[serde(default)]
    pub ally_count: u32,
    #[serde(default)]
    pub enemy_count: u32,
}

/// 检查单场对局中目标玩家与当前玩家的关系
fn inspect_game_fate(
    detail: &serde_json::Value,
    target_puuid: &str,
    current_summoner_id: u64,
) -> Option<(String, Option<i32>, Option<u64>)> {
    let queue_id = detail.get("queueId").and_then(|v| v.as_i64()).unwrap_or(0);
    let participants = detail.get("participants").and_then(|v| v.as_array())?;
    let identities = detail
        .get("participantIdentities")
        .and_then(|v| v.as_array())?;

    let mut current_pid: Option<i64> = None;
    let mut target_pid: Option<i64> = None;

    for ident in identities {
        let player_data = match ident.get("player") {
            Some(p) => p,
            None => continue,
        };
        let puuid = player_data
            .get("puuid")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let summoner_id = player_data
            .get("summonerId")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let pid = match ident.get("participantId").and_then(|v| v.as_i64()) {
            Some(id) => id,
            None => continue,
        };

        if puuid == target_puuid {
            target_pid = Some(pid);
        }
        if summoner_id == current_summoner_id {
            current_pid = Some(pid);
        }
    }

    let current_pid = current_pid?;
    let target_pid = target_pid?;

    if current_pid == target_pid {
        return None;
    }

    let mut target_champion_id: Option<i32> = None;
    let mut current_team: Option<i64> = None;
    let mut target_team: Option<i64> = None;

    for p in participants {
        let pid = match p.get("participantId").and_then(|v| v.as_i64()) {
            Some(id) => id,
            None => continue,
        };

        let team_val = if queue_id == 1700 {
            p.get("stats")
                .and_then(|s| s.get("subteamPlacement"))
                .and_then(|v| v.as_i64())
        } else {
            p.get("teamId").and_then(|v| v.as_i64())
        };

        if pid == current_pid {
            current_team = team_val;
        }
        if pid == target_pid {
            target_team = team_val;
            target_champion_id = p
                .get("championId")
                .and_then(|v| v.as_i64())
                .map(|id| id as i32);
        }
    }

    let ct = current_team?;
    let tt = target_team?;
    let flag = if ct == tt {
        "ally".to_string()
    } else {
        "enemy".to_string()
    };
    let game_creation = detail.get("gameCreation").and_then(|v| v.as_u64());
    Some((flag, target_champion_id, game_creation))
}

// ─── 对局详情进程内缓存 ───
// 宿命检测中同局玩家的候选 gameId 高度重叠，且已结束对局的详情不可变，
// 缓存可避免同一 gameId 的完整详情被并发/重复下载；容量满时整体清空即可。

const GAME_DETAIL_CACHE_CAP: usize = 32;

fn game_detail_cache() -> &'static Mutex<HashMap<u64, Arc<serde_json::Value>>> {
    static CACHE: OnceLock<Mutex<HashMap<u64, Arc<serde_json::Value>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn game_detail_cache_get(game_id: u64) -> Option<Arc<serde_json::Value>> {
    game_detail_cache()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(&game_id)
        .cloned()
}

fn game_detail_cache_insert(game_id: u64, detail: Arc<serde_json::Value>) {
    let mut cache = game_detail_cache()
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if cache.len() >= GAME_DETAIL_CACHE_CAP {
        cache.clear();
    }
    cache.insert(game_id, detail);
}

/// 前端独立调用的单个玩家宿命获取接口（传入候选 game_id 列表，按顺序取最近一场共同对局并统计历史交手次数）
#[tauri::command]
pub async fn get_player_fate_info(
    game_ids: Option<Vec<u64>>,
    target_puuid: String,
    current_summoner_id: u64,
    app_state: State<'_, AppState>,
) -> Result<PlayerFateInfo, String> {
    // 去重并保留传入顺序（调用方保证越靠前优先级越高）
    let mut candidate_game_ids = Vec::new();
    if let Some(ids) = game_ids {
        for id in ids {
            if id > 0 && !candidate_game_ids.contains(&id) {
                candidate_game_ids.push(id);
            }
        }
    }

    if candidate_game_ids.is_empty() {
        return Ok(PlayerFateInfo {
            fate_flag: None,
            recently_champion_name: None,
            game_id: None,
            game_creation: None,
            ally_count: 0,
            enemy_count: 0,
        });
    }

    // 锁内只提取连接参数，尽早释放读锁，避免 HTTP 请求期间阻塞 monitor 重连
    let (auth, base, http_client) = {
        let lock = app_state.lcu().await?;
        let lcu = lock.as_ref().ok_or("LCU未连接")?;
        (
            build_auth_header(&lcu.token),
            format!("https://127.0.0.1:{}", lcu.port),
            lcu.http_client.clone(),
        )
    };

    // 复用 LCU 并发信号量，避免宿命检测打满 LCU 连接
    let semaphore = {
        let lock = app_state.api_semaphore.read().await;
        lock.clone()
    };

    // 并发拉取候选对局详情，结果按候选顺序回填，保证"最先命中"取的是优先级最高的候选。
    // 详情先查进程内缓存（命中不占信号量），未命中才请求 LCU 并回填缓存
    let mut handles = Vec::with_capacity(candidate_game_ids.len());
    for &gid in &candidate_game_ids {
        let auth = auth.clone();
        let base = base.clone();
        let http_client = http_client.clone();
        let semaphore = semaphore.clone();
        let target_puuid = target_puuid.clone();
        handles.push(tokio::spawn(async move {
            if let Some(detail) = game_detail_cache_get(gid) {
                return inspect_game_fate(&detail, &target_puuid, current_summoner_id);
            }
            let _permit = match semaphore.acquire().await {
                Ok(p) => p,
                Err(_) => return None,
            };
            let url = format!("{}/lol-match-history/v1/games/{}", base, gid);
            let resp = match http_client
                .get(&url)
                .header("Authorization", &auth)
                .send()
                .await
            {
                Ok(r) => r,
                Err(_) => return None,
            };
            let detail: serde_json::Value = match resp.json().await {
                Ok(d) => d,
                Err(_) => return None,
            };
            let detail = Arc::new(detail);
            game_detail_cache_insert(gid, detail.clone());
            inspect_game_fate(&detail, &target_puuid, current_summoner_id)
        }));
    }

    let mut first_flag: Option<String> = None;
    let mut first_cid: Option<i32> = None;
    let mut first_game_id: Option<u64> = None;
    let mut first_game_creation: Option<u64> = None;
    let mut ally_count = 0u32;
    let mut enemy_count = 0u32;

    for (idx, h) in handles.into_iter().enumerate() {
        if let Ok(Some((flag, cid, creation))) = h.await {
            if flag == "ally" {
                ally_count += 1;
            } else {
                enemy_count += 1;
            }
            if first_flag.is_none() {
                first_flag = Some(flag);
                first_cid = cid;
                first_game_id = Some(candidate_game_ids[idx]);
                first_game_creation = creation;
            }
        }
    }

    let recently_champion_name = if let Some(cid) = first_cid {
        let assets = app_state.game_data.read().await;
        assets.champions.get(&cid).cloned()
    } else {
        None
    };

    Ok(PlayerFateInfo {
        fate_flag: first_flag,
        recently_champion_name,
        game_id: first_game_id,
        game_creation: first_game_creation,
        ally_count,
        enemy_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn inspect_game_fate_extracts_relation_champion_and_game_creation() {
        let detail = json!({
            "queueId": 450,
            "gameCreation": 1_705_329_000_000_u64,
            "participantIdentities": [
                { "participantId": 1, "player": { "puuid": "me-puuid", "summonerId": 1001 } },
                { "participantId": 2, "player": { "puuid": "ally-puuid", "summonerId": 1002 } },
                { "participantId": 6, "player": { "puuid": "enemy-puuid", "summonerId": 2001 } }
            ],
            "participants": [
                { "participantId": 1, "teamId": 100, "championId": 81 },
                { "participantId": 2, "teamId": 100, "championId": 64 },
                { "participantId": 6, "teamId": 200, "championId": 157 }
            ]
        });

        let ally_res = inspect_game_fate(&detail, "ally-puuid", 1001);
        assert_eq!(
            ally_res,
            Some(("ally".to_string(), Some(64), Some(1_705_329_000_000)))
        );

        let enemy_res = inspect_game_fate(&detail, "enemy-puuid", 1001);
        assert_eq!(
            enemy_res,
            Some(("enemy".to_string(), Some(157), Some(1_705_329_000_000)))
        );

        // 自身不判定关系
        assert!(inspect_game_fate(&detail, "me-puuid", 1001).is_none());
    }
}
