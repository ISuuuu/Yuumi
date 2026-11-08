//! 运行时数据路径解析与便携版支持

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

const PORTABLE_MARKER_FILE: &str = "portable.flag";

static PORTABLE: AtomicBool = AtomicBool::new(false);
static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

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
