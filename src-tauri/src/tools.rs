use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::State;
use tauri_plugin_opener::OpenerExt;

use crate::config::WEGAME_MARKER;
use crate::{build_auth_header, lcu::client::lcu_request, AppState};

// ─── 创建 5v5 训练营 ───

#[derive(Deserialize)]
pub struct CreateLobbyParams {
    pub lobby_name: String,
    pub password: Option<String>,
}

/// 创建 5v5 自定义训练营房间
#[tauri::command]
pub async fn create_5v5_practice_lobby(
    params: CreateLobbyParams,
    app_state: State<'_, AppState>,
) -> Result<String, String> {
    // 锁内只提取连接参数（http_client 克隆是 Arc 浅拷贝），立即释放读锁，
    // 避免跨 HTTP await 持有锁阻塞 monitor 重连写锁
    let (port, token, http_client) = {
        let lock = app_state.lcu().await?;
        let lcu = lock.as_ref().unwrap();
        (lcu.port, lcu.token.clone(), lcu.http_client.clone())
    };

    let url = format!("https://127.0.0.1:{}/lol-lobby/v1/lobby", port);
    let auth = build_auth_header(&token);

    let body = serde_json::json!({
        "customGameLobby": {
            "configuration": {
                "gameMode": "CLASSIC",
                "gameMutator": "",
                "gameServerRegion": "",
                "mapId": 11,
                "mutators": { "id": 1 },
                "spectatorPolicy": "AllAllowed",
                "teamSize": 5
            },
            "lobbyName": params.lobby_name,
            "lobbyPassword": params.password.unwrap_or_default()
        },
        "isCustom": true
    });

    let resp = http_client
        .post(&url)
        .header("Authorization", auth)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if resp.status().is_success() {
        Ok("训练营房间已创建".to_string())
    } else {
        Err(format!("创建房间失败: HTTP {}", resp.status()))
    }
}

// ─── 大乱斗摇号换回 ───

/// 大乱斗 (ARAM) 摇号后换回原英雄。
/// 逻辑：先 reroll，再从 bench 换回之前暂存的英雄。
#[tauri::command]
pub async fn aram_reroll_and_swap_back(app_state: State<'_, AppState>) -> Result<String, String> {
    // 锁内只提取连接参数，立即释放读锁，避免跨多个 HTTP await 持有锁阻塞 monitor 重连写锁
    let (port, token, http_client) = {
        let lock = app_state.lcu().await?;
        let lcu = lock.as_ref().unwrap();
        (lcu.port, lcu.token.clone(), lcu.http_client.clone())
    };

    let auth = build_auth_header(&token);
    let base = format!("https://127.0.0.1:{}", port);

    // 第一步：获取当前选择的英雄 ID
    let sel_url = format!("{}/lol-champ-select/v1/session/my-selection", base);
    let sel_resp = http_client
        .get(&sel_url)
        .header("Authorization", &auth)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let selection: serde_json::Value = sel_resp.json().await.map_err(|e| e.to_string())?;
    let original_champion = selection
        .get("championId")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    if original_champion == 0 {
        return Err("未选择英雄，无法摇号换回".to_string());
    }

    // 第二步：reroll
    let reroll_url = format!("{}/lol-champ-select/v1/session/my-selection/reroll", base);
    let reroll_resp = http_client
        .post(&reroll_url)
        .header("Authorization", &auth)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !reroll_resp.status().is_success() {
        return Err(format!("摇号失败: HTTP {}", reroll_resp.status()));
    }

    // 第三步：从 bench 换回原英雄
    let swap_url = format!(
        "{}/lol-champ-select/v1/session/bench/swap/{}",
        base, original_champion
    );
    let swap_resp = http_client
        .post(&swap_url)
        .header("Authorization", &auth)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if swap_resp.status().is_success() {
        Ok(format!("摇号换回成功 (原英雄: {})", original_champion))
    } else {
        Err(format!("换回失败: HTTP {}", swap_resp.status()))
    }
}

// ─── 一键应用符文页 ───

