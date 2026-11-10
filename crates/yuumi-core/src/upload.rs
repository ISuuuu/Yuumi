use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadPayload {
    pub match_info: MatchInfo,
    pub participants: Vec<ParticipantInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchInfo {
    pub match_id: u64,
    pub game_mode: String,
    pub game_type: String,
    pub queue_id: i32,
    pub game_creation: String,
    pub game_duration: u64,
    pub game_version: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub participants: Vec<ParticipantInfo>,
}

/// 上传的 participant 字段严格对齐 Python build_upload_payload
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticipantInfo {
    pub summoner_name: String,
    pub puuid: String,
    pub team_id: i32,
    pub champion_id: i32,
    pub champion_name: String,
    // Python: SummonerSpell1Id / SummonerSpell2Id（首字母大写）
    #[serde(rename = "SummonerSpell1Id")]
    pub summoner_spell1_id: i32,
    #[serde(rename = "SummonerSpell2Id")]
    pub summoner_spell2_id: i32,
    // Python: HexTech0 ~ HexTech4（首字母大写）
    #[serde(rename = "HexTech0")]
    pub hextech0: i32,
    #[serde(rename = "HexTech1")]
    pub hextech1: i32,
    #[serde(rename = "HexTech2")]
    pub hextech2: i32,
    #[serde(rename = "HexTech3")]
    pub hextech3: i32,
    #[serde(rename = "HexTech4")]
    pub hextech4: i32,
    pub win: bool,
    pub kills: i32,
    pub deaths: i32,
    pub assists: i32,
    // 伤害数据（对英雄）
    pub total_damage_dealt_to_champions: i32,
    pub physical_damage_dealt_to_champions: i32,
    pub magic_damage_dealt_to_champions: i32,
    pub true_damage_dealt_to_champions: i32,
    // 伤害数据（所有目标）
    pub total_damage_dealt: i32,
    pub physical_damage_dealt: i32,
    pub magic_damage_dealt: i32,
    pub true_damage_dealt: i32,
    // 承受伤害
    pub total_damage_taken: i32,
    pub physical_damage_taken: i32,
    pub magical_damage_taken: i32,
    // 治疗
    pub total_heal: i32,
    // 经济与补刀
    pub gold_earned: i32,
    pub gold_spent: i32,
    pub total_minions_killed: i32,
    pub neutral_minions_killed: i32,
    // 装备
    pub item0: i32,
    pub item1: i32,
    pub item2: i32,
    pub item3: i32,
    pub item4: i32,
    pub item5: i32,
    pub item6: i32,
    pub role_bound_item: i32,
    // 视野
    pub vision_score: i32,
    pub wards_placed: i32,
    pub wards_killed: i32,
    pub vision_wards_bought_in_game: i32,
    // 英雄等级
    pub champ_level: i32,
    // 多杀
    pub double_kills: i32,
    pub triple_kills: i32,
    pub quadra_kills: i32,
    pub penta_kills: i32,
    pub largest_multi_kill: i32,
    // 连杀
    pub largest_killing_spree: i32,
    pub killing_sprees: i32,
    // 全场最多标识
    pub most_kills: bool,
    pub most_assists: bool,
    pub most_damage_dealt: bool,
    pub most_damage_taken: bool,
    pub most_gold_earned: bool,
    pub most_turret_kills: bool,
    pub most_healing_done: bool,
    // 目标
    pub turret_kills: i32,
    pub inhibitor_kills: i32,
    pub first_blood_kill: bool,
    pub first_blood_assist: bool,
    pub first_tower_kill: bool,
    pub first_tower_assist: bool,
    // 伤害（对塔/目标）
    pub damage_dealt_to_turrets: i32,
    pub damage_dealt_to_objectives: i32,
    // 符文
    pub perk_primary_style: i32,
    pub perk_sub_style: i32,
}

// ─── LCU 原始数据结构 ───

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDetail {
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
    spell1_id: Option<i32>,
    spell2_id: Option<i32>,
    #[serde(default)]
    stats: ParticipantStats,
    // participant 层的海克斯强化数据（对齐 Python extract_hextech_ids 从 participant 读取）
    #[serde(default)]
    augments: Vec<i32>,
    #[serde(default)]
    player_augment1: i32,
    #[serde(default)]
    player_augment2: i32,
    #[serde(default)]
    player_augment3: i32,
    #[serde(default)]
    player_augment4: i32,
    #[serde(default)]
    player_augment5: i32,
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
    // 伤害（对英雄）
    #[serde(default)]
    total_damage_dealt_to_champions: i32,
    #[serde(default)]
    physical_damage_dealt_to_champions: i32,
    #[serde(default)]
    magic_damage_dealt_to_champions: i32,
    #[serde(default)]
    true_damage_dealt_to_champions: i32,
    // 伤害（所有目标）
    #[serde(default)]
    total_damage_dealt: i32,
    #[serde(default)]
    physical_damage_dealt: i32,
    #[serde(default)]
    magic_damage_dealt: i32,
    #[serde(default)]
    true_damage_dealt: i32,
    // 承受伤害
    #[serde(default)]
    total_damage_taken: i32,
    #[serde(default)]
    physical_damage_taken: i32,
    #[serde(default)]
    magical_damage_taken: i32,
    // 治疗
    #[serde(default)]
    total_heal: i32,
    // 经济与补刀
    #[serde(default)]
    gold_earned: i32,
    #[serde(default)]
    gold_spent: i32,
    #[serde(default)]
    total_minions_killed: i32,
    #[serde(default)]
    neutral_minions_killed: i32,
    // 装备
    #[serde(default)]
    item0: i32,
    #[serde(default)]
    item1: i32,
    #[serde(default)]
    item2: i32,
    #[serde(default)]
    item3: i32,
    #[serde(default)]
    item4: i32,
    #[serde(default)]
    item5: i32,
    #[serde(default)]
    item6: i32,
    #[serde(default)]
    role_bound_item: i32,
    // 视野
    #[serde(default)]
    vision_score: i32,
    #[serde(default)]
    wards_placed: i32,
    #[serde(default)]
    wards_killed: i32,
    #[serde(default)]
    vision_wards_bought_in_game: i32,
    // 英雄等级
    #[serde(default)]
    champ_level: i32,
    // 多杀
    #[serde(default)]
    double_kills: i32,
    #[serde(default)]
    triple_kills: i32,
    #[serde(default)]
    quadra_kills: i32,
    #[serde(default)]
    penta_kills: i32,
    #[serde(default)]
    largest_multi_kill: i32,
    // 连杀
    #[serde(default)]
    largest_killing_spree: i32,
    #[serde(default)]
    killing_sprees: i32,
    // 全场最多标识
    #[serde(default)]
    most_kills: bool,
    #[serde(default)]
    most_assists: bool,
    #[serde(default)]
    most_damage_dealt: bool,
    #[serde(default)]
    most_damage_taken: bool,
    #[serde(default)]
    most_gold_earned: bool,
    #[serde(default)]
    most_turret_kills: bool,
    #[serde(default)]
    most_healing_done: bool,
    // 目标
    #[serde(default)]
    turret_kills: i32,
    #[serde(default)]
    inhibitor_kills: i32,
    #[serde(default)]
    first_blood_kill: bool,
    #[serde(default)]
    first_blood_assist: bool,
    #[serde(default)]
    first_tower_kill: bool,
    #[serde(default)]
    first_tower_assist: bool,
    // 伤害（对塔/目标）
    #[serde(default)]
    damage_dealt_to_turrets: i32,
    #[serde(default)]
    damage_dealt_to_objectives: i32,
    // 符文
    #[serde(default)]
    perk_primary_style: i32,
    #[serde(default)]
    perk_sub_style: i32,
    // 海克斯强化（用于 extract_hextech_ids）
    #[serde(default)]
    augments: Vec<i32>,
    #[serde(default)]
    player_augment1: i32,
    #[serde(default)]
    player_augment2: i32,
    #[serde(default)]
    player_augment3: i32,
    #[serde(default)]
    player_augment4: i32,
    #[serde(default)]
    player_augment5: i32,
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

// ─── Smart Split 构建 ───

pub fn build_upload_payload(
    game_detail: &GameDetail,
    current_summoner_puuid: Option<&str>,
    champion_names: &std::collections::HashMap<i32, String>,
) -> UploadPayload {
    let game_creation_iso = game_detail
        .game_creation
        .and_then(|ms| {
            let secs = (ms / 1000) as i64;
            chrono::DateTime::from_timestamp(secs, 0).map(|dt| {
                dt.with_timezone(&chrono::Local)
                    .format("%Y-%m-%dT%H:%M:%S")
                    .to_string()
            })
        })
        .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string());

    let match_info = MatchInfo {
        match_id: game_detail.game_id.unwrap_or(0),
        game_mode: game_detail.game_mode.clone().unwrap_or_default(),
        game_type: game_detail.game_type.clone().unwrap_or_default(),
        queue_id: game_detail.queue_id.unwrap_or(0),
        game_creation: game_creation_iso,
        game_duration: game_detail.game_duration.unwrap_or(0),
        game_version: game_detail.game_version.clone().unwrap_or_default(),
        participants: Vec::new(),
    };

    let pid_to_player: std::collections::HashMap<i32, &PlayerInfo> = game_detail
        .participant_identities
        .iter()
        .filter_map(|ident| {
            let pid = ident.participant_id?;
            let player = ident.player.as_ref()?;
            Some((pid, player))
        })
        .collect();

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
            let hextech = extract_hextech_ids(&p.stats, p);

            ParticipantInfo {
                summoner_name,
                puuid,
                team_id: p.team_id.unwrap_or(0),
                champion_id: p.champion_id.unwrap_or(0),
                champion_name: champion_names
                    .get(&p.champion_id.unwrap_or(0))
                    .cloned()
                    .unwrap_or_else(|| "Unknown".to_string()),
                summoner_spell1_id: p.spell1_id.unwrap_or(0),
                summoner_spell2_id: p.spell2_id.unwrap_or(0),
                hextech0: hextech[0],
                hextech1: hextech[1],
                hextech2: hextech[2],
                hextech3: hextech[3],
                hextech4: hextech[4],
                win: p.stats.win,
                kills: p.stats.kills,
                deaths: p.stats.deaths,
                assists: p.stats.assists,
                total_damage_dealt_to_champions: p.stats.total_damage_dealt_to_champions,
                physical_damage_dealt_to_champions: p.stats.physical_damage_dealt_to_champions,
                magic_damage_dealt_to_champions: p.stats.magic_damage_dealt_to_champions,
                true_damage_dealt_to_champions: p.stats.true_damage_dealt_to_champions,
                total_damage_dealt: p.stats.total_damage_dealt,
                physical_damage_dealt: p.stats.physical_damage_dealt,
                magic_damage_dealt: p.stats.magic_damage_dealt,
                true_damage_dealt: p.stats.true_damage_dealt,
                total_damage_taken: p.stats.total_damage_taken,
                physical_damage_taken: p.stats.physical_damage_taken,
                magical_damage_taken: p.stats.magical_damage_taken,
                total_heal: p.stats.total_heal,
                gold_earned: p.stats.gold_earned,
                gold_spent: p.stats.gold_spent,
                total_minions_killed: p.stats.total_minions_killed,
                neutral_minions_killed: p.stats.neutral_minions_killed,
                item0: p.stats.item0,
                item1: p.stats.item1,
                item2: p.stats.item2,
                item3: p.stats.item3,
                item4: p.stats.item4,
                item5: p.stats.item5,
                item6: p.stats.item6,
                role_bound_item: p.stats.role_bound_item,
                vision_score: p.stats.vision_score,
                wards_placed: p.stats.wards_placed,
                wards_killed: p.stats.wards_killed,
                vision_wards_bought_in_game: p.stats.vision_wards_bought_in_game,
                champ_level: p.stats.champ_level,
                double_kills: p.stats.double_kills,
                triple_kills: p.stats.triple_kills,
                quadra_kills: p.stats.quadra_kills,
                penta_kills: p.stats.penta_kills,
                largest_multi_kill: p.stats.largest_multi_kill,
                largest_killing_spree: p.stats.largest_killing_spree,
                killing_sprees: p.stats.killing_sprees,
                most_kills: p.stats.most_kills,
                most_assists: p.stats.most_assists,
                most_damage_dealt: p.stats.most_damage_dealt,
                most_damage_taken: p.stats.most_damage_taken,
                most_gold_earned: p.stats.most_gold_earned,
                most_turret_kills: p.stats.most_turret_kills,
                most_healing_done: p.stats.most_healing_done,
                turret_kills: p.stats.turret_kills,
                inhibitor_kills: p.stats.inhibitor_kills,
                first_blood_kill: p.stats.first_blood_kill,
                first_blood_assist: p.stats.first_blood_assist,
                first_tower_kill: p.stats.first_tower_kill,
                first_tower_assist: p.stats.first_tower_assist,
                damage_dealt_to_turrets: p.stats.damage_dealt_to_turrets,
                damage_dealt_to_objectives: p.stats.damage_dealt_to_objectives,
                perk_primary_style: p.stats.perk_primary_style,
                perk_sub_style: p.stats.perk_sub_style,
            }
        })
        .collect();

    // Smart Split
    let mut inner_participants = Vec::new();
    let mut outer_participants = Vec::new();

    if let Some(target_puuid) = current_summoner_puuid {
        let mut found = false;

        for p in all_participants.drain(..) {
            if !found && p.puuid == target_puuid {
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

/// 对齐 Python _extract_augment_ids + extract_hextech_ids。
/// 同时从 stats 层和 participant 层提取 augments + playerAugment1~5，去重后返回 5 个值。
fn extract_hextech_ids(stats: &ParticipantStats, participant: &RawParticipant) -> [i32; 5] {
    let mut seen = std::collections::HashSet::new();
    let mut ordered: Vec<i32> = Vec::new();

    // 对齐 Python: for source in (stats, participant)
    // 来源 1: stats 层
    for &id in &stats.augments {
        if id != 0 && seen.insert(id) {
            ordered.push(id);
        }
    }
    for id in [
        stats.player_augment1,
        stats.player_augment2,
        stats.player_augment3,
        stats.player_augment4,
        stats.player_augment5,
    ] {
        if id != 0 && seen.insert(id) {
            ordered.push(id);
        }
    }

    // 来源 2: participant 层（对齐 Python 从 participant 读取）
    for &id in &participant.augments {
        if id != 0 && seen.insert(id) {
            ordered.push(id);
        }
    }
    for id in [
        participant.player_augment1,
        participant.player_augment2,
        participant.player_augment3,
        participant.player_augment4,
        participant.player_augment5,
    ] {
        if id != 0 && seen.insert(id) {
            ordered.push(id);
        }
    }

    let mut result = [0i32; 5];
    for (i, &val) in ordered.iter().take(5).enumerate() {
        result[i] = val;
    }
    result
}

use crate::lcu::client::LcuClient;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::Mutex;

/// 持久化上传队列；每次上传前落盘，保证进程中断后可恢复。
#[derive(Clone)]
pub struct UploadQueue {
    db_path: PathBuf,
    client: reqwest::Client,
    lock: Arc<Mutex<()>>,
    completed: Arc<Mutex<std::collections::HashSet<u64>>>,
}

impl UploadQueue {
    pub fn new(db_path: PathBuf) -> Result<Self, String> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let queue = Self {
            db_path,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|e| e.to_string())?,
            lock: Arc::new(Mutex::new(())),
            completed: Arc::new(Mutex::new(std::collections::HashSet::new())),
        };
        queue.open()?.execute_batch("CREATE TABLE IF NOT EXISTS pending_uploads (game_id INTEGER PRIMARY KEY, payload TEXT NOT NULL, retry_count INTEGER NOT NULL DEFAULT 0, last_error TEXT NOT NULL DEFAULT '')").map_err(|e| e.to_string())?;
        Ok(queue)
    }
    fn open(&self) -> Result<rusqlite::Connection, String> {
        rusqlite::Connection::open(&self.db_path).map_err(|e| e.to_string())
    }
    /// 构建完整载荷；读取 LCU 时不使用外部服务 HTTP 客户端。
    pub async fn fetch_payload(
        lcu: &LcuClient,
        game_id: u64,
        champions: &std::collections::HashMap<i32, String>,
    ) -> Result<UploadPayload, String> {
        let summoner = lcu
            .request("GET", "/lol-summoner/v1/current-summoner", None, false)
            .await?;
        let puuid = summoner
            .get("puuid")
            .and_then(|v| v.as_str())
            .filter(|value| !value.is_empty())
            .ok_or("当前召唤师缺少 puuid")?;
        let detail = lcu
            .request(
                "GET",
                &format!("/lol-match-history/v1/games/{game_id}"),
                None,
                false,
            )
            .await?;
        let detail: GameDetail = serde_json::from_value(detail).map_err(|e| e.to_string())?;
        ensure_membership(&detail, puuid)?;
        let payload = build_upload_payload(&detail, Some(puuid), champions);
        if payload.match_info.match_id != game_id || game_id == 0 {
            return Err("对局 ID 无效或不匹配".into());
        }
        Ok(payload)
    }
    /// 持久化并立即尝试上传；串行处理同一队列，避免重复并发请求。
    pub async fn enqueue(&self, base_url: &str, payload: UploadPayload) -> Result<String, String> {
        if base_url.trim().is_empty() {
            return Err("未配置上传 API 地址".into());
        }
        validate_url(base_url, false)?;
        let _guard = self.lock.lock().await;
        let id = payload.match_info.match_id;
        if id == 0 {
            return Err("对局 ID 无效".into());
        }
        if self.completed.lock().await.contains(&id) {
            return Ok("exists".into());
        }
        let json = serde_json::to_string(&payload).map_err(|e| e.to_string())?;
        {
            let conn = self.open()?;
            conn.execute("INSERT INTO pending_uploads(game_id,payload) VALUES(?1,?2) ON CONFLICT(game_id) DO UPDATE SET payload=excluded.payload", rusqlite::params![id as i64,json]).map_err(|e| e.to_string())?;
            prune_pending(&conn)?;
        }
        self.send_pending(base_url, &payload).await
    }
    async fn send_pending(
        &self,
        base_url: &str,
        payload: &UploadPayload,
    ) -> Result<String, String> {
        let result = self.post_single(base_url, payload).await;
        let conn = self.open()?;
        match &result {
            Ok(_) => {
                conn.execute(
                    "DELETE FROM pending_uploads WHERE game_id=?1",
                    [payload.match_info.match_id as i64],
                )
                .map_err(|e| e.to_string())?;
            }
            Err(e) => {
                conn.execute("UPDATE pending_uploads SET retry_count=retry_count+1,last_error=?1 WHERE game_id=?2", rusqlite::params![e,payload.match_info.match_id as i64]).map_err(|e| e.to_string())?;
            }
        }
        if result.is_ok() {
            self.completed
                .lock()
                .await
                .insert(payload.match_info.match_id);
        }
        result
    }
    /// 发送单条载荷：30 秒预算内最多尝试 6 次（对齐旧版 `post_payload_to_api`）。
    /// 4xx 客户端错误（参数/鉴权等）重试无意义，当场直接放弃（仍会落盘等待人工/后续触发）。
    async fn post_single(&self, base_url: &str, payload: &UploadPayload) -> Result<String, String> {
        const UPLOAD_RETRY_BUDGET: Duration = Duration::from_secs(30);
        let deadline = tokio::time::Instant::now() + UPLOAD_RETRY_BUDGET;
        let mut last_err = String::new();
        for attempt in 0..=5u32 {
            // 预算耗尽后立即返回 Err，让调用方落盘而非被外层超时打断
            if tokio::time::Instant::now() >= deadline {
                last_err = format!("重试预算耗尽: {last_err}");
                break;
            }
            let response = self
                .client
                .post(upload_url(base_url, false))
                .json(payload)
                .send()
                .await;
            match response {
                Ok(response) => {
                    let status = response.status();
                    if status.is_client_error() {
                        return Err(format!(
                            "对局 {} 上传被拒绝: HTTP {} (客户端错误不重试)",
                            payload.match_info.match_id, status
                        ));
                    }
                    if status.is_success() {
                        match response.json::<serde_json::Value>().await {
                            Ok(json) => {
                                if json.get("success").and_then(|v| v.as_bool()) == Some(true) {
                                    return Ok(
                                        if json.pointer("/data/isNewMatch").and_then(|v| v.as_bool())
                                            == Some(true)
                                        {
                                            "new"
                                        } else {
                                            "exists"
                                        }
                                        .into(),
                                    );
                                }
                                last_err = json
                                    .get("message")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("服务器处理失败")
                                    .to_string();
                            }
                            Err(_) => last_err = "解析响应失败".into(),
                        }
                    } else {
                        last_err = format!("HTTP {status}");
                    }
                }
                Err(e) => last_err = format!("请求失败: {e}"),
            }
            if attempt < 5 {
                let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                if remaining.is_zero() {
                    break;
                }
                log::warn!(
                    "[Upload] 对局 {} 上传失败 ({}), 重试 {}/5",
                    payload.match_info.match_id,
                    last_err,
                    attempt + 1
                );
                tokio::time::sleep(Duration::from_secs(1).min(remaining)).await;
            }
        }
        Err(format!(
            "对局 {} 上传失败: {last_err}",
            payload.match_info.match_id
        ))
    }
    /// 重试持久化记录，最多自动尝试十次；无地址时不会发送请求。
    pub async fn retry_pending(&self, base_url: &str) -> Result<usize, String> {
        if base_url.trim().is_empty() {
            return Ok(0);
        }
        validate_url(base_url, false)?;
        let _guard = self.lock.lock().await;
        let payloads: Vec<UploadPayload> = {
            let conn = self.open()?;
            let mut stmt = conn
                .prepare(
                    "SELECT payload FROM pending_uploads WHERE retry_count < 10 ORDER BY rowid",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|e| e.to_string())?;
            let mut items = Vec::new();
            for row in rows {
                items.push(
                    serde_json::from_str(&row.map_err(|e| e.to_string())?)
                        .map_err(|e| e.to_string())?,
                );
            }
            items
        };
        let mut success = 0;
        for payload in payloads {
            if let Err(e) = self.send_pending(base_url, &payload).await {
                log::warn!("[Upload] 待传对局 {} 重试失败: {e}", payload.match_info.match_id);
            } else {
                success += 1;
            }
        }
        Ok(success)
    }
    /// 与旧服务一致，批量请求按十局分组，失败项保留为待传。
    pub async fn upload_batch(
        &self,
        base_url: &str,
        payloads: &[UploadPayload],
    ) -> Result<(u32, u32), String> {
        if base_url.trim().is_empty() {
            return Err("未配置上传 API 地址".into());
        }
        validate_url(base_url, true)?;
        let _guard = self.lock.lock().await;
        let mut success = 0;
        let mut failed = 0;
        let completed = self.completed.lock().await.clone();
        let mut seen = completed;
        let pending: Vec<_> = payloads
            .iter()
            .filter(|p| seen.insert(p.match_info.match_id))
            .cloned()
            .collect();
        for chunk in pending.chunks(10) {
            {
                let conn = self.open()?;
                for p in chunk {
                    if p.match_info.match_id == 0 {
                        return Err("对局 ID 无效".into());
                    }
                    conn.execute("INSERT INTO pending_uploads(game_id,payload) VALUES(?1,?2) ON CONFLICT(game_id) DO UPDATE SET payload=excluded.payload", rusqlite::params![p.match_info.match_id as i64, serde_json::to_string(p).map_err(|e| e.to_string())?]).map_err(|e| e.to_string())?;
                }
                prune_pending(&conn)?;
            }
            let response = self
                .client
                .post(upload_url(base_url, true))
                .json(chunk)
                .send()
                .await;
            let data = match response {
                Ok(r) if r.status().is_success() => r.json::<serde_json::Value>().await.ok(),
                _ => None,
            };
            if let Some(data) =
                data.filter(|d| d.get("success").and_then(|v| v.as_bool()) == Some(true))
            {
                let count = data
                    .pointer("/data/successCount")
                    .or_else(|| data.pointer("/data/newMatches"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(chunk.len() as u64) as u32;
                let errors = data
                    .pointer("/data/failedCount")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                success += count;
                failed += errors;
                // 服务端不返回失败 ID，部分失败时保留整个批次，以服务端 matchId 去重重试。
                if errors == 0 {
                    for p in chunk {
                        self.completed.lock().await.insert(p.match_info.match_id);
                        self.open()?
                            .execute(
                                "DELETE FROM pending_uploads WHERE game_id=?1",
                                [p.match_info.match_id as i64],
                            )
                            .map_err(|e| e.to_string())?;
                    }
                }
            } else {
                failed += chunk.len() as u32;
            }
        }
        Ok((success, failed))
    }
}

/// 保留最新 100 条待传记录，清理更旧的行（对齐旧版 add_or_update_pending_upload 的清理策略；
/// game_id 为 INTEGER PRIMARY KEY 即 rowid 别名，按 rowid DESC 即插入顺序倒序）。
fn prune_pending(conn: &rusqlite::Connection) -> Result<(), String> {
    conn.execute(
        "DELETE FROM pending_uploads WHERE game_id NOT IN (
            SELECT game_id FROM pending_uploads ORDER BY rowid DESC LIMIT 100
        )",
        [],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

fn ensure_membership(detail: &GameDetail, puuid: &str) -> Result<(), String> {
    if puuid.is_empty()
        || !detail.participant_identities.iter().any(|identity| {
            identity
                .player
                .as_ref()
                .is_some_and(|player| player.puuid == puuid)
        })
    {
        return Err("对局不包含当前召唤师".into());
    }
    Ok(())
}

fn validate_url(base: &str, batch: bool) -> Result<(), String> {
    let trimmed = base.trim();
    if trimmed.contains("://")
        && !trimmed.starts_with("http://")
        && !trimmed.starts_with("https://")
    {
        return Err("上传地址无效".into());
    }
    let normalized = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    };
    let url = url::Url::parse(&normalized).map_err(|e| format!("上传地址无效: {e}"))?;
    let _ = batch;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("上传地址无效".into());
    }
    Ok(())
}

fn upload_url(base: &str, batch: bool) -> String {
    let base = base.trim().trim_end_matches('/');
    let mut url = if base.starts_with("http://") || base.starts_with("https://") {
        base.to_string()
    } else {
        format!("http://{base}")
    };
    if batch {
        if url.ends_with("/api/lol/upload") {
            url.truncate(url.len() - "/api/lol/upload".len());
        }
        url.push_str("/api/lol/upload-batch");
    } else if !url.contains("/api/lol/upload") {
        url.push_str("/api/lol/upload");
    }
    url
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn payload(id: u64) -> UploadPayload {
        UploadPayload {
            match_info: MatchInfo {
                match_id: id,
                game_mode: "CLASSIC".into(),
                game_type: "MATCHED_GAME".into(),
                queue_id: 420,
                game_creation: "2025-01-01T00:00:00".into(),
                game_duration: 100,
                game_version: "1".into(),
                participants: vec![],
            },
            participants: vec![],
        }
    }
    fn db_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "yuumi-upload-{}-{}.sqlite",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }
    async fn server(status: &str, body: &str) -> (String, tokio::task::JoinHandle<String>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("address");
        let response = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\nContent-Type: application/json\r\n\r\n{body}", body.len());
        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let mut data = Vec::new();
            let mut buf = [0; 4096];
            loop {
                let n = stream.read(&mut buf).await.expect("read");
                if n == 0 {
                    break;
                }
                data.extend_from_slice(&buf[..n]);
                if let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&data[..end]);
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|v| v.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if data.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            stream.write_all(response.as_bytes()).await.expect("write");
            String::from_utf8(data).expect("request")
        });
        (format!("http://{addr}"), handle)
    }
    /// 持续返回指定状态并接受全部连接；记录首个请求与请求总数，
    /// 由调用方在断言后 abort 任务（无限循环 accept，JoinHandle 永不自然结束）。
    async fn loop_server(
        status_line: &str,
        body: &str,
    ) -> (
        String,
        std::sync::Arc<std::sync::atomic::AtomicUsize>,
        std::sync::Arc<tokio::sync::Mutex<String>>,
        tokio::task::JoinHandle<()>,
    ) {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("address");
        let response = format!(
            "HTTP/1.1 {status_line}\r\nContent-Length: {}\r\nConnection: close\r\nContent-Type: application/json\r\n\r\n{body}",
            body.len()
        );
        let count = std::sync::Arc::new(AtomicUsize::new(0));
        let first_request = std::sync::Arc::new(tokio::sync::Mutex::new(String::new()));
        let counter = count.clone();
        let first = first_request.clone();
        let handle = tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let mut data = Vec::new();
                let mut buf = [0; 4096];
                loop {
                    match stream.read(&mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            data.extend_from_slice(&buf[..n]);
                            if data.windows(4).any(|w| w == b"\r\n\r\n") {
                                break;
                            }
                        }
                    }
                }
                counter.fetch_add(1, Ordering::SeqCst);
                let mut first = first.lock().await;
                if first.is_empty() {
                    *first = String::from_utf8_lossy(&data).to_string();
                }
                drop(first);
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });
        (format!("http://{addr}"), count, first_request, handle)
    }
    #[tokio::test]
    async fn failed_upload_survives_restart_and_retries() {
        let path = db_path();
        let queue = UploadQueue::new(path.clone()).expect("queue");
        // 500 服务器 + 重试预算：当场重试 6 次后仍失败，落盘等待后续触发
        let (url, count, first_request, handle) = loop_server("500 Error", "{}").await;
        assert!(queue.enqueue(&url, payload(42)).await.is_err());
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 6);
        assert!(first_request
            .lock()
            .await
            .starts_with("POST /api/lol/upload "));
        handle.abort();
        drop(queue);
        let queue = UploadQueue::new(path.clone()).expect("restart");
        assert_eq!(queue.retry_pending("").await.expect("disabled"), 0);
        let (url, request) =
            server("200 OK", r#"{"success":true,"data":{"isNewMatch":true}}"#).await;
        assert_eq!(queue.retry_pending(&url).await.expect("retry"), 1);
        let request = request.await.expect("request");
        assert!(request.contains("\"matchId\":42"));
        assert_eq!(queue.retry_pending(&url).await.expect("empty"), 0);
        drop(queue);
        std::fs::remove_file(path).expect("cleanup");
    }
    #[tokio::test]
    async fn client_error_is_not_retried_immediately() {
        let path = db_path();
        let queue = UploadQueue::new(path.clone()).expect("queue");
        // 4xx 客户端错误当场放弃：只发一次请求，不进入重试预算
        let (url, count, _first, handle) = loop_server("400 Bad Request", "{}").await;
        assert!(queue.enqueue(&url, payload(42)).await.is_err());
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
        handle.abort();
        drop(queue);
        std::fs::remove_file(path).expect("cleanup");
    }
    #[test]
    fn pending_uploads_capped_at_100() {
        let conn = rusqlite::Connection::open_in_memory().expect("db");
        conn.execute_batch("CREATE TABLE pending_uploads (game_id INTEGER PRIMARY KEY, payload TEXT NOT NULL, retry_count INTEGER NOT NULL DEFAULT 0, last_error TEXT NOT NULL DEFAULT '')")
            .expect("schema");
        for id in 1..=105i64 {
            conn.execute(
                "INSERT INTO pending_uploads(game_id,payload) VALUES(?1,'{}')",
                rusqlite::params![id],
            )
            .expect("insert");
        }
        prune_pending(&conn).expect("prune");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM pending_uploads", [], |r| r.get(0))
            .expect("count");
        assert_eq!(count, 100);
        // 保留最新的 100 条（rowid 最大），最旧的 1~5 被清理
        let min_id: i64 = conn
            .query_row("SELECT MIN(game_id) FROM pending_uploads", [], |r| r.get(0))
            .expect("min");
        assert_eq!(min_id, 6);
    }
    #[test]
    fn membership_requires_current_player() {
        let detail: GameDetail = serde_json::from_value(serde_json::json!({
            "participantIdentities": [{"participantId": 1, "player": {"puuid": "owner"}}]
        }))
        .expect("detail");
        assert!(ensure_membership(&detail, "owner").is_ok());
        assert!(ensure_membership(&detail, "other").is_err());
        assert!(ensure_membership(&detail, "").is_err());
    }
    #[test]
    fn invalid_destination_is_rejected() {
        for value in [
            "ftp://example.com",
            "http://",
            "http://user:pass@example.com",
            "https://example.com/?token=x",
        ] {
            assert!(validate_url(value, false).is_err(), "{value}");
        }
        assert!(validate_url("example.com", false).is_ok());
    }
    #[test]
    fn urls_match_legacy_protocol() {
        assert_eq!(
            upload_url("example.test/", false),
            "http://example.test/api/lol/upload"
        );
        assert_eq!(
            upload_url("https://example.test/api/lol/upload", true),
            "https://example.test/api/lol/upload-batch"
        );
    }
}
