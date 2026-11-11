//! 把 `assets/icons/icon.ico` 嵌入 exe（资源 ID 1）。
//!
//! GPUI 0.2.2 在 Windows 上通过 `LoadImageW(module, MAKEINTRESOURCE(1), IMAGE_ICON, ...)`
//! 取得窗口类图标，托盘亦可用 `Icon::from_resource(1, None)` 复用同一份多尺寸图标。

use std::{env, fs, path::PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    let icon_path = manifest_dir.join("assets/icons/icon.ico");
    println!("cargo:rerun-if-changed={}", icon_path.display());

    // 资源文件写入 OUT_DIR，图标路径用绝对路径，避免 rc.exe 对相对路径的解析差异
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap_or_default());
    let rc_path = out_dir.join("yuumi-icon.rc");
    let icon = icon_path.to_string_lossy().replace('\\', "/");
    if let Err(e) = fs::write(&rc_path, format!("1 ICON \"{}\"\n", icon)) {
        eprintln!("写入图标资源脚本失败: {}", e);
        std::process::exit(1);
    }

    if let Err(e) = embed_resource::compile(&rc_path, embed_resource::NONE).manifest_required() {
        eprintln!("嵌入应用图标资源失败: {}", e);
        std::process::exit(1);
    }
}
