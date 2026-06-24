use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{mpsc, Mutex};

// ─── 上传队列管理器 ───

/// 本地去重异步上传队列状态机。
/// 内部维护 `mpsc::channel` + `HashSet<u64>` 去重 + 后台 Worker。
pub struct UploadQueue {
    /// 上传请求发送端（推入 gameId）
    tx: mpsc::Sender<u64>,
    /// 已入队的 gameId 集合（去重用）
    enqueued: Arc<Mutex<HashSet<u64>>>,
}

impl UploadQueue {
    pub fn new(app_handle: AppHandle) -> Self {
        let (tx, rx) = mpsc::channel::<u64>(64);
        let enqueued = Arc::new(Mutex::new(HashSet::new()));

        // 启动后台 Worker（使用 Tauri 异步运行时，避免 setup 阶段无 Tokio runtime）
        let enqueued_clone = enqueued.clone();
        tauri::async_runtime::spawn(upload_worker(app_handle, rx, enqueued_clone));

        Self { tx, enqueued }
    }

    /// 将 gameId 推入上传队列（自动去重）。
    /// 如果该 gameId 已在队列或已上传，直接跳过。
    pub async fn enqueue(&self, game_id: u64) {
        if game_id == 0 {
            return;
        }

        let mut set = self.enqueued.lock().await;
        if set.contains(&game_id) {
            log::debug!("对局 {} 已在上传队列中，跳过", game_id);
            return;
        }
        set.insert(game_id);
        drop(set);

        if let Err(e) = self.tx.try_send(game_id) {
            log::warn!("推入上传队列失败: {}", e);
        } else {
            log::info!("对局 {} 已加入上传队列", game_id);
        }
    }
}

// ─── 后台 Worker ───

/// 串行处理上传队列，每局最多 30 秒超时。
async fn upload_worker(
    app_handle: AppHandle,
    mut rx: mpsc::Receiver<u64>,
    _enqueued: Arc<Mutex<HashSet<u64>>>,
) {
    log::info!("上传 Worker 已启动");

    while let Some(game_id) = rx.recv().await {
        log::info!("开始上传对局: {}", game_id);

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            upload_single_game(&app_handle, game_id),
        )
        .await;

        match result {
            Ok(Ok(status)) => {
                log::info!("对局 {} 上传完成: {}", game_id, status);
                if status == "new" {
                    // 上传成功，向前端推送通知
                    let _ = app_handle.emit(
                        "upload-success",
                        serde_json::json!({ "gameId": game_id }),
                    );
                }
            }
            Ok(Err(e)) => {
                log::warn!("对局 {} 上传失败: {}", game_id, e);
            }
            Err(_) => {
                log::warn!("对局 {} 上传超时 (30s)", game_id);
            }
        }
    }

    log::info!("上传 Worker 已退出");
}

// ─── 状态转换检测 ───

/// 游戏状态转换上下文。由 auto_match agent 维护。
pub struct UploadTrigger {
    prev_phase: String,
    queue: Arc<UploadQueue>,
    puuid: Arc<Mutex<Option<String>>>,
    last_game_id: Arc<Mutex<u64>>,
}

impl UploadTrigger {
    pub fn new(queue: Arc<UploadQueue>) -> Self {
        Self {
            prev_phase: "None".to_string(),
            queue,
            puuid: Arc::new(Mutex::new(None)),
            last_game_id: Arc::new(Mutex::new(0)),
        }
    }

    /// 检测游戏状态转换，符合条件时触发上传。
    ///
    /// 触发条件：当前阶段为 EndOfGame/Lobby/None，
    /// 且前一阶段为 InProgress/GameStart/ChampSelect/ReadyCheck/PreEndOfGame/Reconnect。
    pub async fn on_phase_change(&mut self, phase: &str, app_handle: &AppHandle) {
        let prev = self.prev_phase.clone();
        self.prev_phase = phase.to_string();

        // 只在游戏结束后触发
        if !matches!(phase, "EndOfGame" | "Lobby" | "None") {
            return;
        }
        // 前一阶段必须是游戏相关状态
        if !matches!(
            prev.as_str(),
            "InProgress" | "GameStart" | "ChampSelect" | "ReadyCheck" | "PreEndOfGame" | "Reconnect"
        ) {
            return;
        }

        log::info!(
            "检测到游戏结束转换: {} → {}，准备上传...",
            prev, phase
        );

        // 延迟 2 秒等待 LCU 数据写入
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // 获取 puuid（首次需要查询，后续缓存）
        let puuid = self.get_or_fetch_puuid(app_handle).await;
        let puuid = match puuid {
            Some(p) => p,
            None => {
                log::warn!("无法获取 puuid，跳过上传");
                return;
            }
        };

        // 获取最新一局 gameId
        let latest_game_id = fetch_latest_game_id(app_handle, &puuid).await;
        let game_id = match latest_game_id {
            Some(id) => id,
            None => {
                log::warn!("无法获取最近对局 ID，跳过上传");
                return;
            }
        };

        // 去重：与上次上传对比
        let mut last_id = self.last_game_id.lock().await;
        if game_id == *last_id {
            log::debug!("对局 {} 已上传过，跳过", game_id);
            return;
        }
        *last_id = game_id;
        drop(last_id);

        // 推入上传队列
        self.queue.enqueue(game_id).await;
    }

