use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::{sleep, Duration};

// 静态的 AtomicBool 用来指示是否在游戏中，Rust 1.70+ 支持直接在 static 中使用 const fn
static IN_GAME: AtomicBool = AtomicBool::new(false);

/// 开启/关闭游戏内状态轮询
pub fn set_in_game(in_game: bool) {
    IN_GAME.store(in_game, Ordering::SeqCst);
}

/// 比较玩家名字，忽略 Riot ID 的 #tag 及大小写差异，防止接口返回不一致导致匹配失败
fn compare_names(name_a: &str, name_b: &str) -> bool {
    let clean_a = name_a
        .split('#')
        .next()
        .unwrap_or(name_a)
        .trim()
        .to_lowercase();
    let clean_b = name_b
        .split('#')
        .next()
        .unwrap_or(name_b)
        .trim()
        .to_lowercase();
    clean_a == clean_b
}

/// Fallback 方案：尝试从 LCU 接口获取当前登录召唤师的 displayName
async fn get_summoner_name_from_lcu(app_handle: &AppHandle) -> Option<String> {
    let state = app_handle.state::<crate::AppState>();
    let lcu_lock = state.lcu().await.ok()?;
    let lcu = lcu_lock.as_ref()?;
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
        .ok()?;

    if resp.status().is_success() {
        if let Ok(val) = resp.json::<serde_json::Value>().await {
            // 国服 Riot ID 体系下 displayName 可能为空，优先取 gameName
            let game_name = val
                .get("gameName")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
            if game_name.is_some() {
                return game_name;
            }
            return val
                .get("displayName")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
        }
    }
    None
}

