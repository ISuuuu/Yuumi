use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;

use crate::agents::action::ActionLease;
use crate::agents::supervisor::TaskKind;
use crate::config::FunctionsConfig;
use crate::lcu::client::lcu_request;

// ─── 选人会话数据结构体 ───

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectSession {
    pub actions: Vec<Vec<ChampSelectAction>>,
    #[serde(default)]
    pub local_player_cell_id: i32,
    #[serde(default)]
    pub my_team: Vec<ChampSelectPlayer>,
    #[serde(default)]
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
    /// 队伍编号（1 / 2），ARAM 报边使用
    #[serde(default)]
    pub team: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    is_summoner_spell_set: bool,
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
    crate::spawn_log_panic(async move {
        let mut selection = ChampionSelection::default();
        let mut last_epoch = 0u64;

        while let Some(session) = session_rx.recv().await {
            log::debug!(
                "[bp-agent] session received: cell_id={}, actions_groups={}, my_team_count={}",
                session.local_player_cell_id,
                session.actions.len(),
                session.my_team.len()
            );

            let state = app_handle.state::<crate::AppState>();
            let lifecycle_snapshot = state.lifecycle.snapshot().await;

            // 依据权威生命周期管理器的代际检查：若跨入新对局或不在 ChampSelect 阶段，重置选择状态
            if lifecycle_snapshot.match_epoch != last_epoch
                || lifecycle_snapshot.phase != crate::agents::lifecycle::GamePhase::ChampSelect
            {
                log::info!(
                    "生命周期代际变迁 (epoch {} -> {})，重置 BP 跟踪状态",
                    last_epoch,
                    lifecycle_snapshot.match_epoch
                );
                last_epoch = lifecycle_snapshot.match_epoch;
                selection = ChampionSelection::default();
            }

            // 收到重置信号时，重置 BP 跟踪状态并协同取消在册 BP 任务
            if session.local_player_cell_id == -1 {
                log::info!("收到重置选人代理状态信号，取消在册 BP 任务");
                state
                    .supervisor
                    .cancel_kind(&crate::agents::supervisor::TaskKind::AutoPickLock)
                    .await;
                state
                    .supervisor
                    .cancel_kind(&crate::agents::supervisor::TaskKind::AutoBanLock)
                    .await;
                state
                    .supervisor
                    .cancel_kind(&crate::agents::supervisor::TaskKind::AutoSwap)
                    .await;
                state
                    .supervisor
                    .cancel_kind(&crate::agents::supervisor::TaskKind::AutoTrade)
                    .await;
                selection = ChampionSelection::default();
                continue;
            }

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
            log::info!("自动接受楼层交换: swapId={}", swap.id);
            let url = format!(
                "/lol-champ-select/v1/session/swaps/pick-order/{}/accept",
                swap.id
            );

            // 重置选人状态，以便重新触发
            selection.is_champion_picked_completed = false;

            let state = app_handle.state::<crate::AppState>();
            let guard = state.lifecycle.capture_guard(true).await;
            let app_handle_clone = app_handle.clone();

            let _ = state
                .supervisor
                .spawn(
                    crate::agents::supervisor::TaskKind::AutoSwap,
                    format!("AutoSwap(id={})", swap.id),
                    move |cancel_token| async move {
                        tokio::select! {
                            _ = cancel_token.cancelled() => {
                                log::info!("后台交换接受任务已协同取消");
                                return;
                            }
                            _ = tokio::time::sleep(std::time::Duration::from_millis(500)) => {}
                        }

                        if cancel_token.is_cancelled() {
                            return;
                        }

                        let state = app_handle_clone.state::<crate::AppState>();
                        let current_lifecycle = state.lifecycle.snapshot().await;
                        if let Err(e) = guard.ensure_valid(&current_lifecycle) {
                            log::info!("后台交换接受任务已失效 (生命周期变迁: {})", e);
                            return;
                        }

                        lcu_post(&app_handle_clone, &url).await;
                    },
                )
                .await;
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
            log::info!("自动接受英雄交换: tradeId={}", trade.id);
            let url = format!("/lol-champ-select/v1/session/trades/{}/accept", trade.id);

            let state = app_handle.state::<crate::AppState>();
            let guard = state.lifecycle.capture_guard(true).await;
            let app_handle_clone = app_handle.clone();

            let _ = state
                .supervisor
                .spawn(
                    crate::agents::supervisor::TaskKind::AutoTrade,
                    format!("AutoTrade(id={})", trade.id),
                    move |cancel_token| async move {
                        tokio::select! {
                            _ = cancel_token.cancelled() => {
                                log::info!("后台英雄交换接受任务已协同取消");
                                return;
                            }
                            _ = tokio::time::sleep(std::time::Duration::from_millis(500)) => {}
                        }

                        if cancel_token.is_cancelled() {
                            return;
                        }

                        let state = app_handle_clone.state::<crate::AppState>();
                        let current_lifecycle = state.lifecycle.snapshot().await;
                        if let Err(e) = guard.ensure_valid(&current_lifecycle) {
                            log::info!("后台英雄交换接受任务已失效 (生命周期变迁: {})", e);
                            return;
                        }

                        lcu_post(&app_handle_clone, &url).await;
                    },
                )
                .await;
            return;
        }
    }
}

// ─── 2.3 自动亮英雄/展示意图 ───

async fn do_auto_show(
    app_handle: &AppHandle,
    session: &ChampSelectSession,
    cfg: &FunctionsConfig,
    _selection: &mut ChampionSelection,
) {
    log::debug!(
        "[autoShow] called, enable_auto_hover_champion={}",
        cfg.enable_auto_hover_champion
    );
    if !cfg.enable_auto_hover_champion || session.bench_enabled {
        return;
    }

    let cell_id = session.local_player_cell_id;

    // 从会话数据直接判断是否已经亮过/选过英雄（无状态，避免残留）
    if let Some(player) = session.my_team.iter().find(|p| p.cell_id == cell_id) {
        log::debug!(
            "[autoShow] player: champion_id={}, champion_pick_intent={}",
            player.champion_id,
            player.champion_pick_intent
        );
        if player.champion_id != 0 || player.champion_pick_intent != 0 {
            log::debug!("[autoShow] skip: already has champion/shown");
            return;
        }
    } else {
        log::warn!("[autoShow] cannot find player with cell_id={}", cell_id);
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
    log::debug!("[autoShow] pos={}, candidates={:?}", pos, candidates);

    if candidates.is_empty() {
        log::warn!("[autoShow] abort: no champion candidates");
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
        log::debug!(
            "[autoShow] PATCH action {} with champion_id={}",
            action.id,
            champion_id
        );
        let ok = lcu_patch_action(app_handle, action.id, champion_id, false).await;
        log::debug!("[autoShow] PATCH result: {}", ok);
    } else {
        log::warn!("[autoShow] cannot find pick action for cell_id={}", cell_id);
    }
}

// ─── 2.4 超时前自动锁定（不阻塞主循环） ───

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

    // 找到当前进行中的 pick action
    let target_action = session.actions.iter().flatten().find(|a| {
        a.actor_cell_id == cell_id && a.action_type == "pick" && a.is_in_progress && !a.completed
    });

    let action = match target_action {
        Some(a) => a,
        None => return,
    };

    // 计算等待时间：剩余时间 - 4 秒
    let sleep_ms = session
        .timer
        .as_ref()
        .and_then(|t| t.adjusted_time_left_in_phase)
        .unwrap_or(10000);
    let sleep_secs = (sleep_ms / 1000).saturating_sub(4);

    let state = app_handle.state::<crate::AppState>();
    let lifecycle_state = state.lifecycle.snapshot().await;
    let session_id = match lifecycle_state.session_id.clone() {
        Some(sid) => sid,
        None => {
            log::warn!("自动锁定跳过: 当前未处于有效的选人生命周期中");
            return;
        }
    };
    let guard = state.lifecycle.capture_guard(true).await;

    let mut initial_champion_id = action.champion_id;
    if initial_champion_id == 0 {
        if let Some(player) = session.my_team.iter().find(|p| p.cell_id == cell_id) {
            if player.champion_pick_intent > 0 {
                initial_champion_id = player.champion_pick_intent;
            } else if player.champion_id > 0 {
                initial_champion_id = player.champion_id;
            }
        }
    }

    let lease = crate::agents::action::ActionLease::new(
        guard,
        session_id,
        action.id,
        cell_id,
        "pick".to_string(),
        initial_champion_id,
    );

    // 标记防止重复进入
    selection.is_champion_picked_completed = true;

    log::info!(
        "自动锁定: 签发 ActionLease (action={}, cell={}), 等待 {} 秒后校验锁定",
        action.id,
        cell_id,
        sleep_secs
    );

    let app_handle_clone = app_handle.clone();
    let desc = format!("AutoPickLock(action={}, cell={})", action.id, cell_id);
    let spawn_res = state
        .supervisor
        .spawn(
            crate::agents::supervisor::TaskKind::AutoPickLock,
            desc,
            move |cancel_token| async move {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        log::info!("自动锁定任务在等待期被协同取消");
                        return;
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_secs(sleep_secs)) => {}
                }

                if cancel_token.is_cancelled() {
                    return;
                }

                // 重新获取最新会话数据
                let fresh_session = match lcu_get_session(&app_handle_clone).await {
                    Some(s) => s,
                    None => return,
                };

                let state = app_handle_clone.state::<crate::AppState>();
                let current_lifecycle = state.lifecycle.snapshot().await;

                // 核心：使用 ActionLease 进行目标一致性终极双重断言（消除 TOCTOU）
                if let Err(e) = lease.verify_target(&current_lifecycle, &fresh_session) {
                    log::warn!("自动锁定因目标校验不通过而放弃: {}", e);
                    return;
                }

                // 收集不可选的英雄（已被他人选走 + 已禁用）
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

                // 再次从 fresh_session 提取实际选定的 champion_id
                let actual_champion_id = fresh_session
                    .actions
                    .iter()
                    .flatten()
                    .find(|a| a.id == lease.action_id)
                    .map(|a| a.champion_id)
                    .unwrap_or(0);

                let target_champion_id = if actual_champion_id > 0 {
                    actual_champion_id
                } else {
                    lease.expected_champion_id
                };

                if target_champion_id == 0 {
                    log::warn!("自动锁定放弃: 玩家尚未选择任何英雄");
                    return;
                }

                if cant_select.contains(&target_champion_id) {
                    log::warn!(
                        "自动锁定放弃: 目标英雄 {} 已被禁用或被选走",
                        target_champion_id
                    );
                    return;
                }

                log::info!(
                    "ActionLease 验证通过，向 LCU 提交超时自动锁定: champion={}",
                    target_champion_id
                );
                lcu_patch_action(&app_handle_clone, lease.action_id, target_champion_id, true)
                    .await;
            },
        )
        .await;

    if let Err(e) = spawn_res {
        log::warn!("自动锁定任务派发失败: {}", e);
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

    // 模式判定（与战绩页共用同一份 queueId→模式映射）
    let mode = match session.queue_id {
        Some(q) => crate::parsers::match_parser::queue_id_to_opgg_mode(q),
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
    _selection: &mut ChampionSelection,
) {
    if !cfg.enable_auto_ban_champion {
        return;
    }

    let cell_id = session.local_player_cell_id;

    // 从会话数据判断：找到本地玩家的 ban action，如果已结束则跳过
    let ban_action = session
        .actions
        .iter()
        .flatten()
        .find(|a| a.actor_cell_id == cell_id && a.action_type == "ban" && a.is_in_progress);

    let action = match ban_action {
        Some(a) => a,
        None => return, // 不在 ban 阶段，或已经 ban 完
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
        return;
    }

    let action_id = action.id;
    let state = app_handle.state::<crate::AppState>();
    let current_lifecycle = state.lifecycle.snapshot().await;
    let guard = state.lifecycle.capture_guard(true).await;
    let session_id = current_lifecycle
        .session_id
        .clone()
        .unwrap_or_else(|| "unknown-session".to_string());

    let lease = ActionLease::new(
        guard,
        session_id,
        action_id,
        cell_id,
        "ban".to_string(),
        candidates[0],
    );

    let delay = cfg.auto_ban_delay;
    let candidates = candidates.clone();
    let app_handle = app_handle.clone();

    let desc = format!("AutoBanLock(action={}, cell={})", action_id, cell_id);
    let _ = state
        .supervisor
        .spawn(TaskKind::AutoBanLock, desc, move |cancel_token| async move {
            if delay > 0.0 {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        log::info!("[supervisor] 自动禁人任务被协作取消: action_id={}", action_id);
                        return;
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_secs_f64(delay)) => {}
                }
            }

            // 延迟或等待结束后重新拉取最新 session，结合当前生命周期执行 TOCTOU 校验
            let fresh_session = match lcu_get_session(&app_handle).await {
                Some(s) => s,
                None => {
                    log::warn!("[supervisor] 自动禁人放弃: 无法获取最新 session");
                    return;
                }
            };
            let state = app_handle.state::<crate::AppState>();
            let fresh_lifecycle = state.lifecycle.snapshot().await;

            if let Err(e) = lease.verify_target(&fresh_lifecycle, &fresh_session) {
                log::warn!("[supervisor] 自动禁人前置校验未通过 (TOCTOU防护生效): {}", e);
                return;
            }

            // 过滤期间新出现的禁用英雄，取第一个仍可选的候选
            let new_bans: Vec<i32> = fresh_session
                .bans
                .my_team_bans
                .iter()
                .chain(fresh_session.bans.their_team_bans.iter())
                .copied()
                .collect();
            let champion_id = match candidates.iter().find(|c| !new_bans.contains(c)) {
                Some(c) => *c,
                None => {
                    log::warn!("[supervisor] 自动禁人放弃: 候选英雄已被全部禁用");
                    return;
                }
            };

            log::info!("自动禁用英雄: {} (action_id={})", champion_id, action_id);
            lcu_patch_action(&app_handle, action_id, champion_id, true).await;
        })
        .await;
}

