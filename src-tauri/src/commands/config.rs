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

/// 校验关键配置字段，防止恶意篡改
fn validate_config(cfg: &crate::config::AppConfig) -> Result<(), String> {
    let url = &cfg.general.upload_api_url;
    if !url.is_empty() && !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("upload_api_url 必须以 http:// 或 https:// 开头".to_string());
    }
    let srv = &cfg.general.signalr_server_url;
    if !srv.is_empty() && !srv.starts_with("http://") && !srv.starts_with("https://") {
        return Err("signalr_server_url 必须以 http:// 或 https:// 开头".to_string());
    }
    if !cfg.personalization.theme_color.starts_with('#') {
        return Err("theme_color 必须是以 # 开头的颜色值".to_string());
    }
    if !(1..=32).contains(&cfg.functions.api_concurrency_number) {
        return Err("api_concurrency_number 必须在 1 到 32 之间".to_string());
    }
    Ok(())
}

/// 更新配置（接收完整 AppConfig JSON，写入内存并持久化）
#[tauri::command]
pub async fn update_config(
    new_config: crate::config::AppConfig,
    app_state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    validate_config(&new_config)?;

    let (old_enable, old_mode, old_realtime, old_api_url, old_user_id, old_api_concurrency) = {
        let lock = app_state.config.read().await;
        (
            lock.functions.enable_auto_create_lobby,
            lock.functions.default_game_mode,
            lock.functions.lcu_realtime_enabled,
            lock.general.upload_api_url.clone(),
            if !lock.general.signalr_user_id.is_empty() {
                lock.general.signalr_user_id.clone()
            } else if !lock.functions.lcu_user_id.is_empty() {
                lock.functions.lcu_user_id.clone()
            } else {
                "lcu_user_001".to_string()
            },
            lock.functions.api_concurrency_number,
        )
    };

    let enable_changed = new_config.functions.enable_auto_create_lobby != old_enable;
    let mode_changed = new_config.functions.default_game_mode != old_mode;

    let new_user_id = if !new_config.general.signalr_user_id.is_empty() {
        new_config.general.signalr_user_id.clone()
    } else if !new_config.functions.lcu_user_id.is_empty() {
        new_config.functions.lcu_user_id.clone()
    } else {
        "lcu_user_001".to_string()
    };

    let signalr_changed = new_config.functions.lcu_realtime_enabled != old_realtime
        || new_config.general.upload_api_url != old_api_url
        || new_user_id != old_user_id;

    let hide_tft_changed = {
        let lock = app_state.config.read().await;
        lock.functions.hide_tft != new_config.functions.hide_tft
    };

    {
        let mut cfg = app_state.config.write().await;
        *cfg = new_config.clone();
        cfg.save();
    }

    if hide_tft_changed {
        if let Some(tray) = app_handle.tray_by_id("main_tray") {
            if let Ok(new_menu) = build_tray_menu(&app_handle, new_config.functions.hide_tft) {
                let _ = tray.set_menu(Some(new_menu));
            }
        }
    }

    if enable_changed || mode_changed {
        let _ = app_state
            .gameflow_tx
            .try_send(crate::agents::auto_match::GameflowEvent::ResetLobbyState);
    }

    if signalr_changed {
        if new_config.functions.lcu_realtime_enabled
            && !new_config.general.upload_api_url.is_empty()
        {
            log::info!("配置更新，重新启动 SignalR Hub 远程反代");
            let server_url = new_config.general.upload_api_url.clone();
            crate::signalr::start(app_handle, server_url, new_user_id);
        } else {
            log::info!("配置更新，停止 SignalR Hub 远程反代");
            crate::signalr::stop().await;
        }
    }

    if new_config.functions.api_concurrency_number != old_api_concurrency {
        let mut sem_lock = app_state.api_semaphore.write().await;
        *sem_lock = Arc::new(Semaphore::new(
            new_config.functions.api_concurrency_number as usize,
        ));
        log::info!(
            "运行时 API 并发限制数更新为: {}",
            new_config.functions.api_concurrency_number
        );
    }

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
