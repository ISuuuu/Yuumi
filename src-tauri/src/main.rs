// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // 便携版更新 helper 进程入口：必须在一切初始化（含 runtime::init）之前判定
    if yuumi_lib::run_portable_update_helper_if_requested() {
        return;
    }

    // 便携模式识别与数据目录解析：必须在一切逻辑（尤其 config 加载）之前
    yuumi_lib::runtime::init();

    // 在任何 TLS 连接之前安装 ring 作为全局 rustls CryptoProvider，
    // 避免 ring 与 aws-lc-rs 共存时 rustls 无法自动选择而 panic。
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls ring crypto provider");

    // 读取配置并初始化日志（在 Tauri 启动前，确保尽早写日志）
    let config = yuumi_lib::config::AppConfig::load();
    yuumi_lib::logging::init(config.general.log_level);

    yuumi_lib::run()
}