    async fn get_or_fetch_puuid(&self, app_handle: &AppHandle) -> Option<String> {
        // 检查缓存
        {
            let cached = self.puuid.lock().await;
            if let Some(ref p) = *cached {
                return Some(p.clone());
            }
        }

        // 从 LCU 获取
        let state = app_handle.state::<crate::AppState>();
        let lock = state.lcu_client.read().await;
        let lcu = lock.as_ref()?;

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
            .ok()?;

        let data: Value = resp.json().await.ok()?;
        let puuid = data.get("puuid")?.as_str()?.to_string();

        // 缓存
        let mut cached = self.puuid.lock().await;
        *cached = Some(puuid.clone());

        Some(puuid)
    }
}

// ─── 单局上传逻辑 ───

/// 上传单局对局数据。
/// 构建 Smart Split Payload → POST 到外部 API。
async fn upload_single_game(app_handle: &AppHandle, game_id: u64) -> Result<String, String> {
    let (http_client, auth, base) = {
        let state = app_handle.state::<crate::AppState>();
        let lock = state.lcu_client.read().await;
        let lcu = lock.as_ref().ok_or("LCU 未连接")?;
        (
            lcu.http_client.clone(),
            crate::build_auth_header(&lcu.token),
            format!("https://127.0.0.1:{}", lcu.port),
        )
    };

    // 获取当前召唤师名称
    let current_name = get_current_summoner_name(&http_client, &auth, &base).await;

    // 获取对局详情
    let detail_url = format!("{}/lol-match-history/v1/games/{}", base, game_id);
    let resp = http_client
        .get(&detail_url)
        .header("Authorization", &auth)
        .send()
        .await
        .map_err(|e| format!("获取对局详情失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("获取对局详情: HTTP {}", resp.status()));
    }

    let game_detail: GameDetail = resp
        .json()
        .await
        .map_err(|e| format!("解析对局详情失败: {}", e))?;

    // 构建 Smart Split Payload
    let payload = build_upload_payload(&game_detail, current_name.as_deref());

    log::info!(
        "对局数据构建完成: matchId={}, 内层{}人, 外层{}人",
        payload.match_info.match_id,
        payload.match_info.participants.len(),
        payload.participants.len()
    );

    // POST 到外部 API（需要配置 upload_url）
    // TODO: 从配置中读取 upload_url，发送 POST 请求
    // 当前仅日志输出
    if let Ok(json) = serde_json::to_string_pretty(&payload) {
        log::debug!("上传 Payload:\n{}", json);
    }

    Ok("new".to_string())
}

async fn get_current_summoner_name(
    http: &reqwest::Client,
    auth: &str,
    base: &str,
) -> Option<String> {
    let url = format!("{}/lol-summoner/v1/current-summoner", base);
    let resp = http.get(&url).header("Authorization", auth).send().await.ok()?;
    let data: Value = resp.json().await.ok()?;
    data.get("gameName")?.as_str().map(|s| s.to_string())
}

/// 获取最新一局的 gameId
async fn fetch_latest_game_id(app_handle: &AppHandle, puuid: &str) -> Option<u64> {
    let state = app_handle.state::<crate::AppState>();
    let lock = state.lcu_client.read().await;
    let lcu = lock.as_ref()?;

    let url = format!(
        "https://127.0.0.1:{}/lol-match-history/v1/products/lol/{}/matches?begIndex=0&endIndex=1",
        lcu.port, puuid
    );
    let auth = crate::build_auth_header(&lcu.token);

    let resp = lcu
        .http_client
        .get(&url)
        .header("Authorization", auth)
        .send()
        .await
        .ok()?;

    let data: Value = resp.json().await.ok()?;
    data.get("games")?
        .get("games")?
        .as_array()?
        .first()?
        .get("gameId")?
        .as_u64()
}

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

// ─── Smart Split 构建 ───

fn build_upload_payload(
    game_detail: &GameDetail,
    current_summoner_name: Option<&str>,
) -> UploadPayload {
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
            let (hextech0, hextech1, hextech2) = extract_hextech_ids(&p.stats);

            ParticipantInfo {
                summoner_name,
                puuid,
                team_id: p.team_id.unwrap_or(0),
                champion_id: p.champion_id.unwrap_or(0),
                champion_name: String::new(),
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

    // Smart Split
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

fn extract_hextech_ids(stats: &ParticipantStats) -> (i32, i32, i32) {
    let hextech0 = if is_hextech_item(stats.item6) {
        stats.item6
    } else {
        0
    };
    (hextech0, 0, 0)
}

fn is_hextech_item(item_id: i32) -> bool {
    (3000..=3100).contains(&item_id)
}
