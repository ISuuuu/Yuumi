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

/// lol_path 列表中的特殊标记：表示该条目为「启动 WeGame」而非真实客户端路径
pub const WEGAME_MARKER: &str = "WeGame";

/// 读取客户端配置路径与 WeGame 路径（优先从 config.json 读取，兜底自动检测）
pub fn load_client_paths() -> (Vec<String>, String) {
    let config_file = yuumi_core::runtime::app_data_dir().join("config.json");
    if config_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&config_file) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                let lol_paths = v
                    .get("General")
                    .and_then(|g| g.get("LolPath"))
                    .and_then(|lp| lp.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect::<Vec<String>>()
                    })
                    .unwrap_or_default();
                let wegame_path = v
                    .get("General")
                    .and_then(|g| g.get("WegamePath"))
                    .and_then(|wp| wp.as_str())
                    .unwrap_or("")
                    .to_string();
                if !lol_paths.is_empty() || !wegame_path.is_empty() {
                    return (lol_paths, wegame_path);
                }
            }
        }
    }

    // 兜底：若未读取到配置，则尝试自动检测 LOL 路径与 WeGame
    let mut default_paths = Vec::new();
    let mut default_wegame = String::new();
    if let Some(wegame_exe) = find_wegame_exe() {
        default_paths.push(WEGAME_MARKER.to_string());
        default_wegame = wegame_exe.to_string_lossy().to_string();
    }
    if let Some(lol_dir) = detect_lol_path() {
        if !default_paths.contains(&lol_dir) {
            default_paths.push(lol_dir);
        }
    }
    (default_paths, default_wegame)
}

/// 启动 LOL 客户端（指定路径或从配置中的 lol_path 查找）
pub fn launch_lol_client(path: Option<&str>, wegame_path: Option<&str>) -> Result<(), String> {
    // 1. 检查是否已有客户端在运行
    use sysinfo::System;
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let already_running = sys.processes().values().any(|p| {
        p.name()
            .to_string_lossy()
            .eq_ignore_ascii_case("leagueclientux.exe")
    });
    if already_running {
        log::info!("客户端已在运行，跳过启动");
        return Ok(());
    }

    // 2. 指定了路径则直接用
    if let Some(p) = path {
        if p == WEGAME_MARKER || p.eq_ignore_ascii_case("wegame") {
            return launch_wegame(wegame_path);
        }
        if let Some(exe) = find_executable(p) {
            log::info!("启动 LOL 客户端: {:?}", exe);
            return check_and_launch(&exe);
        }
        return Err(format!(
            "在 {} 中未找到启动程序 (TCLS/client.exe 或 LeagueClient.exe)",
            p
        ));
    }

    // 3. 否则遍历从配置中读取到的路径
    let (paths, default_wegame) = load_client_paths();
    for p in &paths {
        if p == WEGAME_MARKER || p.eq_ignore_ascii_case("wegame") {
            let wp = if !default_wegame.is_empty() {
                Some(default_wegame.as_str())
            } else {
                wegame_path
            };
            return launch_wegame(wp);
        }
        if let Some(exe) = find_executable(p) {
            log::info!("启动 LOL 客户端: {:?}", exe);
            return check_and_launch(&exe);
        }
    }

    Err("未找到 LeagueClient.exe / Client.exe，请先在设置中配置客户端路径".to_string())
}

/// 智能探测客户端执行文件的辅助函数
fn find_executable(base_path: &str) -> Option<PathBuf> {
    let p = Path::new(base_path);
    let check_list = &[
        ("", "LeagueClient.exe"),
        ("", "Client.exe"),
        ("", "client.exe"),
        ("TCLS", "client.exe"),
        ("TCLS", "Client.exe"),
        ("LeagueClient", "LeagueClient.exe"),
        ("../TCLS", "client.exe"),
        ("../TCLS", "Client.exe"),
        ("../LeagueClient", "LeagueClient.exe"),
    ];

    for (sub_dir, exe_name) in check_list {
        let exe = if sub_dir.is_empty() {
            p.join(exe_name)
        } else {
            p.join(sub_dir).join(exe_name)
        };
        if exe.exists() {
            return Some(exe);
        }
    }
    None
}

