//! 设置驱动的 GPUI 云端服务、待传队列和多杀截图生命周期。
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use tokio::{
    sync::{watch, Mutex},
    task::JoinHandle,
};
use yuumi_core::{
    config::AppConfig,
    lcu::{
        client::{stable_hash, LcuClient},
        game_data::GameDataAssets,
    },
    runtime,
    signalr::{self, HubConfig, HubStatus},
    upload::UploadQueue,
};

use super::{
    live_lcu::{ConnectionStatus, LiveLcuService},
    screenshot::{self, ScreenshotMonitor},
};

#[derive(Clone, Default, PartialEq, Eq)]
struct Options {
    upload_url: Option<String>,
    screenshot: bool,
    levels: Vec<u32>,
    screenshot_path: String,
    in_game: bool,
}

pub struct CloudTasks {
    options: watch::Sender<Options>,
    hub_config: Option<HubConfig>,
    hub_task: Option<JoinHandle<()>>,
    hub_status: watch::Sender<HubStatus>,
    capture_task: JoinHandle<()>,
    queues: Arc<Mutex<HashMap<String, UploadQueue>>>,
    live: LiveLcuService,
}

impl CloudTasks {
    pub fn new(live: LiveLcuService) -> Self {
        let (options, capture_options) = watch::channel(Options::default());
        let (hub_status, _) = watch::channel(HubStatus::Disabled);
        let capture_task = tokio::spawn(capture_loop(capture_options, live.subscribe_status()));
        Self {
            options,
            hub_config: None,
            hub_task: None,
            hub_status,
            capture_task,
            queues: Arc::default(),
            live,
        }
    }

    pub fn subscribe_status(&self) -> watch::Receiver<HubStatus> {
        self.hub_status.subscribe()
    }

    pub fn apply(&mut self, config: &AppConfig) {
        self.options.send_if_modified(|options| {
            let next = Options {
                upload_url: configured_upload(config),
                screenshot: config.functions.enable_screenshot_on_multikill,
                levels: config.functions.screenshot_on_multikill_levels.clone(),
                screenshot_path: config.functions.screenshot_save_path.clone(),
                in_game: options.in_game,
            };
            if *options == next {
                return false;
            }
            *options = next;
            true
        });

        let next = HubConfig::from_app(config);
        if let Ok(Some(ref next)) = next {
            if self.hub_config.as_ref() == Some(next)
                && self
                    .hub_task
                    .as_ref()
                    .is_some_and(|task| !task.is_finished())
            {
                return;
            }
        }
        if let Some(task) = self.hub_task.take() {
            task.abort();
        }
        self.hub_config = None;
        match next {
            Ok(Some(config)) => {
                self.hub_status.send_replace(HubStatus::Connecting);
                if let Some(runtime) = runtime::tokio_handle() {
                    self.hub_task = Some(signalr::spawn(
                        &runtime,
                        config.clone(),
                        self.live.client.clone(),
                        self.live.subscribe_cloud_events(),
                        self.hub_status.clone(),
                    ));
                    self.hub_config = Some(config);
                }
            }
            Ok(None) => {
                self.hub_status
                    .send_if_modified(|status| change_status(status, HubStatus::Disabled));
            }
            Err(_) => {
                self.hub_status
                    .send_if_modified(|status| change_status(status, HubStatus::InvalidConfig));
            }
        }
    }

    pub fn set_in_game(&self, in_game: bool) {
        self.options.send_if_modified(|options| {
            if options.in_game == in_game {
                return false;
            }
            options.in_game = in_game;
            true
        });
    }