// ─── 自动选人 ───

async fn do_auto_pick(
    app_handle: &AppHandle,
    session: &ChampSelectSession,
    cfg: &FunctionsConfig,
    _selection: &mut ChampionSelection,
) {
    if !cfg.enable_auto_select_champion || session.bench_enabled {
        return;
    }

    let cell_id = session.local_player_cell_id;

    // 从会话数据判断是否已选英雄（无状态）
    if let Some(player) = session.my_team.iter().find(|p| p.cell_id == cell_id) {
        if player.champion_id != 0 || player.champion_pick_intent != 0 {
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

    log::info!("自动选择英雄: {} (预选，不锁定)", champion_id);

    lcu_patch_action(app_handle, action.id, champion_id, false).await;
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

    if !cfg.enable_auto_set_spells {
        selection.is_summoner_spell_set = true;
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
        selection.is_summoner_spell_set = true;
        return;
    }

    log::info!("自动设置召唤师技能: {} / {}", spells[0], spells[1]);

    let body = serde_json::json!({ "spell1Id": spells[0], "spell2Id": spells[1] });
    let ok = lcu_patch_session(
        app_handle,
        "/lol-champ-select/v1/session/my-selection",
        &body,
    )
    .await;
    if ok {
        selection.is_summoner_spell_set = true;
    }
}

// ─── LCU API 调用（统一走 client::lcu_request，复用信号量/重试/白名单） ───

/// PATCH action（选人/禁人/亮英雄）
async fn lcu_patch_action(
    app_handle: &AppHandle,
    action_id: i32,
    champion_id: i32,
    completed: bool,
) -> bool {
    let url = format!("/lol-champ-select/v1/session/actions/{}", action_id);
    let body = serde_json::json!({
        "championId": champion_id,
        "completed": completed,
    });
    let state = app_handle.state::<crate::AppState>();
    lcu_request(state.inner(), "PATCH", &url, Some(body))
        .await
        .is_ok()
}

/// 通用 PATCH 请求到 LCU
async fn lcu_patch_session(app_handle: &AppHandle, path: &str, body: &Value) -> bool {
    let state = app_handle.state::<crate::AppState>();
    lcu_request(state.inner(), "PATCH", path, Some(body.clone()))
        .await
        .is_ok()
}

/// 通用 POST 请求到 LCU
async fn lcu_post(app_handle: &AppHandle, path: &str) -> bool {
    let state = app_handle.state::<crate::AppState>();
    lcu_request(state.inner(), "POST", path, None).await.is_ok()
}

/// GET 当前选人会话（autoComplete 重取数据用）
async fn lcu_get_session(app_handle: &AppHandle) -> Option<ChampSelectSession> {
    let state = app_handle.state::<crate::AppState>();
    let value = lcu_request(state.inner(), "GET", "/lol-champ-select/v1/session", None)
        .await
        .ok()?;
    serde_json::from_value(value).ok()
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