#[derive(Deserialize)]
pub struct RunePageParams {
    pub name: String,
    pub primary_style_id: i32,
    pub sub_style_id: i32,
    pub selected_perk_ids: Vec<i32>,
}

/// 一键应用符文页：获取当前 → 删除 → 创建新页
#[tauri::command]
pub async fn apply_rune_page(
    params: RunePageParams,
    app_state: State<'_, AppState>,
) -> Result<String, String> {
    apply_rune_page_core(
        app_state.inner(),
        &params.name,
        params.primary_style_id,
        params.sub_style_id,
        &params.selected_perk_ids,
    )
    .await?;
    Ok("符文页已应用".to_string())
}

/// 应用符文页核心逻辑（供 command 与自动选人共用）
pub(crate) async fn apply_rune_page_core(
    app_state: &AppState,
    name: &str,
    primary_style_id: i32,
    sub_style_id: i32,
    selected_perk_ids: &[i32],
) -> Result<(), String> {
    // 第一步：获取当前符文页，若可删除则删除
    if let Ok(page) = lcu_request(app_state, "GET", "/lol-perks/v1/currentpage", None).await {
        if page
            .get("isDeletable")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            if let Some(page_id) = page.get("id").and_then(|v| v.as_i64()) {
                if page_id > 0 {
                    let _ = lcu_request(
                        app_state,
                        "DELETE",
                        &format!("/lol-perks/v1/pages/{}", page_id),
                        None,
                    )
                    .await;
                }
            }
        }
    }

    // 第二步：创建新符文页
    let body = serde_json::json!({
        "name": name,
        "primaryStyleId": primary_style_id,
        "subStyleId": sub_style_id,
        "selectedPerkIds": selected_perk_ids,
        "current": true,
    });
    lcu_request(app_state, "POST", "/lol-perks/v1/pages", Some(body)).await?;
    Ok(())
}

// ─── 英雄皮肤数据 ───

#[derive(serde::Serialize)]
pub struct SkinEntry {
    pub id: i32,
    pub name: String,
    pub load_screen_path: String,
}

#[derive(serde::Deserialize)]
struct LcuSkin {
    id: i32,
    name: String,
    #[serde(rename = "loadScreenPath")]
    load_screen_path: Option<String>,
}

#[derive(serde::Deserialize)]
struct LcuChampionDetails {
    skins: Vec<LcuSkin>,
}

/// 根据英雄 ID 获取皮肤列表 (直接从 LCU 静态资源加载)
#[tauri::command]
pub async fn get_champion_skins(
    champion_id: i32,
    app_state: State<'_, AppState>,
) -> Result<Vec<SkinEntry>, String> {
    // 锁内只提取连接参数，立即释放读锁，避免跨 HTTP await 持有锁阻塞 monitor 重连写锁
    let (port, token, http_client) = {
        let lock = app_state.lcu().await?;
        let lcu = lock.as_ref().unwrap();
        (lcu.port, lcu.token.clone(), lcu.http_client.clone())
    };
    let auth = build_auth_header(&token);
    let base = format!("https://127.0.0.1:{}", port);

    let url = format!(
        "{}/lol-game-data/assets/v1/champions/{}.json",
        base, champion_id
    );
    let resp = http_client
        .get(&url)
        .header("Authorization", &auth)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!(
            "LCU 返回错误 [{}]: 无法加载该英雄的皮肤数据",
            resp.status().as_u16()
        ));
    }

    let details: LcuChampionDetails = resp.json().await.map_err(|e| e.to_string())?;

    let skins = details
        .skins
        .into_iter()
        .map(|s| SkinEntry {
            id: s.id,
            name: s.name,
            load_screen_path: s.load_screen_path.unwrap_or_else(|| {
                format!(
                    "/lol-game-data/assets/v1/champion-loadscreens/{}/{}.jpg",
                    champion_id, s.id
                )
            }),
        })
        .collect();

    Ok(skins)
}

// ─── OP.GG 数据代理 ───

