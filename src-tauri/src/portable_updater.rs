//! 便携版自动更新（zip 覆盖方案）。
//!
//! 原理（参考 NyaTerm）：
//! - updater 插件无条件注册，便携版用 `windows-x86_64-portable` 作为 target key，
//!   从同一个 latest.json 拉取便携 zip（`check()` 遇 `TargetNotFound` 视为无更新）；
//! - 下载的 zip 解压到 `%TEMP%/yuumi-portable-update-<uuid>/payload`，
//!   只提取 `yuumi.exe` + `portable.flag`，跳过 `data/` 目录条目；
//! - helper 是「当前 exe 的拷贝」：`%TEMP%/.../yuumi-update-helper.exe`，
//!   以 `--yuumi-portable-update-helper <parentPid> <payload> <target> <workDir>` 参数
//!   spawn，主进程 `app.exit(0)`；helper 等父进程退出后原子替换 exe 并重启；
//! - helper 入口必须在 main() 最开头、runtime 初始化之前判定。

use std::ffi::OsString;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{Emitter, State};
use tauri_plugin_updater::Update;
use uuid::Uuid;

use crate::runtime;
use crate::updater::UpdateInfo;

const PORTABLE_ROOT: &str = "Yuumi-portable";
const PORTABLE_EXE: &str = "yuumi.exe";
const PORTABLE_MARKER: &str = "portable.flag";
const HELPER_FLAG: &str = "--yuumi-portable-update-helper";
const CLEANUP_ENV: &str = "YUUMI_PORTABLE_UPDATE_CLEANUP";
const WORK_DIR_PREFIX: &str = "yuumi-portable-update-";
const MAX_ARCHIVE_ENTRIES: usize = 128;
const MAX_PAYLOAD_BYTES: u64 = 512 * 1024 * 1024;
const STALE_WORK_DIR_AGE: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableUpdateInfo {
    pub version: String,
    pub date: Option<String>,
    pub body: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableUpdateProgress {
    downloaded: u64,
    total: u64,
}

#[derive(Debug)]
struct StagedPortableUpdate {
    work_dir: PathBuf,
    helper_exe: PathBuf,
    payload_exe: PathBuf,
    payload_marker: PathBuf,
}

#[derive(Default)]
pub struct PortableUpdateState {
    downloading: AtomicBool,
    staged: Mutex<Option<StagedPortableUpdate>>,
}

struct DownloadGuard<'a>(&'a AtomicBool);

impl Drop for DownloadGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// 检查便携版是否有新版本。
#[tauri::command]
pub async fn check_portable_update(
    app: tauri::AppHandle,
) -> Result<Option<PortableUpdateInfo>, String> {
    ensure_portable_runtime()?;
    Ok(portable_update(&app, std::env::consts::ARCH)
        .await?
        .map(|update| PortableUpdateInfo {
            version: update.version,
            date: update.date.and_then(|date| date.to_string().into()),
            body: update.body,
        }))
}

/// 启动时后台静默检查便携版更新，发现新版本则通知前端（不自动下载）。
pub async fn start_background_check(app: tauri::AppHandle) {
    let info = match check_portable_update(app.clone()).await {
        Ok(Some(info)) => info,
        Ok(None) => {
            log::info!("便携版后台更新检查：已是最新版本");
            return;
        }
        Err(e) => {
            log::warn!("便携版后台更新检查失败: {e}");
            return;
        }
    };
    let update_info = UpdateInfo {
        version: info.version,
        current_version: app.package_info().version.to_string(),
        notes: info.body,
        pub_date: info.date,
    };
    log::info!("便携版检查到新版本 v{}，通知前端", update_info.version);
    let _ = app.emit("updater://update-available", &update_info);
}

