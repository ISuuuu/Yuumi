//! 系统与平台级辅助工具服务 (System Tools Service)
//! 包含检测 LOL 安装目录、资源管理器打开文件夹、打开日志目录、清理客户端/本地缓存、修复 LCU 客户端窗口缩放与位置

use std::path::{Path, PathBuf};
use std::process::Command;

/// 自动检测 LOL 客户端安装路径
/// 1. 优先通过 `sysinfo` 检测运行中的 `LeagueClientUx.exe` 进程路径推断根目录
/// 2. 若未运行，通过 `reg query HKCU\SOFTWARE\Tencent\LOL /v InstallPath` 从 Windows 注册表读取安装目录
pub fn detect_lol_path() -> Option<String> {
    // 1. 优先从运行中的客户端进程推断
    use sysinfo::System;
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    for process in sys.processes().values() {
        let name = process.name().to_string_lossy().to_lowercase();
        if name == "leagueclientux.exe" {
            if let Some(exe_path) = process.exe() {
                let mut dir = exe_path.parent();
                while let Some(d) = dir {
                    if d.join("LeagueClient.exe").exists()
                        || d.join("Client.exe").exists()
                        || d.join("client.exe").exists()
                    {
                        return Some(d.to_string_lossy().replace('\\', "/"));
                    }
                    dir = d.parent();
                }
                // 兜底：返回 exe 的上两级目录
                if let Some(parent) = exe_path.parent() {
                    if let Some(root) = parent.parent() {
                        return Some(root.to_string_lossy().replace('\\', "/"));
                    }
                }
            }
        }
    }

    // 2. 进程未运行，从 Windows 注册表获取国服 LOL 路径
    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = Command::new("reg")
            .args(["query", r"HKCU\SOFTWARE\Tencent\LOL", "/v", "InstallPath"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if line.contains("InstallPath") {
                        if let Some(pos) = line.find("REG_SZ") {
                            let raw_path = line[pos + 6..].trim();
                            if !raw_path.is_empty() {
                                let mut path = raw_path.replace('\\', "/");
                                if path.len() >= 2 && path.as_bytes()[1] == b':' {
                                    let drive =
                                        path.chars().next().unwrap().to_uppercase().to_string();
                                    path = format!("{}{}", drive, &path[1..]);
                                }

                                // 国服安装目录下常见 TCLS 目录
                                let tcls_dir = Path::new(&path).join("TCLS");
                                if tcls_dir.exists() {
                                    return Some(tcls_dir.to_string_lossy().replace('\\', "/"));
                                }

                                return Some(path);
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

/// 打开通用原生文件夹选择对话框，返回选中的目录路径
pub fn select_folder() -> Option<String> {
    rfd::FileDialog::new()
        .set_title("选择英雄联盟客户端安装路径")
        .pick_folder()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
}

/// 调用原生资源管理器打开指定路径
pub fn open_folder<P: AsRef<Path>>(path: P) -> Result<(), String> {
    let p = path.as_ref();
    if !p.exists() {
        return Err(format!("目标目录不存在: {}", p.display()));
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(p)
            .spawn()
            .map_err(|e| format!("打开资源管理器失败: {}", e))?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Command::new("xdg-open")
            .arg(p)
            .spawn()
            .map_err(|e| format!("打开文件夹失败: {}", e))?;
        Ok(())
    }
}

/// 打开应用日志文件夹（不存在则自动创建）
pub fn open_log_folder() -> Result<PathBuf, String> {
    // 优先使用当前 exe 同级 log 目录，或 %APPDATA%/Yuumi/log/
    let log_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("log")))
        .or_else(|| {
            yuumi_core::runtime::installed_data_dir()
                .parent()
                .map(|p| p.join("Yuumi").join("log"))
        })
        .unwrap_or_else(|| PathBuf::from("log"));

    if !log_dir.exists() {
        let _ = std::fs::create_dir_all(&log_dir);
    }

    open_folder(&log_dir)?;
    Ok(log_dir)
}

/// 清理客户端 CEF 缓存或本地图片与静态资源缓存
/// - 如果传入 `client_path`，优先扫描并清理 LOL 客户端目录下的 `cefCache` / `Cache` / `GPUCache`
/// - 同时清理 Yuumi 本地应用缓存目录（如 %APPDATA%/Yuumi/cache 或 data/cache）
pub fn clear_game_cache(client_path: Option<&str>) -> Result<String, String> {
    let mut total_deleted = 0u32;

    // 1. 若提供了客户端路径，清理其 CEF 缓存目录
    if let Some(cp) = client_path {
        let root = Path::new(cp);
        let candidate_dirs = [
            root.join("cefCache"),
            root.join("LeagueClient").join("cefCache"),
            root.join("Game").join("cefCache"),
            root.join("Cache"),
            root.join("GPUCache"),
        ];

        for dir in &candidate_dirs {
            if dir.exists() && dir.is_dir() {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            if std::fs::remove_dir_all(&path).is_ok() {
                                total_deleted += 1;
                            }
                        } else if path.is_file() && std::fs::remove_file(&path).is_ok() {
                            total_deleted += 1;
                        }
                    }
                }
            }
        }
    }

    // 2. 清理 Yuumi 自身的本地静态资源与图片缓存
    let local_cache_dir = yuumi_core::runtime::app_data_dir().join("cache");
    if local_cache_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&local_cache_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if std::fs::remove_dir_all(&path).is_ok() {
                        total_deleted += 1;
                    }
                } else if path.is_file() && std::fs::remove_file(&path).is_ok() {
                    total_deleted += 1;
                }
            }
        }
    }

    Ok(format!(
        "清理完成，共移除 {} 个缓存文件/目录",
        total_deleted
    ))
}