/// 从 OP.GG API 获取英雄梯队/出装数据（代理请求，避免前端 CORS，缓存与客户端复用见 lcu::opgg）
#[tauri::command]
pub async fn fetch_opgg_data(
    region: String,
    mode: String,
    tier: String,
    champion_id: Option<i32>,
    position: Option<String>,
    app_state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let cache_key = format!(
        "{}_{}_{}_{:?}_{:?}",
        region, mode, tier, champion_id, position
    );

    let url = match champion_id {
        Some(id) => {
            let pos = position.unwrap_or_else(|| "none".into());
            if mode == "arena" {
                format!(
                    "https://lol-api-champion.op.gg/api/{}/champions/{}",
                    region, id
                )
            } else {
                format!(
                    "https://lol-api-champion.op.gg/api/{}/champions/{}/{}/{}",
                    region, mode, id, pos
                )
            }
        }
        None => format!(
            "https://lol-api-champion.op.gg/api/{}/champions/{}",
            region, mode
        ),
    };

    crate::lcu::opgg::get_json(
        app_state.inner(),
        &url,
        &[("tier", tier.as_str())],
        &cache_key,
    )
    .await
}

// ─── 修复 LCU 客户端窗口 ───

/// 清除本地游戏资源缓存（头像、装备、技能、符文、强化图标）
#[tauri::command]
pub async fn clear_game_cache() -> Result<String, String> {
    let cache_dir = crate::runtime::app_data_dir().join("cache");

    if !cache_dir.exists() {
        return Ok("缓存目录不存在，无需清除".to_string());
    }

    let mut count = 0u32;
    for entry in std::fs::read_dir(&cache_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            if std::fs::remove_dir_all(&path).is_ok() {
                count += 1;
            }
        } else if path.is_file() && std::fs::remove_file(&path).is_ok() {
            count += 1;
        }
    }

    Ok(format!("已清除 {} 个缓存文件/目录", count))
}

/// 打开日志文件夹
#[tauri::command]
pub async fn open_log_folder(app: tauri::AppHandle) -> Result<String, String> {
    let log_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("log")))
        .unwrap_or_else(|| std::path::PathBuf::from("log"));

    if !log_dir.exists() {
        let _ = std::fs::create_dir_all(&log_dir);
    }

    app.opener()
        .open_path(log_dir.to_string_lossy().as_ref(), None::<&str>)
        .map_err(|e| e.to_string())?;

    Ok("已打开日志文件夹".to_string())
}

/// 获取当前 LCU 客户端缩放比例（用于窗口修复）
#[tauri::command]
pub async fn get_lcu_zoom(app_state: State<'_, AppState>) -> Result<f64, String> {
    // 锁内只提取连接参数，立即释放读锁，避免跨 HTTP await 持有锁阻塞 monitor 重连写锁
    let (port, token, http_client) = {
        let lock = app_state.lcu().await?;
        let lcu = lock.as_ref().unwrap();
        (lcu.port, lcu.token.clone(), lcu.http_client.clone())
    };

    let url = format!("https://127.0.0.1:{}/riotclient/zoom-scale", port);
    let auth = build_auth_header(&token);

    let resp = http_client
        .get(&url)
        .header("Authorization", auth)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if resp.status().is_success() {
        let zoom: f64 = resp.json().await.map_err(|e| e.to_string())?;
        Ok(zoom)
    } else {
        Err(format!("获取缩放失败: HTTP {}", resp.status()))
    }
}

/// 修复 LCU 客户端窗口（黑屏/缩放/转圈）。
/// 通过系统命令强制重新设置窗口属性。
#[tauri::command]
pub async fn fix_lcu_window(app_state: State<'_, AppState>) -> Result<String, String> {
    // 获取当前缩放比例（锁内只提取连接参数，立即释放读锁）
    let (pid, zoom) = {
        let (pid, port, token, http_client) = {
            let lock = app_state.lcu().await?;
            let lcu = lock.as_ref().unwrap();
            (
                lcu.pid,
                lcu.port,
                lcu.token.clone(),
                lcu.http_client.clone(),
            )
        };
        let url = format!("https://127.0.0.1:{}/riotclient/zoom-scale", port);
        let auth = build_auth_header(&token);
        let resp = http_client
            .get(&url)
            .header("Authorization", auth)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let zoom = if resp.status().is_success() {
            resp.json::<f64>().await.map_err(|e| e.to_string())?
        } else {
            return Err(format!("获取缩放失败: HTTP {}", resp.status()));
        };
        (pid, zoom)
    };

    // 通过 Win32 API 直接操作窗口，替代旧的 PowerShell 脚本方案
    #[cfg(target_os = "windows")]
    {
        fix_lcu_window_win32(zoom, pid)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = zoom;
        Err("仅 Windows 平台支持窗口修复".to_string())
    }
}