    /// ids 为空时仅重试同一账号、同一目标地址的待传记录。
    pub fn upload_matches(&self, owner: String, ids: Vec<u64>, batch: bool) {
        self.schedule_upload(owner, ids, batch, false);
    }
    pub fn upload_finished(&self, owner: String, id: Option<u64>) {
        self.schedule_upload(owner, id.into_iter().collect(), false, true);
    }
    fn schedule_upload(&self, owner: String, ids: Vec<u64>, batch: bool, finished: bool) {
        let options = self.options.subscribe();
        let Some(url) = options.borrow().upload_url.clone() else {
            log::debug!("[Upload] 未配置有效的上传地址，跳过上传任务");
            return;
        };
        if owner.is_empty() {
            return;
        }
        let live = self.live.clone();
        let queues = self.queues.clone();
        runtime::spawn_background(async move {
            let status = live.subscribe_status();
            let transfer = async {
                if finished {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
                let client = if ids.is_empty() && !finished {
                    None
                } else {
                    live.get_client().await
                };
                let assets = live.get_game_data_assets().await;
                upload(&url, &owner, ids, batch, finished, client, assets, queues).await
            };
            tokio::select! {
                biased;
                _ = upload_scope_changed(options, status, &url, &owner) => {}
                result = transfer => {
                    if let Err(error) = result {
                        log::warn!("[Upload] 操作未完成: {error}");
                    }
                }
            }
        });
    }
}

impl Drop for CloudTasks {
    fn drop(&mut self) {
        self.capture_task.abort();
        if let Some(task) = self.hub_task.take() {
            task.abort();
        }
    }
}

fn configured_upload(config: &AppConfig) -> Option<String> {
    let address = config.general.upload_api_url.trim();
    if address.is_empty() {
        return None;
    }
    let address = if address.contains("://") {
        address.to_string()
    } else {
        format!("http://{address}")
    };
    let mut url = reqwest::Url::parse(&address).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    let path = url.path().trim_end_matches('/');
    let path = path
        .strip_suffix("/api/lol/upload")
        .unwrap_or(path)
        .to_string();
    url.set_path(&path);
    Some(url.as_str().trim_end_matches('/').to_string())
}

fn change_status(status: &mut HubStatus, next: HubStatus) -> bool {
    let changed = *status != next;
    *status = next;
    changed
}

fn current_owner(status: &ConnectionStatus) -> Option<&str> {
    match status {
        ConnectionStatus::Connected { summoner, .. } => {
            summoner.as_ref().as_ref().map(|s| s.puuid.as_str())
        }
        _ => None,
    }
}

async fn upload_scope_changed(
    mut options: watch::Receiver<Options>,
    mut status: watch::Receiver<ConnectionStatus>,
    url: &str,
    owner: &str,
) {
    loop {
        if options.borrow_and_update().upload_url.as_deref() != Some(url)
            || current_owner(&status.borrow_and_update()) != Some(owner)
        {
            return;
        }
        tokio::select! {
            changed = options.changed() => { if changed.is_err() { return } }
            changed = status.changed() => { if changed.is_err() { return } }
        }
    }
}

async fn upload(
    url: &str,
    owner: &str,
    mut ids: Vec<u64>,
    batch: bool,
    finished: bool,
    client: Option<LcuClient>,
    assets: Option<Arc<GameDataAssets>>,
    queues: Arc<Mutex<HashMap<String, UploadQueue>>>,
) -> Result<(), String> {
    // 按目标地址与账号隔离持久化队列，切换账号或服务器时不会串传。
    let key = stable_hash(&format!("{url}\n{owner}"));
    let queue = {
        let mut queues = queues.lock().await;
        if let Some(queue) = queues.get(&key) {
            queue.clone()
        } else {
            let path = runtime::app_data_dir()
                .join("uploads")
                .join(format!("{key}.sqlite"));
            let queue = tokio::task::spawn_blocking(move || UploadQueue::new(path))
                .await
                .map_err(|_| "上传队列初始化中断")??;
            queues.insert(key, queue.clone());
            queue
        }
    };
    if ids.is_empty() && !finished {
        queue.retry_pending(url).await?;
        return Ok(());
    }
    let client = client.ok_or("LCU 未连接")?;
    if finished {
        if let Err(error) = queue.retry_pending(url).await {
            log::warn!("[Upload] 待传重试失败: {error}");
        }
    }
    let player = client.get("/lol-summoner/v1/current-summoner").await?;
    if player["puuid"].as_str() != Some(owner) {
        return Err("当前账号已变化".into());
    }
    if finished && ids.is_empty() {
        // Only use a recently ended match; never infer an old match as the current game.
        let path =
            format!("/lol-match-history/v1/products/lol/{owner}/matches?begIndex=0&endIndex=0");
        if let Ok(history) = client.get(&path).await {
            let game = &history["games"]["games"][0];
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let end = game["gameCreation"]
                .as_u64()
                .map(|created| created / 1000 + game["gameDuration"].as_u64().unwrap_or(0));
            if end.is_some_and(|end| end <= now && now - end <= 20 * 60) {
                if let Some(id) = game["gameId"].as_u64().filter(|id| *id > 0) {
                    ids.push(id);
                }
            }
        }
        if ids.is_empty() {
            log::info!("[Upload] 未能定位刚结束的对局，仅重试待传队列");
            return queue.retry_pending(url).await.map(|_| ());
        }
    }
    let names = assets
        .as_ref()
        .map(|assets| &assets.champions)
        .cloned()
        .unwrap_or_default();
    let mut seen = HashSet::new();
    let mut payloads = Vec::new();
    for id in ids.into_iter().filter(|id| *id > 0 && seen.insert(*id)) {
        match UploadQueue::fetch_payload(&client, id, &names).await {
            Ok(payload) => payloads.push(payload),
            Err(_) if batch => log::warn!("[Upload] 跳过无法读取的对局"),
            Err(_) => return Err("无法读取结算对局".into()),
        }
    }
    if batch {
        let (success, failed) = queue.upload_batch(url, &payloads).await?;
        log::info!("[Upload] 批量上传完成: 成功={} 失败={}", success, failed);
    } else {
        for payload in payloads {
            let outcome = queue.enqueue(url, payload).await?;
            log::info!("[Upload] 对局上传完成: {outcome}");
        }
        queue.retry_pending(url).await?;
    }
    Ok(())
}

async fn capture_loop(
    mut options: watch::Receiver<Options>,
    status: watch::Receiver<ConnectionStatus>,
) {
    // 仅用于固定的本机游戏数据端口，拒绝代理和重定向。
    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_millis(800))
        .no_proxy()
        .danger_accept_invalid_certs(true)
        .redirect(reqwest::redirect::Policy::none())
        .build()
    else {
        return;
    };
    let mut monitor = ScreenshotMonitor::default();
    let mut player = String::new();
    loop {
        let config = options.borrow_and_update().clone();
        if !config.screenshot || !config.in_game {
            monitor.reset();
            player.clear();
            if options.changed().await.is_err() {
                return;
            }
            continue;
        }
        if player.is_empty() {
            player = live_player_name(&client)
                .await
                .unwrap_or_else(|| match &*status.borrow() {
                    ConnectionStatus::Connected { summoner, .. } => summoner
                        .as_ref()
                        .as_ref()
                        .map(|player| player.full_name())
                        .unwrap_or_default(),
                    _ => String::new(),
                });
        }
        if !player.is_empty() {
            if let Ok(response) = client
                .get("https://127.0.0.1:2999/liveclientdata/eventdata")
                .send()
                .await
            {
                if let Ok(response) = response.error_for_status() {
                    if let Ok(json) = response.text().await {
                        let latest = options.borrow().clone();
                        if latest.in_game && latest.screenshot {
                            if let Ok(reasons) =
                                monitor.process(&json, &player, true, &latest.levels)
                            {
                                for reason in reasons {
                                    let name = player.clone();
                                    let path = latest.screenshot_path.clone();
                                    match tokio::task::spawn_blocking(move || {
                                        screenshot::capture(&name, &reason, &path)
                                    })
                                    .await
                                    {
                                        Ok(Ok(_)) => log::info!("[Screenshot] 已保存对局截图"),
                                        _ => log::warn!(
                                            "[Screenshot] 保存失败，请检查游戏窗口和截图目录"
                                        ),
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(500)) => {}
            changed = options.changed() => { if changed.is_err() { return } }
        }
    }
}

async fn live_player_name(client: &reqwest::Client) -> Option<String> {
    client
        .get("https://127.0.0.1:2999/liveclientdata/activeplayername")
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json::<String>()
        .await
        .ok()
        .filter(|name| !name.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upload_scope_uses_canonical_target() {
        let mut config = AppConfig::default();
        config.general.upload_api_url = "HTTP://Example.com:80/api/lol/upload/".into();
        assert_eq!(
            configured_upload(&config).as_deref(),
            Some("http://example.com")
        );
        // 上传开关（functions.upload_enabled）只门禁查询页触发路径，
        // 游戏结束自动上传仅依赖 URL 配置（对齐 Vue 原版 UploadTrigger）。
        config.functions.upload_enabled = false;
        assert_eq!(
            configured_upload(&config).as_deref(),
            Some("http://example.com")
        );
        config.general.upload_api_url = "http://user:password@example.com".into();
        assert!(configured_upload(&config).is_none());
    }
}
