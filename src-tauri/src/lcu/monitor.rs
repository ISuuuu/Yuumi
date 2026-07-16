use std::path::PathBuf;
use std::sync::Arc;
use std::sync::LazyLock;
use sysinfo::System;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};

use super::game_data::GameDataAssets;
use crate::LcuClient;

/// 日志脱敏：将命令行中的 token 值替换为 ***
fn sanitize_cmdline(cmd: &str) -> String {
    static RE: LazyLock<regex_lite::Regex> =
        LazyLock::new(|| regex_lite::Regex::new(r"--remoting-auth-token=\S+").unwrap());
    RE.replace_all(cmd, "--remoting-auth-token=***").to_string()
}

const POLL_INTERVAL: Duration = Duration::from_secs(2);
/// Readiness probe: how many times to retry, and interval between retries.
const PROBE_MAX_RETRIES: u32 = 5;
const PROBE_INTERVAL: Duration = Duration::from_secs(2);

/// 启动 LCU 进程轮询监测器。
/// 两种检测方式并用，确保可靠连接：
/// 1. 读取 lockfile（更快、更可靠）
/// 2. 解析进程命令行参数（备用）
pub fn start(
    app_handle: AppHandle,
    lcu_state: Arc<RwLock<Option<LcuClient>>>,
    game_data: Arc<RwLock<GameDataAssets>>,
) {
    crate::spawn_log_panic(async move {
        let mut was_connected = false;
        let mut sys = System::new();
        let mut consecutive_misses: u32 = 0;
        // 每 MISS_THRESHOLD 次连续未找到 LCU，做一次全量重建（兜底增量刷新的边界情况）
        const MISS_THRESHOLD: u32 = 10;

        loop {
            sleep(POLL_INTERVAL).await;
            // 增量刷新进程列表，避免每 2 秒全量重建带来的 CPU 和内存开销
            sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

            // 优先尝试从 lockfile 获取，备用从进程参数获取，最后 WMIC 兜底（需管理员）
            let mut lcu_info = find_via_lockfile(&sys)
                .or_else(|| find_via_cmdline(&sys))
                .or_else(find_via_wmic);

            // 连续未找到 LCU 达到阈值时，全量重建进程树作为兜底
            if lcu_info.is_none() {
                consecutive_misses += 1;
                if consecutive_misses.is_multiple_of(MISS_THRESHOLD) {
                    log::debug!("连续 {} 次未找到 LCU，执行全量进程刷新", consecutive_misses);
                    sys = System::new_all();
                    // 用全量数据再试一次（lockfile + cmdline，WMIC 不依赖 sys 无需重试）
                    lcu_info = find_via_lockfile(&sys).or_else(|| find_via_cmdline(&sys));
                    if lcu_info.is_some() {
                        consecutive_misses = 0;
                    }
                }
            } else {
                consecutive_misses = 0;
            }

            // 诊断日志：异步写入，避免阻塞 tokio 工作线程
            if lcu_info.is_none() {
                let processes_snapshot: Vec<_> = sys
                    .processes()
                    .iter()
                    .filter(|(_, p)| {
                        p.name()
                            .to_string_lossy()
                            .to_lowercase()
                            .contains("leagueclientux")
                    })
                    .map(|(pid, p)| {
                        format!(
                            "PID={:?}, Name={:?}, EXE={:?}, CMD={:?}",
                            pid,
                            p.name(),
                            p.exe(),
                            p.cmd()
                        )
                    })
                    .collect();
                tokio::task::spawn_blocking(move || {
                    let debug_path = std::env::temp_dir().join("yuumi_lcu_debug.txt");
                    if let Ok(mut file) = std::fs::File::create(&debug_path) {
                        use std::io::Write;
                        let _ = writeln!(file, "====== 实时 LOL 进程诊断 ======");
                        for entry in &processes_snapshot {
                            let _ = writeln!(file, "找到进程: {}", entry);
                        }
                        let _ = writeln!(file, "===============================");
                    }
                });
            } else {
                let debug_path = std::env::temp_dir().join("yuumi_lcu_debug.txt");
                let _ = std::fs::remove_file(&debug_path);
            }

            // ── 阶段 1: 只读检查是否需要重连（不持有写锁）──
            let needs_reconnect = {
                let lock = lcu_state.read().await;
                match &lcu_info {
                    Some((pid, port, token, _server)) => match lock.as_ref() {
                        Some(client) => {
                            client.pid != *pid || client.port != *port || client.token != *token
                        }
                        None => true,
                    },
                    None => false,
                }
            };
            // 读锁已释放

            match lcu_info {
                Some((pid, port, token, server)) => {
                    if needs_reconnect {
                        // ── 阶段 2: 在获取写锁之前探测 LCU HTTP 是否就绪 ──
                        log::info!(
                            "检测到 LCU: pid={}, port={}, server={:?}, 等待 HTTP 服务器就绪...",
                            pid,
                            port,
                            server
                        );

                        if let Err(msg) = probe_lcu_readiness(port, &token).await {
                            log::warn!("LCU 就绪探测失败，跳过本轮: {}", msg);
                            // 不写入状态，下个轮询周期自动重试
                        } else {
                            // ── 阶段 3: 探测通过，构建客户端并提交状态 ──
                            match reqwest::Client::builder()
                                .danger_accept_invalid_certs(true)
                                .no_proxy()
                                .build()
                            {
                                Ok(http_client) => {
                                    let client = LcuClient {
                                        pid,
                                        port,
                                        token: token.clone(),
                                        server: server.clone(),
                                        http_client,
                                    };
                                    // 写锁仅短暂持有
                                    {
                                        let mut lock = lcu_state.write().await;
                                        *lock = Some(client);
                                    }
                                    was_connected = true;

                                    // 异步加载游戏资源映射（不阻塞监控循环）
                                    let gd = game_data.clone();
                                    let app_handle_for_gd = app_handle.clone();
                                    let token_for_gd = token.clone();
                                    crate::spawn_log_panic(async move {
                                        match reqwest::Client::builder()
                                            .danger_accept_invalid_certs(true)
                                            .no_proxy()
                                            .build()
                                        {
                                            Ok(http) => {
                                                let tmp_lcu = LcuClient {
                                                    pid,
                                                    port,
                                                    token: token_for_gd.clone(),
                                                    server: None,
                                                    http_client: http,
                                                };
                                                let assets =
                                                    super::game_data::fetch_game_data_assets(
                                                        &tmp_lcu,
                                                    )
                                                    .await;
                                                *gd.write().await = assets;
                                                log::info!("游戏资源已更新");
                                                let _ =
                                                    app_handle_for_gd.emit("game-data-ready", ());
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
                }
                None => {
                    // ── 断开处理 ──
                    if was_connected {
                        log::info!("LCU 已断开");
                        {
                            let mut lock = lcu_state.write().await;
                            *lock = None;
                        }
                        was_connected = false;

                        // 取消 WS 连接循环，避免在 LOL 退出后后台持续尝试连接旧端口并打印日志
                        {
                            use tauri::Manager;
                            let state = app_handle.state::<crate::AppState>();
                            let mut old_tx = state.ws_cancel_tx.lock().unwrap();
                            if let Some(tx) = old_tx.take() {
                                let _ = tx.send(true);
                            }
                        }

                        let gd = game_data.clone();
                        crate::spawn_log_panic(async move {
                            *gd.write().await = GameDataAssets::default();
                        });
                        let _ = app_handle.emit("lcu-client-ended", ());
                    } else {
                        let mut lock = lcu_state.write().await;
                        if lock.is_some() {
                            *lock = None;
                        }
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
fn find_via_lockfile(sys: &System) -> Option<(u32, u16, String, Option<String>)> {
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
        return None; // lockfile 未就绪或格式不完整（短暂竞态，下次轮询即可）
    }

    let pid: u32 = parts[1].parse().ok()?;
    let port: u16 = parts[2].parse().ok()?;
    let password = parts[3].to_string();

    // lockfile 不含大区信息，从进程命令行补充提取 --rso_platform_id=
    let server = extract_server_from_sys(sys);

    log::debug!(
        "从 lockfile 读取: pid={}, port={}, token=***, server={:?}",
        pid,
        port,
        server
    );
    Some((pid, port, password, server))
}

/// 方式二：从进程命令行参数提取
fn find_via_cmdline(sys: &System) -> Option<(u32, u16, String, Option<String>)> {
    for (pid, process) in sys.processes() {
        let name = process.name().to_string_lossy().to_lowercase();
        if name == "leagueclientux.exe" || name == "leagueclientux" {
            let cmd: Vec<String> = process
                .cmd()
                .iter()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect();
            let mut cmd_str = cmd.join(" ");

            #[cfg(target_os = "windows")]
            if cmd_str.is_empty() {
                if let Some(win_cmd) = get_cmdline_windows(pid.as_u32()) {
                    cmd_str = win_cmd;
                }
            }

            log::debug!(
                "找到 LCU 进程，命令行整句为: {}",
                sanitize_cmdline(&cmd_str)
            );

            if cmd_str.is_empty() {
                log::warn!("LCU 进程命令行为空");
                continue;
            }

            let mut port: Option<u16> = None;
            let mut token: Option<String> = None;
            let mut server: Option<String> = None;

            // 提取 --app-port=
            if let Some(p_idx) = cmd_str.find("--app-port=") {
                let sub = &cmd_str[p_idx + 11..];
                let end = sub.find(|c: char| !c.is_numeric()).unwrap_or(sub.len());
                port = sub[..end].parse::<u16>().ok();
            }

            // 提取 --remoting-auth-token=
            if let Some(t_idx) = cmd_str.find("--remoting-auth-token=") {
                let sub = &cmd_str[t_idx + 22..];
                let end = sub.find([' ', '"', '\'']).unwrap_or(sub.len());
                token = Some(sub[..end].to_string());
            }

            // 提取 --rso_platform_id=（登录大区标识，用于 SGP 观战等）
            if let Some(s_idx) = cmd_str.find("--rso_platform_id=") {
                let sub = &cmd_str[s_idx + 18..];
                let end = sub.find([' ', '"', '\'']).unwrap_or(sub.len());
                server = Some(sub[..end].to_string());
            }

            log::debug!(
                "从命令行解析结果: port={:?}, token=***, server={:?}",
                port,
                server
            );

            // 只有成功提取到了合规的凭据才返回，避免因遇到无权/僵尸同名进程导致提前退出
            if let (Some(p), Some(t)) = (port, token) {
                return Some((pid.as_u32(), p, t, server));
            }
        }
    }
    None
}

/// 方式三：通过 WMIC 获取命令行参数
fn find_via_wmic() -> Option<(u32, u16, String, Option<String>)> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let output = std::process::Command::new("wmic")
            .args([
                "process",
                "WHERE",
                "name='LeagueClientUx.exe'",
                "GET",
                "commandline",
            ])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW: 阻止黑窗口/终端闪烁
            .output()
            .ok()?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        // 提取 --app-port=
        let port = regex_find_number(&stdout, &RE_APP_PORT)?;
        // 提取 --remoting-auth-token=
        let token = regex_find_value(&stdout, &RE_AUTH_TOKEN)?;
        // 提取 --rso_platform_id=
        let server = regex_find_value(&stdout, &RE_PLATFORM_ID);

        log::debug!(
            "从 WMIC 解析结果: port={}, token=***, server={:?}",
            port,
            server
        );

        // WMIC 不返回 PID，lockfile 方式才是主路径；此处 PID 仅作标识，不影响连接建立
        Some((0, port, token, server))
    }
    #[cfg(not(target_os = "windows"))]
    None
}

fn regex_find_number(haystack: &str, re: &regex_lite::Regex) -> Option<u16> {
    let cap = re.captures(haystack)?;
    cap.get(1)?.as_str().parse::<u16>().ok()
}

fn regex_find_value(haystack: &str, re: &regex_lite::Regex) -> Option<String> {
    let cap = re.captures(haystack)?;
    Some(cap.get(1)?.as_str().to_string())
}

/// 预编译的 WMIC 解析正则（避免每次调用重新编译）
static RE_APP_PORT: LazyLock<regex_lite::Regex> =
    LazyLock::new(|| regex_lite::Regex::new(r"--app-port=(\d+)").unwrap());
static RE_AUTH_TOKEN: LazyLock<regex_lite::Regex> =
    LazyLock::new(|| regex_lite::Regex::new(r#"--remoting-auth-token=([^"\s]+)"#).unwrap());
static RE_PLATFORM_ID: LazyLock<regex_lite::Regex> =
    LazyLock::new(|| regex_lite::Regex::new(r#"--rso_platform_id=([^"\s]+)"#).unwrap());

/// 查找 LeagueClientUx.exe 的可执行文件所在目录
fn find_lcu_exe_dir(sys: &System) -> Option<PathBuf> {
    for process in sys.processes().values() {
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

/// 从 LeagueClientUx.exe 进程命令行提取 --rso_platform_id=（登录大区标识）。
/// lockfile 方式不含大区信息，需要用此函数补充。
fn extract_server_from_sys(sys: &System) -> Option<String> {
    for (pid, process) in sys.processes() {
        let name = process.name().to_string_lossy().to_lowercase();
        if name == "leagueclientux.exe" || name == "leagueclientux" {
            let mut cmd_str: String = process
                .cmd()
                .iter()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join(" ");

            #[cfg(target_os = "windows")]
            if cmd_str.is_empty() {
                if let Some(win_cmd) = get_cmdline_windows(pid.as_u32()) {
                    cmd_str = win_cmd;
                }
            }

            if let Some(s_idx) = cmd_str.find("--rso_platform_id=") {
                let sub = &cmd_str[s_idx + 18..];
                let end = sub.find([' ', '"', '\'']).unwrap_or(sub.len());
                return Some(sub[..end].to_string());
            }
        }
    }
    None
}

/// 探测 LCU HTTP 服务器是否真正可接受请求。
/// 在 monitor 写入共享状态之前调用，防止在服务器尚未就绪时触发前端 API 调用。
/// 最多重试 `PROBE_MAX_RETRIES` 次，间隔 `PROBE_INTERVAL`。
async fn probe_lcu_readiness(port: u16, token: &str) -> Result<(), String> {
    let auth = crate::build_auth_header(token);
    let url = format!("https://127.0.0.1:{}/system/v1/builds", port);

    let http = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .no_proxy()
        .build()
        .map_err(|e| format!("创建探测 HTTP 客户端失败: {}", e))?;

    for attempt in 1..=PROBE_MAX_RETRIES {
        match http
            .get(&url)
            .header("Authorization", &auth)
            .timeout(Duration::from_secs(5))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                log::info!(
                    "LCU HTTP 服务器就绪 (第 {}/{} 次探测)",
                    attempt,
                    PROBE_MAX_RETRIES
                );
                return Ok(());
            }
            Ok(resp) => {
                log::debug!(
                    "LCU 探测未就绪: HTTP {} (第 {}/{} 次)",
                    resp.status(),
                    attempt,
                    PROBE_MAX_RETRIES
                );
            }
            Err(e) => {
                log::debug!(
                    "LCU 探测失败: {} (第 {}/{} 次)",
                    e,
                    attempt,
                    PROBE_MAX_RETRIES
                );
            }
        }
        if attempt < PROBE_MAX_RETRIES {
            sleep(PROBE_INTERVAL).await;
        }
    }

    Err(format!(
        "LCU HTTP 服务器在 {} 次探测后仍未就绪",
        PROBE_MAX_RETRIES
    ))
}

/// Windows 专用的底层命令行查询方法，使用 NtQueryInformationProcess (ProcessCommandLineInformation) 绕过普通权限限制
#[cfg(target_os = "windows")]
fn get_cmdline_windows(pid: u32) -> Option<String> {
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Copy, Clone)]
    struct UNICODE_STRING {
        length: u16,
        maximum_length: u16,
        buffer: *mut u16,
    }

    extern "system" {
        fn OpenProcess(desired_access: u32, inherit_handle: i32, process_id: u32) -> *mut c_void;

        fn CloseHandle(handle: *mut c_void) -> i32;

        fn NtQueryInformationProcess(
            process_handle: *mut c_void,
            process_information_class: u32,
            process_information: *mut c_void,
            process_information_length: u32,
            return_length: *mut u32,
        ) -> i32;
    }

    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const PROCESS_COMMAND_LINE_INFORMATION: u32 = 60;

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }

        let mut return_length: u32 = 0;
        let _status = NtQueryInformationProcess(
            handle,
            PROCESS_COMMAND_LINE_INFORMATION,
            std::ptr::null_mut(),
            0,
            &mut return_length,
        );

        if return_length == 0 {
            CloseHandle(handle);
            return None;
        }

        let mut buffer: Vec<u8> = vec![0; return_length as usize];
        let status = NtQueryInformationProcess(
            handle,
            PROCESS_COMMAND_LINE_INFORMATION,
            buffer.as_mut_ptr() as *mut c_void,
            return_length,
            &mut return_length,
        );

        CloseHandle(handle);

        if status < 0 {
            return None;
        }

        let unicode_str = *(buffer.as_ptr() as *const UNICODE_STRING);

        let offset = unicode_str.buffer as usize - buffer.as_ptr() as usize;
        if offset + (unicode_str.length as usize) <= buffer.len() {
            let u16_slice =
                std::slice::from_raw_parts(unicode_str.buffer, (unicode_str.length / 2) as usize);
            return Some(String::from_utf16_lossy(u16_slice));
        } else {
            let header_size = std::mem::size_of::<UNICODE_STRING>();
            if header_size + (unicode_str.length as usize) <= buffer.len() {
                let ptr = buffer.as_ptr().add(header_size) as *const u16;
                let u16_slice = std::slice::from_raw_parts(ptr, (unicode_str.length / 2) as usize);
                return Some(String::from_utf16_lossy(u16_slice));
            }
        }
        None
    }
}