#[cfg(target_os = "windows")]
fn fix_lcu_window_win32(zoom: f64, target_pid: u32) -> Result<String, String> {
    use std::ffi::c_void;
    use std::ptr;

    extern "system" {
        fn FindWindowW(lpClassName: *const u16, lpWindowName: *const u16) -> *mut c_void;
        fn ShowWindow(hWnd: *mut c_void, nCmdShow: i32) -> i32;
        fn SetWindowPos(
            hWnd: *mut c_void,
            hWndInsertAfter: *mut c_void,
            X: i32,
            Y: i32,
            cx: i32,
            cy: i32,
            uFlags: u32,
        ) -> i32;
        fn GetWindowThreadProcessId(hWnd: *mut c_void, lpdwProcessId: *mut u32) -> u32;
        fn EnumWindows(
            lpEnumFunc: Option<unsafe extern "system" fn(*mut c_void, *mut c_void) -> i32>,
            lParam: *mut c_void,
        ) -> i32;
    }

    const SW_RESTORE: i32 = 9;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_SHOWWINDOW: u32 = 0x0040;

    unsafe {
        let mut hwnd = {
            let class_name: Vec<u16> = "RiotWindow\0".encode_utf16().collect();
            FindWindowW(class_name.as_ptr(), ptr::null())
        };

        if hwnd.is_null() {
            // 未直接命中 RiotWindow 时，按已知 pid 枚举窗口定位（复用 AppState 中的 pid，免全量进程扫描）
            if target_pid == 0 {
                return Err("未找到 LCU 窗口".to_string());
            }
            struct EnumData {
                target_pid: u32,
                hwnd: *mut c_void,
            }

            unsafe extern "system" fn enum_callback(hwnd: *mut c_void, lparam: *mut c_void) -> i32 {
                let data = &mut *(lparam as *mut EnumData);
                let mut pid: u32 = 0;
                GetWindowThreadProcessId(hwnd, &mut pid);
                if pid == data.target_pid {
                    data.hwnd = hwnd;
                    0
                } else {
                    1
                }
            }

            let mut data = EnumData {
                target_pid,
                hwnd: ptr::null_mut(),
            };
            EnumWindows(Some(enum_callback), (&mut data as *mut EnumData).cast());
            hwnd = data.hwnd;
        }

        if hwnd.is_null() {
            return Err("未找到 LCU 窗口".to_string());
        }

        ShowWindow(hwnd, SW_RESTORE);
        SetWindowPos(
            hwnd,
            ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOSIZE | SWP_NOMOVE | SWP_NOZORDER | SWP_SHOWWINDOW,
        );

        Ok(format!("窗口已修复 (zoom={})", zoom))
    }
}

fn get_persisted_settings_path(lol_paths: &[String]) -> Option<PathBuf> {
    // 跳过 WeGame 标记，取第一个真实客户端路径
    let real_path = lol_paths.iter().find(|p| *p != WEGAME_MARKER)?;
    let p = Path::new(real_path);
    let base_dir = if p.is_file() { p.parent()? } else { p };
    Some(
        base_dir
            .join("Game")
            .join("Config")
            .join("PersistedSettings.json"),
    )
}

