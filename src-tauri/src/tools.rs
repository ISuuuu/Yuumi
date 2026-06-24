use serde::Deserialize;
use tauri::State;
use tokio::sync::RwLock;

use crate::{build_auth_header, LcuClient};
use std::sync::Arc;

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
    lcu_state: State<'_, Arc<RwLock<Option<LcuClient>>>>,
) -> Result<String, String> {
    let lock = lcu_state.read().await;
    let lcu = lock
        .as_ref()
        .ok_or("LCU 未连接，请先启动英雄联盟客户端")?;

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
    lcu_state: State<'_, Arc<RwLock<Option<LcuClient>>>>,
) -> Result<String, String> {
    let lock = lcu_state.read().await;
    let lcu = lock
        .as_ref()
        .ok_or("LCU 未连接，请先启动英雄联盟客户端")?;

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
    lcu_state: State<'_, Arc<RwLock<Option<LcuClient>>>>,
) -> Result<String, String> {
    let lock = lcu_state.read().await;
    let lcu = lock
        .as_ref()
        .ok_or("LCU 未连接，请先启动英雄联盟客户端")?;

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

// ─── 修复 LCU 客户端窗口 ───

/// 获取当前 LCU 客户端缩放比例（用于窗口修复）
#[tauri::command]
pub async fn get_lcu_zoom(
    lcu_state: State<'_, Arc<RwLock<Option<LcuClient>>>>,
) -> Result<f64, String> {
    let lock = lcu_state.read().await;
    let lcu = lock
        .as_ref()
        .ok_or("LCU 未连接")?;

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
    lcu_state: State<'_, Arc<RwLock<Option<LcuClient>>>>,
) -> Result<String, String> {
    // 获取当前缩放比例
    let zoom = {
        let lock = lcu_state.read().await;
        let lcu = lock.as_ref().ok_or("LCU 未连接")?;
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

