use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::fs;
use serde::Deserialize;
use tauri::State;
use tauri_plugin_opener::OpenerExt;

use crate::{build_auth_header, AppState};

static OPGG_CACHE: OnceLock<Mutex<HashMap<String, OpggCacheEntry>>> = OnceLock::new();
const OPGG_CACHE_MAX_ENTRIES: usize = 100;
const OPGG_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(600); // 10 分钟

struct OpggCacheEntry {
    data: serde_json::Value,
    inserted_at: std::time::Instant,
}

fn build_opgg_client(enable_proxy: bool, proxy_addr: &str) -> reqwest::Client {
    let mut builder = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36");

    if enable_proxy && !proxy_addr.is_empty() {
        let proxy_url = if proxy_addr.contains("://") {
            proxy_addr.to_string()
        } else {
            format!("http://{}", proxy_addr)
        };
        if let Ok(proxy) = reqwest::Proxy::all(&proxy_url) {
            builder = builder.proxy(proxy);
            log::info!("OP.GG 请求已配置代理: {}", proxy_url);
        } else {
            log::warn!("无效的 OP.GG 代理地址: {}", proxy_addr);
        }
    }

    builder.build().unwrap_or_else(|_| reqwest::Client::new())
}

fn get_opgg_cache() -> &'static Mutex<HashMap<String, OpggCacheEntry>> {
    OPGG_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}


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
    let lock = app_state.lcu().await?;
    let lcu = lock.as_ref().unwrap();

    let url = format!("https://127.0.0.1:{}/lol-lobby/v1/lobby", lcu.port);
    let auth = build_auth_header(&lcu.token);

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

    let resp = lcu
        .http_client
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
pub async fn aram_reroll_and_swap_back(
    app_state: State<'_, AppState>,
) -> Result<String, String> {
    let lock = app_state.lcu().await?;
    let lcu = lock.as_ref().unwrap();

    let auth = build_auth_header(&lcu.token);
    let base = format!("https://127.0.0.1:{}", lcu.port);

    // 第一步：获取当前选择的英雄 ID
    let sel_url = format!(
        "{}/lol-champ-select/v1/session/my-selection",
        base
    );
    let sel_resp = lcu
        .http_client
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
    let reroll_url = format!(
        "{}/lol-champ-select/v1/session/my-selection/reroll",
        base
    );
    let reroll_resp = lcu
        .http_client
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
    let swap_resp = lcu
        .http_client
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
    let lock = app_state.lcu().await?;
    let lcu = lock.as_ref().unwrap();

    let auth = build_auth_header(&lcu.token);
    let base = format!("https://127.0.0.1:{}", lcu.port);

    // 第一步：获取当前符文页
    let get_url = format!("{}/lol-perks/v1/currentpage", base);
    let get_resp = lcu
        .http_client
        .get(&get_url)
        .header("Authorization", &auth)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if get_resp.status().is_success() {
        let page: serde_json::Value = get_resp.json().await.map_err(|e| e.to_string())?;
        if page.get("isDeletable").and_then(|v| v.as_bool()).unwrap_or(false) {
            let page_id = page.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
            if page_id > 0 {
                let del_url = format!("{}/lol-perks/v1/pages/{}", base, page_id);
                let _ = lcu
                    .http_client
                    .delete(&del_url)
                    .header("Authorization", &auth)
                    .send()
                    .await;
            }
        }
    }

    // 第二步：创建新符文页
    let create_url = format!("{}/lol-perks/v1/pages", base);
    let body = serde_json::json!({
        "name": params.name,
        "primaryStyleId": params.primary_style_id,
        "subStyleId": params.sub_style_id,
        "selectedPerkIds": params.selected_perk_ids,
        "current": true,
    });

    let create_resp = lcu
        .http_client
        .post(&create_url)
        .header("Authorization", &auth)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if create_resp.status().is_success() {
        Ok("符文页已应用".to_string())
    } else {
        Err(format!("创建符文页失败: HTTP {}", create_resp.status()))
    }
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
    let lock = app_state.lcu().await?;
    let lcu = lock.as_ref().unwrap();
    let auth = build_auth_header(&lcu.token);
    let base = format!("https://127.0.0.1:{}", lcu.port);

    let url = format!("{}/lol-game-data/assets/v1/champions/{}.json", base, champion_id);
    let resp = lcu.http_client.get(&url)
        .header("Authorization", &auth)
        .send().await.map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("LCU 返回错误 [{}]: 无法加载该英雄的皮肤数据", resp.status().as_u16()));
    }

    let details: LcuChampionDetails = resp.json().await.map_err(|e| e.to_string())?;
    
    let skins = details.skins.into_iter().map(|s| SkinEntry {
        id: s.id,
        name: s.name,
        load_screen_path: s.load_screen_path.unwrap_or_else(|| {
            format!("/lol-game-data/assets/v1/champion-loadscreens/{}/{}.jpg", champion_id, s.id)
        }),
    }).collect();

    Ok(skins)
}

