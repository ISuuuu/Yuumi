use chrono::Local;
use log::{LevelFilter, Log, Metadata, Record};
use std::fs::{self, File, OpenOptions};
use std::io::{Result as IoResult, Write};
use std::path::PathBuf;
use std::sync::Mutex;

/// 自动轮转的日志文件写入器。
/// - 按天命名：`yuumi_2026-06-25.log`
/// - 单文件最大 2MB，超出后后缀递增：`yuumi_2026-06-25-1.log`
struct RotatingFileWriter {
    dir: PathBuf,
    basename: String,
    suffix: String,
    max_size: u64,
    max_files: u32,
    current_date: String,
    current_index: u32,
    current_size: u64,
    file: Option<File>,
}

impl RotatingFileWriter {
    fn new(dir: PathBuf, max_size: u64, max_files: u32) -> Self {
        Self {
            dir,
            basename: "yuumi".into(),
            suffix: "log".into(),
            max_size,
            max_files,
            current_date: String::new(),
            current_index: 0,
            current_size: 0,
            file: None,
        }
    }

    /// 文件名：`yuumi_2026-06-25.log` 或 `yuumi_2026-06-25-1.log`
    fn make_filename(&self, date: &str, index: u32) -> String {
        if index == 0 {
            format!("{}_{}.{}", self.basename, date, self.suffix)
        } else {
            format!("{}_{}-{}.{}", self.basename, date, index, self.suffix)
        }
    }

    fn open_file(&mut self) -> IoResult<()> {
        fs::create_dir_all(&self.dir)?;
        let name = self.make_filename(&self.current_date, self.current_index);
        let path = self.dir.join(&name);
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        self.current_size = file.metadata().map(|m| m.len()).unwrap_or(0);
        self.file = Some(file);
        Ok(())
    }

    /// 清理超出保留天数的旧日志文件
    fn cleanup_old_files(&self) {
        let cutoff = Local::now().date_naive() - chrono::Duration::days(self.max_files as i64);
        let prefix = format!("{}_", self.basename);
        let ext = format!(".{}", self.suffix);

        let Ok(entries) = fs::read_dir(&self.dir) else { return };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with(&prefix) || !name.ends_with(&ext) {
                continue;
            }
            // 提取日期：yuumi_YYYY-MM-DD[-N].log → 取 YYYY-MM-DD（前 10 字符）
            let middle = &name[prefix.len()..name.len() - ext.len()];
            if middle.len() >= 10 {
                let date_part = &middle[..10];
                if let Ok(file_date) = chrono::NaiveDate::parse_from_str(date_part, "%Y-%m-%d") {
                    if file_date < cutoff {
                        let _ = fs::remove_file(entry.path());
                    }
                }
            }
        }
    }

    fn write_log(&mut self, record: &Record) -> IoResult<()> {
        let now = Local::now();
        let today = now.format("%Y-%m-%d").to_string();

        // 日期变更 → 新文件
        if today != self.current_date {
            self.current_date = today;
            self.current_index = 0;
            self.file = None;
            self.cleanup_old_files();
        }

        if self.file.is_none() {
            self.open_file()?;
        }

        // 超过大小限制 → 轮转
        if self.current_size >= self.max_size {
            self.current_index += 1;
            self.file = None;
            self.open_file()?;
        }

        let ts = now.format("%Y-%m-%d %H:%M:%S%.3f");
        let level = record.level();
        let module = record.module_path_static().unwrap_or("unknown");
        let line = record.line().unwrap_or(0);

        let msg = format!(
            "{} [{:<5}] {}:{} - {}\n",
            ts, level, module, line, record.args()
        );

        if let Some(ref mut f) = self.file {
            f.write_all(msg.as_bytes())?;
            f.flush()?;
            self.current_size += msg.len() as u64;
        }

        Ok(())
    }
}

/// 全局日志器（文件 + 可选 stderr）
struct Logger {
    file: Mutex<RotatingFileWriter>,
    stderr_level: LevelFilter,
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        // 写入文件
        if let Ok(mut writer) = self.file.lock() {
            let _ = writer.write_log(record);
        }

        // Debug 模式同时输出到 stderr
        if cfg!(debug_assertions) && record.level() <= self.stderr_level {
            let ts = Local::now().format("%H:%M:%S%.3f");
            eprintln!(
                "{} [{:<5}] {} - {}",
                ts,
                record.level(),
                record.module_path_static().unwrap_or("unknown"),
                record.args()
            );
        }
    }

    fn flush(&self) {
        if let Ok(writer) = self.file.lock() {
            if let Some(ref f) = writer.file {
                let _ = f.sync_data();
            }
        }
    }
}

/// 初始化日志系统。
/// - 日志文件：`<exe_dir>/log/yuumi_2026-06-25.log`，超过 2MB 后 `yuumi_2026-06-25-1.log`
/// - 日志内容：`2026-06-25 11:55:26.123 [INFO ] module:42 - message`
/// - Debug 模式下同时输出到 stderr
/// - log_level: 20=Debug, 30=Warn, 40=Info
pub fn init(log_level: u32) {
    let level = match log_level {
        0 | 10 => LevelFilter::Debug,
        1 | 20 | 30 | 40 => LevelFilter::Info,
        2 => LevelFilter::Error,
        _ => LevelFilter::Debug, // Default to Debug as requested
    };

    let log_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("log")))
        .unwrap_or_else(|| PathBuf::from("log"));

    let logger = Logger {
        file: Mutex::new(RotatingFileWriter::new(
            log_dir.clone(),
            2_000_000, // 单文件最大 2MB
            30,        // 保留最近 30 天
        )),
        stderr_level: LevelFilter::Info,
    };

    log::set_max_level(level);
    match log::set_boxed_logger(Box::new(logger)) {
        Ok(_) => {
            log::info!("日志系统已启动，级别={:?}，目录={}", level, log_dir.display());
        }
        Err(e) => {
            eprintln!("日志系统初始化失败: {}", e);
        }
    }
}
