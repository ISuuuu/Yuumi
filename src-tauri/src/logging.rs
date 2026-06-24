use flexi_logger::{Cleanup, Criterion, Duplicate, FileSpec, Logger, Naming};
use log::LevelFilter;

/// 初始化日志系统。
/// - 日志文件写入 `<exe_dir>/log/` 目录（按天轮转，保留 30 天）
/// - Debug 模式下同时输出到控制台
/// - log_level: 20=Debug, 30=Warn, 40=Info
pub fn init(log_level: u32) {
    let level = match log_level {
        0..=20 => LevelFilter::Debug,
        21..=30 => LevelFilter::Warn,
        31..=40 => LevelFilter::Info,
        _ => LevelFilter::Info,
    };

    // 日志目录：<exe_dir>/log/
    let log_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("log")))
        .unwrap_or_else(|| std::path::PathBuf::from("log"));

    let duplicate = if cfg!(debug_assertions) {
        Duplicate::Info  // Debug 模式：控制台也输出 Info 及以上
    } else {
        Duplicate::None  // Release 模式：仅文件
    };

    match Logger::try_with_str(level.to_string())
        .unwrap()
        .log_to_file(
            FileSpec::default()
                .directory(&log_dir)
                .basename("yuumi")
                .suffix("log"),
        )
        .duplicate_to_stderr(duplicate)
        .rotate(
            Criterion::Age(flexi_logger::Age::Day),
            Naming::Timestamps,
            Cleanup::KeepLogFiles(30),
        )
        .start()
    {
        Ok(_) => {
            log::info!("日志系统已启动，级别={:?}，目录={}", level, log_dir.display());
        }
        Err(e) => {
            eprintln!("日志系统初始化失败: {}", e);
        }
    }
}
