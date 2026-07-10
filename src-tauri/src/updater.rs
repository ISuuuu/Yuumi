use tauri::AppHandle;
use tauri::Emitter;
use tauri::Manager;
use tauri_plugin_updater::UpdaterExt;

use crate::AppState;

/// 更新信息结构体，返回给前端
#[derive(Debug, serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    pub notes: Option<String>,
    pub pub_date: Option<String>,
}

/// 下载进度事件
#[derive(Debug, serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    /// 已下载字节数
    pub downloaded: u64,
    /// 总字节数（服务器未返回 Content-Length 时为 None）
    pub total: Option<u64>,
    /// 百分比 0-100，total 未知时为 None
    pub percent: Option<f64>,
}

/// 从 AppState 读取代理配置，返回 (enable_proxy, proxy_addr)
async fn get_proxy_config(app: &AppHandle) -> (bool, String) {
    let state = app.state::<AppState>();
    let cfg = state.config.read().await;
    (
        cfg.general.enable_github_proxy,
        cfg.general.github_proxy_addr.clone(),
    )
}

/// 已后台下载完成的待安装更新（仅存储字节，Update 实例在 install 时重新获取）
pub struct PendingUpdate {
    pub bytes: Vec<u8>,
    pub info: UpdateInfo,
}

/// 构建 UpdaterBuilder，若用户启用了 GitHub 代理则自动注入
macro_rules! build_updater {
    ($app:expr) => {{
        let (enable_proxy, proxy_addr) = get_proxy_config($app).await;
        let mut builder = $app.updater_builder();
        if enable_proxy && !proxy_addr.is_empty() {
            // proxy_addr 格式为 "host:port"，补全为 http://host:port
            let proxy_url =
                if proxy_addr.starts_with("http://") || proxy_addr.starts_with("https://") {
                    proxy_addr.clone()
                } else {
                    format!("http://{}", proxy_addr)
                };
            match proxy_url.parse::<url::Url>() {
                Ok(url) => {
                    builder = builder.proxy(url);
                    log::info!("更新器已启用 GitHub 代理: {proxy_url}");
                }
                Err(e) => {
                    log::warn!("代理地址解析失败，将直连: {e}");
                }
            }
        }
        builder
            .build()
            .map_err(|e| format!("无法初始化更新器: {e}"))
    }};
}

/// 检查是否有新版本，若有则自动触发后台下载（不阻塞返回）
/// 返回 Some(UpdateInfo) 表示有更新，None 表示已是最新
#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    let state = app.state::<AppState>();

    // 1. 如果有已下载好的待安装更新，直接返回其信息，不用发起网络请求
    {
        let pending = state.pending_update.lock().unwrap();
        if let Some(p) = &*pending {
            log::info!("已有下载完成的待安装更新 v{}，直接返回", p.info.version);
            return Ok(Some(p.info.clone()));
        }
    }

    // 2. 如果后台更新正在下载，则直接返回当前正在下载的更新信息，不用发起网络请求
    if state
        .is_downloading
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        let downloading = state.downloading_update.lock().unwrap();
        if let Some(info) = &*downloading {
            log::info!(
                "后台下载已在进行中 (v{})，直接返回当前下载的更新信息",
                info.version
            );
            return Ok(Some(info.clone()));
        }
    }

    let current = app.package_info().version.to_string();
    let updater = build_updater!(&app)?;

    match updater.check().await {
        Ok(Some(update)) => {
            let info = UpdateInfo {
                version: update.version.clone(),
                current_version: current,
                notes: update.body.clone(),
                pub_date: update.date.map(|d| d.to_string()),
            };
            // 检查到新版本后通知前端并自动触发后台下载
            log::info!("手动检查发现新版本 v{}，自动开始后台下载", info.version);
            let _ = app.emit("updater://update-available", &info);
            let app_clone = app.clone();
            let info_for_dl = info.clone();
            crate::spawn_log_panic(async move {
                background_download_update(app_clone, update, info_for_dl).await;
            });
            Ok(Some(info))
        }
        Ok(None) => Ok(None),
        Err(e) => Err(format!("检查更新失败: {e}")),
    }
}