/// 开启多杀截图的后台守护任务
pub fn start(app_handle: AppHandle) {
    crate::spawn_log_panic(async move {
        let mut last_processed_event_id: i32 = -1;
        let mut active_player_name: Option<String> = None;

        // 创建专用 HTTP 客户端，设置 short 超时以防连接卡住
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .no_proxy()
            .timeout(Duration::from_millis(800))
            .build()
            .unwrap_or_default();

        loop {
            if !IN_GAME.load(Ordering::SeqCst) {
                // 非游戏中，重置状态，转入 2s 一次的挂起休眠
                last_processed_event_id = -1;
                active_player_name = None;
                sleep(Duration::from_secs(2)).await;
                continue;
            }

            // 1. 获取当前控制的玩家名字（API 会在游戏正式载入完成后可用）
            if active_player_name.is_none() {
                let mut lcd_name = None;
                match client
                    .get("https://127.0.0.1:2999/liveclientdata/activeplayername")
                    .send()
                    .await
                {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            match resp.text().await {
                                Ok(name) => {
                                    let name_clean = name.trim_matches('"').to_string();
                                    if !name_clean.is_empty() {
                                        lcd_name = Some(name_clean);
                                    }
                                }
                                Err(e) => {
                                    log::warn!("读取 activeplayername 响应失败: {}", e);
                                }
                            }
                        } else {
                            log::warn!(
                                "获取 activeplayername 接口返回错误状态码: {}",
                                resp.status()
                            );
                        }
                    }
                    Err(e) => {
                        log::debug!(
                            "尝试连接游戏内 Live Client Data 端口失败 (游戏可能未完全载入): {}",
                            e
                        );
                    }
                }

                if let Some(name) = lcd_name {
                    active_player_name = Some(name.clone());
                    log::info!("已获取到当前游戏内角色名 (LCD): {:?}", active_player_name);
                } else {
                    // Fallback: 尝试从 LCU 获取当前召唤师名字
                    if let Some(lcu_name) = get_summoner_name_from_lcu(&app_handle).await {
                        active_player_name = Some(lcu_name.clone());
                        log::info!(
                            "已获取到当前游戏内角色名 (LCU Fallback): {:?}",
                            active_player_name
                        );
                    }
                }
            }

            // 2. 获取游戏内实时事件列表
            if let Some(player_name) = &active_player_name {
                match client
                    .get("https://127.0.0.1:2999/liveclientdata/eventdata")
                    .send()
                    .await
                {
                    Ok(resp) => {
                        #[derive(serde::Deserialize, Debug)]
                        #[allow(non_snake_case)]
                        struct LiveEvent {
                            EventID: i32,
                            EventName: String,
                            KillerName: Option<String>,
                            LegendaryTo: Option<String>,
                            Recipient: Option<String>,
                            KillStreak: Option<u32>,
                        }

                        #[derive(serde::Deserialize, Debug)]
                        #[allow(non_snake_case)]
                        struct LiveEventsResponse {
                            Events: Vec<LiveEvent>,
                        }

                        match resp.json::<LiveEventsResponse>().await {
                            Ok(events_resp) => {
                                // 首次运行，初始化 last_processed_event_id 为当前最新事件，防止拉起旧数据
                                if last_processed_event_id == -1 {
                                    last_processed_event_id = events_resp
                                        .Events
                                        .iter()
                                        .map(|e| e.EventID)
                                        .max()
                                        .unwrap_or(-1);
                                    log::info!(
                                        "游戏内事件监控初始化，当前最大 EventID: {}",
                                        last_processed_event_id
                                    );
                                }

                                for event in events_resp.Events {
                                    if event.EventID > last_processed_event_id {
                                        last_processed_event_id = event.EventID;

                                        log::info!(
                                            "收到局内新事件: ID={}, Name={}, Killer={:?}, LegendaryTo={:?}, Recipient={:?}, Streak={:?}",
                                            event.EventID,
                                            event.EventName,
                                            event.KillerName,
                                            event.LegendaryTo,
                                            event.Recipient,
                                            event.KillStreak
                                        );

                                        // 获取用户配置
                                        let cfg = {
                                            let state = app_handle.state::<crate::AppState>();
                                            let lock = state.config.read().await;
                                            lock.functions.clone()
                                        };

                                        if !cfg.enable_screenshot_on_multikill {
                                            continue;
                                        }

                                        let mut should_capture = false;
                                        let mut capture_reason = String::new();

                                        // 判定三/四/五杀
                                        if event.EventName == "Multikill" {
                                            if let (Some(killer), Some(streak)) =
                                                (&event.KillerName, event.KillStreak)
                                            {
                                                if compare_names(killer, player_name)
                                                    && cfg
                                                        .screenshot_on_multikill_levels
                                                        .contains(&streak)
                                                {
                                                    should_capture = true;
                                                    capture_reason = match streak {
                                                        3 => "TripleKill".to_string(),
                                                        4 => "QuadraKill".to_string(),
                                                        5 => "PentaKill".to_string(),
                                                        _ => format!("Multikill_{}", streak),
                                                    };
                                                }
                                            }
                                        }

                                        // 判定超神 (连续击杀达到 8 人)
                                        if event.EventName == "Legendary" {
                                            // 容错匹配：尝试从 LegendaryTo, KillerName, Recipient 字段中提取触发者名字
                                            let triggered_player = event
                                                .LegendaryTo
                                                .as_ref()
                                                .or(event.KillerName.as_ref())
                                                .or(event.Recipient.as_ref());

                                            if let Some(killer) = triggered_player {
                                                if compare_names(killer, player_name)
                                                    && cfg
                                                        .screenshot_on_multikill_levels
                                                        .contains(&8)
                                                {
                                                    should_capture = true;
                                                    capture_reason = "Legendary".to_string();
                                                }
                                            }
                                        }

                                        if should_capture {
                                            log::info!(
                                                "检测到符合截图条件的事件: {}，触发异步截图...",
                                                capture_reason
                                            );
                                            let app_clone = app_handle.clone();
                                            let player_name_clone = player_name.clone();
                                            let custom_save_path = cfg.screenshot_save_path.clone();
                                            // 异步执行截图，静默进行，绝不阻塞轮询事件循环
                                            tokio::spawn(async move {
                                                if let Err(e) = execute_screenshot(
                                                    app_clone,
                                                    &player_name_clone,
                                                    &capture_reason,
                                                    &custom_save_path,
                                                )
                                                .await
                                                {
                                                    log::error!("自动截图处理失败: {}", e);
                                                }
                                            });
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                log::warn!("解析 Live Client Data eventdata JSON 失败: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("获取 Live Client Data eventdata 失败: {}", e);
                    }
                }
            }

            // 游戏处于对局中时，每隔 1.0 秒检测一次事件
            sleep(Duration::from_secs(1)).await;
        }
    });
}

/// 执行窗口截图并保存至本地文件系统（所有耗时阻塞操作均在专用阻塞线程中进行）
async fn execute_screenshot(
    app: AppHandle,
    player_name: &str,
    reason: &str,
    custom_path: &str,
) -> Result<(), String> {
    // 1. 解析目标文件夹目录
    let mut save_dir = if !custom_path.is_empty() {
        std::path::PathBuf::from(custom_path)
    } else {
        let mut path = dirs::picture_dir().ok_or_else(|| "无法定位系统图片文件夹".to_string())?;
        path.push("Yuumi_Screenshots");
        path
    };

    // 确保截图保存目录存在
    if !save_dir.exists() {
        std::fs::create_dir_all(&save_dir).map_err(|e| format!("创建截图目录失败: {}", e))?;
    }

    // 2. 生成文件名: [类型]_[玩家名称]_[时间戳].png
    let now = chrono::Local::now();
    let filename = format!(
        "{}_{}_{}.png",
        reason,
        player_name,
        now.format("%Y%m%d_%H%M%S")
    );
    save_dir.push(filename);
    let save_path_str = save_dir.to_string_lossy().to_string();

    // 3. 截取屏幕数据：优先定位 LOL 游戏窗口所在的显示器并截取该显示器，如无窗口则截取主屏幕
    let img_buffer =
        tokio::task::spawn_blocking(move || -> Result<xcap::image::RgbaImage, String> {
            use xcap::{Monitor, Window};
            let windows = Window::all().map_err(|e| e.to_string())?;
            // 匹配 LOL 游戏窗口
            let lol_window = windows
                .into_iter()
                .find(|w| w.title() == "League of Legends (TM) Client");
            let monitors = Monitor::all().map_err(|e| e.to_string())?;

            if let Some(w) = lol_window {
                // 获取 LOL 窗口的中心点坐标
                let win_cx = w.x() + (w.width() as i32) / 2;
                let win_cy = w.y() + (w.height() as i32) / 2;

                // 寻找包含窗口中心点的显示器
                let target_monitor = monitors.iter().find(|m| {
                    let m_x = m.x();
                    let m_y = m.y();
                    let m_w = m.width() as i32;
                    let m_h = m.height() as i32;
                    win_cx >= m_x && win_cx < m_x + m_w && win_cy >= m_y && win_cy < m_y + m_h
                });

                if let Some(m) = target_monitor {
                    log::info!("定位到 LOL 游戏所在的显示器，执行显示器级别截屏");
                    let img = m.capture_image().map_err(|e| e.to_string())?;
                    return Ok(img);
                }
            }

            // 回退逻辑：截取主显示器
            log::warn!("未定位到游戏所在显示器，将回退截取主显示器");
            let main_monitor = monitors
                .first()
                .ok_or_else(|| "未检测到任何可用显示器".to_string())?;
            let img = main_monitor.capture_image().map_err(|e| e.to_string())?;
            Ok(img)
        })
        .await
        .map_err(|e| format!("截图子线程异常: {}", e))??;

    // 4. 将图像二进制数据写入本地磁盘保存为 PNG
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        img_buffer
            .save(&save_dir)
            .map_err(|e| format!("写入磁盘失败: {}", e))
    })
    .await
    .map_err(|e| format!("保存子线程异常: {}", e))??;

    log::info!("截图保存成功，文件路径: {}", save_path_str);

    // 5. 广播事件给前端（仅日志记录或界面组件状态修改，不触发弹窗）
    let _ = app.emit("screenshot-saved", save_path_str);

    Ok(())
}
