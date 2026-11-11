//! 运行时数据路径解析、便携版支持与全局 Tokio 运行时句柄

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use tokio::runtime::Handle;

const PORTABLE_MARKER_FILE: &str = "portable.flag";

static PORTABLE: AtomicBool = AtomicBool::new(false);
static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();
static TOKIO_HANDLE: OnceLock<Handle> = OnceLock::new();

/// 注册全局 Tokio 运行时句柄。
///
/// GPUI 的 `background_executor` 由 Windows 线程池驱动，线程上没有 Tokio reactor 上下文，
/// 在其中直接执行 reqwest 等网络 I/O 会 panic（"there is no reactor running"）。
/// 因此线程池任务中的异步 I/O 必须通过本句柄投递回运行时执行。
pub fn set_tokio_handle(handle: Handle) {
    if TOKIO_HANDLE.set(handle).is_err() {
        log::warn!("[Runtime] Tokio 运行时句柄已注册，忽略重复设置");
    }
}

/// 获取全局 Tokio 运行时句柄（未注册时返回 None）
pub fn tokio_handle() -> Option<Handle> {
    TOKIO_HANDLE.get().cloned()
}

/// 在全局运行时上投递后台任务；未注册运行时则记录错误并丢弃该任务
pub fn spawn_background<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    match tokio_handle() {
        Some(handle) => {
            handle.spawn(future);
        }
        None => log::error!("[Runtime] Tokio 运行时未初始化，后台任务被丢弃"),
    }
}

/// 初始化运行时环境
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
}

pub fn is_portable() -> bool {
    PORTABLE.load(Ordering::SeqCst)
}

pub fn app_data_dir() -> &'static Path {
    DATA_DIR.get_or_init(installed_data_dir)
}

pub fn current_exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
}

pub fn installed_data_dir() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("Yuumi");
    path
}
