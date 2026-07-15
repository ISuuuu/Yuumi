use crate::AppState;
use tauri::window::Effect;
use tauri::Manager;

/// 自动检测 LOL 客户端安装路径（从运行中的 LeagueClientUx.exe 推断，或从 Windows 注册表兜底）
#[tauri::command]
pub fn detect_lol_path() -> Result<Option<String>, String> {
    use sysinfo::System;
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    // 1. 优先从运行中的客户端进程推断
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
                        return Ok(Some(d.to_string_lossy().to_string()));
                    }
                    dir = d.parent();
                }
                // 兜底：返回 exe 的上两级
                if let Some(parent) = exe_path.parent() {
                    if let Some(root) = parent.parent() {
                        return Ok(Some(root.to_string_lossy().to_string()));
                    }
                }
            }
        }
    }

    // 2. 进程未运行，则按照 Python 逻辑，尝试从 Windows 注册表获取国服 LOL 路径
    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = std::process::Command::new("reg")
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
                                // 统一成正斜杠格式，规范盘符大小写
                                let mut path = raw_path.replace("\\", "/");
                                if path.len() >= 2 && path.as_bytes()[1] == b':' {
                                    let drive =
                                        path.chars().next().unwrap().to_uppercase().to_string();
                                    path = format!("{}{}", drive, &path[1..]);
                                }

                                // 如果是国服，注册表读出来的安装目录下有 TCLS 目录
                                let tcls_dir = std::path::Path::new(&path).join("TCLS");
                                if tcls_dir.exists() {
                                    return Ok(Some(tcls_dir.to_string_lossy().replace("\\", "/")));
                                }

                                return Ok(Some(path));
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(None)
}

/// 打开原生文件夹选择对话框，返回用户选择的路径
#[tauri::command]
pub fn select_lol_folder() -> Result<Option<String>, String> {
    let folder = rfd::FileDialog::new()
        .set_title("选择英雄联盟客户端安装目录")
        .pick_folder();
    Ok(folder.map(|p| p.to_string_lossy().to_string()))
}

/// 打开原生文件夹选择对话框，支持自定义标题和默认起始目录，返回用户选择的路径
#[tauri::command]
pub fn select_folder(
    title: Option<String>,
    default_path: Option<String>,
) -> Result<Option<String>, String> {
    let mut dialog = rfd::FileDialog::new();
    if let Some(t) = title {
        dialog = dialog.set_title(&t);
    }

    // 确定起始定位的目录
    let mut start_path = None;
    if let Some(ref dp) = default_path {
        if !dp.is_empty() {
            start_path = Some(std::path::PathBuf::from(dp));
        }
    }

    // 如果没有指定（或者为空），则使用默认的 "图片/Yuumi_Screenshots" 目录
    let path_to_set = match start_path {
        Some(p) => p,
        None => {
            if let Some(mut p) = dirs::picture_dir() {
                p.push("Yuumi_Screenshots");
                let _ = std::fs::create_dir_all(&p); // 确保它存在
                p
            } else {
                std::path::PathBuf::new()
            }
        }
    };

    if path_to_set.exists() {
        dialog = dialog.set_directory(path_to_set);
    }

    let folder = dialog.pick_folder();
    Ok(folder.map(|p| p.to_string_lossy().to_string()))
}