/// 查询游戏设置（PersistedSettings.json）是否已被锁定（只读）
#[tauri::command]
pub async fn get_game_settings_readonly(app_state: State<'_, AppState>) -> Result<bool, String> {
    let cfg = app_state.config.read().await;
    let path = get_persisted_settings_path(&cfg.general.lol_path)
        .ok_or_else(|| "未配置英雄联盟客户端路径".to_string())?;

    if !path.exists() {
        return Ok(false);
    }

    let metadata = fs::metadata(&path).map_err(|e| format!("获取文件元数据失败: {}", e))?;

    Ok(metadata.permissions().readonly())
}

/// 锁定/解锁游戏设置（修改 PersistedSettings.json 的只读属性）
#[tauri::command]
pub async fn set_game_settings_readonly(
    readonly: bool,
    app_state: State<'_, AppState>,
) -> Result<String, String> {
    let cfg = app_state.config.read().await;
    let path = get_persisted_settings_path(&cfg.general.lol_path)
        .ok_or_else(|| "未配置英雄联盟客户端路径".to_string())?;

    if !path.exists() {
        return Err(
            "游戏配置文件 PersistedSettings.json 不存在，请先登录一次游戏以自动生成该文件"
                .to_string(),
        );
    }

    let metadata = fs::metadata(&path).map_err(|e| format!("获取文件元数据失败: {}", e))?;
    let mut permissions = metadata.permissions();
    permissions.set_readonly(readonly);

    fs::set_permissions(&path, permissions).map_err(|e| format!("修改文件属性失败: {}", e))?;

    if readonly {
        Ok("游戏设置已锁定（只读状态）".to_string())
    } else {
        Ok("游戏设置已解锁（可读写状态）".to_string())
    }
}

// ─── OP.GG MCP API - 云顶之弈热门阵容 ───

/// 附加羁绊/英雄名称/图标映射到 MCP 阵容数据（fetch_tft_meta_maps 内部有 Value 形态缓存，命中时仅浅克隆）
async fn attach_tft_meta_maps(parsed: &mut serde_json::Value) {
    // 映射字典不依赖 LCU 客户端（走内存/磁盘/CDragon 缓存），无需持锁
    let meta_maps = crate::parsers::tft::fetch_tft_meta_maps(None).await;
    if let Some(obj) = parsed.as_object_mut() {
        if let Some(trait_map) = meta_maps.get("trait_name_map") {
            obj.insert("trait_name_map".to_string(), trait_map.clone());
        }
        if let Some(champ_icon_map) = meta_maps.get("champion_icon_map") {
            obj.insert("champion_icon_map".to_string(), champ_icon_map.clone());
        }
        if let Some(champ_name_map) = meta_maps.get("champion_name_map") {
            obj.insert("champion_name_map".to_string(), champ_name_map.clone());
        }
        if let Some(item_name_map) = meta_maps.get("item_name_map") {
            obj.insert("item_name_map".to_string(), item_name_map.clone());
        }
        if let Some(item_icon_map) = meta_maps.get("item_icon_map") {
            obj.insert("item_icon_map".to_string(), item_icon_map.clone());
        }
    }
}

/// 从 OP.GG MCP API 获取云顶之弈当前版本热门强势阵容
#[tauri::command]
pub async fn fetch_tft_meta_decks(
    app_state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let cache_key = "tft_meta_decks".to_string();

    // 尝试内存缓存（阵容本体与图标映射均有独立缓存）
    if let Some(mut parsed) = crate::lcu::opgg::get_cached(&cache_key) {
        attach_tft_meta_maps(&mut parsed).await;
        return Ok(parsed);
    }

    let request_body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "tft_list_meta_decks",
            "arguments": {
                "desired_output_fields": [
                    "id",
                    "name",
                    "cost",
                    "teamCode",
                    "badge",
                    "stat",
                    "traits",
                    "units",
                    "early",
                    "middle"
                ]
            }
        }
    });

    let raw = crate::lcu::opgg::post_json(
        app_state.inner(),
        "https://mcp-api.op.gg/mcp",
        &request_body,
    )
    .await?;

    // MCP 错误检查
    if let Some(err) = raw.get("error") {
        let msg = err["message"].as_str().unwrap_or("未知 MCP 错误");
        return Err(format!("OP.GG MCP 返回错误: {}", msg));
    }

    // 提取 content[].text 中的 JSON 字符串
    let content_text = raw["result"]["content"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|first| first["text"].as_str())
        .ok_or_else(|| "OP.GG MCP 响应格式异常: 缺少 result.content[].text".to_string())?;

    log::debug!(
        "OP.GG MCP TFT 原始响应: {}",
        &content_text[..content_text.len().min(500)]
    );

    let mut parsed: serde_json::Value = serde_json::from_str(content_text)
        .map_err(|e| format!("解析 OP.GG MCP 数据失败: {}", e))?;

    // 附加 TFT 羁绊中文名称映射 + 英雄图标映射 + 英雄中文名称映射（LCU 优先，CDragon 兜底）
    attach_tft_meta_maps(&mut parsed).await;

    // 写入内存缓存（缓存的是含映射的完整 payload；命中时仍会再刷新一次映射）
    crate::lcu::opgg::put_cached(cache_key, parsed.clone());

    Ok(parsed)
}

