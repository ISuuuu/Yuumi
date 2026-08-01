use crate::AppState;
use std::sync::Arc;
use tauri::menu::{MenuBuilder, MenuItem, PredefinedMenuItem};
use tokio::sync::Semaphore;

/// 获取完整配置
#[tauri::command]
pub async fn get_config(
    app_state: tauri::State<'_, AppState>,
) -> Result<crate::config::AppConfig, String> {
    let cfg = app_state.config.read().await;
    Ok(cfg.clone())
}

/// 读取配置加载错误信息（前端启动时调用，读取后自动清除错误文件）
#[tauri::command]
pub fn get_config_load_error() -> Option<String> {
    crate::config::AppConfig::take_load_error()
}

/// 配置变更集合：记录 `old → new` 之间哪些字段发生了影响运行时的变化，
/// 副作用层据此决定要执行哪些运行时操作。
#[derive(Debug, Default)]
struct ConfigChanges {
    /// hide_tft 变更 → 重建托盘菜单
    rebuild_tray: bool,
    /// 自动建厅开关/默认模式变更 → 重置大厅状态
    reset_lobby: bool,
    /// SignalR 反代相关变更 → 重启/停止 Hub
    signalr_restart: bool,
    /// 上传 API 地址变更 → 触发挂起队列重试
    trigger_upload_retry: bool,
    /// API 并发数变更 → 重建信号量
    api_concurrency_changed: bool,
}

/// 检测 old → new 之间影响运行时的配置变更
fn detect_changes(old: &crate::config::AppConfig, new: &crate::config::AppConfig) -> ConfigChanges {
    let signalr_restart = new.functions.lcu_realtime_enabled != old.functions.lcu_realtime_enabled
        || new.general.upload_api_url != old.general.upload_api_url
        || new.signalr_user_id() != old.signalr_user_id();

    ConfigChanges {
        rebuild_tray: new.functions.hide_tft != old.functions.hide_tft,
        reset_lobby: new.functions.enable_auto_create_lobby
            != old.functions.enable_auto_create_lobby
            || new.functions.default_game_mode != old.functions.default_game_mode,
        signalr_restart,
        trigger_upload_retry: new.general.upload_api_url != old.general.upload_api_url
            && !new.general.upload_api_url.is_empty(),
        api_concurrency_changed: new.functions.api_concurrency_number
            != old.functions.api_concurrency_number,
    }
}

/// 应用配置变更带来的运行时副作用
async fn apply_side_effects(
    app_state: &tauri::State<'_, AppState>,
    app_handle: &tauri::AppHandle,
    new_config: &crate::config::AppConfig,
    changes: ConfigChanges,
) {
    if changes.rebuild_tray {
        if let Some(tray) = app_handle.tray_by_id("main_tray") {
            if let Ok(new_menu) = build_tray_menu(app_handle, new_config.functions.hide_tft) {
                let _ = tray.set_menu(Some(new_menu));
            }
        }
    }

    if changes.reset_lobby {
        let _ = app_state
            .gameflow_tx
            .try_send(crate::agents::auto_match::GameflowEvent::ResetLobbyState);
    }

    if changes.signalr_restart {
        if new_config.functions.lcu_realtime_enabled
            && !new_config.general.upload_api_url.is_empty()
        {
            log::info!("配置更新，重新启动 SignalR Hub 远程反代");
            let server_url = new_config.general.upload_api_url.clone();
            let user_id = new_config.signalr_user_id();
            crate::signalr::start(app_handle.clone(), server_url, user_id);
        } else {
            log::info!("配置更新，停止 SignalR Hub 远程反代");
            crate::signalr::stop().await;
        }
    }

    if changes.trigger_upload_retry {
        log::info!("上传 API 地址发生变更，正在触发挂起战绩队列重试...");
        app_state.upload_queue.trigger_pending_retry().await;
    }

    if changes.api_concurrency_changed {
        let mut sem_lock = app_state.api_semaphore.write().await;
        *sem_lock = Arc::new(Semaphore::new(
            new_config.functions.api_concurrency_number as usize,
        ));
        log::info!(
            "运行时 API 并发限制数更新为: {}",
            new_config.functions.api_concurrency_number
        );
    }
}

/// 更新配置（接收完整 AppConfig JSON，写入内存并持久化）
#[tauri::command]
pub async fn update_config(
    new_config: crate::config::AppConfig,
    app_state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    new_config.validate()?;

    let old_config = app_state.config.read().await;
    let changes = detect_changes(&old_config, &new_config);
    drop(old_config);

    {
        let mut cfg = app_state.config.write().await;
        *cfg = new_config.clone();
        cfg.save();
    }

    apply_side_effects(&app_state, &app_handle, &new_config, changes).await;

    Ok(())
}

/// 读取「关闭时最小化到托盘」开关状态
#[tauri::command]
pub async fn get_close_to_tray(app_state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let cfg = app_state.config.read().await;
    Ok(cfg.general.enable_close_to_tray.unwrap_or(false))
}

/// 构建系统托盘菜单
pub fn build_tray_menu<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    hide_tft: bool,
) -> Result<tauri::menu::Menu<R>, tauri::Error> {
    let mut builder = MenuBuilder::new(app)
        .item(&MenuItem::with_id(app, "home", "主页", true, None::<&str>)?)
        .item(&MenuItem::with_id(
            app,
            "career",
            "生涯",
            true,
            None::<&str>,
        )?)
        .item(&MenuItem::with_id(
            app,
            "search",
            "战绩查询",
            true,
            None::<&str>,
        )?)
        .item(&MenuItem::with_id(
            app,
            "gameinfo",
            "对局信息",
            true,
            None::<&str>,
        )?);

    if !hide_tft {
        builder = builder.item(&MenuItem::with_id(app, "tft", "TFT", true, None::<&str>)?);
    }

    let menu = builder
        .item(&MenuItem::with_id(
            app,
            "tools",
            "其他功能",
            true,
            None::<&str>,
        )?)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&MenuItem::with_id(
            app,
            "settings",
            "设置",
            true,
            None::<&str>,
        )?)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&MenuItem::with_id(app, "exit", "退出", true, None::<&str>)?)
        .build()?;

    Ok(menu)
}
