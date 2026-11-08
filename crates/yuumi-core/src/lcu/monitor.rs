use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::OnceLock;
use std::time::Duration;
use sysinfo::System;
use tokio::time::sleep;

use super::client::build_auth_header;

/// LCU 连接信息
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LcuConnectionInfo {
    pub pid: u32,
    pub port: u16,
    pub token: String,
    pub server: Option<String>,
}

/// 进程监控轮询事件
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonitorEvent {
    Connected(LcuConnectionInfo),
    Disconnected,
}

/// 日志脱敏：将命令行中的 token 值替换为 ***
fn sanitize_cmdline(cmd: &str) -> String {
    static RE: LazyLock<regex_lite::Regex> =
        LazyLock::new(|| regex_lite::Regex::new(r"--remoting-auth-token=\S+").unwrap());
    RE.replace_all(cmd, "--remoting-auth-token=***").to_string()
}

const POLL_INTERVAL: Duration = Duration::from_secs(2);
const POLL_INTERVAL_STABLE: Duration = Duration::from_secs(5);
const PROBE_MAX_RETRIES: u32 = 5;
const PROBE_INTERVAL: Duration = Duration::from_secs(2);
const WMIC_THROTTLE_ROUNDS: u32 = 5;
const MISS_THRESHOLD: u32 = 10;

/// 快速单次探测当前系统中的 LCU 进程并获取连接凭据（非阻塞同步函数，内部仅刷新并检索对应进程与 lockfile/命令行）
pub fn detect_lcu() -> Option<LcuConnectionInfo> {
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let mut cached_dir = None;
    find_via_lockfile(&sys, &mut cached_dir)
        .or_else(|| find_via_cmdline(&sys))
        .map(|(pid, port, token, server)| LcuConnectionInfo {
            pid,
            port,
            token,
            server,
        })
}

/// 启动 LCU 进程轮询监测器。
/// 通过 `tokio::sync::mpsc::Sender<MonitorEvent>` 或返回一个 Receiver 提供事件流。
pub fn start_monitor() -> tokio::sync::mpsc::Receiver<MonitorEvent> {
    let (tx, rx) = tokio::sync::mpsc::channel(32);
    tokio::spawn(async move {
        run_monitor_loop(tx).await;
    });
    rx
}

/// 监控核心循环
pub async fn run_monitor_loop(tx: tokio::sync::mpsc::Sender<MonitorEvent>) {
    let was_connected = Arc::new(AtomicBool::new(false));
    let reconnect_in_flight = Arc::new(AtomicBool::new(false));
    let mut sys = System::new();
    let mut consecutive_misses: u32 = 0;
    let mut cached_exe_dir: Option<PathBuf> = None;
    let mut current_info: Option<LcuConnectionInfo> = None;

    struct ReconnectGuard(Arc<AtomicBool>);
    impl Drop for ReconnectGuard {
        fn drop(&mut self) {
            self.0.store(false, Ordering::SeqCst);
        }
    }

    loop {
        sleep(if was_connected.load(Ordering::Relaxed) {
            POLL_INTERVAL_STABLE
        } else {
            POLL_INTERVAL
        })
        .await;

        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        let mut lcu_raw =
            find_via_lockfile(&sys, &mut cached_exe_dir).or_else(|| find_via_cmdline(&sys));

        if lcu_raw.is_none() && consecutive_misses.is_multiple_of(WMIC_THROTTLE_ROUNDS) {
            lcu_raw = find_via_wmic().await;
        }

        if lcu_raw.is_none() {
            consecutive_misses += 1;
            if consecutive_misses.is_multiple_of(MISS_THRESHOLD) {
                log::debug!("连续 {} 次未找到 LCU，执行全量进程刷新", consecutive_misses);
                sys = System::new_all();
                lcu_raw = find_via_lockfile(&sys, &mut cached_exe_dir)
                    .or_else(|| find_via_cmdline(&sys));
                if lcu_raw.is_some() {
                    consecutive_misses = 0;
                }
            }
        } else {
            consecutive_misses = 0;
        }

        let lcu_info = lcu_raw.map(|(pid, port, token, server)| LcuConnectionInfo {
            pid,
            port,
            token,
            server,
        });

        let needs_reconnect = match (&lcu_info, &current_info) {
            (Some(new_info), Some(curr)) => new_info != curr,
            (Some(_), None) => true,
            (None, _) => false,
        };

        match lcu_info {
            Some(info) => {
                if needs_reconnect && !reconnect_in_flight.swap(true, Ordering::SeqCst) {
                    log::info!(
                        "检测到 LCU: pid={}, port={}, server={:?}, 等待 HTTP 就绪...",
                        info.pid,
                        info.port,
                        info.server
                    );

                    let tx = tx.clone();
                    let was_connected = was_connected.clone();
                    let reconnect_in_flight = reconnect_in_flight.clone();
                    let target_info = info.clone();

                    tokio::spawn(async move {
                        let _guard = ReconnectGuard(reconnect_in_flight);
                        if let Err(msg) = probe_lcu_readiness(target_info.port, &target_info.token).await {
                            log::warn!("LCU 就绪探测失败，跳过本轮: {}", msg);
                        } else {
                            log::info!("LCU HTTP 服务器已就绪: port={}", target_info.port);
                            was_connected.store(true, Ordering::Relaxed);
                            let _ = tx.send(MonitorEvent::Connected(target_info)).await;
                        }
                    });
                    current_info = Some(info);
                }
            }
            None => {
                if was_connected.swap(false, Ordering::SeqCst) {
                    log::info!("LCU 已断开");
                    cached_exe_dir = None;
                    current_info = None;
                    let _ = tx.send(MonitorEvent::Disconnected).await;
                } else {
                    current_info = None;
                }
            }
        }
    }
}

