//! 对局回放状态机与调度中心 (ReplayStore)
//!
//! 对齐 Vue ReplayButton.vue 的状态机模型：
//! - 共用状态存储：无论是战绩查询详情面板还是生涯卡片，同一 game_id 共享同一份状态
//! - 防连点去重：Checking / Downloading 状态下禁止重复触发
//! - 自动轮询：下载中按 1.5s 间隔轮询，上限 40 次 (60s)
//! - 失败重试：Error 状态再次点击即重新触发检查与下载

use crate::match_model::ReplayState;
use crate::services::live_lcu::LiveLcuService;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

static REPLAY_STORE: OnceLock<Arc<RwLock<HashMap<u64, ReplayState>>>> = OnceLock::new();

fn get_store() -> Arc<RwLock<HashMap<u64, ReplayState>>> {
    REPLAY_STORE
        .get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
        .clone()
}

/// 获取指定对局当前的回放状态
pub fn get_replay_state(game_id: u64) -> ReplayState {
    let store = get_store();
    let map = store.read().unwrap();
    map.get(&game_id).copied().unwrap_or(ReplayState::Idle)
}

/// 设置指定对局的回放状态
pub fn set_replay_state(game_id: u64, state: ReplayState) {
    let store = get_store();
    let mut map = store.write().unwrap();
    map.insert(game_id, state);
}

/// 触发回放动作（检查 / 下载 / 播放 / 重试）
pub fn trigger_replay_action<F>(game_id: u64, live_service: LiveLcuService, on_notify: F)
where
    F: Fn() + Send + Sync + 'static,
{
    let current = get_replay_state(game_id);

    // 1. 下载中或检查中：防重复点击去重
    if matches!(current, ReplayState::Checking | ReplayState::Downloading) {
        log::info!("[ReplayStore] game_id={} 正在处理中，忽略重复点击", game_id);
        return;
    }

    // 2. 不支持的对局：不重复尝试
    if matches!(current, ReplayState::Unsupported) {
        log::warn!("[ReplayStore] game_id={} 回放不可用或已过期", game_id);
        return;
    }

    let notify = Arc::new(on_notify);

    // 3. 已就绪可观看：直接调用播放
    if matches!(current, ReplayState::Watch) {
        let n = notify.clone();
        yuumi_core::runtime::spawn_background(async move {
            if let Some(client) = live_service.get_client().await {
                log::info!("[ReplayStore] 启动播放对局回放: game_id={}", game_id);
                if let Err(e) = yuumi_core::parsers::replay::watch_game_replay(&client, game_id).await {
                    log::error!("[ReplayStore] 启动回放失败: {}", e);
                    set_replay_state(game_id, ReplayState::Error);
                    n();
                }
            }
        });
        return;
    }

    // 4. Idle 或 Error：先进入 Checking，查询当前状态
    set_replay_state(game_id, ReplayState::Checking);
    notify();

    let n = notify.clone();
    yuumi_core::runtime::spawn_background(async move {
        let Some(client) = live_service.get_client().await else {
            log::warn!("[ReplayStore] LCU 客户端未连接，无法获取回放");
            set_replay_state(game_id, ReplayState::Error);
            n();
            return;
        };

        // 检查 LCU 端的初始回放状态
        match yuumi_core::parsers::replay::get_replay_status(&client, game_id).await {
            Ok(raw_status) => {
                let s = raw_status.to_ascii_lowercase();
                if s == "watch" || s == "found" {
                    set_replay_state(game_id, ReplayState::Watch);
                    n();
                    // 已就绪，直接唤起播放
                    let _ = yuumi_core::parsers::replay::watch_game_replay(&client, game_id).await;
                } else if s == "unsupported"
                    || s == "incompatible"
                    || s == "missingorexpired"
                    || s == "lost"
                {
                    set_replay_state(game_id, ReplayState::Unsupported);
                    n();
                } else if s == "downloading" {
                    set_replay_state(game_id, ReplayState::Downloading);
                    n();
                    start_polling_replay(client, game_id, n.clone()).await;
                } else {
                    // 需要触发下载
                    set_replay_state(game_id, ReplayState::Downloading);
                    n();
                    if let Err(e) = yuumi_core::parsers::replay::download_game_replay(&client, game_id).await {
                        log::error!("[ReplayStore] 触发下载回放失败: {}", e);
                        set_replay_state(game_id, ReplayState::Error);
                        n();
                        return;
                    }
                    start_polling_replay(client, game_id, n.clone()).await;
                }
            }
            Err(e) => {
                log::error!("[ReplayStore] 获取回放状态失败: {}", e);
                set_replay_state(game_id, ReplayState::Error);
                n();
            }
        }
    });
}

/// 轮询回放下载进度（最多 40 次，每次间隔 1.5 秒，共 60 秒）
async fn start_polling_replay(
    client: yuumi_core::lcu::client::LcuClient,
    game_id: u64,
    notify: Arc<dyn Fn() + Send + Sync>,
) {
    const MAX_POLLS: usize = 40;
    const POLL_INTERVAL: Duration = Duration::from_millis(1500);

    for _ in 1..=MAX_POLLS {
        tokio::time::sleep(POLL_INTERVAL).await;

        match yuumi_core::parsers::replay::get_replay_status(&client, game_id).await {
            Ok(raw_status) => {
                let s = raw_status.to_ascii_lowercase();
                if s == "watch" || s == "found" {
                    log::info!("[ReplayStore] 回放下载完成，转入 Watch: game_id={}", game_id);
                    set_replay_state(game_id, ReplayState::Watch);
                    notify();
                    return;
                } else if s == "unsupported"
                    || s == "incompatible"
                    || s == "missingorexpired"
                    || s == "lost"
                {
                    log::warn!("[ReplayStore] 回放不可用: game_id={}, status={}", game_id, s);
                    set_replay_state(game_id, ReplayState::Unsupported);
                    notify();
                    return;
                } else if s == "error" {
                    log::error!("[ReplayStore] 回放下载失败: game_id={}", game_id);
                    set_replay_state(game_id, ReplayState::Error);
                    notify();
                    return;
                }
            }
            Err(_) => {
                // 忽略偶发的单次轮询异常，继续等待
            }
        }
    }

    log::warn!("[ReplayStore] 回放下载轮询超时: game_id={}", game_id);
    set_replay_state(game_id, ReplayState::Error);
    notify();
}
