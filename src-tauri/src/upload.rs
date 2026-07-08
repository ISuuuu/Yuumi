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
        crate::spawn_log_panic(upload_worker(app_handle, rx, enqueued_clone));

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
    enqueued: Arc<Mutex<HashSet<u64>>>,
) {
    log::info!("上传 Worker 已启动");

    while let Some(game_id) = rx.recv().await {
        log::info!("开始上传对局: {}", game_id);

        // 从配置读取上传 URL（每次重新读取，支持运行时修改）
        let upload_url = {
            let state = app_handle.state::<crate::AppState>();
            let cfg = state.config.read().await;
            let raw = cfg.general.upload_api_url.clone();
            if raw.is_empty() {
                log::warn!("对局 {} 跳过上传: 未配置上传 API 地址", game_id);
                // 未配置 API 时也移出去重缓存，避免后续配置填上后依然被拦截无法上传
                let mut set = enqueued.lock().await;
                set.remove(&game_id);
                continue;
            }
            build_upload_url(&raw)
        };

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            upload_single_game(&app_handle, game_id, &upload_url),
        )
        .await;

        match result {
            Ok(Ok(status)) => {
                log::info!("对局 {} 上传完成: {}", game_id, status);
                if status == "new" {
                    // 上传成功，向前端推送通知
                    let _ =
                        app_handle.emit("upload-success", serde_json::json!({ "gameId": game_id }));
                }
            }
            Ok(Err(e)) => {
                log::warn!("对局 {} 上传失败: {}", game_id, e);
                // 失败时从去重集合中移出，允许后续重试
                let mut set = enqueued.lock().await;
                set.remove(&game_id);
            }
            Err(_) => {
                log::warn!("对局 {} 上传超时 (30s)", game_id);
                // 超时从去重集合中移出，允许后续重试
                let mut set = enqueued.lock().await;
                set.remove(&game_id);
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
    current_game_id: Arc<Mutex<Option<u64>>>,
    /// 标记当前对局的上传是否已完成，避免退回 Lobby/None 时重复查询 LCU
    upload_completed: Arc<Mutex<bool>>,
}

impl UploadTrigger {
    pub fn new(queue: Arc<UploadQueue>) -> Self {
        Self {
            prev_phase: "None".to_string(),
            queue,
            puuid: Arc::new(Mutex::new(None)),
            last_game_id: Arc::new(Mutex::new(0)),
            current_game_id: Arc::new(Mutex::new(None)),
            upload_completed: Arc::new(Mutex::new(false)),
        }
    }

    /// 检测游戏状态转换，符合条件时触发上传。
    ///
    /// 触发条件：当前阶段为 EndOfGame/Lobby/None，
    /// 且前一阶段为 InProgress/GameStart/ChampSelect/ReadyCheck/PreEndOfGame/Reconnect。
    pub async fn on_phase_change(&mut self, phase: &str, app_handle: &AppHandle) {
        let prev = self.prev_phase.clone();
        self.prev_phase = phase.to_string();

        if matches!(phase, "ChampSelect" | "ReadyCheck") {
            *self.current_game_id.lock().await = None;
        }

        if matches!(phase, "GameStart" | "InProgress") {
            *self.upload_completed.lock().await = false;
            match fetch_gameflow_game_id(app_handle).await {
                Ok(Some(id)) => {
                    log::info!("记录当前对局 ID: {} (phase={})", id, phase);
                    *self.current_game_id.lock().await = Some(id);
                }
                Ok(None) => log::debug!("当前 gameflow session 暂无 gameId (phase={})", phase),
                Err(e) => log::debug!("读取当前对局 ID 失败 (phase={}): {}", phase, e),
            }
            return;
        }

        // 只在游戏结束后触发
        if !matches!(phase, "EndOfGame" | "Lobby" | "None") {
            return;
        }
        // 前一阶段必须是实际游戏运行状态（排除选人/确认阶段以避免秒退或拒绝匹配触发上传）
        if !matches!(
            prev.as_str(),
            "InProgress" | "GameStart" | "PreEndOfGame" | "Reconnect"
        ) {
            return;
        }

        log::info!("检测到游戏结束转换: {} → {}，准备上传...", prev, phase);

        // 如果该对局已完成上传，跳过重复处理
        {
            let completed = self.upload_completed.lock().await;
            if *completed {
                log::debug!("该对局的上传已完成，跳过重复处理 (phase={})", phase);
                return;
            }
        }

        // 延迟 2 秒等待 LCU 数据写入
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // 获取 puuid（首次需要查询，后续缓存）
        let puuid = match self.get_or_fetch_puuid(app_handle).await {
            Ok(p) => p,
            Err(e) => {
                log::warn!("无法获取 puuid，跳过上传: {}", e);
                return;
            }
        };

        // 优先使用游戏进行中记录的当前对局 ID；LCU 战绩列表在结算后可能短暂滞后。
        let game_id = match *self.current_game_id.lock().await {
            Some(id) => id,
            None => match fetch_latest_game_id(app_handle, &puuid).await {
                Ok(id) => id,
                Err(e) => {
                    log::warn!("无法获取最近对局 ID，跳过上传: {}", e);
                    return;
                }
            },
        };
        // 去重：与上次上传对比
        let mut last_id = self.last_game_id.lock().await;
        if game_id == *last_id {
            log::debug!("对局 {} 已上传过，跳过", game_id);
            *self.current_game_id.lock().await = None;
            *self.upload_completed.lock().await = true;
            return;
        }
        *last_id = game_id;
        drop(last_id);

        // 推入上传队列
        self.queue.enqueue(game_id).await;
        *self.current_game_id.lock().await = None;
        *self.upload_completed.lock().await = true;
    }

    async fn get_or_fetch_puuid(&self, app_handle: &AppHandle) -> Result<String, String> {
        // 检查缓存
        {
            let cached = self.puuid.lock().await;
            if let Some(ref p) = *cached {
                return Ok(p.clone());
            }
        }

        // 从 LCU 获取
        let state = app_handle.state::<crate::AppState>();
        let lock = state.lcu_client.read().await;
        let lcu = lock.as_ref().ok_or("LCU 未连接")?;

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
            .map_err(|e| format!("请求当前召唤师信息失败: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("获取当前召唤师信息: HTTP {}", resp.status()));
        }

        let data: Value = resp
            .json()
            .await
            .map_err(|e| format!("解析当前召唤师信息失败: {}", e))?;

        let puuid = data
            .get("puuid")
            .and_then(|v| v.as_str())
            .ok_or("当前召唤师信息中缺少 puuid 字段")?
            .to_string();

        // 缓存
        let mut cached = self.puuid.lock().await;
        *cached = Some(puuid.clone());

        Ok(puuid)
    }
}

// ─── URL 构建（对齐 Python get_upload_url / get_batch_upload_url）───

/// 确保 URL 带有协议前缀（默认 http://）
fn ensure_scheme(url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else {
        format!("http://{}", url)
    }
}

/// 拼接单次上传 URL（对齐 Python get_upload_url）
fn build_upload_url(base_url: &str) -> String {
    let mut url = ensure_scheme(base_url.trim_end_matches('/'));
    if !url.contains("/api/lol/upload") {
        url.push_str("/api/lol/upload");
    }
    url
}

/// 拼接批量上传 URL（对齐 Python get_batch_upload_url）
fn build_batch_upload_url(base_url: &str) -> String {
    let mut url = ensure_scheme(base_url.trim_end_matches('/'));
    if url.ends_with("/api/lol/upload") {
        url.truncate(url.len() - "/api/lol/upload".len());
    }
    url.push_str("/api/lol/upload-batch");
    url
}

// ─── 单局上传逻辑 ───

/// 创建用于外部 API 的 reqwest Client（正常 SSL 验证，30 秒超时）。
/// 与 LCU Client（danger_accept_invalid_certs=true）分开，避免影响外部 HTTPS 请求。
fn external_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

/// 上传单局对局数据。
/// 构建 Smart Split Payload → POST 到外部 API。
async fn upload_single_game(
    app_handle: &AppHandle,
    game_id: u64,
    upload_url: &str,
) -> Result<String, String> {
    let (lcu_client, auth, base) = {
        let state = app_handle.state::<crate::AppState>();
        let lock = state.lcu_client.read().await;
        let lcu = lock.as_ref().ok_or("LCU 未连接")?;
        (
            lcu.http_client.clone(),
            crate::build_auth_header(&lcu.token),
            format!("https://127.0.0.1:{}", lcu.port),
        )
    };

    // 获取当前召唤师名称和 puuid
    let current_summoner_info = get_current_summoner_info(&lcu_client, &auth, &base).await;
    let current_puuid = current_summoner_info
        .as_ref()
        .map(|(_, puuid)| puuid.as_str());

    // 获取对局详情
    let detail_url = format!("{}/lol-match-history/v1/games/{}", base, game_id);
    let resp = lcu_client
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
    let champion_names = {
        let state = app_handle.state::<crate::AppState>();
        let gd = state.game_data.read().await;
        gd.champions.clone()
    };
    let payload = build_upload_payload(&game_detail, current_puuid, &champion_names);

    log::info!(
        "对局数据构建完成: matchId={}, 内层{}人, 外层{}人",
        payload.match_info.match_id,
        payload.match_info.participants.len(),
        payload.participants.len()
    );

    // POST 到外部 API（最多重试 5 次，使用独立的外部 HTTP Client）
    let ext_client = external_http_client();
    let mut last_err = String::new();
    for retry in 0..=5 {
        match ext_client
            .post(upload_url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
        {
            Ok(resp) => {
                if resp.status().is_success() {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        if json
                            .get("success")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                        {
                            let data = json.get("data");
                            let is_new = data
                                .and_then(|d| d.get("isNewMatch"))
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            if is_new {
                                log::info!("对局 {} 上传成功 (new)", game_id);
                                return Ok("new".to_string());
                            } else {
                                log::info!("对局 {} 已存在 (exists)", game_id);
                                return Ok("exists".to_string());
                            }
                        }
                        let msg = json
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("未知错误");
                        last_err = format!("服务器处理失败: {}", msg);
                    } else {
                        last_err = "解析响应失败".to_string();
                    }
                } else {
                    last_err = format!("HTTP {}", resp.status());
                }
            }
            Err(e) => {
                last_err = format!("请求失败: {}", e);
            }
        }

        if retry < 5 {
            log::warn!(
                "对局 {} 上传失败 ({}), 重试 {}/5",
                game_id,
                last_err,
                retry + 1
            );
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }

    Err(format!("对局 {} 上传失败: {}", game_id, last_err))
}

async fn get_current_summoner_info(
    http: &reqwest::Client,
    auth: &str,
    base: &str,
) -> Option<(String, String)> {
    let url = format!("{}/lol-summoner/v1/current-summoner", base);
    let resp = match http.get(&url).header("Authorization", auth).send().await {
        Ok(r) => r,
        Err(e) => {
            log::warn!("获取召唤师信息失败: {}", e);
            return None;
        }
    };
    let data: Value = match resp.json().await {
        Ok(d) => d,
        Err(e) => {
            log::warn!("解析召唤师信息失败: {}", e);
            return None;
        }
    };
    let puuid = data.get("puuid")?.as_str()?.to_string();
    let name = data.get("gameName")?.as_str()?.to_string();
    Some((name, puuid))
}

fn extract_gameflow_game_id(data: &Value) -> Option<u64> {
    data.get("gameData")
        .and_then(|game_data| game_data.get("gameId"))
        .and_then(|id| id.as_u64())
        .filter(|id| *id > 0)
}

async fn fetch_gameflow_game_id(app_handle: &AppHandle) -> Result<Option<u64>, String> {
    let state = app_handle.state::<crate::AppState>();
    let lock = state.lcu_client.read().await;
    let lcu = lock.as_ref().ok_or("LCU 未连接")?;

    let url = format!("https://127.0.0.1:{}/lol-gameflow/v1/session", lcu.port);
    let auth = crate::build_auth_header(&lcu.token);

    let resp = lcu
        .http_client
        .get(&url)
        .header("Authorization", auth)
        .send()
        .await
        .map_err(|e| format!("请求 gameflow session 失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("获取 gameflow session: HTTP {}", resp.status()));
    }

    let data: Value = resp
        .json()
        .await
        .map_err(|e| format!("解析 gameflow session 失败: {}", e))?;

    Ok(extract_gameflow_game_id(&data))
}
/// 获取最新一局的 gameId
async fn fetch_latest_game_id(app_handle: &AppHandle, puuid: &str) -> Result<u64, String> {
    let state = app_handle.state::<crate::AppState>();
    let lock = state.lcu_client.read().await;
    let lcu = lock.as_ref().ok_or("LCU 未连接")?;

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
        .map_err(|e| format!("请求最近对局列表失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("获取最近对局列表: HTTP {}", resp.status()));
    }

    let data: Value = resp
        .json()
        .await
        .map_err(|e| format!("解析最近对局列表失败: {}", e))?;

    let games_arr = data
        .get("games")
        .and_then(|g| g.get("games"))
        .and_then(|g| g.as_array());

    match games_arr.and_then(|arr| arr.first()) {
        Some(first) => first
            .get("gameId")
            .and_then(|id| id.as_u64())
            .ok_or_else(|| "最近对局条目缺少 gameId 字段".to_string()),
        None => Err("最近对局列表为空".to_string()),
    }
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

/// 上传的 participant 字段严格对齐 Python build_upload_payload
#[derive(Debug, Clone, Serialize)]
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

fn build_upload_payload(
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

// ─── 批量上传逻辑 ───

/// 批量上传结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchUploadResult {
    pub success_count: u32,
    pub failed_count: u32,
    pub error: Option<String>,
}

/// 通过 gameId 列表批量上传对局数据（自动分批，每批 10 场）。
async fn batch_upload_by_ids(
    app_handle: &AppHandle,
    game_ids: &[u64],
    batch_url: &str,
) -> BatchUploadResult {
    let (lcu_client, auth, base) = {
        let state = app_handle.state::<crate::AppState>();
        let lock = state.lcu_client.read().await;
        match lock.as_ref() {
            Some(lcu) => (
                lcu.http_client.clone(),
                crate::build_auth_header(&lcu.token),
                format!("https://127.0.0.1:{}", lcu.port),
            ),
            None => {
                return BatchUploadResult {
                    success_count: 0,
                    failed_count: game_ids.len() as u32,
                    error: Some("LCU 未连接".to_string()),
                };
            }
        }
    };

    let current_summoner_info = get_current_summoner_info(&lcu_client, &auth, &base).await;
    let current_puuid = current_summoner_info
        .as_ref()
        .map(|(_, puuid)| puuid.as_str());

    // 获取英雄名称映射
    let champion_names = {
        let state = app_handle.state::<crate::AppState>();
        let gd = state.game_data.read().await;
        gd.champions.clone()
    };

    // 逐个获取对局详情并构建 payload
    let mut payloads: Vec<UploadPayload> = Vec::new();
    for &game_id in game_ids {
        let detail_url = format!("{}/lol-match-history/v1/games/{}", base, game_id);
        match lcu_client
            .get(&detail_url)
            .header("Authorization", &auth)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => match resp.json::<GameDetail>().await {
                Ok(detail) => {
                    payloads.push(build_upload_payload(
                        &detail,
                        current_puuid,
                        &champion_names,
                    ));
                }
                Err(e) => {
                    log::warn!("批量上传: 解析对局 {} 详情失败: {}", game_id, e);
                }
            },
            Ok(resp) => {
                log::warn!(
                    "批量上传: 获取对局 {} 详情失败: HTTP {}",
                    game_id,
                    resp.status()
                );
            }
            Err(e) => {
                log::warn!("批量上传: 获取对局 {} 详情请求失败: {}", game_id, e);
            }
        }
    }

    if payloads.is_empty() {
        log::error!("批量上传: 全部 {} 场对局详情获取失败", game_ids.len());
        return BatchUploadResult {
            success_count: 0,
            failed_count: game_ids.len() as u32,
            error: Some(format!("全部 {} 场对局详情获取失败", game_ids.len())),
        };
    }

    let total = payloads.len() as u32;

    // 分批上传（每批 10 场）
    let mut total_success: u32 = 0;
    let mut total_failed: u32 = 0;
    let mut error_msg: Option<String> = None;

    // 使用独立的外部 HTTP Client（正常 SSL 验证）
    let ext_client = external_http_client();

    for (i, chunk) in payloads.chunks(10).enumerate() {
        log::info!("批量上传第 {} 批，本批 {} 场对局", i + 1, chunk.len());

        match ext_client
            .post(batch_url)
            .header("Content-Type", "application/json")
            .json(chunk)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                let status = resp.status();
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        if json
                            .get("success")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                        {
                            let data = json.get("data");
                            let success = data
                                .and_then(|d| d.get("successCount").or(d.get("newMatches")))
                                .and_then(|v| v.as_u64())
                                .unwrap_or(chunk.len() as u64)
                                as u32;
                            let failed = data
                                .and_then(|d| d.get("failedCount"))
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0) as u32;
                            total_success += success;
                            total_failed += failed;
                            log::info!(
                                "批量上传第 {} 批完成: 成功={}, 失败={}",
                                i + 1,
                                success,
                                failed
                            );
                        } else {
                            let msg = json
                                .get("message")
                                .and_then(|v| v.as_str())
                                .unwrap_or("未知错误");
                            total_failed += chunk.len() as u32;
                            error_msg = Some(format!("服务器处理失败: {}", msg));
                            log::warn!("批量上传第 {} 批服务器返回失败: {}", i + 1, msg);
                        }
                    }
                    Err(e) => {
                        total_failed += chunk.len() as u32;
                        error_msg = Some(format!("解析响应 JSON 失败 (HTTP {}): {}", status, e));
                        log::warn!("批量上传第 {} 批解析响应失败: {}", i + 1, e);
                    }
                }
            }
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                let preview = if body.len() > 200 {
                    &body[..200]
                } else {
                    &body
                };
                total_failed += chunk.len() as u32;
                error_msg = Some(format!("HTTP {}: {}", status, preview));
                log::warn!("批量上传第 {} 批 HTTP 错误 {}: {}", i + 1, status, preview);
            }
            Err(e) => {
                total_failed += chunk.len() as u32;
                let detail = format!("{}", e);
                // 连接失败停止后续批次
                log::error!("批量上传连接失败，停止后续批次: {}", detail);
                error_msg = Some(format!("连接失败: {}", detail));
                break;
            }
        }
    }

    log::info!(
        "批量上传完成: 成功={}, 失败={}, 总计={}",
        total_success,
        total_failed,
        total
    );

    BatchUploadResult {
        success_count: total_success,
        failed_count: total_failed,
        error: error_msg,
    }
}

