use tauri::{AppHandle, Manager};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

use crate::config::FunctionsConfig;
use crate::lcu::client::lcu_request;
use std::sync::Arc;

/// 创建预设大厅后 LCU 会瞬时闪回 None（Lobby→None 抖动）的防抖窗口。
/// 该窗口内出现的 None 视为抖动，不重置建厅状态，避免重复建厅把玩家踢出小队。
const LOBBY_FLICKER_WINDOW: std::time::Duration = std::time::Duration::from_millis(2000);

/// 建厅重试的最大轮数（每轮间隔 2 秒）。
/// LCU 刚启动的前十几秒会返回 CURRENT_PLAYER_PUUID_NOT_FOUND 等暂态错误，
/// 十余轮足够覆盖；过长的重试链会在 LCU 换代时拖住全局 API 信号量。
const LOBBY_CREATE_MAX_ATTEMPTS: u32 = 12;

/// 自动创建大厅的共享状态（建厅重试在后台任务中执行，需跨任务共享标志）
#[derive(Default)]
struct LobbyState {
    created: bool,
    /// 已有建厅任务在后台重试，防止多条路径（阶段事件 / 延迟复查 / 手动重置）
    /// 并发启动多个建厅任务。
    running: bool,
    /// 本轮「空闲(None)」阶段是否已处理过。
    /// LCU 关闭或重连过程中会在极短时间内重复推送同一阶段（实测一次关闭可产生
    /// 上百条 gameflow-phase=None），若逐条重置建厅标志并重新启动建厅任务，会并发
    /// 堆出上百个任务把全局 API 信号量（默认仅 5 个许可）吃满，导致新 LCU 就绪后
    /// 建厅请求排队数十秒才发出。
    idle_handled: bool,
    /// 处理上述空闲阶段时对应的 LCU 实例（pid + port）。
    /// 用来区分「同一 LCU 会话内的重复 None 事件」与「LCU 重启后的全新会话」：
    /// 后者必须重新触发一次建厅，前者必须忽略。
    idle_lcu: Option<(u32, u16)>,
    last_create: Option<std::time::Instant>,
}

type LobbyStateHandle = Arc<std::sync::Mutex<LobbyState>>;

// ─── 游戏流程事件 ───