/// 方式一：从 lockfile 读取 port 和 token
///
/// LCU 格式：`name:pid:port:password:protocol`
pub fn find_via_lockfile(
    sys: &System,
    cached_exe_dir: &mut Option<PathBuf>,
) -> Option<(u32, u16, String, Option<String>)> {
    let exe_dir = if let Some(cached) = cached_exe_dir.clone() {
        let p = cached.join("lockfile");
        let alt = cached.parent().map(|par| par.join("lockfile"));
        if p.exists() || alt.is_some_and(|ap| ap.exists()) {
            cached
        } else {
            let fresh = find_lcu_exe_dir(sys)?;
            *cached_exe_dir = Some(fresh.clone());
            fresh
        }
    } else {
        let fresh = find_lcu_exe_dir(sys)?;
        *cached_exe_dir = Some(fresh.clone());
        fresh
    };

    let mut lockfile_path = exe_dir.join("lockfile");
    if !lockfile_path.exists() {
        if let Some(parent) = exe_dir.parent() {
            let backup_path = parent.join("lockfile");
            if backup_path.exists() {
                lockfile_path = backup_path;
            }
        }
    }

    let content = std::fs::read_to_string(&lockfile_path).ok().or_else(|| {
        *cached_exe_dir = None;
        None
    })?;

    let parts: Vec<&str> = content.trim().split(':').collect();
    if parts.len() < 4 {
        return None;
    }

    let pid: u32 = parts[1].parse().ok()?;
    if sys.process(sysinfo::Pid::from_u32(pid)).is_none() {
        *cached_exe_dir = None;
        return None;
    }
    let port: u16 = parts[2].parse().ok()?;
    let password = parts[3].to_string();
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
pub fn find_via_cmdline(sys: &System) -> Option<(u32, u16, String, Option<String>)> {
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
                continue;
            }

            if let Some((port, token, server)) = parse_cmdline_args(&cmd_str) {
                return Some((pid.as_u32(), port, token, server));
            }
        }
    }
    None
}

/// 解析命令行字符串提取 port, token, server
pub fn parse_cmdline_args(cmd_str: &str) -> Option<(u16, String, Option<String>)> {
    let mut port: Option<u16> = None;
    let mut token: Option<String> = None;
    let mut server: Option<String> = None;

    if let Some(p_idx) = cmd_str.find("--app-port=") {
        let sub = &cmd_str[p_idx + 11..];
        let end = sub.find(|c: char| !c.is_numeric()).unwrap_or(sub.len());
        port = sub[..end].parse::<u16>().ok();
    }

    if let Some(t_idx) = cmd_str.find("--remoting-auth-token=") {
        let sub = &cmd_str[t_idx + 22..];
        let end = sub.find([' ', '"', '\'']).unwrap_or(sub.len());
        token = Some(sub[..end].to_string());
    }

    if let Some(s_idx) = cmd_str.find("--rso_platform_id=") {
        let sub = &cmd_str[s_idx + 18..];
        let end = sub.find([' ', '"', '\'']).unwrap_or(sub.len());
        server = Some(sub[..end].to_string());
    }

    if let (Some(p), Some(t)) = (port, token) {
        Some((p, t, server))
    } else {
        None
    }
}