// ─── Tauri 命令 ───

/// 单场上传 Tauri 命令（供 Career 自动上传 / Search 手动调用）
#[tauri::command]
pub async fn upload_single_match(
    game_id: u64,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    let queue = {
        let state = app_handle.state::<crate::AppState>();
        state.upload_queue.clone()
    };
    queue.enqueue(game_id).await;
    Ok("已加入上传队列".to_string())
}

/// 批量上传 Tauri 命令（供 Search 页面自动/手动调用）
#[tauri::command]
pub async fn batch_upload_matches(
    game_ids: Vec<u64>,
    app_handle: tauri::AppHandle,
    app_state: tauri::State<'_, crate::AppState>,
) -> Result<BatchUploadResult, String> {
    if game_ids.is_empty() {
        return Ok(BatchUploadResult {
            success_count: 0,
            failed_count: 0,
            error: None,
        });
    }

    let raw_url = app_state.config.read().await.general.upload_api_url.clone();
    if raw_url.is_empty() {
        return Ok(BatchUploadResult {
            success_count: 0,
            failed_count: game_ids.len() as u32,
            error: Some("未配置上传 API 地址，请在设置 → 通用中配置".to_string()),
        });
    }
    let batch_url = build_batch_upload_url(&raw_url);
    log::info!(
        "[batch_upload] 开始批量上传 {} 场对局, url={}",
        game_ids.len(),
        batch_url
    );

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(120),
        batch_upload_by_ids(&app_handle, &game_ids, &batch_url),
    )
    .await
    .unwrap_or(BatchUploadResult {
        success_count: 0,
        failed_count: game_ids.len() as u32,
        error: Some("批量上传超时 (120s)".to_string()),
    });

    log::info!(
        "[batch_upload] 完成: 成功={}, 失败={}",
        result.success_count,
        result.failed_count
    );
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::extract_gameflow_game_id;
    use serde_json::json;

    #[test]
    fn extracts_game_id_from_gameflow_session() {
        let data = json!({
            "gameData": {
                "gameId": 300900190786u64
            }
        });

        assert_eq!(extract_gameflow_game_id(&data), Some(300900190786));
    }

    #[test]
    fn ignores_missing_or_zero_gameflow_game_id() {
        assert_eq!(extract_gameflow_game_id(&json!({})), None);
        assert_eq!(
            extract_gameflow_game_id(&json!({ "gameData": { "gameId": 0 } })),
            None
        );
    }
}