/// 下载便携版更新并解压暂存（通过 Channel 推送进度）。
#[tauri::command]
pub async fn download_portable_update(
    app: tauri::AppHandle,
    state: State<'_, PortableUpdateState>,
    on_progress: Channel<PortableUpdateProgress>,
) -> Result<(), String> {
    ensure_portable_runtime()?;
    ensure_install_directory_writable(&current_exe_dir()?)?;

    state
        .downloading
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| "便携版更新下载已在进行中，请稍候".to_string())?;
    let _guard = DownloadGuard(&state.downloading);

    let update = portable_update(&app, std::env::consts::ARCH)
        .await?
        .ok_or_else(|| "没有可用的便携版更新".to_string())?;
    let mut downloaded = 0_u64;
    let bytes = update
        .download(
            |chunk_len, total| {
                downloaded = downloaded.saturating_add(chunk_len as u64);
                let _ = on_progress.send(PortableUpdateProgress {
                    downloaded,
                    total: total.unwrap_or(0),
                });
            },
            || {},
        )
        .await
        .map_err(|e| format!("便携版更新下载或签名校验失败: {e}"))?;
    if bytes.len() as u64 > MAX_PAYLOAD_BYTES {
        return Err("便携版更新包大小超出允许上限".to_string());
    }
    let _ = on_progress.send(PortableUpdateProgress {
        downloaded: bytes.len() as u64,
        total: bytes.len() as u64,
    });

    let staged = stage_verified_archive(&bytes)?;
    let mut slot = state
        .staged
        .lock()
        .map_err(|_| "便携版更新状态不可用".to_string())?;
    if let Some(previous) = slot.replace(staged) {
        let _ = fs::remove_dir_all(previous.work_dir);
    }
    Ok(())
}

/// 应用已下载的便携版更新（spawn helper 后主进程退出）。
#[tauri::command]
pub fn apply_portable_update(
    app: tauri::AppHandle,
    state: State<'_, PortableUpdateState>,
) -> Result<(), String> {
    ensure_portable_runtime()?;
    if state.downloading.load(Ordering::Acquire) {
        return Err("便携版更新仍在下载中".to_string());
    }

    let staged = state
        .staged
        .lock()
        .map_err(|_| "便携版更新状态不可用".to_string())?
        .take()
        .ok_or_else(|| "没有已下载的便携版更新".to_string())?;

    if !staged.helper_exe.is_file()
        || !staged.payload_exe.is_file()
        || !staged.payload_marker.is_file()
    {
        let _ = fs::remove_dir_all(&staged.work_dir);
        return Err("便携版更新文件不完整".to_string());
    }

    let target_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let spawn_result = Command::new(&staged.helper_exe)
        .arg(HELPER_FLAG)
        .arg(std::process::id().to_string())
        .arg(&staged.payload_exe)
        .arg(&target_exe)
        .arg(&staged.work_dir)
        .spawn();

    if let Err(error) = spawn_result {
        let _ = fs::remove_dir_all(&staged.work_dir);
        return Err(error.to_string());
    }

    app.exit(0);
    Ok(())
}

fn ensure_portable_runtime() -> Result<(), String> {
    if !cfg!(windows) || !runtime::is_portable() {
        return Err("便携版更新仅支持 Windows 便携版".to_string());
    }
    Ok(())
}

/// 用 `windows-x86_64-portable` target 构建更新器并 check。
/// 未配置该 target（`TargetNotFound`/`TargetsNotFound`）时视为无更新。
async fn portable_update(app: &tauri::AppHandle, arch: &str) -> Result<Option<Update>, String> {
    let target = portable_target_for_arch(arch)?;
    let updater = crate::updater::updater_builder(app)
        .await?
        .target(target)
        .build()
        .map_err(|e| format!("便携版更新器初始化失败: {e}"))?;
    match updater.check().await {
        Ok(update) => Ok(update),
        Err(
            tauri_plugin_updater::Error::TargetNotFound(_)
            | tauri_plugin_updater::Error::TargetsNotFound(_),
        ) => Ok(None),
        Err(e) => Err(format!("便携版更新检查失败: {e}")),
    }
}

fn portable_target_for_arch(arch: &str) -> Result<&'static str, String> {
    match arch {
        "x86_64" => Ok("windows-x86_64-portable"),
        "aarch64" => Ok("windows-aarch64-portable"),
        other => Err(format!("不支持的便携版更新架构: {other}")),
    }
}

fn current_exe_dir() -> Result<PathBuf, String> {
    std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "无法定位可执行文件目录".to_string())
}

