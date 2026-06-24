use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager};

// ─── 上传数据结构 ───

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadPayload {
    pub match_info: MatchInfo,
    pub participants: Vec<ParticipantInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchInfo {
    pub match_id: u64,
    pub game_mode: String,
    pub game_type: String,
    pub queue_id: i32,
    pub game_creation: String,
    pub game_duration: u64,
    pub game_version: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub participants: Vec<ParticipantInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticipantInfo {
    pub summoner_name: String,
    pub puuid: String,
    pub team_id: i32,
    pub champion_id: i32,
    pub champion_name: String,
    pub win: bool,
    pub kills: i32,
    pub deaths: i32,
    pub assists: i32,
    pub total_damage_dealt_to_champions: i32,
    pub total_damage_taken: i32,
    pub gold_earned: i32,
    pub item0: i32,
    pub item1: i32,
    pub hextech0: i32,
    pub hextech1: i32,
    pub hextech2: i32,
}

// ─── LCU 原始数据结构 ───

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GameDetail {
    game_id: Option<u64>,
    game_mode: Option<String>,
    game_type: Option<String>,
    queue_id: Option<i32>,
    game_creation: Option<u64>,
    game_duration: Option<u64>,
    game_version: Option<String>,
    #[serde(default)]
    participants: Vec<RawParticipant>,
    #[serde(default)]
    participant_identities: Vec<ParticipantIdentity>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawParticipant {
    participant_id: Option<i32>,
    team_id: Option<i32>,
    champion_id: Option<i32>,
    #[serde(default)]
    stats: ParticipantStats,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ParticipantStats {
    #[serde(default)]
    win: bool,
    #[serde(default)]
    kills: i32,
    #[serde(default)]
    deaths: i32,
    #[serde(default)]
    assists: i32,
    #[serde(default)]
    total_damage_dealt_to_champions: i32,
    #[serde(default)]
    total_damage_taken: i32,
    #[serde(default)]
    gold_earned: i32,
    #[serde(default)]
    item0: i32,
    #[serde(default)]
    item1: i32,
    #[serde(default)]
    item6: i32,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ParticipantIdentity {
    participant_id: Option<i32>,
    player: Option<PlayerInfo>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlayerInfo {
    #[serde(default)]
    puuid: String,
    game_name: Option<String>,
    summoner_name: Option<String>,
    tag_line: Option<String>,
}

// ─── 核心逻辑：构建上传 Payload ───

/// Smart Split 策略：将当前玩家放入 matchInfo.participants，其余放入外层 participants
fn build_upload_payload(game_detail: &GameDetail, current_summoner_name: Option<&str>) -> UploadPayload {
    let game_creation_iso = game_detail
        .game_creation
        .map(|ms| {
            let secs = ms / 1000;
            let (y, m, d, h, min) = crate::parsers::match_parser::unix_secs_to_ymdhm(secs);
            format!("{:04}-{:02}-{:02}T{:02}:{:02}:00", y, m, d, h, min)
        })
        .unwrap_or_else(|| "1970-01-01T00:00:00".to_string());

    let match_info = MatchInfo {
        match_id: game_detail.game_id.unwrap_or(0),
        game_mode: game_detail.game_mode.clone().unwrap_or_default(),
        game_type: game_detail.game_type.clone().unwrap_or_default(),
        queue_id: game_detail.queue_id.unwrap_or(0),
        game_creation: game_creation_iso,
        game_duration: game_detail.game_duration.unwrap_or(0),
        game_version: game_detail.game_version.clone().unwrap_or_default(),
        participants: Vec::new(), // 稍后填充
    };

    // 构建 participantId → player 映射
    let pid_to_player: std::collections::HashMap<i32, &PlayerInfo> = game_detail
        .participant_identities
        .iter()
        .filter_map(|ident| {
            let pid = ident.participant_id?;
            let player = ident.player.as_ref()?;
            Some((pid, player))
        })
        .collect();

    // 构建所有参与者数据
    let mut all_participants: Vec<ParticipantInfo> = game_detail
        .participants
        .iter()
        .map(|p| {
            let pid = p.participant_id.unwrap_or(0);
            let player = pid_to_player.get(&pid);
            let summoner_name = player
                .map(|pl| {
                    let name = pl
                        .game_name
                        .as_deref()
                        .or(pl.summoner_name.as_deref())
                        .unwrap_or("Unknown");
                    match &pl.tag_line {
                        Some(tag) => format!("{}#{}", name, tag),
                        None => name.to_string(),
                    }
                })
                .unwrap_or_else(|| "Unknown".to_string());

            let puuid = player.map(|pl| pl.puuid.clone()).unwrap_or_default();

            // 从 item6 中提取海克斯科技装备 ID
            let (hextech0, hextech1, hextech2) = extract_hextech_ids(&p.stats);

            ParticipantInfo {
                summoner_name,
                puuid,
                team_id: p.team_id.unwrap_or(0),
                champion_id: p.champion_id.unwrap_or(0),
                champion_name: String::new(), // 前端可通过 championId 查询
                win: p.stats.win,
                kills: p.stats.kills,
                deaths: p.stats.deaths,
                assists: p.stats.assists,
                total_damage_dealt_to_champions: p.stats.total_damage_dealt_to_champions,
                total_damage_taken: p.stats.total_damage_taken,
                gold_earned: p.stats.gold_earned,
                item0: p.stats.item0,
                item1: p.stats.item1,
                hextech0,
                hextech1,
                hextech2,
            }
        })
        .collect();

    // Smart Split：将当前玩家分离到 matchInfo.participants
    let mut inner_participants = Vec::new();
    let mut outer_participants = Vec::new();

    if let Some(target_name) = current_summoner_name {
        let target_lower = target_name.to_lowercase().replace(' ', "");
        let mut found = false;

        for p in all_participants.drain(..) {
            let p_name = p
                .summoner_name
                .split('#')
                .next()
                .unwrap_or("")
                .to_lowercase()
                .replace(' ', "");
            if !found && (p_name == target_lower || p_name.contains(&target_lower)) {
                inner_participants.push(p);
                found = true;
            } else {
                outer_participants.push(p);
            }
        }

        if inner_participants.is_empty() && !outer_participants.is_empty() {
            inner_participants.push(outer_participants.remove(0));
        }
    } else if !all_participants.is_empty() {
        inner_participants.push(all_participants.remove(0));
        outer_participants = all_participants;
    }

    let mut match_info = match_info;
    match_info.participants = inner_participants;

    UploadPayload {
        match_info,
        participants: outer_participants,
    }
}

/// 从装备 stats 中提取海克斯科技装备 ID
fn extract_hextech_ids(stats: &ParticipantStats) -> (i32, i32, i32) {
    // item6 通常是海克斯科技装备槽位
    // 简化实现：item6 为主海克斯装备，item0/item1 中可能包含其他
    // 完整实现需要根据具体装备 ID 范围过滤
    let hextech0 = if is_hextech_item(stats.item6) {
        stats.item6
    } else {
        0
    };
    (hextech0, 0, 0)
}

/// 判断物品 ID 是否为海克斯科技装备（简化判断）
fn is_hextech_item(item_id: i32) -> bool {
    // 海克斯科技装备 ID 范围通常在 3000-3100 之间
    (3000..=3100).contains(&item_id)
}

// ─── 对局结束触发上传 ───

/// 对局结束时自动上传数据。
/// 获取最近一局详情 → 构建 Smart Split Payload → POST 到外部 API。
pub async fn on_game_end(app_handle: &AppHandle) {
    let (lcu_port, lcu_token, http_client) = {
        let state = app_handle.state::<crate::AppState>();
        let lock = state.lcu_client.read().await;
        match lock.as_ref() {
            Some(lcu) => (lcu.port, lcu.token.clone(), lcu.http_client.clone()),
            None => return,
        }
    };

    let auth = crate::build_auth_header(&lcu_token);
    let base = format!("https://127.0.0.1:{}", lcu_port);

    // 获取当前召唤师名称（用于 Smart Split）
    let current_name = match http_client
        .get(format!("{}/lol-summoner/v1/current-summoner", base))
        .header("Authorization", &auth)
        .send()
        .await
    {
        Ok(resp) => resp
            .json::<Value>()
            .await
            .ok()
            .and_then(|v| {
                v.get("gameName")
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string())
            }),
        Err(_) => None,
    };

    // 获取 puuid
    let puuid = match http_client
        .get(format!("{}/lol-summoner/v1/current-summoner", base))
        .header("Authorization", &auth)
        .send()
        .await
    {
        Ok(resp) => resp
            .json::<Value>()
            .await
            .ok()
            .and_then(|v| v.get("puuid").and_then(|p| p.as_str()).map(|s| s.to_string())),
        Err(_) => None,
    };

    let puuid = match puuid {
        Some(p) => p,
        None => {
            log::warn!("无法获取 puuid，跳过上传");
            return;
        }
    };

    // 获取最近一局的 gameId
    let games_url = format!(
        "{}/lol-match-history/v1/products/lol/{}/matches?begIndex=0&endIndex=1",
        base, puuid
    );
    let game_id = match http_client
        .get(&games_url)
        .header("Authorization", &auth)
        .send()
        .await
    {
        Ok(resp) => resp
            .json::<Value>()
            .await
            .ok()
            .and_then(|v| {
                v.get("games")?
                    .get("games")?
                    .as_array()?
                    .first()?
                    .get("gameId")?
                    .as_u64()
            }),
        Err(_) => None,
    };

    let game_id = match game_id {
        Some(id) => id,
        None => {
            log::warn!("无法获取最近对局 ID，跳过上传");
            return;
        }
    };

    // 获取对局详情
    let detail_url = format!("{}/lol-match-history/v1/games/{}", base, game_id);
    let game_detail: GameDetail = match http_client
        .get(&detail_url)
        .header("Authorization", &auth)
        .send()
        .await
    {
        Ok(resp) => match resp.json().await {
            Ok(d) => d,
            Err(e) => {
                log::warn!("解析对局详情失败: {}", e);
                return;
            }
        },
        Err(e) => {
            log::warn!("获取对局详情失败: {}", e);
            return;
        }
    };

    // 构建 Smart Split Payload
    let payload = build_upload_payload(&game_detail, current_name.as_deref());

    log::info!(
        "对局数据已构建: matchId={}, 内层{}人, 外层{}人",
        payload.match_info.match_id,
        payload.match_info.participants.len(),
        payload.participants.len()
    );

    // TODO: 上传到外部 API（需要在配置中添加 upload_url）
    // 当前仅日志输出，待配置系统扩展后启用实际上传
    match serde_json::to_string_pretty(&payload) {
        Ok(json) => {
            log::debug!("上传 Payload:\n{}", json);
            // 实际上传示例：
            // let upload_url = cfg.upload_url;
            // http_client.post(upload_url).json(&payload).send().await;
        }
        Err(e) => {
            log::error!("序列化上传数据失败: {}", e);
        }
    }
}
