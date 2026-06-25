use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;

use crate::config::FunctionsConfig;

// ─── 选人会话数据结构体 ───

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectSession {
    pub actions: Vec<Vec<ChampSelectAction>>,
    pub local_player_cell_id: i32,
    pub my_team: Vec<ChampSelectPlayer>,
    pub bans: ChampSelectBans,
    #[serde(default)]
    pub pick_order_swaps: Vec<ChampSelectSwap>,
    #[serde(default)]
    pub trades: Vec<ChampSelectTrade>,
    pub timer: Option<ChampSelectTimer>,
    #[serde(default)]
    pub bench_enabled: bool,
    pub queue_id: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectAction {
    pub actor_cell_id: i32,
    #[serde(default)]
    pub champion_id: i32,
    #[serde(default)]
    pub completed: bool,
    pub id: i32,
    #[serde(default)]
    pub is_in_progress: bool,
    #[serde(rename = "type")]
    pub action_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectPlayer {
    pub cell_id: i32,
    #[serde(default)]
    pub champion_id: i32,
    #[serde(default)]
    pub champion_pick_intent: i32,
    #[serde(default)]
    pub assigned_position: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectBans {
    #[serde(default)]
    pub my_team_bans: Vec<i32>,
    #[serde(default)]
    pub their_team_bans: Vec<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectSwap {
    pub id: i32,
    #[serde(default)]
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectTrade {
    pub id: i32,
    #[serde(default)]
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectTimer {
    pub adjusted_time_left_in_phase: Option<u64>,
    pub phase: Option<String>,
}

// ─── BP 状态追踪 ───

#[derive(Debug, Default)]
struct ChampionSelection {
    is_champion_picked: bool,
    is_champion_banned: bool,
    is_summoner_spell_set: bool,
    is_champion_showed: bool,
    is_champion_picked_completed: bool,
    _opgg_show_champion_id: i32,
}

// ─── OPGG 事件数据 ───

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpggBuildEvent {
    pub champion_id: i32,
    pub mode: String,
    pub position: String,
}

// ─── 启动 BP Agent ───

pub fn start(app_handle: AppHandle, mut session_rx: mpsc::Receiver<ChampSelectSession>) {
    tauri::async_runtime::spawn(async move {
        let mut selection = ChampionSelection::default();

        while let Some(session) = session_rx.recv().await {
            let cfg = {
                let app_state = app_handle.state::<crate::AppState>();
                let cfg_lock = app_state.config.read().await;
                cfg_lock.functions.clone()
            };

            // 执行顺序：亮英雄 → 禁人 → 选人 → 技能 → 交换 → 锁定 → OPGG
            do_auto_show(&app_handle, &session, &cfg, &mut selection).await;
            do_auto_ban(&app_handle, &session, &cfg, &mut selection).await;
            do_auto_pick(&app_handle, &session, &cfg, &mut selection).await;
            do_auto_spell(&app_handle, &session, &cfg, &mut selection).await;
            do_auto_swap(&app_handle, &session, &cfg, &mut selection).await;
            do_auto_trade(&app_handle, &session, &cfg, &mut selection).await;
            do_auto_complete(&app_handle, &session, &cfg, &mut selection).await;
            do_show_opgg_build(&app_handle, &session, &cfg, &mut selection).await;
        }

        log::info!("BP Agent 已退出");
    });
}

// ─── 2.1 自动接受楼层交换 ───

async fn do_auto_swap(
    app_handle: &AppHandle,
    session: &ChampSelectSession,
    cfg: &FunctionsConfig,
    selection: &mut ChampionSelection,
) {
    if !cfg.auto_accept_ceil_swap {
        return;
    }

    for swap in &session.pick_order_swaps {
        if swap.state == "RECEIVED" {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            log::info!("自动接受楼层交换: swapId={}", swap.id);
            let url = format!(
                "/lol-champ-select/v1/session/swaps/pick-order/{}/accept",
                swap.id
            );
            if lcu_post(app_handle, &url).await {
                // 重置选人状态，以便重新触发
                selection.is_champion_picked_completed = false;
            }
            return;
        }
    }
}

// ─── 2.2 自动接受英雄交换 ───

async fn do_auto_trade(
    app_handle: &AppHandle,
    session: &ChampSelectSession,
    cfg: &FunctionsConfig,
    _selection: &mut ChampionSelection,
) {
    if !cfg.auto_accept_champ_trade {
        return;
    }

    for trade in &session.trades {
        if trade.state == "RECEIVED" {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            log::info!("自动接受英雄交换: tradeId={}", trade.id);
            let url = format!(
                "/lol-champ-select/v1/session/trades/{}/accept",
                trade.id
            );
            lcu_post(app_handle, &url).await;
            return;
        }
    }
}

// ─── 2.3 自动亮英雄/展示意图 ───

async fn do_auto_show(
    app_handle: &AppHandle,
    session: &ChampSelectSession,
    cfg: &FunctionsConfig,
    selection: &mut ChampionSelection,
) {
    if selection.is_champion_showed {
        return;
    }
    if !cfg.enable_auto_hover_champion {
        return;
    }

    let cell_id = session.local_player_cell_id;

    // 检查是否已选/已亮
    if let Some(player) = session.my_team.iter().find(|p| p.cell_id == cell_id) {
        if player.champion_id != 0 || player.champion_pick_intent != 0 {
            selection.is_champion_showed = true;
            return;
        }
    }

    // 获取位置 + 候选列表
    let pos = session
        .my_team
        .iter()
        .find(|p| p.cell_id == cell_id)
        .map(|p| p.assigned_position.as_str())
        .unwrap_or("");

    let mut candidates = get_position_select_candidates(pos, cfg);
    candidates.extend(cfg.auto_select_champion.iter().copied());

    if candidates.is_empty() {
        selection.is_champion_showed = true;
        return;
    }

    let champion_id = candidates[0];

    // 找到 pick action，PATCH 但 completed=false（亮英雄不锁定）
    let pick_action = session
        .actions
        .iter()
        .rev()
        .flatten()
        .find(|a| a.actor_cell_id == cell_id && a.action_type == "pick");

    if let Some(action) = pick_action {
        log::info!("自动亮英雄: {} (intent)", champion_id);
        if lcu_patch_action(app_handle, action.id, champion_id, false).await {
            selection.is_champion_showed = true;
        }
    }
}

// ─── 2.4 超时前自动锁定 ───

async fn do_auto_complete(
    app_handle: &AppHandle,
    session: &ChampSelectSession,
    cfg: &FunctionsConfig,
    selection: &mut ChampionSelection,
) {
    if !cfg.auto_select_confirm_on_timeout || selection.is_champion_picked_completed {
        return;
    }

    let cell_id = session.local_player_cell_id;

    // 找到 pick action 并检查状态
    let mut action_in_progress = None;
    for action_group in session.actions.iter().rev() {
        for action in action_group {
            if action.actor_cell_id != cell_id || action.action_type != "pick" {
                continue;
            }
            if !action.is_in_progress {
                return;
            }
            if action.completed {
                selection.is_champion_picked_completed = true;
                return;
            }
            action_in_progress = Some(action.clone());
            break;
        }
    }

    let _action = match action_in_progress {
        Some(a) => a,
        None => return,
    };

    selection.is_champion_picked_completed = true;

    // 计算等待时间：剩余时间 - 4 秒
    let sleep_ms = session
        .timer
        .as_ref()
        .and_then(|t| t.adjusted_time_left_in_phase)
        .unwrap_or(10000);
    let sleep_secs = (sleep_ms / 1000).saturating_sub(4);
    let sleep_secs = sleep_secs.max(0);

    log::info!("自动锁定: 等待 {} 秒后锁定", sleep_secs);
    tokio::time::sleep(std::time::Duration::from_secs(sleep_secs)).await;

    // 重新获取会话数据（经过等待后状态可能已变化）
    let fresh_session = match lcu_get_session(app_handle).await {
        Some(s) => s,
        None => return,
    };

    // 收集不可选的英雄（已被其他人选走 + 已禁用）
    let mut cant_select: Vec<i32> = Vec::new();
    for ag in &fresh_session.actions {
        for a in ag {
            if a.action_type == "pick" && a.completed && a.actor_cell_id != cell_id {
                cant_select.push(a.champion_id);
            }
        }
    }
    cant_select.extend(fresh_session.bans.my_team_bans.iter());
    cant_select.extend(fresh_session.bans.their_team_bans.iter());

    // 找到当前玩家的 pick action
    let mut champion_intent = 0;
    let mut action_id = 0;
    for ag in &fresh_session.actions {
        for a in ag {
            if a.actor_cell_id != cell_id || a.action_type != "pick" {
                continue;
            }
            if a.completed {
                return; // 已锁定，无需操作
            }
            champion_intent = a.champion_id;
            action_id = a.id;
        }
    }

    if champion_intent == 0 || action_id == 0 {
        return;
    }

    // 只有当英雄未被其他人选走时才锁定
    if !cant_select.contains(&champion_intent) {
        log::info!("超时自动锁定英雄: {}", champion_intent);
        lcu_patch_action(app_handle, action_id, champion_intent, true).await;
    }
}

// ─── 2.5 自动展示 OPGG 推荐配置 ───

async fn do_show_opgg_build(
    app_handle: &AppHandle,
    session: &ChampSelectSession,
    _cfg: &FunctionsConfig,
    selection: &mut ChampionSelection,
) {
    let cell_id = session.local_player_cell_id;

    // 检查 pick action 是否已完成（英雄已锁定）
    for ag in &session.actions {
        for a in ag {
            if a.actor_cell_id == cell_id && a.action_type == "pick" && !a.completed {
                return; // 还没锁定，不展示
            }
        }
    }

    // 获取位置和英雄 ID
    let (position, champion_id) = match session.my_team.iter().find(|p| p.cell_id == cell_id) {
        Some(player) => {
            let pos = player.assigned_position.clone();
            let champ_id = if player.champion_id != 0 {
                player.champion_id
            } else {
                player.champion_pick_intent
            };
            (pos, champ_id)
        }
        None => return,
    };

    if champion_id == 0 || champion_id == selection._opgg_show_champion_id {
        return;
    }

    // 位置映射为 OPGG 格式
    let opgg_position = match position.as_str() {
        "top" => "TOP",
        "jungle" => "JUNGLE",
        "middle" => "MID",
        "bottom" => "ADC",
        "utility" => "SUPPORT",
        _ => "",
    };

    // 模式判定
    let mode = match session.queue_id {
        Some(450) => "aram",
        Some(1700) | Some(1710) => "arena",
        Some(1300) => "nexus_blitz",
        Some(900) | Some(1900) => "urf",
        Some(_) => "ranked",
        None => {
            if session.bench_enabled {
                "aram"
            } else if session.my_team.len() == 2 {
                "arena"
            } else {
                "ranked"
            }
        }
    };

    selection._opgg_show_champion_id = champion_id;

    log::info!(
        "OPGG 构建展示: champion={}, mode={}, pos={}",
        champion_id,
        mode,
        opgg_position
    );

    let event = OpggBuildEvent {
        champion_id,
        mode: mode.to_string(),
        position: opgg_position.to_string(),
    };
    let _ = app_handle.emit("opgg-build-ready", event);
}

// ─── 自动禁人 ───

async fn do_auto_ban(
    app_handle: &AppHandle,
    session: &ChampSelectSession,
    cfg: &FunctionsConfig,
    selection: &mut ChampionSelection,
) {
    if !cfg.enable_auto_ban_champion || selection.is_champion_banned {
        return;
    }

    let cell_id = session.local_player_cell_id;

    let ban_action = session.actions.iter().flatten().find(|a| {
        a.actor_cell_id == cell_id && a.action_type == "ban" && a.is_in_progress
    });

    let action = match ban_action {
        Some(a) => a,
        None => return,
    };

    let pos = session
        .my_team
        .iter()
        .find(|p| p.cell_id == cell_id)
        .map(|p| p.assigned_position.as_str())
        .unwrap_or("");

    let mut candidates = get_position_ban_candidates(pos, cfg);
    candidates.extend(cfg.auto_ban_champion.iter().copied());

    let all_bans: Vec<i32> = session
        .bans
        .my_team_bans
        .iter()
        .chain(session.bans.their_team_bans.iter())
        .copied()
        .collect();
    candidates.retain(|c| !all_bans.contains(c));

    if cfg.auto_ban_delay > 0.0 {
        tokio::time::sleep(std::time::Duration::from_secs_f64(cfg.auto_ban_delay)).await;
    }

    if cfg.pretend_ban {
        let intents: Vec<i32> = session
            .my_team
            .iter()
            .map(|p| p.champion_pick_intent)
            .filter(|&id| id != 0)
            .collect();
        candidates.retain(|c| !intents.contains(c));
    }

    if candidates.is_empty() {
        selection.is_champion_banned = true;
        return;
    }

    let champion_id = candidates[0];
    log::info!("自动禁用英雄: {}", champion_id);

    if lcu_patch_action(app_handle, action.id, champion_id, true).await {
        selection.is_champion_banned = true;
    }
}

// ─── 自动选人 ───

async fn do_auto_pick(
    app_handle: &AppHandle,
    session: &ChampSelectSession,
    cfg: &FunctionsConfig,
    selection: &mut ChampionSelection,
) {
    if !cfg.enable_auto_select_champion || selection.is_champion_picked {
        return;
    }

    let cell_id = session.local_player_cell_id;

    if let Some(player) = session.my_team.iter().find(|p| p.cell_id == cell_id) {
        if player.champion_id != 0 || player.champion_pick_intent != 0 {
            selection.is_champion_picked = true;
            return;
        }
    }

    let all_bans: Vec<i32> = session
        .bans
        .my_team_bans
        .iter()
        .chain(session.bans.their_team_bans.iter())
        .copied()
        .collect();

    let pos = session
        .my_team
        .iter()
        .find(|p| p.cell_id == cell_id)
        .map(|p| p.assigned_position.as_str())
        .unwrap_or("");

    let mut candidates = get_position_select_candidates(pos, cfg);
    candidates.extend(cfg.auto_select_champion.iter().copied());
    candidates.retain(|c| !all_bans.contains(c));

    if candidates.is_empty() {
        selection.is_champion_picked = true;
        return;
    }

    let champion_id = candidates[0];

    let pick_action = session
        .actions
        .iter()
        .rev()
        .flatten()
        .find(|a| a.actor_cell_id == cell_id && a.action_type == "pick");

    let action = match pick_action {
        Some(a) => a,
        None => return,
    };

    log::info!("自动选择英雄: {}", champion_id);

    if lcu_patch_action(app_handle, action.id, champion_id, true).await {
        selection.is_champion_picked = true;
    }
}

// ─── 自动设置召唤师技能 ───

async fn do_auto_spell(
    app_handle: &AppHandle,
    session: &ChampSelectSession,
    cfg: &FunctionsConfig,
    selection: &mut ChampionSelection,
) {
    if selection.is_summoner_spell_set {
        return;
    }
    selection.is_summoner_spell_set = true;

    if !cfg.enable_auto_set_spells {
        return;
    }

    let cell_id = session.local_player_cell_id;
    let pos = session
        .my_team
        .iter()
        .find(|p| p.cell_id == cell_id)
        .map(|p| p.assigned_position.as_str())
        .unwrap_or("");

    let mut spells = get_position_spell_candidates(pos, cfg);

    if spells.contains(&54) || spells.is_empty() {
        spells = cfg.auto_set_summoner_spell.clone();
    }
    if spells.len() < 2 || spells.contains(&54) {
        log::debug!("召唤师技能未配置完整，跳过");
        return;
    }

    log::info!("自动设置召唤师技能: {} / {}", spells[0], spells[1]);

    let body = serde_json::json!({ "spell1Id": spells[0], "spell2Id": spells[1] });
    lcu_patch_session(app_handle, "/lol-champ-select/v1/session/my-selection", &body).await;
}

// ─── LCU API 调用 ───

/// PATCH action（选人/禁人/亮英雄）
async fn lcu_patch_action(
    app_handle: &AppHandle,
    action_id: i32,
    champion_id: i32,
    completed: bool,
) -> bool {
    let url = format!(
        "/lol-champ-select/v1/session/actions/{}",
        action_id
    );
    let body = serde_json::json!({
        "championId": champion_id,
        "completed": completed,
    });
    lcu_patch_session(app_handle, &url, &body).await
}

/// 通用 PATCH 请求到 LCU
async fn lcu_patch_session(app_handle: &AppHandle, path: &str, body: &Value) -> bool {
    let app_state = app_handle.state::<crate::AppState>();
    let lock = app_state.lcu_client.read().await;
    let lcu = match lock.as_ref() {
        Some(c) => c,
        None => {
            log::warn!("LCU 未连接");
            return false;
        }
    };

    let url = format!("https://127.0.0.1:{}{}", lcu.port, path);
    let auth = crate::build_auth_header(&lcu.token);

    match lcu
        .http_client
        .patch(&url)
        .header("Authorization", auth)
        .json(body)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            log::debug!("LCU PATCH {} 成功", path);
            true
        }
        Ok(resp) => {
            log::warn!("LCU PATCH {} 失败: HTTP {}", path, resp.status());
            false
        }
        Err(e) => {
            log::error!("LCU PATCH {} 请求失败: {}", path, e);
            false
        }
    }
}

/// 通用 POST 请求到 LCU
async fn lcu_post(app_handle: &AppHandle, path: &str) -> bool {
    let app_state = app_handle.state::<crate::AppState>();
    let lock = app_state.lcu_client.read().await;
    let lcu = match lock.as_ref() {
        Some(c) => c,
        None => return false,
    };

    let url = format!("https://127.0.0.1:{}{}", lcu.port, path);
    let auth = crate::build_auth_header(&lcu.token);

    match lcu
        .http_client
        .post(&url)
        .header("Authorization", auth)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => true,
        Ok(resp) => {
            log::warn!("LCU POST {} 失败: HTTP {}", path, resp.status());
            false
        }
        Err(e) => {
            log::error!("LCU POST {} 请求失败: {}", path, e);
            false
        }
    }
}

/// GET 当前选人会话（autoComplete 重取数据用）
async fn lcu_get_session(app_handle: &AppHandle) -> Option<ChampSelectSession> {
    let app_state = app_handle.state::<crate::AppState>();
    let lock = app_state.lcu_client.read().await;
    let lcu = lock.as_ref()?;

    let url = format!(
        "https://127.0.0.1:{}/lol-champ-select/v1/session",
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

    if !resp.status().is_success() {
        return None;
    }

    resp.json::<ChampSelectSession>().await.ok()
}

// ─── 候选列表工具函数 ───

fn get_position_select_candidates(pos: &str, cfg: &FunctionsConfig) -> Vec<i32> {
    match pos {
        "top" => cfg.auto_select_champion_top.clone(),
        "jungle" => cfg.auto_select_champion_jug.clone(),
        "middle" => cfg.auto_select_champion_mid.clone(),
        "bottom" => cfg.auto_select_champion_bot.clone(),
        "utility" => cfg.auto_select_champion_sup.clone(),
        _ => Vec::new(),
    }
}

fn get_position_ban_candidates(pos: &str, cfg: &FunctionsConfig) -> Vec<i32> {
    match pos {
        "top" => cfg.auto_ban_champion_top.clone(),
        "jungle" => cfg.auto_ban_champion_jug.clone(),
        "middle" => cfg.auto_ban_champion_mid.clone(),
        "bottom" => cfg.auto_ban_champion_bot.clone(),
        "utility" => cfg.auto_ban_champion_sup.clone(),
        _ => Vec::new(),
    }
}

fn get_position_spell_candidates(pos: &str, cfg: &FunctionsConfig) -> Vec<i32> {
    match pos {
        "top" => cfg.auto_set_summoner_spell_top.clone(),
        "jungle" => cfg.auto_set_summoner_spell_jug.clone(),
        "middle" => cfg.auto_set_summoner_spell_mid.clone(),
        "bottom" => cfg.auto_set_summoner_spell_bot.clone(),
        "utility" => cfg.auto_set_summoner_spell_sup.clone(),
        _ => Vec::new(),
    }
}