fn ensure_install_directory_writable(directory: &Path) -> Result<(), String> {
    let probe = directory.join(format!(".yuumi-update-write-test-{}", Uuid::new_v4()));
    fs::write(&probe, b"update-write-test").map_err(|e| e.to_string())?;
    fs::remove_file(probe).map_err(|e| e.to_string())?;
    Ok(())
}

fn stage_verified_archive(bytes: &[u8]) -> Result<StagedPortableUpdate, String> {
    let work_dir = std::env::temp_dir().join(format!("{WORK_DIR_PREFIX}{}", Uuid::new_v4()));
    fs::create_dir(&work_dir).map_err(|e| e.to_string())?;

    let result = (|| {
        let payload_dir = work_dir.join("payload");
        fs::create_dir(&payload_dir).map_err(|e| e.to_string())?;
        extract_portable_payload(bytes, &payload_dir)?;

        let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let helper_exe = work_dir.join("yuumi-update-helper.exe");
        fs::copy(current_exe, &helper_exe).map_err(|e| e.to_string())?;

        Ok(StagedPortableUpdate {
            payload_exe: payload_dir.join(PORTABLE_EXE),
            payload_marker: payload_dir.join(PORTABLE_MARKER),
            helper_exe,
            work_dir: work_dir.clone(),
        })
    })();

    if result.is_err() {
        let _ = fs::remove_dir_all(&work_dir);
    }
    result
}

/// 安全解压便携 zip：只提取 exe + portable.flag，跳过 data/，拒绝路径逃逸与符号链接。
fn extract_portable_payload(bytes: &[u8], destination: &Path) -> Result<(), String> {
    let cursor = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| format!("便携版更新包无效: {e}"))?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err("便携版更新包内文件数量超出上限".to_string());
    }

    let mut found_exe = false;
    let mut found_marker = false;
    let mut payload_bytes = 0_u64;

    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|e| format!("读取便携版更新包失败: {e}"))?;
        let enclosed = entry
            .enclosed_name()
            .ok_or_else(|| "便携版更新包含不安全路径".to_string())?;
        let mut components = enclosed.components();
        if components.next() != Some(Component::Normal(PORTABLE_ROOT.as_ref())) {
            return Err("便携版更新包根目录不符合预期".to_string());
        }
        let mut relative = PathBuf::new();
        for component in components {
            let Component::Normal(name) = component else {
                return Err("便携版更新包含不安全相对路径".to_string());
            };
            relative.push(name);
        }
        if relative.as_os_str().is_empty() || entry.is_dir() {
            continue;
        }
        if entry.is_symlink() {
            return Err("便携版更新包含符号链接".to_string());
        }

        if relative.starts_with("data") {
            continue;
        }

        let output = if relative == Path::new(PORTABLE_EXE) {
            if found_exe {
                return Err("便携版更新包包含重复的 yuumi.exe".to_string());
            }
            found_exe = true;
            destination.join(PORTABLE_EXE)
        } else if relative == Path::new(PORTABLE_MARKER) {
            if found_marker {
                return Err("便携版更新包包含重复的 portable.flag".to_string());
            }
            found_marker = true;
            destination.join(PORTABLE_MARKER)
        } else {
            return Err(format!("便携版更新包包含意外文件: {}", relative.display()));
        };

        payload_bytes = payload_bytes.saturating_add(entry.size());
        if payload_bytes > MAX_PAYLOAD_BYTES {
            return Err("便携版更新包大小超出允许上限".to_string());
        }

        let mut file = fs::File::create(output).map_err(|e| e.to_string())?;
        let copied = std::io::copy(&mut entry.take(MAX_PAYLOAD_BYTES + 1), &mut file)
            .map_err(|e| e.to_string())?;
        if copied > MAX_PAYLOAD_BYTES {
            return Err("便携版更新包单文件超出允许上限".to_string());
        }
    }

    if !found_exe || !found_marker {
        return Err("便携版更新包缺少 yuumi.exe 或 portable.flag".to_string());
    }
    Ok(())
}