// ─── OP.GG 数据代理 ───

/// 从 OP.GG API 获取英雄梯队/出装数据（代理请求，避免前端 CORS，使用内存缓存和复用客户端）
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

    // 尝试从内存缓存中读取（检查 TTL）
    if let Ok(cache) = get_opgg_cache().lock() {
        if let Some(entry) = cache.get(&cache_key) {
            if entry.inserted_at.elapsed() < OPGG_CACHE_TTL {
                log::info!("OP.GG 缓存命中: {}", cache_key);
                return Ok(entry.data.clone());
            }
        }
    }

    let url = match champion_id {
        Some(id) => {
            let pos = position.unwrap_or_else(|| "none".into());
            if mode == "arena" {
                format!("https://lol-api-champion.op.gg/api/{}/champions/{}", region, id)
            } else {
                format!("https://lol-api-champion.op.gg/api/{}/champions/{}/{}/{}", region, mode, id, pos)
            }
        }
        None => format!("https://lol-api-champion.op.gg/api/{}/champions/{}", region, mode),
    };

    let (enable_proxy, proxy_addr) = {
        let cfg = app_state.config.read().await;
        (cfg.general.enable_opgg_proxy, cfg.general.opgg_proxy_addr.clone())
    };

    let client = build_opgg_client(enable_proxy, &proxy_addr);
    let resp = client
        .get(&url)
        .query(&[("tier", tier.as_str())])
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let data: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

    // 写入内存缓存（超限时淘汰最旧条目）
    if let Ok(mut cache) = get_opgg_cache().lock() {
        if cache.len() >= OPGG_CACHE_MAX_ENTRIES {
            if let Some(oldest_key) = cache.iter().min_by_key(|(_, e)| e.inserted_at).map(|(k, _)| k.clone()) {
                cache.remove(&oldest_key);
            }
        }
        log::info!("OP.GG 缓存写入: {}", cache_key);
        cache.insert(cache_key, OpggCacheEntry { data: data.clone(), inserted_at: std::time::Instant::now() });
    }

    Ok(data)
}

// ─── 修复 LCU 客户端窗口 ───