// ─── CMD 方式观战（绕开 Already in gameflow）───

#[derive(Deserialize)]
pub struct SpectateDirectlyParams {
    pub summoner_name: String,
}

/// CMD 方式观战：通过 SGP 获取观战凭据，直接启动 League of Legends.exe。
/// 与 LCU API 方式（/lol-spectator/v1/spectate/launch）相比，可绕开
/// "Already in gameflow" 错误，无需等待客户端 gameflow 状态切换。
#[tauri::command]
pub async fn spectate_directly(
    params: SpectateDirectlyParams,
    app_state: State<'_, AppState>,
) -> Result<String, String> {
    let name = params.summoner_name.trim().to_string();
    if name.is_empty() {
        return Err("请输入召唤师名称".to_string());
    }

    // 锁内只提取连接参数（http_client 克隆是 Arc 浅拷贝），立即释放读锁，
    // 避免跨多个 HTTP await（含最长 15s 超时的 SGP 请求）持有锁阻塞 monitor 重连写锁
    let (port, token, http_client, server) = {
        let lock = app_state.lcu().await?;
        let lcu = lock.as_ref().unwrap();
        (
            lcu.port,
            lcu.token.clone(),
            lcu.http_client.clone(),
            lcu.server.clone(),
        )
    };
    let auth = build_auth_header(&token);
    let lcu_base = format!("https://127.0.0.1:{}", port);

    // ── 1. 获取大区标识 ──
    let server = server
        .ok_or_else(|| "无法获取大区信息（--rso_platform_id），请重启客户端后重试".to_string())?;
    let server_lower = server.to_lowercase();

    if !crate::lcu::sgp::is_tencent_server(&server_lower) {
        return Err(format!(
            "CMD 观战仅支持腾讯大区，当前大区 {} 不支持",
            server
        ));
    }

    // ── 2. 通过 LCU 获取召唤师 puuid ──
    let summoner_url = format!("{}/lol-summoner/v1/summoners", lcu_base);
    let summoner_resp = http_client
        .get(&summoner_url)
        .header("Authorization", &auth)
        .query(&[("name", &name)])
        .send()
        .await
        .map_err(|e| format!("获取召唤师信息失败: {}", e))?;

    if !summoner_resp.status().is_success() {
        return Err(format!(
            "未找到召唤师 \"{}\" (HTTP {})",
            name,
            summoner_resp.status().as_u16()
        ));
    }

    let summoner_data: serde_json::Value = summoner_resp
        .json()
        .await
        .map_err(|e| format!("解析召唤师数据失败: {}", e))?;
    let puuid = summoner_data
        .get("puuid")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "召唤师数据中缺少 puuid".to_string())?
        .to_string();

    // ── 3. 获取 SGP accessToken（30 分钟缓存复用）与共享客户端 ──
    let sgp_token = crate::lcu::sgp::get_sgp_token(port, &auth).await?;

    // ── 4. 构建 SGP base URL 并请求观战凭据 ──
    let sgp_base = crate::lcu::sgp::sgp_base_url(&server_lower);

    let sgp_client = crate::lcu::sgp::get_sgp_client();

    let sgp_url = format!(
        "{}/gsm/v1/ledge/spectator/region/{}/puuid/{}",
        sgp_base, server, puuid
    );

    log::info!("CMD 观战: 请求 SGP 完整 URL = {}", sgp_url);

    let sgp_resp = sgp_client
        .get(&sgp_url)
        .header("Authorization", format!("Bearer {}", sgp_token))
        .send()
        .await
        .map_err(|e| format!("SGP 请求失败: {}", e))?;

    if !sgp_resp.status().is_success() {
        let status = sgp_resp.status();
        let body = sgp_resp.text().await.unwrap_or_default();
        log::warn!("SGP 观战请求失败: HTTP {}, body: {}", status, body);

        let friendly_err = if status == reqwest::StatusCode::NOT_FOUND
            || status == reqwest::StatusCode::METHOD_NOT_ALLOWED
            || body.contains("NOT_IN_GAME")
            || body.contains("not found")
        {
            "该召唤师当前不在游戏中".to_string()
        } else {
            format!("获取观战数据失败 (HTTP {})", status.as_u16())
        };
        return Err(friendly_err);
    }

    let sgp_data: serde_json::Value = sgp_resp
        .json()
        .await
        .map_err(|e| format!("解析 SGP 响应失败: {}", e))?;

    let credentials = sgp_data
        .get("playerCredentials")
        .ok_or_else(|| "该召唤师当前不在游戏中".to_string())?;

    let observer_ip = credentials
        .get("observerServerIp")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "观战凭据缺少 observerServerIp".to_string())?;
    let observer_port = credentials
        .get("observerServerPort")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| "观战凭据缺少 observerServerPort".to_string())?;
    let encryption_key = credentials
        .get("observerEncryptionKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "观战凭据缺少 observerEncryptionKey".to_string())?;
    let game_id = credentials
        .get("gameId")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| "观战凭据缺少 gameId".to_string())?;

    // ── 5. 定位 Game 目录并启动 League of Legends.exe ──
    let cfg = app_state.config.read().await;
    // 跳过 WeGame 标记，取第一个真实客户端路径
    let lol_path = cfg
        .general
        .lol_path
        .iter()
        .find(|p| *p != WEGAME_MARKER)
        .cloned()
        .ok_or_else(|| "未配置英雄联盟客户端路径，请在设置中配置".to_string())?;
    drop(cfg);

    // 优先尝试 lol_path/Game（Yuumi 配置的是含 LeagueClient.exe 的根目录）
    // 回退尝试 lol_path/../Game（兼容 lol_path 指向 LeagueClient 子目录的情况）
    let game_dir = {
        let primary = std::path::Path::new(&lol_path).join("Game");
        if primary.join("League of Legends.exe").exists() {
            primary
        } else {
            let fallback = std::path::Path::new(&lol_path)
                .parent()
                .map(|p| p.join("Game"))
                .unwrap_or(primary.clone());
            if fallback.join("League of Legends.exe").exists() {
                fallback
            } else {
                return Err(format!(
                    "未找到游戏可执行文件。\n尝试过:\n  {}\n  {}\n请在设置中确认客户端安装路径",
                    primary.join("League of Legends.exe").display(),
                    fallback.join("League of Legends.exe").display()
                ));
            }
        }
    };
    let game_exe = game_dir.join("League of Legends.exe");

    log::info!(
        "CMD 观战: 启动 {:?} spectator {}:{} {} {} {} (cwd={:?})",
        game_exe,
        observer_ip,
        observer_port,
        encryption_key,
        game_id,
        server,
        game_dir
    );

    std::process::Command::new(&game_exe)
        .args([
            "spectator",
            &format!("{}:{}", observer_ip, observer_port),
            encryption_key,
            &game_id.to_string(),
            &server,
        ])
        .current_dir(&game_dir)
        .spawn()
        .map_err(|e| format!("启动游戏客户端失败: {}", e))?;

    Ok(format!("观战启动成功（CMD 方式），目标: {}", name))
}
