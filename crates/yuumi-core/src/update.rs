//! Release checks and cached release notes. Supports standard Windows targets
//! (`windows-x86_64`, `windows-x86_64-portable`).
use base64::{engine::general_purpose::STANDARD, Engine};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path, time::Duration};

const MAX_PACKAGE: usize = 512 * 1024 * 1024;
pub const RELEASES_URL: &str = "https://api.github.com/repos/ISuuuu/Yuumi/releases";

const DEFAULT_UPDATE_ENDPOINT: &str =
    "https://github.com/ISuuuu/Yuumi/releases/latest/download/latest.json";
const DEFAULT_UPDATE_PUBLIC_KEY: &str =
    "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDgzQjk5RURCMTE1MTM5MEIKUldRTE9WRVIyNTY1Z3pVZWRIQzh1bythRWZ3bWJsdTdoeS9uUTEvL0ZBa3I1bXZEd1o3cWpRbTYK";

/// Use the default release manifest and signing key unless overridden at build time.
pub fn update_channel() -> Result<(String, String), String> {
    let endpoint = option_env!("YUUMI_UPDATE_ENDPOINT")
        .or(option_env!("YUUMI_GPUI_UPDATE_ENDPOINT"))
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_UPDATE_ENDPOINT);
    let key = option_env!("YUUMI_UPDATE_PUBLIC_KEY")
        .or(option_env!("YUUMI_GPUI_UPDATE_PUBLIC_KEY"))
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_UPDATE_PUBLIC_KEY);
    Ok((endpoint.into(), key.into()))
}

/// Backward compatible alias for `update_channel`.
pub fn gpui_update_channel() -> Result<(String, String), String> {
    update_channel()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    pub notes: Option<String>,
    pub pub_date: Option<String>,
    pub url: String,
    pub signature: String,
    pub target: String,
}
#[derive(Deserialize)]
struct Manifest {
    version: String,
    notes: Option<String>,
    pub_date: Option<String>,
    platforms: HashMap<String, Artifact>,
}
#[derive(Deserialize)]
struct Artifact {
    url: String,
    signature: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReleaseEntry {
    pub tag: String,
    pub published_at: String,
    pub body: String,
}
#[derive(Deserialize)]
struct GithubRelease {
    #[serde(default)]
    tag_name: String,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    body: Option<String>,
}

fn trusted_transport(address: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(address) else {
        return false;
    };
    (url.scheme() == "https"
        || (url.scheme() == "http"
            && matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))))
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
}

fn is_supported_target(target: &str) -> bool {
    matches!(
        target,
        "windows-x86_64"
            | "windows-x86_64-portable"
            | "windows-aarch64"
            | "windows-aarch64-portable"
    )
}

