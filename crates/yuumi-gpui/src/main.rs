// MSVC 链接器创建 .lib/.exp 时输出的 "正在创建库" 提示，非代码问题，静默处理
#![allow(linker_messages)]
// Release 构建使用 Windows GUI 子系统，防止启动时弹出控制台命令行窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use gpui::prelude::*;
use gpui::{px, Application, Size, TitlebarOptions, WindowOptions};

mod agent_settings_view;
mod app_window;
mod asset_loader;
mod assets;
mod bench_overlay;
mod career_view;
mod components;
mod game_info_view;
mod i18n;
mod match_model;
mod opgg;
pub mod platform;
pub mod replay_store;
pub mod services;
mod theme;
mod tray;
pub mod views;

use app_window::{main_window_bounds, scale_factor_override_for, MainWindow};

// 主窗标题常量沿用原有 crate 级访问路径（bench_overlay / platform 直接引用 crate::BENCH_WINDOW_TITLE）
pub use app_window::{BENCH_WINDOW_TITLE, MAIN_WINDOW_TITLE};

fn main() {
    if yuumi_core::update::run_portable_helper_if_requested() {
        return;
    }
    // 显式安装全局默认 CryptoProvider，防止 rustls 0.23+ 多 provider 特性冲突
    let _ = rustls::crypto::ring::default_provider().install_default();

    // 0. 初始化全局 Tokio 异步运行时，供后台网络 I/O、WebSocket 与 LCU 轮询使用
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("初始化 Tokio 运行时失败");
    // 注册全局句柄：GPUI 的 background_executor 运行在 Windows 线程池上，
    // 线程池任务需要靠该句柄把网络 I/O 投递回运行时，否则会 panic（no reactor running）
    yuumi_core::runtime::set_tokio_handle(runtime.handle().clone());
    let _runtime_guard = runtime.enter();

    // 1. Windows 单实例互斥量检测（Named Mutex）
    let mutex_name = "Global\\Yuumi_SingleInstance_Mutex";
    let _instance_guard = match platform::acquire_single_instance(mutex_name) {
        Ok(guard) => Some(guard),
        Err(platform::SingleInstanceError::AlreadyRunning) => {
            eprintln!("检测到 Yuumi 已在运行中，激活现有窗口...");
            if let Some(hwnd) = platform::find_main_window_hwnd() {
                platform::restore_and_foreground(hwnd);
            }
            return;
        }
        Err(e) => {
            eprintln!("创建单实例互斥量警告: {:?}", e);
            None
        }
    };

    // 2. 初始化平台日志系统（输出到标准错误与调试输出）
    yuumi_core::runtime::init();
    if let Err(error) =
        services::logging::init(yuumi_core::config::AppConfig::load().general.log_level)
    {
        eprintln!("日志初始化失败: {error}");
    }
    log::info!("Yuumi 原生 Direct3D 11/DirectWrite 渲染宿主正在启动...");

    // 3. 创建 GPUI 原生应用程序宿主（挂载内嵌图标与 Logo 资源）
    let app = Application::new().with_assets(assets::Assets);

    app.run(move |cx| {
        let config = yuumi_core::config::AppConfig::load();
        // 创建主应用窗口（无边框，1200×910 居中，可调整大小）
        let window_options = WindowOptions {
            window_bounds: Some(main_window_bounds(cx)),
            window_min_size: Some(Size {
                width: px(880.0),
                height: px(600.0),
            }),
            // 彻底禁用操作系统传统标题栏（appears_transparent=true 仍隐藏系统标题栏）；
            // title 用于 Win32 侧精确定位主窗口：单实例二次启动时激活已有窗口依赖它
            titlebar: Some(TitlebarOptions {
                title: Some(MAIN_WINDOW_TITLE.into()),
                appears_transparent: true,
                traffic_light_position: None,
            }),
            // 设置页 DPI 缩放（Auto=跟随系统，100/125/150=固定缩放）
            scale_factor_override: scale_factor_override_for(&config.personalization.dpi_scale),
            focus: true,
            is_movable: true,
            ..Default::default()
        };

        cx.open_window(window_options, |window, cx| {
            cx.new(|cx| {
                let mut view = MainWindow::new(cx);
                view.initialize_window_appearance(window);
                view
            })
        })
        .expect("创建 Yuumi 主窗口失败");

        // 应用全局 API 并发限制。
        yuumi_core::lcu::client::set_api_concurrency(
            config.functions.api_concurrency_number as usize,
        );
    });
}