/// 下载并安装更新，安装完成后自动重启
/// 通过 `updater://progress` 事件向前端推送下载进度
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    if state
        .is_downloading
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        return Err("更新正在后台下载中，请稍候...".to_string());
    }
    if state.pending_update.lock().unwrap().is_some() {
        return Err("更新已下载完成，请重启应用进行安装".to_string());
    }

    let updater = build_updater!(&app)?;

    let update = updater
        .check()
        .await
        .map_err(|e| format!("检查更新失败: {e}"))?
        .ok_or_else(|| "没有可用的更新".to_string())?;

    // 克隆 app handle 用于在 closure 中发送 event
    let app_for_progress = app.clone();

    let downloaded_bytes = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let downloaded_bytes_clone = downloaded_bytes.clone();

    update
        .download_and_install(
            move |chunk_length, total| {
                let current = downloaded_bytes_clone
                    .fetch_add(chunk_length as u64, std::sync::atomic::Ordering::Relaxed)
                    + chunk_length as u64;
                let percent = total.map(|t| {
                    if t > 0 {
                        current as f64 / t as f64 * 100.0
                    } else {
                        0.0
                    }
                });
                let progress = DownloadProgress {
                    downloaded: current,
                    total,
                    percent,
                };
                // 忽略发送失败（窗口可能已关闭）
                let _ = app_for_progress.emit("updater://progress", progress);
            },
            || {
                log::info!("更新下载完成，准备安装");
            },
        )
        .await
        .map_err(|e| format!("下载/安装更新失败: {e}"))?;

    // 安装完成后重启
    app.restart();
}

/// 静默后台下载新版本（检查 + 下载），不阻塞前端
/// 启动时自动检测调用此入口
pub async fn start_background_download(app: AppHandle) {
    let updater = match build_updater!(&app) {
        Ok(u) => u,
        Err(e) => {
            log::warn!("后台更新初始化失败: {e}");
            return;
        }
    };

    let update = match updater.check().await {
        Ok(Some(u)) => u,
        Ok(None) => {
            log::info!("后台更新检查：已是最新版本");
            return;
        }
        Err(e) => {
            log::warn!("后台更新检查失败: {e}");
            return;
        }
    };

    let info = UpdateInfo {
        version: update.version.clone(),
        current_version: app.package_info().version.to_string(),
        notes: update.body.clone(),
        pub_date: update.date.map(|d| d.to_string()),
    };

    // 通知前端有新版本
    let _ = app.emit("updater://update-available", &info);

    background_download_update(app, update, info).await;
}

/// 下载更新并保存到 AppState（共享给 start_background_download 和 check_update）
/// 使用 is_downloading 标志防止并发下载。
async fn background_download_update(
    app: AppHandle,
    update: tauri_plugin_updater::Update,
    info: UpdateInfo,
) {
    // 防重入：已有下载进行中则跳过
    {
        let state = app.state::<AppState>();
        if state
            .is_downloading
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            log::info!("后台下载已在进行中，跳过重复下载");
            return;
        }
        state
            .is_downloading
            .store(true, std::sync::atomic::Ordering::Relaxed);

        let mut downloading = state.downloading_update.lock().unwrap();
        *downloading = Some(info.clone());
    }
    let app_for_progress = app.clone();
    let downloaded_bytes = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let downloaded_bytes_clone = downloaded_bytes.clone();

    log::info!("开始后台静默下载更新 v{} ...", info.version);

    match update
        .download(
            |chunk_length, total| {
                let current = downloaded_bytes_clone
                    .fetch_add(chunk_length as u64, std::sync::atomic::Ordering::Relaxed)
                    + chunk_length as u64;
                let percent = total.map(|t| {
                    if t > 0 {
                        current as f64 / t as f64 * 100.0
                    } else {
                        0.0
                    }
                });
                let progress = DownloadProgress {
                    downloaded: current,
                    total,
                    percent,
                };
                let _ = app_for_progress.emit("updater://progress", progress);
            },
            || {
                log::info!("后台更新下载完成");
            },
        )
        .await
    {
        Ok(bytes) => {
            log::info!("后台更新下载成功 ({} bytes)", bytes.len());
            let state = app.state::<AppState>();
            state
                .is_downloading
                .store(false, std::sync::atomic::Ordering::Relaxed);

            let mut downloading = state.downloading_update.lock().unwrap();
            *downloading = None;
            drop(downloading);

            let mut pending = state.pending_update.lock().unwrap();
            *pending = Some(PendingUpdate {
                bytes,
                info: info.clone(),
            });
            drop(pending);
            let _ = app.emit("updater://download-ready", &info);
        }
        Err(e) => {
            log::warn!("后台更新下载失败: {e}");
            let state = app.state::<AppState>();
            state
                .is_downloading
                .store(false, std::sync::atomic::Ordering::Relaxed);

            let mut downloading = state.downloading_update.lock().unwrap();
            *downloading = None;
            drop(downloading);

            let _ = app.emit("updater://download-error", format!("{}", e));
        }
    }
}

/// 安装已下载的待更新版本（从 AppState 读取已保存的字节）
#[tauri::command]
pub async fn install_pending_update(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let pending = state
        .pending_update
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| "没有待安装的更新".to_string())?;

    let updater = build_updater!(&app)?;
    let update = updater
        .check()
        .await
        .map_err(|e| format!("检查更新失败: {e}"))?
        .ok_or_else(|| "没有可用的更新".to_string())?;

    log::info!("正在安装已下载的更新 v{} ...", pending.info.version);
    update
        .install(&pending.bytes)
        .map_err(|e| format!("安装更新失败: {e}"))?;

    log::info!("更新安装成功，重启应用");
    app.restart();
}