/// 智能转换：若是 Riot 纳管的外服，改由 RiotClientServices.exe 启动
fn check_and_launch(exe: &Path) -> Result<(), String> {
    let mut riot_service = None;
    if exe.file_name().map(|n| n.to_string_lossy().to_lowercase())
        == Some("leagueclient.exe".to_string())
    {
        if let Some(parent) = exe.parent() {
            let same_level = parent.join("RiotClientServices.exe");
            if same_level.exists() {
                riot_service = Some(same_level);
            } else if let Some(grandparent) = parent.parent() {
                let parent_level = grandparent.join("RiotClientServices.exe");
                if parent_level.exists() {
                    riot_service = Some(parent_level);
                }
            }
        }
    }

    if let Some(service) = riot_service {
        log::info!("检测到外服 Riot 纳管客户端，改由 RiotClientServices.exe 启动");
        let is_pbe = exe.to_string_lossy().to_lowercase().contains("pbe");
        let patchline = if is_pbe { "pbe" } else { "live" };
        let args = &[
            "--launch-product=league_of_legends",
            &format!("--launch-patchline={}", patchline),
        ];
        log::info!("启动 Riot 服务: {:?} {:?}", service, args);
        spawn_executable(&service, args)
    } else {
        log::info!("常规方式启动客户端: {:?}", exe);
        spawn_executable(exe, &[])
    }
}

/// 启动 WeGame 客户端
fn launch_wegame(configured_path: Option<&str>) -> Result<(), String> {
    if let Some(p) = configured_path {
        let trimmed = p.trim();
        if !trimmed.is_empty() {
            let path = Path::new(trimmed);
            let exe = if path.is_file() {
                path.to_path_buf()
            } else {
                path.join("wegame.exe")
            };
            if exe.exists() {
                log::info!("启动 WeGame (配置路径): {:?}", exe);
                return spawn_executable(&exe, &[]);
            }
            log::warn!("配置的 WeGame 路径无效 ({:?})，回退自动探测", exe);
        }
    }

    let exe = find_wegame_exe().ok_or_else(|| {
        "未找到 WeGame。请先安装 WeGame，或在设置中手动配置 WeGame 路径。".to_string()
    })?;
    log::info!("启动 WeGame (自动探测): {:?}", exe);
    spawn_executable(&exe, &[])
}

/// 探测 WeGame 可执行文件位置
pub fn find_wegame_exe() -> Option<PathBuf> {
    // 1. 注册表 InstallPath
    #[cfg(target_os = "windows")]
    for key in [
        r"HKCU\SOFTWARE\Tencent\WeGame",
        r"HKLM\SOFTWARE\Tencent\WeGame",
        r"HKLM\SOFTWARE\WOW6432Node\Tencent\WeGame",
    ] {
        if let Some(dir) = reg_string_value(key, "InstallPath") {
            let exe = Path::new(&dir).join("wegame.exe");
            if exe.exists() {
                return Some(exe);
            }
        }
    }

    // 2. 注册表卸载项
    if let Some(exe) = find_wegame_from_uninstall() {
        return Some(exe);
    }

    // 3. 常见安装目录
    for letter in b'A'..=b'Z' {
        let drive = format!("{}:\\", letter as char);
        if !Path::new(&drive).exists() {
            continue;
        }
        for sub_dir in ["Program Files", "Program Files (x86)"] {
            let exe = Path::new(&drive)
                .join(sub_dir)
                .join("WeGame")
                .join("wegame.exe");
            if exe.exists() {
                return Some(exe);
            }
        }
    }
    for dir in [r"D:\WeGame", r"E:\WeGame"] {
        let exe = Path::new(dir).join("wegame.exe");
        if exe.exists() {
            return Some(exe);
        }
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let exe = Path::new(&local).join("WeGame").join("wegame.exe");
        if exe.exists() {
            return Some(exe);
        }
    }

    // 4. 进程兜底：WeGame 运行中时推断
    {
        use sysinfo::System;
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        for process in sys.processes().values() {
            let name = process.name().to_string_lossy().to_lowercase();
            if name == "wegame.exe" || name == "wegame" {
                if let Some(exe_path) = process.exe() {
                    return Some(exe_path.to_path_buf());
                }
            }
        }
    }

    None
}

