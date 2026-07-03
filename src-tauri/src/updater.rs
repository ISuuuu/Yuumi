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

/// 构建 UpdaterBuilder，若用户启用了 GitHub 代理则自动注入
macro_rules! build_updater {
    ($app:expr) => {{
        let (enable_proxy, proxy_addr) = get_proxy_config($app).await;
        let mut builder = $app.updater_builder();
        if enable_proxy && !proxy_addr.is_empty() {
            // proxy_addr 格式为 "host:port"，补全为 http://host:port
            let proxy_url = if proxy_addr.starts_with("http://") || proxy_addr.starts_with("https://") {
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
        builder.build().map_err(|e| format!("无法初始化更新器: {e}"))
    }};
}

/// 检查是否有新版本
/// 返回 Some(UpdateInfo) 表示有更新，None 表示已是最新
#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    let current = app.package_info().version.to_string();
    let updater = build_updater!(&app)?;

    match updater.check().await {
        Ok(Some(update)) => Ok(Some(UpdateInfo {
            version: update.version.clone(),
            current_version: current,
            notes: update.body.clone(),
            pub_date: update.date.map(|d| d.to_string()),
        })),
        Ok(None) => Ok(None),
        Err(e) => Err(format!("检查更新失败: {e}")),
    }
}

/// 下载并安装更新，安装完成后自动重启
/// 通过 `updater://progress` 事件向前端推送下载进度
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
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
                let current = downloaded_bytes_clone.fetch_add(chunk_length as u64, std::sync::atomic::Ordering::Relaxed) + chunk_length as u64;
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
