use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use serde::Deserialize;
use tauri::State;
use tauri_plugin_opener::OpenerExt;

use crate::{build_auth_header, AppState};

static OPGG_CACHE: OnceLock<Mutex<HashMap<String, serde_json::Value>>> = OnceLock::new();

fn build_opgg_client(enable_proxy: bool, proxy_addr: &str) -> reqwest::Client {
    let mut builder = reqwest::Client::builder()
        .danger_accept_invalid_certs(true);

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

fn get_opgg_cache() -> &'static Mutex<HashMap<String, serde_json::Value>> {
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

    // 尝试从内存缓存中读取
    if let Ok(cache) = get_opgg_cache().lock() {
        if let Some(cached_val) = cache.get(&cache_key) {
            log::info!("OP.GG 缓存命中: {}", cache_key);
            return Ok(cached_val.clone());
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

    // 写入内存缓存
    if let Ok(mut cache) = get_opgg_cache().lock() {
        log::info!("OP.GG 缓存写入: {}", cache_key);
        cache.insert(cache_key, data.clone());
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
    let log_dir = dirs::config_dir()
        .ok_or("无法获取 AppData 路径")?
        .join("Yuumi")
        .join("logs");

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

    // 使用 PowerShell 查找并修复 League Client 窗口
    // 原 Python 实现使用 fix_lcu_window.exe，这里用 PowerShell 替代
    let ps_script = format!(
        r#"
        Add-Type @"
            using System;
            using System.Runtime.InteropServices;
            public class WinAPI {{
                [DllImport("user32.dll")]
                public static extern bool SetWindowPos(IntPtr hWnd, IntPtr hWndInsertAfter, int X, int Y, int cx, int cy, uint uFlags);
                [DllImport("user32.dll")]
                public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
                [DllImport("user32.dll")]
                public static extern IntPtr FindWindow(string lpClassName, string lpWindowName);
            }}
"@
        $procs = Get-Process -Name "LeagueClientUx" -ErrorAction SilentlyContinue
        if ($procs) {{
            $hWnd = $procs[0].MainWindowHandle
            if ($hWnd -ne [IntPtr]::Zero) {{
                # SW_RESTORE = 9
                [WinAPI]::ShowWindow($hWnd, 9)
                # SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED = 0x0003 | 0x0020
                [WinAPI]::SetWindowPos($hWnd, [IntPtr]::Zero, 0, 0, 0, 0, 0x0043)
                Write-Output "窗口已修复 (zoom={0})"
            }} else {{
                Write-Output "未找到窗口句柄"
            }}
        }} else {{
            Write-Output "未找到 LeagueClientUx 进程"
        }}
        "#,
        zoom
    );

    tokio::task::spawn_blocking(move || {
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &ps_script])
            .output()
            .map_err(|e| format!("执行 PowerShell 失败: {}", e))
            .and_then(|output| {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                if stdout.is_empty() {
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    Err(stderr)
                } else {
                    Ok(stdout.trim().to_string())
                }
            })
    })
    .await
    .map_err(|e| format!("任务执行失败: {}", e))?
}

use std::path::{Path, PathBuf};
use std::fs;

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