/// 修复 League of Legends (LCU) 客户端主窗口
/// 查找类名或标题为 "RiotWindow" / "League of Legends" 的窗口，重置其窗口缩放与位置到标准分辨率
pub fn fix_client_window() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        fix_client_window_win32()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("仅 Windows 平台支持客户端窗口修复".to_string())
    }
}

#[cfg(target_os = "windows")]
fn fix_client_window_win32() -> Result<String, String> {
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
        fn EnumWindows(
            lpEnumFunc: Option<unsafe extern "system" fn(*mut c_void, *mut c_void) -> i32>,
            lParam: *mut c_void,
        ) -> i32;
        fn GetWindowTextW(hWnd: *mut c_void, lpString: *mut u16, nMaxCount: i32) -> i32;
    }

    const SW_RESTORE: i32 = 9;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_SHOWWINDOW: u32 = 0x0040;

    unsafe {
        // 1. 优先按 RiotWindow 类名查找
        let class_name: Vec<u16> = "RiotWindow\0".encode_utf16().collect();
        let mut hwnd = FindWindowW(class_name.as_ptr(), ptr::null());

        // 2. 如果未直接找到，枚举顶层窗口匹配标题包含 "League of Legends"
        if hwnd.is_null() {
            struct EnumData {
                hwnd: *mut c_void,
            }

            unsafe extern "system" fn enum_callback(hwnd: *mut c_void, lparam: *mut c_void) -> i32 {
                let data = &mut *(lparam as *mut EnumData);
                let mut buf = [0u16; 512];
                let len = GetWindowTextW(hwnd, buf.as_mut_ptr(), 512);
                if len > 0 {
                    let title = String::from_utf16_lossy(&buf[..len as usize]);
                    if title.contains("League of Legends") || title.contains("英雄联盟") {
                        data.hwnd = hwnd;
                        return 0; // 找到即终止枚举
                    }
                }
                1
            }

            let mut data = EnumData {
                hwnd: ptr::null_mut(),
            };
            EnumWindows(Some(enum_callback), (&mut data as *mut EnumData).cast());
            hwnd = data.hwnd;
        }

        if hwnd.is_null() {
            return Err("未找到英雄联盟客户端 (LCU) 窗口，请确认客户端已启动".to_string());
        }

        // 还原窗口最小化状态
        ShowWindow(hwnd, SW_RESTORE);

        // 重置为 1600x900 或标准居中大小
        // 1280x720 或 1600x900 为标准 100% 分辨率
        let width = 1600;
        let height = 900;
        let x = 100;
        let y = 80;

        SetWindowPos(
            hwnd,
            ptr::null_mut(),
            x,
            y,
            width,
            height,
            SWP_NOZORDER | SWP_SHOWWINDOW,
        );

        Ok("客户端窗口已成功重置为 1600x900 (100% 原始缩放)".to_string())
    }
}
