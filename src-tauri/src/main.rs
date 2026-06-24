// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // 在任何 TLS 连接之前安装 ring 作为全局 rustls CryptoProvider，
    // 避免 ring 与 aws-lc-rs 共存时 rustls 无法自动选择而 panic。
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls ring crypto provider");

    yuumi_lib::run()
}