/// 从注册表卸载项中查找 WeGame
fn find_wegame_from_uninstall() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let uninstall_roots = [
            r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
            r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
            r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        ];
        for root in uninstall_roots {
            let list_output = match Command::new("reg").args(["query", root]).output() {
                Ok(o) => o,
                Err(_) => continue,
            };
            if !list_output.status.success() {
                continue;
            }
            for line in String::from_utf8_lossy(&list_output.stdout).lines() {
                let subkey = line.trim();
                if !subkey.starts_with(root) || subkey.len() <= root.len() {
                    continue;
                }
                let name = match reg_string_value(subkey, "DisplayName") {
                    Some(n) => n,
                    None => continue,
                };
                if !name.to_lowercase().contains("wegame") {
                    continue;
                }
                if let Some(dir) = reg_string_value(subkey, "InstallLocation") {
                    let exe = Path::new(&dir).join("wegame.exe");
                    if exe.exists() {
                        return Some(exe);
                    }
                }
                if let Some(icon) = reg_string_value(subkey, "DisplayIcon") {
                    let clean = icon
                        .split(',')
                        .next()
                        .unwrap_or("")
                        .trim()
                        .trim_matches('"');
                    if !clean.is_empty() {
                        let icon_path = Path::new(clean);
                        if icon_path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_lowercase())
                            == Some("wegame.exe".to_string())
                            && icon_path.exists()
                        {
                            return Some(icon_path.to_path_buf());
                        }
                    }
                }
            }
        }
    }
    None
}

/// 从注册表读取指定键的 REG_SZ 字符串值
#[cfg(target_os = "windows")]
fn reg_string_value(key: &str, name: &str) -> Option<String> {
    let output = Command::new("reg")
        .args(["query", key, "/v", name])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.contains(name) {
            if let Some(pos) = line.find("REG_SZ") {
                let raw = line[pos + 6..].trim();
                if !raw.is_empty() {
                    return Some(raw.replace('\\', "/"));
                }
            }
        }
    }
    None
}

/// 启动可执行文件并处理 UAC 提升
fn spawn_executable(exe: &Path, args: &[&str]) -> Result<(), String> {
    let mut cmd = Command::new(exe);
    cmd.args(args);
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    if let Some(parent) = exe.parent() {
        cmd.current_dir(parent);
    }
    match cmd.spawn() {
        Ok(_) => Ok(()),
        Err(e) => {
            let os_err = e.raw_os_error();
            if os_err == Some(740) || os_err == Some(5) {
                log::info!("启动客户端遇到权限限制 ({:?})，尝试提升权限启动...", os_err);
                #[cfg(target_os = "windows")]
                {
                    let working_dir = exe
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let escape_ps_string = |s: &str| -> String { s.replace('\'', "''") };
                    let escaped_exe = escape_ps_string(&exe.to_string_lossy());
                    let escaped_working_dir = escape_ps_string(&working_dir);

                    let args_str = args
                        .iter()
                        .map(|arg| format!("'{}'", escape_ps_string(arg)))
                        .collect::<Vec<String>>()
                        .join(", ");

                    let command_str = if args_str.is_empty() {
                        format!(
                            "Start-Process -FilePath '{}' -WorkingDirectory '{}' -Verb RunAs",
                            escaped_exe, escaped_working_dir
                        )
                    } else {
                        format!(
                            "Start-Process -FilePath '{}' -ArgumentList {} -WorkingDirectory '{}' -Verb RunAs",
                            escaped_exe, args_str, escaped_working_dir
                        )
                    };

                    let status = Command::new("powershell")
                        .args([
                            "-NoProfile",
                            "-WindowStyle",
                            "Hidden",
                            "-Command",
                            &command_str,
                        ])
                        .spawn();
                    if status.is_ok() {
                        return Ok(());
                    }
                }
            }
            Err(format!("启动失败: {}", e))
        }
    }
}