/// 方式三：通过 WMIC 获取命令行参数
pub async fn find_via_wmic() -> Option<(u32, u16, String, Option<String>)> {
    #[cfg(target_os = "windows")]
    {
        let output = tokio::process::Command::new("wmic")
            .args([
                "process",
                "WHERE",
                "name='LeagueClientUx.exe'",
                "GET",
                "commandline",
            ])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .output()
            .await
            .ok()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let port = regex_find_number(&stdout, &RE_APP_PORT)?;
        let token = regex_find_value(&stdout, &RE_AUTH_TOKEN)?;
        let server = regex_find_value(&stdout, &RE_PLATFORM_ID);

        log::debug!(
            "从 WMIC 解析结果: port={}, token=***, server={:?}",
            port,
            server
        );

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

static RE_APP_PORT: LazyLock<regex_lite::Regex> =
    LazyLock::new(|| regex_lite::Regex::new(r"--app-port=(\d+)").unwrap());
static RE_AUTH_TOKEN: LazyLock<regex_lite::Regex> =
    LazyLock::new(|| regex_lite::Regex::new(r#"--remoting-auth-token=([^"\s]+)"#).unwrap());
static RE_PLATFORM_ID: LazyLock<regex_lite::Regex> =
    LazyLock::new(|| regex_lite::Regex::new(r#"--rso_platform_id=([^"\s]+)"#).unwrap());

fn find_lcu_exe_dir(sys: &System) -> Option<PathBuf> {
    for process in sys.processes().values() {
        let name = process.name().to_string_lossy().to_lowercase();
        if name == "leagueclientux.exe" || name == "leagueclientux" {
            if let Some(exe_path) = process.exe() {
                return exe_path.parent().map(|p| p.to_path_buf());
            }
        }
    }
    None
}

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

pub fn shared_lcu_http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default()
    })
}

pub async fn probe_lcu_readiness(port: u16, token: &str) -> Result<(), String> {
    let auth = build_auth_header(token);
    let url = format!("https://127.0.0.1:{}/system/v1/builds", port);
    let http = shared_lcu_http_client();

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

#[cfg(target_os = "windows")]
fn get_cmdline_windows(pid: u32) -> Option<String> {
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Copy, Clone)]
    struct UNICODE_STRING {
        length: u16,
        _maximum_length: u16,
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

        let header_size = std::mem::size_of::<UNICODE_STRING>();
        if buffer.len() < header_size {
            return None;
        }

        let unicode_str = std::ptr::read_unaligned(buffer.as_ptr() as *const UNICODE_STRING);
        let byte_len = (unicode_str.length as usize) & !1;
        if byte_len == 0 {
            return None;
        }

        let decode = |start: usize, byte_len: usize| -> String {
            let units: Vec<u16> = buffer[start..start + byte_len]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&c| u16::from_le_bytes(c))
                .collect();
            String::from_utf16_lossy(&units)
        };

        let buf_start = buffer.as_ptr() as usize;
        let str_start = unicode_str.buffer as usize;

        if str_start >= buf_start {
            let offset = str_start - buf_start;
            if offset + byte_len <= buffer.len() {
                return Some(decode(offset, byte_len));
            }
        }

        if header_size + byte_len <= buffer.len() {
            return Some(decode(header_size, byte_len));
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cmdline_args() {
        let cmd = r#""C:\Riot Games\League of Legends\LeagueClientUx.exe" "--app-port=54321" "--remoting-auth-token=secret123" "--rso_platform_id=HN1""#;
        let res = parse_cmdline_args(cmd);
        assert_eq!(res, Some((54321, "secret123".to_string(), Some("HN1".to_string()))));

        let cmd2 = r#"--app-port=12345 --remoting-auth-token=abcd"#;
        let res2 = parse_cmdline_args(cmd2);
        assert_eq!(res2, Some((12345, "abcd".to_string(), None)));

        let cmd_invalid = r#"--foo=bar"#;
        assert_eq!(parse_cmdline_args(cmd_invalid), None);
    }

    #[test]
    fn test_sanitize_cmdline() {
        let cmd = "--app-port=12345 --remoting-auth-token=super_secret_token --other=1";
        let sanitized = sanitize_cmdline(cmd);
        assert!(!sanitized.contains("super_secret_token"));
        assert!(sanitized.contains("--remoting-auth-token=***"));
    }
}
