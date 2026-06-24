use std::path::PathBuf;
use std::sync::Arc;
use sysinfo::System;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};

use crate::LcuClient;
use super::game_data::GameDataAssets;

const POLL_INTERVAL: Duration = Duration::from_secs(2);

#[cfg(target_os = "windows")]
mod win_privilege {
    use std::ptr;

    #[repr(C)]
    struct LUID {
        low_part: u32,
        high_part: i32,
    }

    #[repr(C)]
    struct LUID_AND_ATTRIBUTES {
        luid: LUID,
        attributes: u32,
    }

    #[repr(C)]
    struct TOKEN_PRIVILEGES {
        privilege_count: u32,
        privileges: [LUID_AND_ATTRIBUTES; 1],
    }

    const TOKEN_ADJUST_PRIVILEGES: u32 = 0x0020;
    const TOKEN_QUERY: u32 = 0x0008;
    const SE_PRIVILEGE_ENABLED: u32 = 0x00000002;

    #[link(name = "advapi32")]
    extern "system" {
        fn OpenProcessToken(
            process_handle: *mut std::ffi::c_void,
            desired_access: u32,
            token_handle: *mut *mut std::ffi::c_void,
        ) -> i32;

        fn LookupPrivilegeValueW(
            system_name: *const u16,
            name: *const u16,
            luid: *mut LUID,
        ) -> i32;

        fn AdjustTokenPrivileges(
            token_handle: *mut std::ffi::c_void,
            disable_all_privileges: i32,
            new_state: *const TOKEN_PRIVILEGES,
            buffer_length: u32,
            previous_state: *mut TOKEN_PRIVILEGES,
            return_length: *mut u32,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
    }

    pub unsafe fn enable_debug_privilege() -> bool {
        let mut token_handle: *mut std::ffi::c_void = ptr::null_mut();
        if OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token_handle,
        ) == 0
        {
            return false;
        }

        let priv_name: Vec<u16> = "SeDebugPrivilege\0".encode_utf16().collect();
        let mut luid = LUID { low_part: 0, high_part: 0 };

        if LookupPrivilegeValueW(ptr::null(), priv_name.as_ptr(), &mut luid) == 0 {
            CloseHandle(token_handle);
            return false;
        }

        let tp = TOKEN_PRIVILEGES {
            privilege_count: 1,
            privileges: [LUID_AND_ATTRIBUTES {
                luid,
                attributes: SE_PRIVILEGE_ENABLED,
            }],
        };

        let result = AdjustTokenPrivileges(
            token_handle,
            0,
            &tp,
            std::mem::size_of::<TOKEN_PRIVILEGES>() as u32,
            ptr::null_mut(),
            ptr::null_mut(),
        );

        CloseHandle(token_handle);
        result != 0
    }
}

/// 启动 LCU 进程轮询监测器。
/// 两种检测方式并用，确保可靠连接：
/// 1. 读取 lockfile（更快、更可靠）
/// 2. 解析进程命令行参数（备用）
pub fn start(
    app_handle: AppHandle,
    lcu_state: Arc<RwLock<Option<LcuClient>>>,
    game_data: Arc<RwLock<GameDataAssets>>,
) {
    #[cfg(target_os = "windows")]
    unsafe {
        let success = win_privilege::enable_debug_privilege();
        log::info!("启用 SeDebugPrivilege 特权: {}", success);
    }

    tauri::async_runtime::spawn(async move {
        let mut was_connected = false;

        loop {
            sleep(POLL_INTERVAL).await;
            // 每次循环重新初始化 System 进程树，彻底杜绝 sysinfo 增量/局部刷新可能导致的平台进程未更新 Bug
            let sys = System::new_all();

            // 优先尝试从 lockfile 获取，备用从进程参数获取
            let lcu_info = find_via_lockfile(&sys).or_else(|| find_via_cmdline(&sys));

            // 诊断日志
            if lcu_info.is_none() {
                if let Ok(mut file) = std::fs::File::create("E:\\Code\\Yuumi\\lcu_debug.txt") {
                    use std::io::Write;
                    let _ = writeln!(file, "====== 实时 LOL 进程诊断 ======");
                    for (pid, process) in sys.processes() {
                        let name = process.name().to_string_lossy();
                        if name.to_lowercase().contains("leagueclientux") {
                            let _ = writeln!(
                                file,
                                "找到进程: PID={:?}, Name={:?}, EXE={:?}, CMD={:?}",
                                pid,
                                name,
                                process.exe(),
                                process.cmd()
                            );
                        }
                    }
                    let _ = writeln!(file, "===============================");
                }
            } else {
                let _ = std::fs::remove_file("E:\\Code\\Yuumi\\lcu_debug.txt");
            }


            let mut lock = lcu_state.write().await;

            match lcu_info {
                Some((pid, port, token)) => {
                    let needs_reconnect = match lock.as_ref() {
                        Some(client) => client.pid != pid || client.port != port || client.token != token,
                        None => true,
                    };

                    if needs_reconnect {
                        log::info!("检测到 LCU: pid={}, port={}, 建立新连接", pid, port);

                        match reqwest::Client::builder()
                             .danger_accept_invalid_certs(true)
                             .build()
                        {
                            Ok(http_client) => {
                                let client = LcuClient {
                                    pid,
                                    port,
                                    token: token.clone(),
                                    http_client,
                                };
                                *lock = Some(client);
                                was_connected = true;
                                // 释放 lcu_state 写锁后再加载游戏资源
                                drop(lock);

                                // 异步加载游戏资源映射（不阻塞监控循环）
                                let gd = game_data.clone();
                                let pid_for_gd = pid;
                                let port_for_gd = port;
                                let token_for_gd = token.clone();
                                tauri::async_runtime::spawn(async move {
                                    match reqwest::Client::builder()
                                        .danger_accept_invalid_certs(true)
                                        .build()
                                    {
                                        Ok(http) => {
                                            let tmp_lcu = LcuClient {
                                                pid: pid_for_gd,
                                                port: port_for_gd,
                                                token: token_for_gd,
                                                http_client: http,
                                            };
                                            let assets = super::game_data::fetch_game_data_assets(&tmp_lcu).await;
                                            *gd.write().await = assets;
                                            log::info!("游戏资源已更新");
                                        }
                                        Err(e) => log::error!("加载游戏资源失败: {}", e),
                                    }
                                });

                                let _ = app_handle.emit(
                                    "lcu-client-started",
                                    serde_json::json!({ "port": port }),
                                );

                                super::ws::connect(app_handle.clone(), port, token);
                            }
                            Err(e) => {
                                log::error!("创建 HTTP 客户端失败: {}", e);
                            }
                        }
                    }
                }
                None => {
                    if was_connected {
                        log::info!("LCU 已断开");
                        *lock = None;
                        was_connected = false;
                        // 清空游戏资源
                        let gd = game_data.clone();
                        tauri::async_runtime::spawn(async move {
                            *gd.write().await = GameDataAssets::default();
                        });
                        let _ = app_handle.emit("lcu-client-ended", ());
                    } else if lock.is_some() {
                        *lock = None;
                    }
                }
            }
        }
    });
}