/// main() 最开头调用。若当前进程是更新 helper 则执行替换并返回 true。
pub fn run_helper_if_requested() -> bool {
    let args: Vec<OsString> = std::env::args_os().collect();
    if args.get(1).and_then(|arg| arg.to_str()) != Some(HELPER_FLAG) {
        return false;
    }

    if let Err(error) = run_helper(&args) {
        let target = args.get(4).map(PathBuf::from);
        if let Some(target_exe) = target.as_deref() {
            write_helper_error(target_exe, &error);
            let mut command = Command::new(target_exe);
            if let Some(work_dir) = args.get(5) {
                command.env(CLEANUP_ENV, work_dir);
            }
            let _ = command.spawn();
        }
    }
    true
}

fn run_helper(args: &[OsString]) -> Result<(), String> {
    if args.len() != 6 {
        return Err("便携版更新 helper 参数无效".to_string());
    }
    let parent_pid = args[2]
        .to_str()
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| "便携版更新父进程 ID 无效".to_string())?;
    let source_exe = PathBuf::from(&args[3]);
    let target_exe = PathBuf::from(&args[4]);
    let work_dir = PathBuf::from(&args[5]);

    wait_for_process_exit(parent_pid)?;
    replace_executable(&source_exe, &target_exe)?;
    Command::new(&target_exe)
        .env(CLEANUP_ENV, &work_dir)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn replace_executable(source_exe: &Path, target_exe: &Path) -> Result<(), String> {
    let target_dir = target_exe
        .parent()
        .ok_or_else(|| "便携版可执行文件无上级目录".to_string())?;
    let new_exe = target_dir.join(".yuumi-update-new.exe");
    let backup_exe = target_dir.join(".yuumi-update-backup.exe");
    let _ = fs::remove_file(&new_exe);
    let _ = fs::remove_file(&backup_exe);
    fs::copy(source_exe, &new_exe).map_err(|e| e.to_string())?;

    commit_executable(&new_exe, target_exe, &backup_exe, |from, to| {
        fs::rename(from, to)
    })?;
    let _ = fs::remove_file(backup_exe);
    Ok(())
}

fn commit_executable<F>(
    new_exe: &Path,
    target_exe: &Path,
    backup_exe: &Path,
    move_new: F,
) -> Result<(), String>
where
    F: FnOnce(&Path, &Path) -> std::io::Result<()>,
{
    fs::rename(target_exe, backup_exe).map_err(|e| e.to_string())?;
    if let Err(error) = move_new(new_exe, target_exe) {
        if let Err(rollback_error) = fs::rename(backup_exe, target_exe) {
            return Err(format!(
                "便携版更新安装失败 ({error})，且恢复原 exe 失败 ({rollback_error})"
            ));
        }
        return Err(error.to_string());
    }
    Ok(())
}

#[cfg(windows)]
fn wait_for_process_exit(process_id: u32) -> Result<(), String> {
    use windows::Win32::Foundation::{CloseHandle, E_INVALIDARG, WAIT_OBJECT_0};
    use windows::Win32::System::Threading::{
        OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
    };

    let process = match unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, process_id) } {
        Ok(process) => process,
        Err(error) if error.code() == E_INVALIDARG => return Ok(()),
        Err(error) => {
            return Err(format!("打开父进程失败: {error}"));
        }
    };
    let result = unsafe { WaitForSingleObject(process, 120_000) };
    let _ = unsafe { CloseHandle(process) };
    if result != WAIT_OBJECT_0 {
        return Err("等待 Yuumi 主进程退出超时".to_string());
    }
    Ok(())
}

#[cfg(not(windows))]
fn wait_for_process_exit(_process_id: u32) -> Result<(), String> {
    Err("便携版更新 helper 仅支持 Windows".to_string())
}

fn write_helper_error(target_exe: &Path, message: &str) {
    let Some(target_dir) = target_exe.parent() else {
        return;
    };
    let log_dir = target_dir.join("data").join("logs");
    if fs::create_dir_all(&log_dir).is_ok() {
        let _ = fs::write(log_dir.join("portable-update-error.log"), message);
    }
}

/// run() 开头调用（不阻塞）。清理上次更新残留的 work_dir 与过期目录。
pub fn schedule_cleanup_from_environment() {
    let explicit_cleanup = std::env::var_os(CLEANUP_ENV).and_then(|raw_path| {
        unsafe {
            std::env::remove_var(CLEANUP_ENV);
        }
        let path = PathBuf::from(raw_path);
        is_portable_work_dir(&path).then_some(path)
    });

    std::thread::spawn(move || {
        if let Some(path) = explicit_cleanup {
            std::thread::sleep(Duration::from_secs(3));
            let _ = fs::remove_dir_all(path);
        }
        cleanup_stale_work_dirs();
    });
}