/// 在系统文件管理器中打开截图保存的目录
#[tauri::command]
pub async fn open_screenshot_folder(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let config_lock = state.config.read().await;
    let custom_path = &config_lock.functions.screenshot_save_path;

    let path = if !custom_path.is_empty() {
        std::path::PathBuf::from(custom_path)
    } else {
        let mut p = dirs::picture_dir().ok_or_else(|| "无法获取系统图片目录".to_string())?;
        p.push("Yuumi_Screenshots");
        p
    };

    if !path.exists() {
        std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// 运行时切换云母效果
#[tauri::command]
pub fn set_mica_effect(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        if enabled {
            window
                .set_effects(tauri::utils::config::WindowEffectsConfig {
                    effects: vec![Effect::Mica],
                    state: None,
                    radius: None,
                    color: None,
                })
                .map_err(|e| e.to_string())?;
        } else {
            window
                .set_effects(tauri::utils::config::WindowEffectsConfig {
                    effects: vec![],
                    state: None,
                    radius: None,
                    color: None,
                })
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// 启动 LOL 客户端（指定路径或从配置中的 lol_path 查找）
#[tauri::command]
pub async fn launch_lol_client(
    app_state: tauri::State<'_, AppState>,
    path: Option<String>,
) -> Result<(), String> {
    // 先检查是否已有客户端在运行
    {
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
    }

    // 智能探测客户端执行文件的辅助函数
    let find_executable = |base_path: &str| -> Option<std::path::PathBuf> {
        let p = std::path::Path::new(base_path);
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
    };

    // 启动可执行文件并处理 UAC 提升的辅助函数
    let spawn_executable = |exe: &std::path::Path, args: &[&str]| -> Result<(), String> {
        let mut cmd = std::process::Command::new(exe);
        cmd.args(args);
        // 关键：设置启动工作目录为 exe 所在的父目录，防止 DLL 加载或配置读取报拒绝访问错误 (os error 5)
        if let Some(parent) = exe.parent() {
            cmd.current_dir(parent);
        }
        match cmd.spawn() {
            Ok(_) => Ok(()),
            Err(e) => {
                let os_err = e.raw_os_error();
                // 拦截 740 (需要提升) 与 5 (拒绝访问) 并尝试以 UAC 管理员提权运行
                if os_err == Some(740) || os_err == Some(5) {
                    log::info!(
                        "启动 LOL 客户端遇到权限限制 ({:?})，尝试提升权限启动...",
                        os_err
                    );
                    #[cfg(target_os = "windows")]
                    {
                        use std::os::windows::process::CommandExt;
                        let working_dir = exe
                            .parent()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();

                        let escape_ps_string = |s: &str| -> String { s.replace("'", "''") };

                        let escaped_exe = escape_ps_string(&exe.to_string_lossy());
                        let escaped_working_dir = escape_ps_string(&working_dir);

                        // 格式化参数传给 PowerShell
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
                                escaped_exe,
                                args_str,
                                escaped_working_dir
                            )
                        };

                        let status = std::process::Command::new("powershell")
                            .creation_flags(0x08000000) // 隐藏 powershell 窗口
                            .args(["-Command", &command_str])
                            .spawn();
                        if status.is_ok() {
                            return Ok(());
                        }
                    }
                }
                Err(format!("启动失败: {}", e))
            }
        }
    };

    // 智能转换：若是 Riot 纳管的外服，改由 RiotClientServices.exe 启动
    let check_and_launch = |exe: std::path::PathBuf| -> Result<(), String> {
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
            spawn_executable(&service, args)?;
        } else {
            log::info!("常规方式启动客户端: {:?}", exe);
            spawn_executable(&exe, &[])?;
        }
        Ok(())
    };

    // 指定了路径则直接用
    if let Some(p) = path {
        if let Some(exe) = find_executable(&p) {
            log::info!("启动 LOL 客户端: {:?}", exe);
            check_and_launch(exe)?;
            return Ok(());
        }
        return Err(format!(
            "在 {} 中未找到启动程序 (TCLS/client.exe 或 LeagueClient.exe)",
            p
        ));
    }

    // 否则遍历配置路径
    let cfg = app_state.config.read().await;
    for p in &cfg.general.lol_path {
        if let Some(exe) = find_executable(p) {
            log::info!("启动 LOL 客户端: {:?}", exe);
            check_and_launch(exe)?;
            return Ok(());
        }
    }
    Err("未找到 LeagueClient.exe / Client.exe，请先在设置中配置客户端路径".to_string())
}

/// 大乱斗板凳席置顶悬浮窗控制命令
#[tauri::command]
pub async fn show_bench_overlay_window(
    app_handle: tauri::AppHandle,
    show: bool,
) -> Result<(), String> {
    let window = app_handle.get_webview_window("bench-overlay");
    if show {
        if let Some(win) = window {
            let _ = win.show();
            let _ = win.set_focus();
            if let Ok(Some(monitor)) = win.current_monitor() {
                let pos = monitor.position().to_logical::<f64>(monitor.scale_factor());
                let size = monitor.size().to_logical::<f64>(monitor.scale_factor());
                let x = pos.x + (size.width - 550.0) / 2.0;
                let y = pos.y; // 动态定位至该显示器的最顶端
                let _ =
                    win.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(x, y)));
            }
        } else {
            // 计算默认的顶部居中位置
            let mut x = 0.0;
            let mut y = 0.0;
            if let Ok(Some(monitor)) = app_handle.primary_monitor() {
                let pos = monitor.position().to_logical::<f64>(monitor.scale_factor());
                let size = monitor.size().to_logical::<f64>(monitor.scale_factor());
                x = pos.x + (size.width - 550.0) / 2.0;
                y = pos.y; // 动态定位至主显示器的最顶端
            }

            let win = tauri::WebviewWindowBuilder::new(
                &app_handle,
                "bench-overlay",
                tauri::WebviewUrl::App("index.html?window=bench-overlay".into()),
            )
            .title("Yuumi - ARAM Bench")
            .inner_size(550.0, 70.0)
            .position(x, y)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .resizable(false)
            .skip_taskbar(true)
            .build()
            .map_err(|e| e.to_string())?;

            let _ = win.show();
        }
    } else {
        if let Some(win) = window {
            let _ = win.close();
        }
    }
    Ok(())
}