/// 方式一：从 lockfile 读取 port 和 token（最可靠）
///
/// LCU 启动时会在安装目录写入 lockfile，格式为：
/// `name:pid:port:password:protocol`
fn find_via_lockfile(sys: &System) -> Option<(u32, u16, String)> {
    // 找到 LeagueClientUx.exe 进程，获取其可执行文件所在目录
    let exe_dir = find_lcu_exe_dir(sys)?;

    let mut lockfile_path = exe_dir.join("lockfile");
    if !lockfile_path.exists() {
        if let Some(parent) = exe_dir.parent() {
            let backup_path = parent.join("lockfile");
            if backup_path.exists() {
                lockfile_path = backup_path;
            }
        }
    }

    let content = std::fs::read_to_string(&lockfile_path).ok()?;

    // 解析 lockfile: name:pid:port:password:protocol
    let parts: Vec<&str> = content.trim().split(':').collect();
    if parts.len() < 4 {
        log::warn!("lockfile 格式异常: {}", content.trim());
        return None;
    }

    let pid: u32 = parts[1].parse().ok()?;
    let port: u16 = parts[2].parse().ok()?;
    let password = parts[3].to_string();

    log::debug!("从 lockfile 读取: pid={}, port={}, token=***", pid, port);
    Some((pid, port, password))
}

/// 方式二：从进程命令行参数提取（备用）
fn find_via_cmdline(sys: &System) -> Option<(u32, u16, String)> {
    for (pid, process) in sys.processes() {
        let name = process.name().to_string_lossy().to_lowercase();
        if name == "leagueclientux.exe" || name == "leagueclientux" {
            let cmd: Vec<String> = process.cmd().iter().map(|arg| arg.to_string_lossy().into_owned()).collect();
            let cmd_str = cmd.join(" ");
            log::info!("找到 LCU 进程，命令行整句为: {}", cmd_str);

            let mut port: Option<u16> = None;
            let mut token: Option<String> = None;

            // 提取 --app-port=
            if let Some(p_idx) = cmd_str.find("--app-port=") {
                let sub = &cmd_str[p_idx + 11..];
                let end = sub.find(|c: char| !c.is_numeric()).unwrap_or(sub.len());
                port = sub[..end].parse::<u16>().ok();
            }

            // 提取 --remoting-auth-token=
            if let Some(t_idx) = cmd_str.find("--remoting-auth-token=") {
                let sub = &cmd_str[t_idx + 22..];
                let end = sub.find(|c: char| c == ' ' || c == '"' || c == '\'').unwrap_or(sub.len());
                token = Some(sub[..end].to_string());
            }
            
            log::info!("从命令行解析结果: port={:?}, token={:?}", port, token);
            
            // 只有成功提取到了合规的凭据才返回，避免因遇到无权/僵尸同名进程导致提前退出
            if let (Some(p), Some(t)) = (port, token) {
                return Some((pid.as_u32(), p, t));
            }
        }
    }
    None
}

/// 查找 LeagueClientUx.exe 的可执行文件所在目录
fn find_lcu_exe_dir(sys: &System) -> Option<PathBuf> {
    for (_, process) in sys.processes() {
        let name = process.name().to_string_lossy().to_lowercase();
        if name == "leagueclientux.exe" || name == "leagueclientux" {
            // 不使用 ? 语法，防止某个特定的同名进程没有 exe() 权限时直接中断整个函数
            if let Some(exe_path) = process.exe() {
                return exe_path.parent().map(|p| p.to_path_buf());
            }
        }
    }
    None
}