#[derive(Debug, Clone)]
pub enum GameflowEvent {
    PhaseChanged(String),
    ReadyCheck(ReadyCheckData),
    ResetLobbyState,
    /// 对局结束荣誉投票（/lol-honor-v2/v1/ballot）
    HonorBallot(HonorBallot),
    /// 收到的游戏邀请列表（/lol-lobby/v2/received-invitations）
    ReceivedInvitations(Vec<ReceivedInvitation>),
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadyCheckData {
    pub state: Option<String>,
    pub player_response: Option<String>,
}

/// 荣誉投票信息
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HonorBallot {
    pub eligible_allies: Vec<HonorEligiblePlayer>,
    pub eligible_opponents: Vec<HonorEligiblePlayer>,
    pub vote_pool: Option<HonorVotePool>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HonorEligiblePlayer {
    #[serde(default)]
    pub bot_player: bool,
    #[serde(default)]
    pub puuid: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HonorVotePool {
    #[serde(default)]
    pub votes: i32,
}

/// 收到的游戏邀请
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceivedInvitation {
    #[serde(default)]
    pub invitation_id: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub can_accept_invitation: Option<bool>,
}

/// 启动游戏流程自动化后台任务。
/// 处理：自动接受匹配、自动重连、自动创建大厅、对局结束上传。
pub fn start(
    app_handle: AppHandle,
    mut rx: mpsc::Receiver<GameflowEvent>,
    upload_trigger: crate::upload::UploadTrigger,
) {
    crate::spawn_log_panic(async move {
        let mut last_phase = get_current_phase(&app_handle).await.unwrap_or_default();
        let lobby_state: LobbyStateHandle = Arc::default();
        let mut upload_trigger = upload_trigger;
        let mut ready_check_accepted = false; // 跟踪是否已接受匹配
        let mut honored = false; // 跟踪当前对局是否已自动荣誉点赞（防 WS 重复推送刷屏）

        while let Some(event) = rx.recv().await {
            let cfg = {
                let state = app_handle.state::<crate::AppState>();
                let lock = state.config.read().await;
                lock.functions.clone()
            };

            match event {
                GameflowEvent::PhaseChanged(phase) => {
                    // 阶段变化时重置接受标记
                    if phase != "ReadyCheck" {
                        ready_check_accepted = false;
                    }
                    // 进入新对局时重置荣誉点赞标记
                    if phase == "InProgress" {
                        honored = false;
                    }
                    handle_phase_change(
                        &app_handle,
                        &phase,
                        &cfg,
                        &lobby_state,
                        &mut last_phase,
                        &mut upload_trigger,
                    )
                    .await;

                    // 进入 ReadyCheck 阶段时触发自动接受匹配后台任务
                    if phase == "ReadyCheck"
                        && cfg.enable_auto_accept_matching
                        && !ready_check_accepted
                    {
                        ready_check_accepted = true;
                        spawn_auto_accept(app_handle.clone(), cfg.auto_accept_matching_delay);
                    }
                }
                GameflowEvent::ReadyCheck(data) => {
                    if last_phase == "ReadyCheck"
                        && cfg.enable_auto_accept_matching
                        && !ready_check_accepted
                    {
                        let already_responded = data
                            .player_response
                            .as_ref()
                            .map(|r| r == "Accepted" || r == "Declined")
                            .unwrap_or(false);
                        if already_responded {
                            log::debug!("收到 ReadyCheck 事件，但玩家已响应，标记为已处理");
                            ready_check_accepted = true;
                        } else {
                            ready_check_accepted = true;
                            spawn_auto_accept(app_handle.clone(), cfg.auto_accept_matching_delay);
                        }
                    }
                }
                GameflowEvent::ResetLobbyState => {
                    log::info!("收到重置大厅创建状态指令，重置为 false");
                    {
                        let mut lobby = lobby_state.lock().unwrap_or_else(|e| e.into_inner());
                        lobby.created = false;
                        lobby.idle_handled = false;
                        lobby.idle_lcu = None;
                    }
                    if last_phase == "None" && cfg.enable_auto_create_lobby {
                        try_create_default_lobby(app_handle.clone(), &cfg, lobby_state.clone());
                    }
                }
                GameflowEvent::HonorBallot(ballot) => {
                    if cfg.enable_auto_honor && !honored {
                        honored = true;
                        spawn_auto_honor(app_handle.clone(), ballot);
                    }
                }
                GameflowEvent::ReceivedInvitations(invitations) => {
                    if cfg.enable_auto_handle_invite {
                        spawn_handle_invitations(app_handle.clone(), invitations);
                    }
                }
            }
        }
    });
}

/// 游戏阶段变化处理
async fn handle_phase_change(
    app_handle: &AppHandle,
    phase: &str,
    cfg: &FunctionsConfig,
    lobby_state: &LobbyStateHandle,
    last_phase: &mut String,
    upload_trigger: &mut crate::upload::UploadTrigger,
) {
    log::info!("游戏阶段: {}", phase);

    let state = app_handle.state::<crate::AppState>();
    // 当离开 ReadyCheck 阶段时，立即协同取消正在等待中的自动接受任务
    if phase != "ReadyCheck" {
        state
            .supervisor
            .cancel_kind(&crate::agents::supervisor::TaskKind::AutoAccept)
            .await;
    }
    // 当离开断线重连阶段时，立即协同取消正在退避等待中的自动重连任务。
    // 自动重连只在 Reconnect 阶段有意义，回到 InProgress 说明客户端已自行恢复连接，
    // 此时必须先唤醒任务终止，而不是让它空等剩余 sleep 再靠阶段校验退出。
    if phase != "Reconnect" {
        state
            .supervisor
            .cancel_kind(&crate::agents::supervisor::TaskKind::AutoReconnect)
            .await;
    }
    // 当离开结算相关阶段时，立即协同取消正在等待中的再来一局任务
    if !matches!(phase, "WaitingForStats" | "PreEndOfGame" | "EndOfGame") {
        state
            .supervisor
            .cancel_kind(&crate::agents::supervisor::TaskKind::AutoPlayAgain)
            .await;
    }

    // 进入对局阶段时开启自动截图事件监听，退出对局阶段时关闭；
    // Reconnect 期间保持局内监听，避免因网络抖动误关截图
    if phase == "InProgress" {
        super::auto_screenshot::set_in_game(true);
        // 缓存本局信息，供对局结束自动记录相遇玩家
        spawn_cache_current_game(app_handle.clone());
    } else if phase != "Reconnect" {
        super::auto_screenshot::set_in_game(false);
    }

    // 进入 "None" 空闲状态时重置大厅创建标志（允许 WS 重连后重新创建）。
    // 但创建预设大厅成功后 LCU 会在极短时间内闪回一次 None（Lobby→None 抖动），
    // 若此时也重置标志会立刻再次建厅，重复 POST 会把玩家踢出小队（"你已被移出小队"）。
    // 因此距上次建厅不足防抖窗口内出现的 None 视为抖动，安排延迟复查，避免误判导致状态卡死。
    //
    // 仅在「新的 LCU 会话」的首条 None 上处理：LCU 关闭/重连时会重复推送同一阶段上百次，
    // 逐条重置并重启建厅任务会并发堆出上百个任务，吃满全局 API 信号量。
    if phase == "None" {
        // 以 LCU 实例为界：同一实例内的重复 None 事件只处理一次，
        // 而 LCU 重启换代后的首条 None 必须正常重置状态并重新建厅。
        let current_lcu = {
            let state = app_handle.state::<crate::AppState>();
            let lock = state.lcu_client.read().await;
            lock.as_ref().map(|l| (l.pid, l.port))
        };
        let already_handled = {
            let mut lobby = lobby_state.lock().unwrap_or_else(|e| e.into_inner());
            if lobby.idle_lcu != current_lcu {
                log::debug!(
                    "LCU 实例已变化（{:?} → {:?}），重置本轮空闲阶段标记",
                    lobby.idle_lcu,
                    current_lcu
                );
                lobby.idle_lcu = current_lcu;
                lobby.idle_handled = false;
                lobby.created = false;
            }
            lobby.idle_handled
        };
        if !already_handled {
            {
                let mut lobby = lobby_state.lock().unwrap_or_else(|e| e.into_inner());
                lobby.idle_handled = true;
            }
            let within_flicker = {
                let lobby = lobby_state.lock().unwrap_or_else(|e| e.into_inner());
                lobby
                    .last_create
                    .map(|t| t.elapsed() < LOBBY_FLICKER_WINDOW)
                    .unwrap_or(false)
            };
            if within_flicker {
                log::debug!("忽略 Lobby→None 抖动（距上次建厅不足防抖窗口），安排延迟复查");
                spawn_verify_lobby_flicker(
                    app_handle.clone(),
                    cfg.clone(),
                    lobby_state.clone(),
                    LOBBY_FLICKER_WINDOW + Duration::from_millis(500),
                );
            } else {
                let mut lobby = lobby_state.lock().unwrap_or_else(|e| e.into_inner());
                lobby.created = false;
            }
        }
    } else {
        // 离开空闲阶段，结束本轮空闲标记，便于下次回到 None 时重新建厅
        let mut lobby = lobby_state.lock().unwrap_or_else(|e| e.into_inner());
        lobby.idle_handled = false;
    }
    let prev_phase = last_phase.clone();
    *last_phase = phase.to_string();

    // 空闲状态 → 自动创建预设大厅（重试循环在后台任务中执行，不阻塞事件循环）
    if phase == "None" && cfg.enable_auto_create_lobby {
        try_create_default_lobby(app_handle.clone(), cfg, lobby_state.clone());
    }

    // 断线重连阶段 → 自动重连（指数退避，最多 5 次）。
    // 仅在 Reconnect 阶段触发：对局正常进行（加载/游戏内）时客户端不开放重连接口，
    // 恒返回 403 "Reconnect is not available"，在 InProgress 阶段发起只是无效请求。
    // 如果前一阶段已经是结算相关阶段或空闲阶段，说明对局已结束，绝不触发重连。
    // 重试链整条交给 supervisor 后台执行：阻塞在主循环里会让后续 ReadyCheck /
    // HonorBallot / ReceivedInvitations 事件最长积压约 7.5 秒。
    // AutoReconnect 采用 RejectNew 策略，重复的事件只会有一条重试链。
    if phase == "Reconnect" && cfg.enable_auto_reconnect {
        if matches!(
            prev_phase.as_str(),
            "WaitingForStats" | "PreEndOfGame" | "EndOfGame" | "Lobby" | "None"
        ) {
            log::info!(
                "前一阶段为 {}，对局已结算/脱离对局，忽略断线重连触发",
                prev_phase
            );
        } else {
            log::info!("检测到断线重连阶段，准备自动重连...");
            spawn_auto_reconnect(app_handle.clone());
        }
    }

    // 再来一局：进入结算相关阶段后，延迟触发"再来一局"
    if cfg.enable_auto_play_again {
        match phase {
            // 等待结算数据：10 秒后仍停留则返回大厅
            "WaitingForStats" => {
                spawn_auto_play_again(app_handle.clone(), Duration::from_millis(10000));
            }
            // 部分模式只有 PreEndOfGame 而无 EndOfGame，等投票出现后执行
            "PreEndOfGame" => {
                spawn_auto_play_again(app_handle.clone(), Duration::from_millis(3250));
            }
            // 正常结算：短缓冲后执行
            "EndOfGame" => {
                spawn_auto_play_again(app_handle.clone(), Duration::from_millis(1575));
            }
            _ => {}
        }
    }

    // 对局结束 → 自动记录相遇玩家（缓存消费一次，PreEndOfGame/EndOfGame 幂等）
    if matches!(phase, "EndOfGame" | "PreEndOfGame") {
        spawn_record_encountered_players(app_handle.clone());
    }

    // ARAM 换边报边：进入选人阶段后检测并播报我方队伍边
    if phase == "ChampSelect" && cfg.enable_auto_aram_team_side {
        spawn_aram_team_side(app_handle.clone(), cfg.aram_team_side_visible_to_team);
    }

    // 选人阶段 → 对带标记的玩家发送聊天提醒
    if phase == "ChampSelect" && cfg.enable_auto_tag_reminder {
        spawn_tag_reminder(app_handle.clone());
    }

    // 状态转换检测 → 上传队列（包含延迟 2 秒 + 去重）
    upload_trigger.on_phase_change(phase, app_handle).await;
}

/// 针对防抖窗口内出现的 None 事件进行延迟复查。
/// 如果防抖窗口过后游戏依然处于 None 且没有有效大厅，说明大厅已丢失，重置状态并重新触发建厅。
fn spawn_verify_lobby_flicker(
    app_handle: AppHandle,
    cfg: FunctionsConfig,
    lobby_state: LobbyStateHandle,
    delay: Duration,
) {
    crate::spawn_log_panic(async move {
        sleep(delay).await;

        if !cfg.enable_auto_create_lobby {
            return;
        }

        let state = app_handle.state::<crate::AppState>();
        let app_state = state.inner();

        if app_state.lcu_client.read().await.as_ref().is_none() {
            return;
        }

        let current_phase =
            match lcu_request(app_state, "GET", "/lol-gameflow/v1/gameflow-phase", None).await {
                Ok(serde_json::Value::String(p)) => p,
                _ => return,
            };

        let lobby_exists = lcu_request(app_state, "GET", "/lol-lobby/v2/lobby", None)
            .await
            .is_ok();

        if current_phase == "None" && !lobby_exists {
            log::warn!("防抖窗口后检测到游戏仍处于 None 且无大厅，大厅已失效，重试建厅...");
            {
                let mut lobby = lobby_state.lock().unwrap_or_else(|e| e.into_inner());
                lobby.created = false;
            }
            try_create_default_lobby(app_handle, &cfg, lobby_state);
        } else {
            log::debug!("防抖窗口后检测到大厅状态正常 (phase={})", current_phase);
        }
    });
}

/// 自动创建预设大厅（对应 Python `_tryCreateDefaultLobby`）。
/// 重试循环放入后台任务执行，避免最长约 1 分钟的重试阻塞 gameflow 事件主循环。
fn try_create_default_lobby(
    app_handle: AppHandle,
    cfg: &FunctionsConfig,
    lobby_state: LobbyStateHandle,
) {
    // 已建成或已有建厅任务在跑则跳过（防止后续事件重复启动建厅任务）
    {
        let mut lobby = lobby_state.lock().unwrap_or_else(|e| e.into_inner());
        if lobby.created || lobby.running {
            return;
        }
        lobby.created = true;
        lobby.running = true;
    }

    let queue_id = cfg.default_game_mode;
    let lobby_state_clone = lobby_state.clone();
    crate::spawn_log_panic(async move {
        log::info!("自动创建预设大厅: queueId={}", queue_id);

        let state = app_handle.state::<crate::AppState>();
        let app_state = state.inner();

        // 绑定本次任务对应的 LCU 实例（pid + port）。
        // LCU 重启后旧实例的 port/token 立即失效，任务必须立刻退出，
        // 否则会持续以连接错误重试、长期占用全局 API 信号量，把新实例的请求一起堵住。
        let bound = {
            let lock = app_state.lcu_client.read().await;
            lock.as_ref().map(|l| (l.pid, l.port))
        };
        let Some((bound_pid, bound_port)) = bound else {
            log::info!("LCU 未连接，跳过创建大厅");
            let mut lobby = lobby_state_clone.lock().unwrap_or_else(|e| e.into_inner());
            lobby.created = false;
            lobby.running = false;
            return;
        };

        for attempt in 0..LOBBY_CREATE_MAX_ATTEMPTS {
            // 检查 LCU 是否仍然是同一实例（未断开、未重启换代）
            let same_lcu = {
                let lock = app_state.lcu_client.read().await;
                lock.as_ref()
                    .is_some_and(|l| l.pid == bound_pid && l.port == bound_port)
            };
            if !same_lcu {
                log::info!("LCU 已断开或重启，停止创建大厅");
                let mut lobby = lobby_state_clone.lock().unwrap_or_else(|e| e.into_inner());
                lobby.created = false;
                lobby.running = false;
                return;
            }

            // 检查当前阶段是否仍为 None
            if let Ok(serde_json::Value::String(phase)) =
                lcu_request(app_state, "GET", "/lol-gameflow/v1/gameflow-phase", None).await
            {
                if !matches!(
                    phase.as_str(),
                    "None" | "" | "WaitingForStats" | "PreEndOfGame"
                ) {
                    log::info!("当前阶段为 {}，跳过创建大厅", phase);
                    let mut lobby = lobby_state_clone.lock().unwrap_or_else(|e| e.into_inner());
                    lobby.created = true;
                    lobby.running = false;
                    lobby.last_create = Some(std::time::Instant::now());
                    return;
                }
            }

            // 检查是否已在有效大厅中，避免重复创建踢出已有队伍
            if lcu_request(app_state, "GET", "/lol-lobby/v2/lobby", None)
                .await
                .is_ok()
            {
                log::info!("当前已在有效大厅中，跳过创建大厅");
                let mut lobby = lobby_state_clone.lock().unwrap_or_else(|e| e.into_inner());
                lobby.created = true;
                lobby.running = false;
                lobby.last_create = Some(std::time::Instant::now());
                return;
            }

            // 尝试创建大厅
            let body = serde_json::json!({ "queueId": queue_id });
            match lcu_request(app_state, "POST", "/lol-lobby/v2/lobby", Some(body)).await {
                Ok(_) => {
                    log::info!(
                        "预设大厅请求已发送成功 (尝试 {})，等待确认状态...",
                        attempt + 1
                    );
                    {
                        let mut lobby = lobby_state_clone.lock().unwrap_or_else(|e| e.into_inner());
                        lobby.last_create = Some(std::time::Instant::now());
                    }

                    // 等待 1 秒缓冲，确认大厅未被客户端刚启动时的初始页面跳转/弹窗冲刷
                    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

                    let phase =
                        lcu_request(app_state, "GET", "/lol-gameflow/v1/gameflow-phase", None)
                            .await;
                    let is_in_lobby = match phase {
                        Ok(serde_json::Value::String(ref p)) => p == "Lobby",
                        _ => false,
                    };
                    let lobby_exists = lcu_request(app_state, "GET", "/lol-lobby/v2/lobby", None)
                        .await
                        .is_ok();

                    if is_in_lobby || lobby_exists {
                        log::info!("预设大厅创建并确认成功 (尝试 {})", attempt + 1);
                        let mut lobby = lobby_state_clone.lock().unwrap_or_else(|e| e.into_inner());
                        lobby.created = true;
                        lobby.running = false;
                        lobby.last_create = Some(std::time::Instant::now());
                        return;
                    } else {
                        log::warn!("预设大厅创建后被客户端重置（未稳定在大厅），继续重试...");
                        let mut lobby = lobby_state_clone.lock().unwrap_or_else(|e| e.into_inner());
                        lobby.last_create = None;
                    }
                }
                Err(e) => {
                    if e.contains("409") {
                        log::info!("创建大厅返回 409 (Conflict)，可能已在房间中，停止重试");
                        let mut lobby = lobby_state_clone.lock().unwrap_or_else(|e| e.into_inner());
                        lobby.created = true;
                        lobby.running = false;
                        lobby.last_create = Some(std::time::Instant::now());
                        return;
                    }
                    log::warn!("创建大厅失败: {}，重试中...", e);
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }

        log::warn!("创建预设大厅：{} 次重试均失败", LOBBY_CREATE_MAX_ATTEMPTS);
        let mut lobby = lobby_state_clone.lock().unwrap_or_else(|e| e.into_inner());
        lobby.created = false;
        lobby.running = false;
    });
}

/// 异步执行延迟接受匹配任务，由 TaskSupervisor 统一管理生命周期与去重。
fn spawn_auto_accept(app_handle: AppHandle, delay_secs: u32) {
    let state = app_handle.state::<crate::AppState>();
    let supervisor = state.supervisor.clone();
    let lifecycle = state.lifecycle.clone();
    let app_handle_clone = app_handle.clone();

    crate::spawn_log_panic(async move {
        let guard = lifecycle.capture_guard(false).await;
        let res = supervisor
            .spawn(
                crate::agents::supervisor::TaskKind::AutoAccept,
                format!("AutoAccept(delay={}s)", delay_secs),
                move |cancel_token| async move {
                    if delay_secs > 0 {
                        log::info!("将在 {} 秒后自动接受匹配...", delay_secs);
                        tokio::select! {
                            _ = cancel_token.cancelled() => {
                                log::info!("自动接受匹配任务已协同取消");
                                return;
                            }
                            _ = tokio::time::sleep(Duration::from_secs(delay_secs as u64)) => {}
                        }
                    } else {
                        log::info!("立即自动接受匹配");
                    }

                    if cancel_token.is_cancelled() {
                        return;
                    }

                    // 延迟后，先校验生命周期与对局代际
                    let current_lifecycle = lifecycle.snapshot().await;
                    if let Err(e) = guard.ensure_valid(&current_lifecycle) {
                        log::info!("自动接受匹配放弃: 生命周期失效 ({})", e);
                        return;
                    }

                    if current_lifecycle.phase != crate::agents::lifecycle::GamePhase::ReadyCheck {
                        log::info!(
                            "延迟后当前游戏阶段为 {}，不再是 ReadyCheck，取消自动接受",
                            current_lifecycle.phase.as_str()
                        );
                        return;
                    }

                    // 再次获取当前 ready check 状态以确认是否已被响应
                    match get_ready_check_status(&app_handle_clone).await {
                        Some(status) => {
                            // 检查玩家响应状态
                            if let Some(ref response) = status.player_response {
                                if response == "Accepted" || response == "Declined" {
                                    log::debug!("延迟后玩家已响应匹配: {}，跳过自动接受", response);
                                    return;
                                }
                            }
                            // 检查是否有错误
                            if let Some(ref error_code) = status.error_code {
                                log::warn!(
                                    "延迟后 Ready check 发生错误: {}，取消自动接受",
                                    error_code
                                );
                                return;
                            }
                        }
                        None => {
                            log::debug!("延迟后无法获取 ready check 状态，取消自动接受");
                            return;
                        }
                    }

                    log::info!("自动接受匹配");
                    lcu_post(&app_handle_clone, "/lol-matchmaking/v1/ready-check/accept").await;
                },
            )
            .await;

        if let Err(e) = res {
            log::debug!("启动自动接受匹配任务未执行: {}", e);
        }
    });
}

/// 自动重连：收到断线重连阶段后立即校验并请求，失败指数退避，最多 5 次。
/// 整条重试链由 supervisor 以 `RejectNew` 策略托管，因此同一时刻只会有一条重试链；
/// 重试期间退避 sleep 不会阻塞 gameflow 事件主循环。
fn spawn_auto_reconnect(app_handle: AppHandle) {
    let state = app_handle.state::<crate::AppState>();
    let supervisor = state.supervisor.clone();
    let lifecycle = state.lifecycle.clone();

    crate::spawn_log_panic(async move {
        let guard = lifecycle.capture_guard(false).await;
        let res = supervisor
            .spawn(
                crate::agents::supervisor::TaskKind::AutoReconnect,
                "AutoReconnect".to_string(),
                move |cancel_token| async move {
                    // 不设固定等待窗口：误触发与否由请求前的权威校验（游戏进程存活 +
                    // LCU 阶段）判定，而非时间。固定等待只会把误判推迟，既拦不住
                    // 「结算写入慢于窗口」的情况，又会拖慢真实掉线的重连。
                    for attempt in 1..=5u64 {
                        if cancel_token.is_cancelled() {
                            log::info!("自动重连任务已协同取消");
                            return;
                        }

                        // 每次重试前都校验仍处于断线重连状态（同一对局内的重连连接态）
                        let current_lifecycle = lifecycle.snapshot().await;
                        if let Err(e) = guard.ensure_valid(&current_lifecycle) {
                            log::info!("自动重连放弃: 生命周期失效 ({})", e);
                            return;
                        }
                        if current_lifecycle.phase != crate::agents::lifecycle::GamePhase::Reconnect
                        {
                            log::info!(
                                "当前游戏阶段为 {}，已脱离断线状态，取消自动重连",
                                current_lifecycle.phase.as_str()
                            );
                            return;
                        }

                        // 发起请求前向 LCU 主动二次校验当前阶段，防止 WS 延迟或对局结束结算闪现
                        if !is_reconnect_still_valid(&app_handle).await {
                            return;
                        }

                        // 重连接口在客户端尚未进入可重连态时返回 HTTP 403。
                        // 去掉固定等待窗口后，Reconnect 事件到达与客户端开放接口之间可能有
                        // 数百毫秒的落差，因此把 403 视作「尚未就绪」走退避重试，而不是
                        // 立即终止整条链——否则真实掉线会因这一瞬间的状态未就绪而彻底放弃。
                        // 每次重试前都会重新校验进程与 LCU 阶段，对局结束不会重复请求。
                        // 注意：不匹配响应文本，避免 Riot 调整 message 后判定失效。
                        let reconnect_ok = {
                            let app_state = app_handle.state::<crate::AppState>();
                            match lcu_request(
                                app_state.inner(),
                                "POST",
                                "/lol-gameflow/v1/reconnect",
                                None,
                            )
                            .await
                            {
                                Ok(_) => true,
                                Err(e) if is_reconnect_unavailable(&e) => {
                                    log::debug!("客户端尚未开放重连接口（HTTP 403），等待后重试");
                                    false
                                }
                                Err(_) => false,
                            }
                        };

                        if reconnect_ok {
                            log::info!("自动重连成功");
                            return;
                        }

                        if attempt < 5 {
                            let delay = Duration::from_millis(500 * (1u64 << (attempt - 1)));
                            log::info!(
                                "重连失败，{}s 后重试（第 {}/5 次）",
                                delay.as_secs_f32(),
                                attempt + 1
                            );
                            tokio::select! {
                                _ = cancel_token.cancelled() => {
                                    log::info!("自动重连任务在退避期被协同取消");
                                    return;
                                }
                                _ = tokio::time::sleep(delay) => {}
                            }
                        }
                    }
                    log::warn!("自动重连 5 次尝试均失败");
                },
            )
            .await;

        if let Err(e) = res {
            log::debug!("启动自动重连任务未执行: {}", e);
        }
    });
}

/// 在发起重连请求前，主动查询 LCU 当前权威状态与游戏进程存活情况，确认仍处于断线重连态。
/// 规避对局结束 Alt+F4 退出时 WebSocket 事件积压或服务端结算延迟导致的误拉起。
async fn is_reconnect_still_valid(app_handle: &AppHandle) -> bool {
    // 1. 游戏进程已退出 → 对局必然已结束（或用户主动退出），不重连。
    //    Alt+F4 主动退出后游戏进程立即消失，LCU 却仍会停留在 Reconnect 阶段等待
    //    服务端结算写入（实测 2~4s 后才推进到 WaitingForStats），此时发起重连
    //    会把游戏进程重新拉起并卡在加载页。真实网络掉线时游戏进程仍在运行，
    //    因此「进程缺失」是区分主动退出与真实掉线的可靠判据。
    if !is_game_process_alive().await {
        log::info!("游戏进程已退出，判定对局已结束，取消自动重连");
        return false;
    }

    let state = app_handle.state::<crate::AppState>();
    let app_state = state.inner();

    // 2. 主动拉取 gameflow-phase
    if let Ok(serde_json::Value::String(phase)) =
        lcu_request(app_state, "GET", "/lol-gameflow/v1/gameflow-phase", None).await
    {
        if phase != "Reconnect" {
            log::info!(
                "主动查询 LCU 阶段为 {}，已脱离断线重连状态，取消自动重连",
                phase
            );
            return false;
        }
    }

    // 3. 主动拉取 gameflow session 中的 phase 字段
    if let Ok(session) = lcu_request(app_state, "GET", "/lol-gameflow/v1/session", None).await {
        if let Some(session_phase) = session.get("phase").and_then(|v| v.as_str()) {
            if session_phase != "Reconnect" {
                log::info!(
                    "Gameflow session 权威阶段为 {}，非重连状态，取消自动重连",
                    session_phase
                );
                return false;
            }
        }
    }

    true
}

/// 判断对局游戏进程（League of Legends.exe）是否仍在运行。
/// 网络掉线时游戏进程存活，重连才有意义；主动退出（Alt+F4）或游戏崩溃后进程消失，
/// 此时客户端的 Reconnect 只是等待结算写入的残留态。
async fn is_game_process_alive() -> bool {
    if !detect_game_process().await {
        return false;
    }
    // 进程仍在时复核一次：Alt+F4 后 Windows 回收进程可能滞后数百毫秒，
    // 单次采样有可能在退出瞬间读到残留进程，把「主动退出」误判为「仍在游戏中」。
    sleep(Duration::from_millis(300)).await;
    detect_game_process().await
}

/// 单次枚举系统进程，判断对局客户端是否在运行。
async fn detect_game_process() -> bool {
    let detected = tauri::async_runtime::spawn_blocking(|| {
        use sysinfo::System;
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        sys.processes().values().any(|p| {
            p.name()
                .to_string_lossy()
                .to_lowercase()
                .starts_with("league of legends")
        })
    })
    .await;

    // 进程枚举任务异常时保守放行，避免误禁真实掉线的自动重连
    detected.unwrap_or(true)
}

/// 判断重连请求是否因「客户端当前不允许重连」而失败。
/// 依据 HTTP 状态码（`lcu_request` 的错误格式为 `LCU 返回错误 [状态码]: 响应体`），
/// 不依赖 Riot 的响应文本，避免 message 变更后判定失效并重新进入无意义重试。
fn is_reconnect_unavailable(error: &str) -> bool {
    error.contains("LCU 返回错误 [403]")
}

/// 获取当前 ready check 状态
async fn get_ready_check_status(app_handle: &AppHandle) -> Option<ReadyCheckStatus> {
    let state = app_handle.state::<crate::AppState>();
    let value = lcu_request(
        state.inner(),
        "GET",
        "/lol-matchmaking/v1/ready-check",
        None,
    )
    .await
    .ok()?;
    serde_json::from_value(value).ok()
}

/// 获取当前游戏阶段
async fn get_current_phase(app_handle: &AppHandle) -> Option<String> {
    let state = app_handle.state::<crate::AppState>();
    let value = lcu_request(
        state.inner(),
        "GET",
        "/lol-gameflow/v1/gameflow-phase",
        None,
    )
    .await
    .ok()?;
    match value {
        serde_json::Value::String(s) => Some(s),
        _ => None,
    }
}

/// Ready check 状态响应
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadyCheckStatus {
    player_response: Option<String>,
    error_code: Option<String>,
}

/// 通用 LCU POST 请求（统一走 lcu_request，复用信号量/重试/白名单）
pub async fn lcu_post(app_handle: &AppHandle, path: &str) -> bool {
    let state = app_handle.state::<crate::AppState>();
    lcu_request(state.inner(), "POST", path, None).await.is_ok()
}

// ─── 自动荣誉点赞 ───

/// 收到荣誉投票后异步执行点赞，不阻塞主事件循环。
fn spawn_auto_honor(app_handle: AppHandle, ballot: HonorBallot) {
    crate::spawn_log_panic(async move {
        // 过滤掉人机与无 puuid 的条目，直接收集可点赞的 puuid（避免后续 unwrap）
        let allies: Vec<&String> = ballot
            .eligible_allies
            .iter()
            .filter(|p| !p.bot_player)
            .filter_map(|p| p.puuid.as_ref())
            .collect();

        if allies.is_empty() {
            log::debug!("荣誉投票：没有可点赞的队友，跳过");
            return;
        }

        // 最多投 votePool.votes 票（默认 1）
        let votes = ballot.vote_pool.map(|p| p.votes).unwrap_or(1).max(1) as usize;
        let count = allies.len().min(votes);

        let state = app_handle.state::<crate::AppState>();
        let app_state = state.inner();

        for puuid in allies.iter().take(count) {
            let body = serde_json::json!({
                "honorType": "HEART",
                "recipientPuuid": puuid,
            });
            if let Err(e) = lcu_request(
                app_state,
                "POST",
                "/lol-honor-v2/v1/honor-player",
                Some(body),
            )
            .await
            {
                log::warn!("自动点赞失败: {}", e);
            }
        }

        // 提交投票
        if let Err(e) = lcu_request(app_state, "POST", "/lol-honor-v2/v1/ballot", None).await {
            log::warn!("提交荣誉投票失败: {}", e);
        }
        log::info!("自动荣誉点赞完成，共 {} 票", count);
    });
}

// ─── 自动处理邀请 ───

/// 收到邀请列表后异步处理，不阻塞主事件循环。
fn spawn_handle_invitations(app_handle: AppHandle, invitations: Vec<ReceivedInvitation>) {
    crate::spawn_log_panic(async move {
        let state = app_handle.state::<crate::AppState>();
        let app_state = state.inner();

        for inv in invitations {
            // 只处理待处理的邀请
            let pending = inv.state.as_deref() == Some("Pending");
            let can_accept = inv.can_accept_invitation.unwrap_or(false);
            let Some(invitation_id) = inv.invitation_id.as_deref() else {
                continue;
            };

            if !pending || !can_accept {
                continue;
            }

            let path = format!(
                "/lol-lobby/v2/received-invitations/{}/accept",
                invitation_id
            );
            match lcu_request(app_state, "POST", &path, None).await {
                Ok(_) => log::info!("已自动接受邀请 {}", invitation_id),
                Err(e) => log::warn!("自动接受邀请 {} 失败: {}", invitation_id, e),
            }
        }
    });
}

// ─── 自动再来一局 ───

/// 延迟后执行"再来一局"，由 TaskSupervisor 统一管理生命周期与去重。
fn spawn_auto_play_again(app_handle: AppHandle, delay: Duration) {
    let state = app_handle.state::<crate::AppState>();
    let supervisor = state.supervisor.clone();
    let lifecycle = state.lifecycle.clone();
    let app_handle_clone = app_handle.clone();

    crate::spawn_log_panic(async move {
        let guard = lifecycle.capture_guard(false).await;
        let res = supervisor
            .spawn(
                crate::agents::supervisor::TaskKind::AutoPlayAgain,
                format!("AutoPlayAgain(delay={:?})", delay),
                move |cancel_token| async move {
                    tokio::select! {
                        _ = cancel_token.cancelled() => {
                            log::info!("自动再来一局任务已协同取消");
                            return;
                        }
                        _ = sleep(delay) => {}
                    }

                    if cancel_token.is_cancelled() {
                        return;
                    }

                    // 延迟后，校验生命周期与对局代际
                    let current_lifecycle = lifecycle.snapshot().await;
                    if let Err(e) = guard.ensure_valid(&current_lifecycle) {
                        log::info!("自动再来一局放弃: 生命周期代际已失效 ({})", e);
                        return;
                    }

                    match current_lifecycle.phase {
                        crate::agents::lifecycle::GamePhase::WaitingForStats
                        | crate::agents::lifecycle::GamePhase::PreEndOfGame
                        | crate::agents::lifecycle::GamePhase::EndOfGame => {
                            log::info!("生命周期校验通过，自动再来一局");
                            lcu_post(&app_handle_clone, "/lol-lobby/v2/play-again").await;
                        }
                        _ => {
                            log::info!(
                                "当前游戏阶段已变为 {}，取消自动再来一局",
                                current_lifecycle.phase.as_str()
                            );
                        }
                    }
                },
            )
            .await;

        if let Err(e) = res {
            log::debug!("启动自动再来一局任务未执行: {}", e);
        }
    });
}

// ─── ARAM 自动报边 ───

/// 轮询等待选人聊天会话 ID（从 /lol-chat/v1/conversations 中找到真正已建立的聊天室）
async fn wait_for_champ_select_conv_id(
    app_state: &crate::AppState,
    room_hint: Option<&str>,
    max_attempts: usize,
) -> Option<String> {
    for attempt in 0..max_attempts {
        if attempt > 0 {
            sleep(Duration::from_millis(500)).await;
        }
        if let Ok(v) = lcu_request(app_state, "GET", "/lol-chat/v1/conversations", None).await {
            if let Some(arr) = v.as_array() {
                // 1. 如果有 room_hint，优先匹配包含该 room name / id 的会话
                if let Some(hint) = room_hint.filter(|s| !s.is_empty()) {
                    if let Some(conv) = arr.iter().find(|c| {
                        let id = c.get("id").and_then(|i| i.as_str()).unwrap_or("");
                        let name = c.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        id.contains(hint) || name == hint
                    }) {
                        if let Some(id) = conv.get("id").and_then(|i| i.as_str()) {
                            return Some(id.to_string());
                        }
                    }
                }

                // 2. 匹配 type == championSelect 或 customGame，或 id 包含 champ-select / champSelect
                if let Some(conv) = arr.iter().find(|c| {
                    let conv_type = c.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    let conv_id = c.get("id").and_then(|i| i.as_str()).unwrap_or("");
                    conv_type == "championSelect"
                        || conv_type == "customGame"
                        || conv_id.contains("champ-select")
                        || conv_id.contains("champSelect")
                }) {
                    if let Some(id) = conv.get("id").and_then(|i| i.as_str()) {
                        return Some(id.to_string());
                    }
                }
            }
        }
    }
    None
}

/// 进入选人阶段后检测并播报我方队伍边（大乱斗/匹配/排位通用），不阻塞主事件循环。
fn spawn_aram_team_side(app_handle: AppHandle, visible_to_team: bool) {
    crate::spawn_log_panic(async move {
        let state = app_handle.state::<crate::AppState>();
        let app_state = state.inner();

        // 轮询等待选人会话就绪（最多尝试 8 次，每次 500ms）
        let mut session_opt = None;
        for _ in 0..8 {
            sleep(Duration::from_millis(500)).await;
            if let Ok(v) = lcu_request(app_state, "GET", "/lol-champ-select/v1/session", None).await
            {
                if v.is_object() {
                    session_opt = Some(v);
                    break;
                }
            }
        }

        let Some(session) = session_opt else {
            log::warn!("选人报边：获取选人会话超时");
            return;
        };

        // 优先从 pin-drop-notification 获取地图阵营
        let mut side = None;
        if let Ok(data) = lcu_request(
            app_state,
            "GET",
            "/lol-champ-select/v1/pin-drop-notification",
            None,
        )
        .await
        {
            if let Some(map_side) = data.get("mapSide").and_then(|v| v.as_str()) {
                if !map_side.is_empty() {
                    side = Some(map_side.to_lowercase());
                }
            }
        }

        // 降级使用 cellId 判断队伍（5v5 中 0-4 为蓝方，5-9 为红方）
        if side.is_none() {
            let cell_id = session
                .get("localPlayerCellId")
                .and_then(|c| c.as_i64())
                .unwrap_or(0);
            if cell_id < 5 {
                side = Some("blue".to_string());
            } else {
                side = Some("red".to_string());
            }
        }

        let Some(side) = side else {
            log::warn!("选人报边：无法判定红蓝方阵营");
            return;
        };

        let side_name = if side == "blue" {
            "蓝色方"
        } else {
            "红色方"
        };

        // 从已建立的聊天会话列表中检索会话 ID（最多等待 10 次 × 500ms）
        let room_hint = session
            .pointer("/chatDetails/chatRoomName")
            .and_then(|v| v.as_str())
            .or_else(|| {
                session
                    .pointer("/chatDetails/multiUserChatId")
                    .and_then(|v| v.as_str())
            });

        let Some(conv_id) = wait_for_champ_select_conv_id(app_state, room_hint, 10).await else {
            log::warn!("选人报边：未找到选人聊天会话");
            return;
        };

        // 适当缓冲等待本地客户端和队友完成进入聊天室（避免消息在入房初始化前被冲刷覆盖）
        sleep(Duration::from_millis(2000)).await;

        // 发送报边消息；visible_to_team 为 false 时使用 celebration 类型（本地私密广播，队友不可见）
        let message = if visible_to_team {
            serde_json::json!({
                "body": format!("[RUSTYUUMI] 本局我方在{}", side_name),
                "type": "chat"
            })
        } else {
            serde_json::json!({
                "body": format!("[RUSTYUUMI] 本局我方在{}", side_name),
                "type": "celebration"
            })
        };
        let path = format!("/lol-chat/v1/conversations/{}/messages", conv_id);

        // 重试最多 3 次发送，确保聊天室通道稳定
        let mut sent = false;
        for attempt in 0..3 {
            if attempt > 0 {
                sleep(Duration::from_millis(600)).await;
            }
            match lcu_request(app_state, "POST", &path, Some(message.clone())).await {
                Ok(_) => {
                    log::info!(
                        "选人报边成功：我方在{}（队友可见={}）",
                        side_name,
                        visible_to_team
                    );
                    sent = true;
                    break;
                }
                Err(e) => {
                    log::warn!("选人报边第 {} 次尝试失败: {}", attempt + 1, e);
                }
            }
        }
        if !sent {
            log::warn!("选人报边最终发送失败");
        }
    });
}

// ─── 保存的玩家：对局相遇记录 ───

/// 异步拉取召唤师信息的辅助函数（优先通过 summonerId，失败则尝试 puuid）
async fn fetch_summoner_info(
    app_state: &crate::AppState,
    summoner_id: i64,
    puuid: &str,
) -> Option<serde_json::Value> {
    if summoner_id > 0 {
        let path = format!("/lol-summoner/v1/summoners/{}", summoner_id);
        if let Ok(info) = lcu_request(app_state, "GET", &path, None).await {
            if info.is_object() {
                return Some(info);
            }
        }
    }
    if !puuid.is_empty() {
        let path = format!("/lol-summoner/v2/summoners/puuid/{}", puuid);
        if let Ok(info) = lcu_request(app_state, "GET", &path, None).await {
            if info.is_object() {
                return Some(info);
            }
        }
    }
    None
}

/// 获取当前召唤师 puuid（失败返回空字符串）
async fn get_self_puuid(app_state: &crate::AppState) -> String {
    match lcu_request(app_state, "GET", "/lol-summoner/v1/current-summoner", None).await {
        Ok(v) => v
            .get("puuid")
            .and_then(|p| p.as_str())
            .unwrap_or("")
            .to_string(),
        Err(_) => String::new(),
    }
}

/// 进入对局阶段后缓存本局信息（gameId/queueId/玩家列表），供对局结束记录相遇
fn spawn_cache_current_game(app_handle: AppHandle) {
    crate::spawn_log_panic(async move {
        let state = app_handle.state::<crate::AppState>();
        let app_state = state.inner();

        // 等待游戏会话就绪
        sleep(Duration::from_millis(1500)).await;

        let session = match lcu_request(app_state, "GET", "/lol-gameflow/v1/session", None).await {
            Ok(v) => v,
            Err(_) => return,
        };
        let Some(game_data) = session.get("gameData") else {
            return;
        };
        let Some(game_id) = game_data.get("gameId").and_then(|v| v.as_i64()) else {
            return;
        };
        let queue_id = game_data
            .get("queueId")
            .and_then(|v| v.as_i64())
            .or_else(|| {
                game_data
                    .get("queue")
                    .and_then(|q| q.get("id"))
                    .and_then(|v| v.as_i64())
            })
            .map(|q| q as i32)
            .unwrap_or(0);

        // 云顶对局不录入路人集（queueId 与前端 MatchHistoryTab 的 TFT_QUEUES 保持一致）
        if matches!(queue_id, 1090 | 1100 | 1130 | 1160) {
            return;
        }

        let mut targets: Vec<(i64, i32, String)> = Vec::new();
        for team in ["teamOne", "teamTwo"] {
            let Some(arr) = game_data.get(team).and_then(|v| v.as_array()) else {
                continue;
            };
            for player in arr {
                let Some(summoner_id) = player.get("summonerId").and_then(|v| v.as_i64()) else {
                    continue;
                };
                let fallback_name = player
                    .get("summonerName")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or("")
                    .to_string();
                let champion_id = player
                    .get("championId")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32;
                targets.push((summoner_id, champion_id, fallback_name));
            }
        }

        // 并发拉取 10 名玩家信息（限流并发 10，避免逐个串行请求拖慢缓存）
        use futures_util::StreamExt;
        let entries: Vec<crate::saved_players::GamePlayerEntry> =
            futures_util::stream::iter(targets)
                .map(|(summoner_id, champion_id, fallback_name)| async move {
                    let info = fetch_summoner_info(app_state, summoner_id, "").await;
                    let info_obj = info.as_ref();
                    // 国服 Riot ID 体系下 displayName 常为空字符串，需先过滤再取 gameName，
                    // 都为空时回退到选人会话内的 summonerName
                    let summoner_name = info_obj
                        .and_then(|i| i.get("displayName"))
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .or_else(|| {
                            info_obj
                                .and_then(|i| i.get("gameName"))
                                .and_then(|v| v.as_str())
                                .filter(|s| !s.is_empty())
                        })
                        .or(if fallback_name.is_empty() {
                            None
                        } else {
                            Some(fallback_name.as_str())
                        })
                        .unwrap_or("")
                        .to_string();
                    let puuid = info_obj
                        .and_then(|i| i.get("puuid"))
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .unwrap_or("")
                        .to_string();
                    let profile_icon_id = info_obj
                        .and_then(|i| i.get("profileIconId"))
                        .and_then(|v| v.as_i64())
                        .filter(|n| *n > 0)
                        .unwrap_or(0) as i32;
                    let tag_line = info_obj
                        .and_then(|i| i.get("tagLine"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    Some(crate::saved_players::GamePlayerEntry {
                        puuid,
                        summoner_name,
                        profile_icon_id,
                        tag_line,
                        champion_id,
                    })
                })
                .buffer_unordered(10)
                .filter_map(|x| async move { x })
                .collect()
                .await;

        let player_count = entries.len();
        if player_count == 0 {
            return;
        }

        *app_state
            .current_game_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(crate::saved_players::CurrentGameCache {
            game_id,
            queue_id,
            players: entries,
        });
        log::info!(
            "已缓存本局信息: gameId={}, queueId={}, 玩家数={}",
            game_id,
            queue_id,
            player_count
        );
    });
}

/// 对局结束后记录所有相遇玩家（自动录入保存的玩家）
fn spawn_record_encountered_players(app_handle: AppHandle) {
    crate::spawn_log_panic(async move {
        let state = app_handle.state::<crate::AppState>();
        let app_state = state.inner();

        // 先获取自己的 puuid，成功后再消费缓存，避免取走后因 LCU 临时不可用丢失本局记录
        let self_puuid = get_self_puuid(app_state).await;
        if self_puuid.is_empty() {
            log::warn!("对局结束：无法获取自己的 puuid，跳过相遇记录");
            return;
        }

        // 取走缓存（避免 PreEndOfGame/EndOfGame 重复记录）
        let cache = app_state
            .current_game_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        let Some(cache) = cache else {
            log::debug!("对局结束：无本局信息缓存，跳过相遇记录");
            return;
        };

        let queue_type = cache.queue_id.to_string();
        let recorded = crate::saved_players::record_encounters(
            app_state,
            cache.players,
            self_puuid,
            cache.game_id,
            queue_type,
        )
        .await;
        log::info!("对局结束相遇记录完成，共 {} 名玩家", recorded);
    });
}

/// 选人阶段对带标记的玩家发送聊天提醒
fn spawn_tag_reminder(app_handle: AppHandle) {
    crate::spawn_log_panic(async move {
        let state = app_handle.state::<crate::AppState>();
        let app_state = state.inner();

        let self_puuid = get_self_puuid(app_state).await;
        if self_puuid.is_empty() {
            return;
        }

        let tagged =
            crate::saved_players::query_tagged_for_reminder(app_state, self_puuid.clone()).await;
        if tagged.is_empty() {
            return;
        }
        let tagged_map: std::collections::HashMap<String, String> = tagged.into_iter().collect();

        // 轮询等待选人会话就绪（最多尝试 6 次，每次 500ms）
        let mut session_opt = None;
        for _ in 0..6 {
            sleep(Duration::from_millis(500)).await;
            if let Ok(v) = lcu_request(app_state, "GET", "/lol-champ-select/v1/session", None).await
            {
                if v.is_object() {
                    session_opt = Some(v);
                    break;
                }
            }
        }

        let Some(session) = session_opt else {
            return;
        };

        let room_hint = session
            .pointer("/chatDetails/chatRoomName")
            .and_then(|v| v.as_str())
            .or_else(|| {
                session
                    .pointer("/chatDetails/multiUserChatId")
                    .and_then(|v| v.as_str())
            });

        let Some(conv_id) = wait_for_champ_select_conv_id(app_state, room_hint, 10).await else {
            log::debug!("标记玩家提醒：未找到选人聊天会话");
            return;
        };

        // 适当缓冲等待本地客户端和队友完成进入聊天室
        sleep(Duration::from_millis(2000)).await;

        // 选人聊天室只包含己方队伍，仅遍历己方即可，避免对对手做无谓查询
        // 并发拉取己方玩家信息并发送提醒（限流并发 5）
        let my_team = session
            .get("myTeam")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        use futures_util::StreamExt;
        let reminded = futures_util::stream::iter(my_team)
            .map(|player| {
                let tagged_map = &tagged_map;
                let conv_id = conv_id.clone();
                async move {
                    let mut puuid = player
                        .get("puuid")
                        .and_then(|p| p.as_str())
                        .unwrap_or("")
                        .to_string();
                    let summoner_id = player
                        .get("summonerId")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0);

                    let mut name = String::new();

                    let info = fetch_summoner_info(app_state, summoner_id, &puuid).await;
                    if let Some(info) = info {
                        if puuid.is_empty() {
                            puuid = info
                                .get("puuid")
                                .and_then(|p| p.as_str())
                                .unwrap_or("")
                                .to_string();
                        }
                        name = info
                            .get("displayName")
                            .and_then(|n| n.as_str())
                            .filter(|s| !s.is_empty())
                            .or_else(|| {
                                info.get("gameName")
                                    .and_then(|n| n.as_str())
                                    .filter(|s| !s.is_empty())
                            })
                            .unwrap_or("")
                            .to_string();
                    }

                    if puuid.is_empty() {
                        return 0;
                    }

                    let Some(tag) = tagged_map.get(&puuid) else {
                        return 0;
                    };

                    if name.is_empty() {
                        name = puuid.clone();
                    }

                    let message = serde_json::json!({
                        "body": format!("[RUSTYUUMI] 玩家 {} 已被标记：{}", name, tag),
                        "type": "celebration"
                    });
                    let path = format!("/lol-chat/v1/conversations/{}/messages", conv_id);
                    match lcu_request(app_state, "POST", &path, Some(message)).await {
                        Ok(_) => {
                            log::info!("已提醒标记玩家: {} ({})", name, tag);
                            1
                        }
                        Err(e) => {
                            log::warn!("标记玩家提醒发送失败: {}", e);
                            0
                        }
                    }
                }
            })
            .buffer_unordered(5)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .sum::<i32>();
        if reminded == 0 {
            log::debug!("标记玩家提醒：本局没有已标记的玩家");
        }
    });
}