/// 清除本地游戏资源缓存（头像、装备、技能、符文、强化图标）
#[tauri::command]
pub async fn clear_game_cache() -> Result<String, String> {
    let cache_dir = dirs::config_dir()
        .ok_or("无法获取 AppData 路径")?
        .join("Yuumi")
        .join("cache");

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
        } else if path.is_file() {
            if std::fs::remove_file(&path).is_ok() {
                count += 1;
            }
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
pub async fn get_lcu_zoom(
    app_state: State<'_, AppState>,
) -> Result<f64, String> {
    let lock = app_state.lcu().await?;
    let lcu = lock.as_ref().unwrap();

    let url = format!("https://127.0.0.1:{}/riotclient/zoom-scale", lcu.port);
    let auth = build_auth_header(&lcu.token);

    let resp = lcu
        .http_client
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
pub async fn fix_lcu_window(
    app_state: State<'_, AppState>,
) -> Result<String, String> {
    // 获取当前缩放比例
    let zoom = {
        let lock = app_state.lcu().await?;
        let lcu = lock.as_ref().unwrap();
        let url = format!("https://127.0.0.1:{}/riotclient/zoom-scale", lcu.port);
        let auth = build_auth_header(&lcu.token);
        let resp = lcu
            .http_client
            .get(&url)
            .header("Authorization", auth)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if resp.status().is_success() {
            resp.json::<f64>().await.map_err(|e| e.to_string())?
        } else {
            return Err(format!("获取缩放失败: HTTP {}", resp.status()));
        }
    };

    // 通过 Win32 API 直接操作窗口，替代旧的 PowerShell 脚本方案
    #[cfg(target_os = "windows")]
    {
        return fix_lcu_window_win32(zoom);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = zoom;
        Err("仅 Windows 平台支持窗口修复".to_string())
    }
}

#[cfg(target_os = "windows")]
fn fix_lcu_window_win32(zoom: f64) -> Result<String, String> {
    use std::ffi::c_void;
    use std::ptr;

    extern "system" {
        fn FindWindowW(lpClassName: *const u16, lpWindowName: *const u16) -> *mut c_void;
        fn ShowWindow(hWnd: *mut c_void, nCmdShow: i32) -> i32;
        fn SetWindowPos(
            hWnd: *mut c_void,
            hWndInsertAfter: *mut c_void,
            X: i32, Y: i32, cx: i32, cy: i32, uFlags: u32,
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

            let target_pid = {
                let sys = sysinfo::System::new_all();
                sys.processes().iter().find_map(|(pid, process)| {
                    let name = process.name().to_string_lossy().to_lowercase();
                    if name == "leagueclientux.exe" || name == "leagueclientux" {
                        Some(pid.as_u32())
                    } else {
                        None
                    }
                })
            };

            if let Some(pid) = target_pid {
                let mut data = EnumData { target_pid: pid, hwnd: ptr::null_mut() };
                EnumWindows(Some(enum_callback), (&mut data as *mut EnumData).cast());
                hwnd = data.hwnd;
            }
        }

        if hwnd.is_null() {
            return Err("未找到 LCU 窗口".to_string());
        }

        ShowWindow(hwnd, SW_RESTORE);
        SetWindowPos(hwnd, ptr::null_mut(), 0, 0, 0, 0,
            SWP_NOSIZE | SWP_NOMOVE | SWP_NOZORDER | SWP_SHOWWINDOW);

        Ok(format!("窗口已修复 (zoom={})", zoom))
    }
}

fn get_persisted_settings_path(lol_paths: &[String]) -> Option<PathBuf> {
    if lol_paths.is_empty() {
        return None;
    }
    let p = Path::new(&lol_paths[0]);
    let base_dir = if p.is_file() {
        p.parent()?
    } else {
        p
    };
    Some(base_dir.join("Game").join("Config").join("PersistedSettings.json"))
}

/// 查询游戏设置（PersistedSettings.json）是否已被锁定（只读）
#[tauri::command]
pub async fn get_game_settings_readonly(
    app_state: State<'_, AppState>,
) -> Result<bool, String> {
    let cfg = app_state.config.read().await;
    let path = get_persisted_settings_path(&cfg.general.lol_path)
        .ok_or_else(|| "未配置英雄联盟客户端路径".to_string())?;

    if !path.exists() {
        return Ok(false);
    }

    let metadata = fs::metadata(&path)
        .map_err(|e| format!("获取文件元数据失败: {}", e))?;
    
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
        return Err("游戏配置文件 PersistedSettings.json 不存在，请先登录一次游戏以自动生成该文件".to_string());
    }

    let metadata = fs::metadata(&path)
        .map_err(|e| format!("获取文件元数据失败: {}", e))?;
    let mut permissions = metadata.permissions();
    permissions.set_readonly(readonly);

    fs::set_permissions(&path, permissions)
        .map_err(|e| format!("修改文件属性失败: {}", e))?;

    if readonly {
        Ok("游戏设置已锁定（只读状态）".to_string())
    } else {
        Ok("游戏设置已解锁（可读写状态）".to_string())
    }
}

// ─── CMD 方式观战（绕开 Already in gameflow）───

/// 腾讯大区白名单（SGP 仅在这些大区可用）
const TENCENT_SERVERS: &[&str] = &[
    "tj100", "hn1", "cq100", "gz100", "nj100", "hn10", "tj101", "bgp2",
];

/// 需要 k8s-sgp 子域名的特殊大区
const K8S_SGP_SERVERS: &[&str] = &["hn1", "hn10", "bgp2"];

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

    let lock = app_state.lcu().await?;
    let lcu = lock.as_ref().unwrap();
    let auth = build_auth_header(&lcu.token);
    let lcu_base = format!("https://127.0.0.1:{}", lcu.port);

    // ── 1. 获取大区标识 ──
    let server = lcu
        .server
        .as_ref()
        .ok_or_else(|| "无法获取大区信息（--rso_platform_id），请重启客户端后重试".to_string())?;
    let server_lower = server.to_lowercase();

    if !TENCENT_SERVERS.contains(&server_lower.as_str()) {
        return Err(format!(
            "CMD 观战仅支持腾讯大区，当前大区 {} 不支持",
            server
        ));
    }

    // ── 2. 通过 LCU 获取召唤师 puuid ──
    let summoner_url = format!("{}/lol-summoner/v1/summoners", lcu_base);
    let summoner_resp = lcu
        .http_client
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

    let summoner_data: serde_json::Value =
        summoner_resp.json().await.map_err(|e| format!("解析召唤师数据失败: {}", e))?;
    let puuid = summoner_data
        .get("puuid")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "召唤师数据中缺少 puuid".to_string())?
        .to_string();

    // ── 3. 通过 LCU 获取 SGP token（/entitlements/v1/token → accessToken）──
    let token_url = format!("{}/entitlements/v1/token", lcu_base);
    let token_resp = lcu
        .http_client
        .get(&token_url)
        .header("Authorization", &auth)
        .send()
        .await
        .map_err(|e| format!("获取 SGP token 失败: {}", e))?;

    if !token_resp.status().is_success() {
        return Err(format!("获取 SGP token 失败: HTTP {}", token_resp.status().as_u16()));
    }

    let token_data: serde_json::Value =
        token_resp.json().await.map_err(|e| format!("解析 SGP token 失败: {}", e))?;
    let sgp_token = token_data
        .get("accessToken")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "SGP token 数据中缺少 accessToken".to_string())?
        .to_string();

    // ── 4. 构建 SGP base URL 并请求观战凭据 ──
    let sgp_base = if K8S_SGP_SERVERS.contains(&server_lower.as_str()) {
        format!("https://{}-k8s-sgp.lol.qq.com:21019", server_lower)
    } else {
        format!("https://{}-sgp.lol.qq.com:21019", server_lower)
    };

    let sgp_client = reqwest::Client::builder()
        .no_proxy()
        .user_agent("RiotClient/78.0.1.1352 (Windows;10;co;red)")
        .http1_only()
        .build()
        .map_err(|e| format!("创建 SGP HTTP 客户端失败: {}", e))?;

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

        let friendly_err = if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::METHOD_NOT_ALLOWED || body.contains("NOT_IN_GAME") || body.contains("not found") {
            "该召唤师当前不在游戏中".to_string()
        } else {
            format!("获取观战数据失败 (HTTP {})", status.as_u16())
        };
        return Err(friendly_err);
    }

    let sgp_data: serde_json::Value =
        sgp_resp.json().await.map_err(|e| format!("解析 SGP 响应失败: {}", e))?;

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
    let lol_path = cfg
        .general
        .lol_path
        .first()
        .ok_or_else(|| "未配置英雄联盟客户端路径，请在设置中配置".to_string())?
        .clone();
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
        game_exe, observer_ip, observer_port, encryption_key, game_id, server, game_dir
    );

    std::process::Command::new(&game_exe)
        .args(&[
            "spectator",
            &format!("{}:{}", observer_ip, observer_port),
            encryption_key,
            &game_id.to_string(),
            server,
        ])
        .current_dir(&game_dir)
        .spawn()
        .map_err(|e| format!("启动游戏客户端失败: {}", e))?;

    Ok(format!("观战启动成功（CMD 方式），目标: {}", name))
}