fn is_portable_work_dir(path: &Path) -> bool {
    path.parent() == Some(std::env::temp_dir().as_path())
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(WORK_DIR_PREFIX))
}

fn cleanup_stale_work_dirs() {
    let Ok(entries) = fs::read_dir(std::env::temp_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_stale_directory = entry.file_type().is_ok_and(|kind| kind.is_dir())
            && is_portable_work_dir(&path)
            && entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .ok()
                .and_then(|modified| modified.elapsed().ok())
                .is_some_and(|age| age >= STALE_WORK_DIR_AGE);
        if is_stale_directory {
            let _ = fs::remove_dir_all(path);
        }
    }
}

/// 校验可执行文件待替换（供测试复用）
#[cfg(test)]
pub(crate) fn test_commit_executable<F>(
    new_exe: &Path,
    target_exe: &Path,
    backup_exe: &Path,
    move_new: F,
) -> Result<(), String>
where
    F: FnOnce(&Path, &Path) -> std::io::Result<()>,
{
    commit_executable(new_exe, target_exe, backup_exe, move_new)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use zip::write::SimpleFileOptions;

    use super::*;

    fn test_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("yuumi-{name}-{}", Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        path
    }

    fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        for (name, bytes) in entries {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn maps_supported_windows_architectures() {
        assert_eq!(
            portable_target_for_arch("x86_64").unwrap(),
            "windows-x86_64-portable"
        );
        assert_eq!(
            portable_target_for_arch("aarch64").unwrap(),
            "windows-aarch64-portable"
        );
        assert!(portable_target_for_arch("x86").is_err());
    }

    #[test]
    fn extracts_program_files_without_touching_data() {
        let destination = test_dir("portable-extract");
        let bytes = archive(&[
            ("Yuumi-portable/yuumi.exe", b"new-exe"),
            ("Yuumi-portable/portable.flag", b""),
            ("Yuumi-portable/data/.keep", b"package-data"),
        ]);

        extract_portable_payload(&bytes, &destination).unwrap();

        assert_eq!(
            fs::read(destination.join(PORTABLE_EXE)).unwrap(),
            b"new-exe"
        );
        assert!(destination.join(PORTABLE_MARKER).is_file());
        assert!(!destination.join("data").exists());
        fs::remove_dir_all(destination).unwrap();
    }

    #[test]
    fn rejects_unsafe_or_incomplete_archives() {
        let destination = test_dir("portable-invalid");
        let unsafe_archive = archive(&[
            ("../yuumi.exe", b"bad"),
            ("Yuumi-portable/portable.flag", b""),
        ]);
        assert!(extract_portable_payload(&unsafe_archive, &destination).is_err());

        let nested_escape = archive(&[
            ("Yuumi-portable/yuumi.exe", b"new-exe"),
            ("Yuumi-portable/portable.flag", b""),
            ("Yuumi-portable/data/../../escape", b"bad"),
        ]);
        assert!(extract_portable_payload(&nested_escape, &destination).is_err());

        let missing_marker = archive(&[("Yuumi-portable/yuumi.exe", b"new-exe")]);
        assert!(extract_portable_payload(&missing_marker, &destination).is_err());
        fs::remove_dir_all(destination).unwrap();
    }

    #[test]
    fn executable_commit_rolls_back_when_final_move_fails() {
        let directory = test_dir("portable-rollback");
        let target = directory.join(PORTABLE_EXE);
        let new_exe = directory.join("new.exe");
        let backup = directory.join("backup.exe");
        fs::write(&target, b"old").unwrap();
        fs::write(&new_exe, b"new").unwrap();

        let result = test_commit_executable(&new_exe, &target, &backup, |_, _| {
            Err(std::io::Error::other("simulated failure"))
        });

        assert!(result.is_err());
        assert_eq!(fs::read(&target).unwrap(), b"old");
        fs::remove_dir_all(directory).unwrap();
    }
}