pub struct UpdateService {
    client: reqwest::Client,
    endpoint: String,
    current_version: String,
    target: String,
    public_key: String,
}
impl UpdateService {
    pub fn new(
        client: reqwest::Client,
        endpoint: String,
        current_version: String,
        target: String,
        public_key: String,
    ) -> Self {
        Self {
            client,
            endpoint,
            current_version,
            target,
            public_key,
        }
    }
    pub async fn check(&self) -> Result<Option<UpdateInfo>, String> {
        if !is_supported_target(&self.target) {
            return Err("更新目标不是受支持的构建".into());
        }
        if self.endpoint.is_empty() || self.public_key.is_empty() {
            return Err("尚未配置可信的更新渠道和公钥".into());
        }
        if !trusted_transport(&self.endpoint) {
            return Err("更新清单地址必须使用 HTTPS".into());
        }
        let manifest: Manifest = self
            .client
            .get(&self.endpoint)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| format!("更新清单无效: {e}"))?;
        let current = semver::Version::parse(self.current_version.trim_start_matches('v'))
            .map_err(|e| e.to_string())?;
        let version = semver::Version::parse(manifest.version.trim_start_matches('v'))
            .map_err(|e| format!("更新版本无效: {e}"))?;
        if version <= current {
            return Ok(None);
        }
        let artifact = manifest
            .platforms
            .get(&self.target)
            .ok_or("更新清单没有匹配的构建")?;
        if !trusted_transport(&artifact.url) || artifact.signature.is_empty() {
            return Err("更新包地址或签名为空".into());
        }
        Ok(Some(UpdateInfo {
            version: manifest.version,
            current_version: self.current_version.clone(),
            notes: manifest.notes,
            pub_date: manifest.pub_date,
            url: artifact.url.clone(),
            signature: artifact.signature.clone(),
            target: self.target.clone(),
        }))
    }
    pub async fn download<F: FnMut(u64, Option<u64>)>(
        &self,
        info: &UpdateInfo,
        mut progress: F,
    ) -> Result<VerifiedUpdate, String> {
        if info.target != self.target
            || !is_supported_target(&self.target)
            || self.public_key.is_empty()
        {
            return Err("更新目标或可信公钥无效".into());
        }
        if !trusted_transport(&info.url) {
            return Err("更新包地址必须使用 HTTPS".into());
        }
        let response = self
            .client
            .get(&info.url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        let total = response.content_length();
        if total.is_some_and(|size| size > MAX_PACKAGE as u64) {
            return Err("更新包超出大小限制".into());
        }
        let mut stream = response.bytes_stream();
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| e.to_string())?;
            if bytes.len().saturating_add(chunk.len()) > MAX_PACKAGE {
                return Err("更新包超出大小限制".into());
            }
            bytes.extend_from_slice(&chunk);
            progress(bytes.len() as u64, total);
        }
        verify(&self.public_key, &info.signature, &bytes)?;
        Ok(VerifiedUpdate {
            info: info.clone(),
            bytes,
            public_key: self.public_key.clone(),
        })
    }
}
/// Only this module can construct a verified package; no installation occurs here.
pub struct VerifiedUpdate {
    pub info: UpdateInfo,
    bytes: Vec<u8>,
    public_key: String,
}
impl VerifiedUpdate {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
fn verify(public_key: &str, signature: &str, bytes: &[u8]) -> Result<(), String> {
    // Tauri stores both minisign documents as base64-encoded text.
    let key_text = String::from_utf8(STANDARD.decode(public_key).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let key_line = key_text.lines().nth(1).ok_or("公钥格式无效")?;
    let key =
        minisign_verify::PublicKey::from_base64(key_line).map_err(|e| format!("公钥无效: {e}"))?;
    let signature_text = String::from_utf8(STANDARD.decode(signature).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let sig = minisign_verify::Signature::decode(&signature_text)
        .map_err(|e| format!("签名无效: {e}"))?;
    key.verify(bytes, &sig, false)
        .map_err(|e| format!("更新签名校验失败: {e}"))
}

/// 读取更新日志磁盘缓存；`max_age` 限定新鲜度，`require_current` 要求缓存包含当前版本。
fn read_cached_entries(
    cache_path: &Path,
    max_age: Option<Duration>,
    current_version: Option<&str>,
    require_current: bool,
) -> Option<Vec<ReleaseEntry>> {
    let text = std::fs::read_to_string(cache_path).ok()?;
    let entries = serde_json::from_str::<Vec<ReleaseEntry>>(&text).ok()?;
    if entries.is_empty() {
        return None;
    }
    if let Some(max_age) = max_age {
        let fresh = std::fs::metadata(cache_path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age < max_age);
        if !fresh {
            return None;
        }
    }
    if require_current {
        let contains_current = current_version.filter(|s| !s.is_empty()).is_none_or(|v| {
            entries
                .iter()
                .any(|e| e.tag.trim_start_matches('v') == v.trim_start_matches('v'))
        });
        if !contains_current {
            return None;
        }
    }
    Some(entries)
}

pub async fn release_changelog(
    client: &reqwest::Client,
    url: &str,
    cache_path: &Path,
    current_version: Option<&str>,
    force_refresh: bool,
) -> Result<Vec<ReleaseEntry>, String> {
    if !force_refresh {
        if let Some(entries) = read_cached_entries(
            cache_path,
            Some(Duration::from_secs(86400)),
            current_version,
            true,
        ) {
            return Ok(entries);
        }
    }
    let fetch = async {
        let response = client
            .get(url)
            .header(reqwest::header::USER_AGENT, "Yuumi")
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let response = response.error_for_status().map_err(|e| e.to_string())?;
        response
            .json::<Vec<GithubRelease>>()
            .await
            .map_err(|e| format!("解析版本数据失败: {e}"))
    };
    let releases: Vec<GithubRelease> = match fetch.await {
        Ok(releases) => releases,
        Err(e) => {
            // GitHub 不可达：常规加载兜底使用本地缓存（允许过期、不要求包含当前版本），
            // 显式刷新仍报错以提示用户网络异常
            if !force_refresh {
                if let Some(entries) = read_cached_entries(cache_path, None, current_version, false)
                {
                    return Ok(entries);
                }
            }
            return Err(e);
        }
    };
    let entries: Vec<ReleaseEntry> = releases
        .into_iter()
        .map(|r| ReleaseEntry {
            tag: r.tag_name,
            published_at: r.published_at.unwrap_or_default(),
            body: r.body.unwrap_or_default(),
        })
        .collect();
    if let Some(parent) = cache_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            log::warn!("创建版本日志缓存目录失败: {e}");
        }
    }
    if let Ok(json) = serde_json::to_vec(&entries) {
        if let Err(e) = std::fs::write(cache_path, json) {
            log::warn!("写入版本日志缓存失败: {e}");
        }
    }
    Ok(entries)
}

impl VerifiedUpdate {
    /// Stage only the signed installer in an isolated directory chosen by the caller.
    pub fn stage_installer(&self, directory: &Path) -> Result<StagedInstaller, String> {
        if self.info.target.ends_with("-portable") {
            return Err("便携版更新尚不支持安装".into());
        }
        if !matches!(
            self.info.target.as_str(),
            "windows-x86_64" | "windows-aarch64"
        ) {
            return Err("不支持的安装版更新目标".into());
        }
        std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
        let path = directory.join("yuumi-installer.exe");
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| format!("暂存安装程序失败: {e}"))?;
        if let Err(e) = file.write_all(&self.bytes).and_then(|_| file.sync_all()) {
            let _ = std::fs::remove_file(&path);
            return Err(e.to_string());
        }
        Ok(StagedInstaller {
            path,
            bytes: self.bytes.clone(),
        })
    }
}
/// A staged installer retains the verified bytes and refuses a changed file.
pub struct StagedInstaller {
    path: std::path::PathBuf,
    bytes: Vec<u8>,
}
impl StagedInstaller {
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn launch(&self, portable_runtime: bool) -> Result<(), String> {
        if portable_runtime {
            return Err("便携版不能运行安装版更新".into());
        }
        #[cfg(windows)]
        {
            use std::io::Read;
            use std::os::windows::fs::OpenOptionsExt;
            // Keep the file open without write/delete sharing until CreateProcess returns.
            let mut locked = std::fs::OpenOptions::new()
                .read(true)
                .share_mode(1)
                .open(&self.path)
                .map_err(|e| e.to_string())?;
            let mut staged = Vec::new();
            locked.read_to_end(&mut staged).map_err(|e| e.to_string())?;
            if staged != self.bytes {
                return Err("暂存的更新文件已被修改".into());
            }
            std::process::Command::new(&self.path)
                .args(["/P", "/R"])
                .spawn()
                .map_err(|e| format!("启动安装程序失败: {e}"))?;
        }
        #[cfg(not(windows))]
        return Err("安装版更新仅支持 Windows".into());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn verifies_minisign_and_rejects_modified_payload() {
        let key = STANDARD.encode("untrusted comment: minisign public key\nRWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3\n");
        let signature = STANDARD.encode("untrusted comment: signature from minisign secret key\nRUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=\ntrusted comment: timestamp:1633700835\tfile:test\tprehashed\nwLMDjy9FLAuxZ3q4NlEvkgtyhrr0gtTu6KC4KBJdITbbOeAi1zBIYo0v4iTgt8jJpIidRJnp94ABQkJAgAooBQ==");
        assert!(verify(&key, &signature, b"test").is_ok());
        assert!(verify(&key, &signature, b"changed").is_err());
        assert!(verify(&key, "invalid", b"test").is_err());
    }
    #[tokio::test]
    async fn refuses_unknown_target_and_missing_trust_configuration() {
        let client = reqwest::Client::new();
        let unknown = UpdateService::new(
            client.clone(),
            "".into(),
            "0.1.0".into(),
            "windows-unknown".into(),
            "".into(),
        );
        assert!(unknown.check().await.is_err());
        let valid = UpdateService::new(
            client,
            "".into(),
            "0.1.0".into(),
            "windows-x86_64".into(),
            "".into(),
        );
        assert!(valid.check().await.is_err());
    }

    #[test]
    fn extracts_yuumi_exe_from_portable_zip() {
        use std::io::Write;
        use zip::write::SimpleFileOptions;
        let cursor = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        writer
            .start_file("Yuumi-portable/yuumi.exe", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"MZfakeexecutable").unwrap();
        writer
            .start_file("Yuumi-portable/portable.flag", SimpleFileOptions::default())
            .unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        let extracted = extract_exe_from_portable_zip(&bytes).unwrap();
        assert_eq!(extracted, b"MZfakeexecutable");
    }
}

/// Verified portable update package (raw exe or zip) staged beside a helper copy.
pub struct StagedPortable {
    payload: std::path::PathBuf,
    helper: std::path::PathBuf,
    public_key: String,
    signature: String,
}
impl VerifiedUpdate {
    pub fn stage_portable(&self, directory: &Path) -> Result<StagedPortable, String> {
        if !matches!(
            self.info.target.as_str(),
            "windows-x86_64-portable" | "windows-aarch64-portable"
        ) {
            return Err("更新目标不是便携版".into());
        }
        if self.bytes.starts_with(b"MZ") {
            // Raw signed executable
        } else if self.bytes.starts_with(b"PK\x03\x04") {
            // Validate portable zip contains yuumi.exe
            let _ = extract_exe_from_portable_zip(&self.bytes)?;
        } else {
            return Err("便携版更新包格式无效".into());
        }
        std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
        let payload = directory.join("yuumi-payload.bin");
        let helper = directory.join("yuumi-helper.exe");
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&payload)
            .map_err(|e| e.to_string())?;
        if let Err(e) = file.write_all(&self.bytes).and_then(|_| file.sync_all()) {
            let _ = std::fs::remove_file(&payload);
            return Err(e.to_string());
        }
        if let Err(e) = std::fs::copy(std::env::current_exe().map_err(|e| e.to_string())?, &helper)
        {
            let _ = std::fs::remove_file(&payload);
            return Err(e.to_string());
        }
        Ok(StagedPortable {
            payload,
            helper,
            public_key: self.public_key.clone(),
            signature: self.info.signature.clone(),
        })
    }
}
impl StagedPortable {
    /// On success the caller must exit immediately so the helper can replace it.
    pub fn launch(&self, portable_runtime: bool) -> Result<(), String> {
        if !portable_runtime || !cfg!(windows) {
            return Err("便携版更新仅支持 Windows 便携运行环境".into());
        }
        let target = std::env::current_exe().map_err(|e| e.to_string())?;
        if !target
            .parent()
            .is_some_and(|p| p.join("portable.flag").is_file())
        {
            return Err("缺少便携版标记".into());
        }
        std::process::Command::new(&self.helper)
            .arg("--yuumi-update-helper")
            .arg(std::process::id().to_string())
            .arg(&self.payload)
            .arg(&target)
            .arg(&self.public_key)
            .arg(&self.signature)
            .spawn()
            .map_err(|e| format!("启动便携更新助手失败: {e}"))?;
        Ok(())
    }
}

