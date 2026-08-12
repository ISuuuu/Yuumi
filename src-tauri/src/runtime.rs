//! 便携版运行时状态。
//!
//! 便携版通过「exe 同目录存在 `portable.flag` 空文件」识别（仅 Windows）。
//! 便携模式下：
//! - 所有数据（配置/DB/缓存/WebView2 缓存）隔离到 `exe 旁/data/` 目录，实现绿色可迁移；
//! - 自动更新通过 `portable_updater.rs` 走 zip 覆盖流程（详见该模块）；
//! - Tauri identifier 动态化，避免与安装版或多份便携副本互相抢占单实例。
//!
//! `init()` 必须在一切业务逻辑（尤其 `config::AppConfig::load()`）之前调用，
//! 因为配置路径依赖此处解析出的数据目录。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

/// 便携版标记文件名（放 exe 同目录）
const PORTABLE_MARKER_FILE: &str = "portable.flag";

static PORTABLE: AtomicBool = AtomicBool::new(false);
static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 检测便携模式并解析数据目录。必须在一切逻辑之前调用一次（main.rs 第一行）。
pub fn init() {
    let portable = cfg!(windows)
        && current_exe_dir()
            .map(|d| d.join(PORTABLE_MARKER_FILE).exists())
            .unwrap_or(false);
    PORTABLE.store(portable, Ordering::SeqCst);

    let data_dir = if portable {
        current_exe_dir()
            .map(|d| d.join("data"))
            .unwrap_or_else(installed_data_dir)
    } else {
        installed_data_dir()
    };
    let _ = DATA_DIR.set(data_dir);

    if portable {
        let webview_dir = DATA_DIR.get().unwrap().join("webview2");
        if let Err(e) = std::fs::create_dir_all(&webview_dir) {
            eprintln!("创建 WebView2 数据目录失败: {e}");
        }
        // WebView2 缓存重定向：必须在 Tauri 初始化 WebView 线程之前设置。
        // 项目为 edition 2021，std::env::set_var 是安全操作。
        #[cfg(windows)]
        unsafe {
            std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &webview_dir);
        }
    }
}

/// 是否为便携版
#[tauri::command]
pub fn is_portable() -> bool {
    PORTABLE.load(Ordering::SeqCst)
}

/// 应用数据根目录：
/// - 安装版：`%APPDATA%/Yuumi`（与原行为完全一致）
/// - 便携版：`exe 旁/data`
pub fn app_data_dir() -> &'static Path {
    DATA_DIR.get().expect("runtime not initialized")
}

/// 便携实例标识：exe 路径的 FNV-1a 64 位哈希（与 `lcu/client.rs` 的 `stable_hash` 同算法），
/// 用于动态 Tauri identifier，跨进程稳定、不同路径互不相同。
pub fn portable_instance_id() -> String {
    let path = current_exe_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .to_string_lossy()
        .to_ascii_lowercase();
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in path.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    format!("{:016x}", hash)
}

/// exe 所在目录
fn current_exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
}

/// 安装版数据目录：`%APPDATA%/Yuumi`
fn installed_data_dir() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("Yuumi");
    path
}