fn extract_exe_from_portable_zip(bytes: &[u8]) -> Result<Vec<u8>, String> {
    use std::io::{Cursor, Read};
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("解析便携 zip 失败: {e}"))?;
    if archive.len() > 128 {
        return Err("便携版更新包内文件数量超出上限".into());
    }
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| format!("读取便携 zip 条目失败: {e}"))?;
        let Some(enclosed) = entry.enclosed_name() else {
            return Err("便携版更新包含不安全路径".into());
        };
        if entry.is_dir() || entry.is_symlink() {
            continue;
        }
        let file_name = enclosed
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.to_ascii_lowercase());
        if matches!(file_name.as_deref(), Some("yuumi.exe")) {
            let mut extracted = Vec::new();
            entry
                .take((MAX_PACKAGE + 1) as u64)
                .read_to_end(&mut extracted)
                .map_err(|e| e.to_string())?;
            if extracted.len() > MAX_PACKAGE || !extracted.starts_with(b"MZ") {
                return Err("便携 zip 中的可执行文件无效".into());
            }
            return Ok(extracted);
        }
    }
    Err("便携 zip 中未找到 yuumi.exe".into())
}

/// Called before application runtime initialization in main; no live install in tests.
pub fn run_portable_helper_if_requested() -> bool {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).and_then(|v| v.to_str()) != Some("--yuumi-update-helper") {
        return false;
    }
    if let Err(e) = run_portable_helper(&args) {
        eprintln!("便携更新失败: {e}");
        #[cfg(windows)]
        if let Some(pid) = args
            .get(2)
            .and_then(|v| v.to_str())
            .and_then(|v| v.parse().ok())
        {
            let _ = wait_for_parent(pid);
        }
        if let Some(target) = args.get(4).map(std::path::PathBuf::from) {
            if target
                .parent()
                .is_some_and(|dir| dir.join("portable.flag").is_file())
            {
                let _ = std::fs::write(target.with_file_name("portable-update-error.log"), &e);
                let _ = std::process::Command::new(target).spawn();
            }
        }
    }
    true
}
#[cfg(windows)]
fn run_portable_helper(args: &[std::ffi::OsString]) -> Result<(), String> {
    use std::io::Read;
    use std::os::windows::fs::OpenOptionsExt;
    if args.len() != 7 {
        return Err("便携更新助手参数无效".into());
    }
    let pid = args[2]
        .to_str()
        .and_then(|v| v.parse::<u32>().ok())
        .ok_or("父进程无效")?;
    let payload = std::path::PathBuf::from(&args[3]);
    let target = std::path::PathBuf::from(&args[4]);
    let key = args[5].to_str().ok_or("公钥无效")?;
    let signature = args[6].to_str().ok_or("签名无效")?;
    if gpui_update_channel()?.1 != key {
        return Err("便携更新公钥不属于此构建".into());
    }
    let target_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase());
    if target_name.as_deref() != Some("yuumi.exe") {
        return Err("便携更新目标不是 Yuumi 程序".into());
    }
    if !target
        .parent()
        .is_some_and(|p| p.join("portable.flag").is_file())
    {
        return Err("缺少便携版标记".into());
    }
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&payload)
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    std::io::Read::by_ref(&mut file)
        .take((MAX_PACKAGE + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_PACKAGE {
        return Err("便携更新文件超出大小限制".into());
    }
    verify(key, signature, &bytes)?;
    let exe_bytes = if bytes.starts_with(b"MZ") {
        bytes
    } else if bytes.starts_with(b"PK\x03\x04") {
        extract_exe_from_portable_zip(&bytes)?
    } else {
        return Err("便携更新文件无效".into());
    };
    wait_for_parent(pid)?;
    let dir = target.parent().ok_or("目标目录无效")?;
    let new = dir.join(".yuumi-update-new.exe");
    let backup = dir.join(".yuumi-update-backup.exe");
    // Do not overwrite pre-existing files belonging to another update.
    let mut out = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&new)
        .map_err(|e| e.to_string())?;
    use std::io::Write;
    out.write_all(&exe_bytes)
        .and_then(|_| out.sync_all())
        .map_err(|e| e.to_string())?;
    drop(out);
    let result = replace_portable(&new, &target, &backup);
    if result.is_ok() {
        let _ = std::process::Command::new(&target).spawn();
    }
    result
}
#[cfg(not(windows))]
fn run_portable_helper(_: &[std::ffi::OsString]) -> Result<(), String> {
    Err("便携更新仅支持 Windows".into())
}
#[cfg(windows)]
fn wait_for_parent(pid: u32) -> Result<(), String> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut std::ffi::c_void;
        fn WaitForSingleObject(handle: *mut std::ffi::c_void, millis: u32) -> u32;
        fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
        fn GetLastError() -> u32;
    }
    let handle = unsafe { OpenProcess(0x00100000, 0, pid) };
    if handle.is_null() {
        // ERROR_INVALID_PARAMETER means the parent already exited.
        if unsafe { GetLastError() } == 87 {
            return Ok(());
        }
        return Err("无法打开更新父进程".into());
    }
    let result = unsafe { WaitForSingleObject(handle, 120_000) };
    unsafe {
        CloseHandle(handle);
    }
    if result != 0 {
        return Err("等待程序退出失败或超时".into());
    }
    Ok(())
}
fn replace_portable(new: &Path, target: &Path, backup: &Path) -> Result<(), String> {
    if backup.exists() {
        return Err("已有便携更新备份，请先处理".into());
    }
    std::fs::rename(target, backup).map_err(|e| e.to_string())?;
    if let Err(e) = std::fs::rename(new, target) {
        return match std::fs::rename(backup, target) {
            Ok(()) => Err(format!("更新失败，已恢复原程序: {e}")),
            Err(restore) => Err(format!("更新失败: {e}；恢复原程序失败: {restore}")),
        };
    }
    let _ = std::fs::remove_file(backup);
    Ok(())
}
#[cfg(test)]
mod integration_tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    async fn server(body: &'static str) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = [0; 4096];
            let _ = socket.read(&mut buffer).await;
            let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        (format!("http://{address}"), task)
    }
    #[tokio::test]
    async fn manifest_checks_target_and_version() {
        let (url, task) = server(r#"{"version":"0.2.0","platforms":{"other-target":{"url":"https://example.test/update","signature":"sig"}}}"#).await;
        let service = UpdateService::new(
            reqwest::Client::new(),
            url,
            "0.1.0".into(),
            "windows-x86_64".into(),
            "key".into(),
        );
        assert!(service.check().await.unwrap_err().contains("匹配"));
        task.await.unwrap();
        let (url, task) = server(r#"{"version":"0.1.0","platforms":{}}"#).await;
        let service = UpdateService::new(
            reqwest::Client::new(),
            url,
            "0.1.0".into(),
            "windows-x86_64".into(),
            "key".into(),
        );
        assert!(service.check().await.unwrap().is_none());
        task.await.unwrap();
        let (url, task) = server(r#"{"version":"0.2.0","platforms":{"windows-x86_64":{"url":"https://example.test/update","signature":"signature"}}}"#).await;
        let service = UpdateService::new(
            reqwest::Client::new(),
            url,
            "0.1.0".into(),
            "windows-x86_64".into(),
            "key".into(),
        );
        assert_eq!(service.check().await.unwrap().unwrap().version, "0.2.0");
        task.await.unwrap();
    }
    #[tokio::test]
    async fn release_cache_is_reused_and_force_refresh_fails_offline() {
        let (url, task) =
            server(r#"[{"tag_name":"v1.0.0","body":"notes","published_at":"2025-01-01"}]"#).await;
        let path = std::env::temp_dir().join(format!(
            "yuumi-release-test-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let client = reqwest::Client::new();
        let first = release_changelog(&client, &url, &path, Some("1.0.0"), false)
            .await
            .unwrap();
        task.await.unwrap();
        let cached = release_changelog(&client, &url, &path, Some("1.0.0"), false)
            .await
            .unwrap();
        assert_eq!(first[0].body, cached[0].body);
        assert!(release_changelog(&client, &url, &path, None, true)
            .await
            .is_err());
        std::fs::remove_file(path).unwrap();
    }
    #[tokio::test]
    async fn stale_cache_falls_back_when_github_unreachable() {
        // 缓存里只有 v1.0.0，而本地版本是 9.9.9：快路径校验不通过，走网络；
        // 网络不可达时应兜底返回缓存内容（force 显式刷新仍报错）
        let path = std::env::temp_dir().join(format!(
            "yuumi-release-stale-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(
            &path,
            r#"[{"tag":"v1.0.0","body":"cached notes","published_at":"2025-01-01"}]"#,
        )
        .unwrap();
        let client = reqwest::Client::new();
        let unreachable = "http://127.0.0.1:1/releases";
        let entries = release_changelog(&client, unreachable, &path, Some("9.9.9"), false)
            .await
            .unwrap();
        assert_eq!(entries[0].body, "cached notes");
        assert!(
            release_changelog(&client, unreachable, &path, Some("9.9.9"), true)
                .await
                .is_err()
        );
        std::fs::remove_file(path).unwrap();
    }
    #[tokio::test]
    async fn signed_download_reports_progress_and_tampering_blocks_install() {
        let (url, task) = server("test").await;
        let key = STANDARD.encode("untrusted comment: minisign public key\nRWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3\n");
        let signature = STANDARD.encode("untrusted comment: signature from minisign secret key\nRUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=\ntrusted comment: timestamp:1633700835\tfile:test\tprehashed\nwLMDjy9FLAuxZ3q4NlEvkgtyhrr0gtTu6KC4KBJdITbbOeAi1zBIYo0v4iTgt8jJpIidRJnp94ABQkJAgAooBQ==");
        let service = UpdateService::new(
            reqwest::Client::new(),
            url.clone(),
            "0.1.0".into(),
            "windows-x86_64".into(),
            key,
        );
        let info = UpdateInfo {
            version: "0.2.0".into(),
            current_version: "0.1.0".into(),
            notes: None,
            pub_date: None,
            url,
            signature,
            target: "windows-x86_64".into(),
        };
        let mut received = 0;
        let update = service
            .download(&info, |count, _| received = count)
            .await
            .unwrap();
        task.await.unwrap();
        assert_eq!(received, 4);
        let dir = std::env::temp_dir().join(format!(
            "yuumi-stage-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let staged = update.stage_installer(&dir).unwrap();
        std::fs::write(staged.path(), b"altered").unwrap();
        assert!(staged.launch(false).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn failed_replacement_restores_original() {
        let dir = std::env::temp_dir().join(format!(
            "yuumi-rollback-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let target = dir.join("app.exe");
        std::fs::write(&target, b"original").unwrap();
        assert!(
            replace_portable(&dir.join("missing.exe"), &target, &dir.join("backup.exe")).is_err()
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"original");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
