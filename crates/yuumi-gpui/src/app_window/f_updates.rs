//! GPUI 更新和发布说明接线。签名发布通道由打包构建注入。
use super::*;
use gpui::Entity;
use std::time::Duration;
use yuumi_core::update::{self, StagedInstaller, StagedPortable, UpdateInfo, UpdateService};

#[derive(Default)]
pub(super) struct UpdateFlow {
    pub(super) checking: bool,
    info: Option<UpdateInfo>,
    progress: Option<(u64, Option<u64>)>,
    staged: Option<StagedUpdate>,
    open: bool,
    dismissed: bool,
    generation: u64,
    cancel: Option<tokio::sync::watch::Sender<bool>>,
    error: Option<String>,
}

enum StagedUpdate {
    Installed(StagedInstaller),
    Portable(StagedPortable),
}
impl StagedUpdate {
    fn launch(&self, portable: bool) -> Result<(), String> {
        match self {
            Self::Installed(s) => s.launch(portable),
            Self::Portable(s) => s.launch(portable),
        }
    }
}

impl MainWindow {
    pub(super) fn check_updates(&mut self, manual: bool, cx: &mut Context<Self>) {
        if self.f_update.checking || self.f_update.progress.is_some() {
            return;
        }
        if self.f_update.info.is_some() {
            self.f_update.open = true;
            self.f_update.dismissed = false;
            cx.notify();
            return;
        }
        let service = match update_service(&self.app_config) {
            Ok(service) => service,
            Err(error) => {
                if manual {
                    // 对照 Vue C05：检查失败以 Toast 呈现
                    self.toast(ToastKind::Error, error, cx);
                    cx.notify();
                }
                return;
            }
        };
        let Some(runtime) = yuumi_core::runtime::tokio_handle() else {
            return;
        };
        self.f_update.checking = true;
        if manual {
            self.settings_view
                .set_feedback("正在检查 GPUI 更新…", false);
        }
        cx.notify();
        let task = runtime.spawn(async move { service.check().await });
        cx.spawn(async move |entity, cx| {
            let result = task.await.unwrap_or_else(|_| Err("版本检查中断".into()));
            let _ = entity.update(cx, |this, cx| {
                this.f_update.checking = false;
                match result {
                    Ok(Some(info)) => {
                        this.has_update = true;
                        this.f_update.info = Some(info.clone());
                        this.f_update.open = manual;
                        this.f_update.dismissed =
                            !manual && !this.app_config.general.enable_check_update;
                        // 对照 Vue C01：手动检查发现新版本以 Toast 呈现（保留更新弹窗）
                        if manual {
                            let version = info.version.clone();
                            this.toast(ToastKind::Success, format!("发现新版本 v{}", version), cx);
                        }
                        if !yuumi_core::runtime::is_portable()
                            && (manual || this.app_config.general.enable_check_update)
                        {
                            this.download_update(manual, cx);
                        }
                    }
                    // 对照 Vue C02：手动检查已是最新版以 Toast 呈现
                    Ok(None) if manual => {
                        this.toast(ToastKind::Success, "当前已是最新 GPUI 版本", cx)
                    }
                    // 对照 Vue C05：手动检查失败以 Toast 呈现
                    Err(error) if manual => this.toast(ToastKind::Error, error, cx),
                    Err(_) => log::warn!("[Update] 启动检查失败"),
                    _ => {}
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn download_update(&mut self, manual: bool, cx: &mut Context<Self>) {
        if self.f_update.progress.is_some() || self.f_update.staged.is_some() {
            return;
        }
        let Some(info) = self.f_update.info.clone() else {
            return;
        };
        let service = match update_service(&self.app_config) {
            Ok(service) => service,
            Err(error) => {
                self.f_update.error = Some(error);
                cx.notify();
                return;
            }
        };
        let Some(runtime) = yuumi_core::runtime::tokio_handle() else {
            return;
        };
        self.f_update.error = None;
        self.f_update.progress = Some((0, None));
        // 对照 Vue C03/C06：用户主动下载才提示「开始下载」；启动自动下载保持静默
        if manual {
            self.toast(
                ToastKind::Success,
                format!("已开始下载 v{}", info.version),
                cx,
            );
        }
        cx.notify();

        self.f_update.generation = self.f_update.generation.wrapping_add(1);
        let generation = self.f_update.generation;
        let (cancel, mut cancelled) = tokio::sync::watch::channel(false);
        self.f_update.cancel = Some(cancel);
        let (progress, mut changes) = tokio::sync::watch::channel((0, None));
        let task = runtime.spawn(async move {
            let package = tokio::select! {
                biased;
                _ = cancelled.changed() => return Err("下载已取消".to_string()),
                result = service.download(&info, |received, total| {
                    progress.send_replace((received, total));
                }) => result?,
            };
            if *cancelled.borrow() {
                return Err("下载已取消".into());
            }
            let directory = yuumi_core::runtime::app_data_dir()
                .join("gpui-update")
                .join(format!(
                    "{}-{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_err(|e| e.to_string())?
                        .as_nanos()
                ));
            let portable = yuumi_core::runtime::is_portable();
            tokio::task::spawn_blocking(move || {
                if *cancelled.borrow() {
                    return Err("下载已取消".into());
                }
                let staged = if portable {
                    package
                        .stage_portable(&directory)
                        .map(StagedUpdate::Portable)
                } else {
                    package
                        .stage_installer(&directory)
                        .map(StagedUpdate::Installed)
                };
                if *cancelled.borrow() {
                    let _ = std::fs::remove_dir_all(&directory);
                    return Err("下载已取消".into());
                }
                staged.map(|staged| (staged, directory))
            })
            .await
            .map_err(|_| "更新暂存中断")?
        });
        cx.spawn(async move |entity, cx| {
            while changes.changed().await.is_ok() {
                let received = *changes.borrow_and_update();
                if entity
                    .update(cx, |this, cx| {
                        if this.f_update.generation == generation
                            && this.f_update.progress.is_some()
                        {
                            this.f_update.progress = Some(received);
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        cx.spawn(async move |entity, cx| {
            let result = task.await.unwrap_or_else(|_| Err("下载任务中断".into()));
            let _ = entity.update(cx, |this, cx| {
                if this.f_update.generation != generation {
                    if let Ok((_, directory)) = result {
                        let _ = std::fs::remove_dir_all(directory);
                    }
                    return;
                }
                this.f_update.progress = None;
                this.f_update.cancel = None;
                match result {
                    Ok((staged, _)) => {
                        this.f_update.staged = Some(staged);
                        this.f_update.dismissed = false;
                    }
                    Err(error) => {
                        this.f_update.error = Some(error.clone());
                        // 对照 Vue C07：下载失败以 Toast 呈现；用户主动取消不提示
                        if manual && error != "下载已取消" {
                            this.toast(ToastKind::Error, format!("下载失败: {}", error), cx);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn install_update(&mut self, cx: &mut Context<Self>) {
        let Some(staged) = self.f_update.staged.take() else {
            return;
        };
        match staged.launch(yuumi_core::runtime::is_portable()) {
            Ok(()) => cx.quit(),
            Err(error) => {
                self.f_update.staged = Some(staged);
                self.f_update.error = Some(error);
                cx.notify();
            }
        }
    }

    pub(super) fn load_release_notes(&mut self, force: bool, cx: &mut Context<Self>) {
        if self.settings_view.release_loading
            || (!force && !self.settings_view.release_notes.is_empty())
        {
            return;
        }
        let client = match release_client(&self.app_config) {
            Ok(client) => client,
            Err(error) => {
                self.settings_view.release_error = Some(error);
                cx.notify();
                return;
            }
        };
        let Some(runtime) = yuumi_core::runtime::tokio_handle() else {
            return;
        };
        self.settings_view.release_loading = true;
        self.settings_view.release_error = None;
        cx.notify();
        let task = runtime.spawn(async move {
            let cache = yuumi_core::runtime::app_data_dir()
                .join("cache")
                .join("gpui-releases.json");
            update::release_changelog(&client, update::RELEASES_URL, &cache, None, force).await
        });
        cx.spawn(async move |entity, cx| {
            let result = task
                .await
                .unwrap_or_else(|_| Err("版本历史读取中断".into()));
            let _ = entity.update(cx, |this, cx| {
                this.settings_view.release_loading = false;
                match result {
                    Ok(entries) => this.settings_view.release_notes = entries,
                    Err(error) => this.settings_view.release_error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn render_update_surface(&self, entity: &Entity<Self>) -> impl IntoElement {
        let Some(info) = &self.f_update.info else {
            return div().into_any_element();
        };
        if self.f_update.dismissed {
            return div().into_any_element();
        }
        if !self.f_update.open {
            let entity = entity.clone();
            let dismiss = entity.clone();
            return div()
                .absolute()
                .right(px(20.0))
                .bottom(px(66.0))
                .flex()
                .gap(px(8.0))
                .child(update_action(
                    if self.f_update.progress.is_some() {
                        "查看更新进度"
                    } else {
                        "查看 GPUI 更新"
                    },
                    move |_, app| {
                        entity.update(app, |view, cx| {
                            view.f_update.open = true;
                            cx.notify();
                        })
                    },
                ))
                .child(update_action("忽略", move |_, app| {
                    dismiss.update(app, |view, cx| {
                        view.f_update.dismissed = true;
                        cx.notify();
                    })
                }))
                .into_any_element();
        }
        let entity_cancel = entity.clone();
        let entity_close = entity.clone();
        let entity_download = entity.clone();
        let entity_install = entity.clone();
        let entity_release = entity.clone();
        div()
            .id("gpui-update-prompt")
            .absolute()
            .right(px(20.0))
            .bottom(px(66.0))
            .occlude()
            .w(px(380.0))
            .p(px(16.0))
            .rounded(Theme::RADIUS_LG)
            .bg(Theme::CARD_BG_HOVER)
            .border_1()
            .border_color(Theme::CARD_BORDER)
            .shadow_lg()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(format!(
                "GPUI 新版本 {}（当前 {}）",
                info.version, info.current_version
            ))
            .when_some(
                info.notes.as_ref().filter(|s| !s.is_empty()),
                |el, notes| {
                    el.child(
                        div()
                            .id("gpui-update-notes")
                            .max_h(px(160.0))
                            .overflow_y_scroll()
                            .text_sm()
                            .child(notes.clone()),
                    )
                },
            )
            .when_some(self.f_update.progress, |el, (received, total)| {
                el.child(format!(
                    "已下载 {:.1} MB{}",
                    received as f64 / 1_048_576.0,
                    total
                        .map(|n| format!(" / {:.1} MB", n as f64 / 1_048_576.0))
                        .unwrap_or_default()
                ))
            })
            .when_some(self.f_update.error.as_ref(), |el, error| {
                el.child(div().text_color(Theme::LOSS_TEXT).child(error.clone()))
            })
            .child(
                div()
                    .flex()
                    .gap(px(8.0))
                    .child(update_action(
                        if self.f_update.progress.is_some() {
                            "后台运行"
                        } else {
                            "稍后提醒"
                        },
                        move |_, app| {
                            entity_close.update(app, |view, cx| {
                                view.f_update.open = false;
                                cx.notify();
                            })
                        },
                    ))
                    .when(
                        self.f_update.progress.is_none() && self.f_update.staged.is_none(),
                        |el| {
                            el.child(update_action("下载更新", move |_, app| {
                                entity_download
                                    .update(app, |view, cx| view.download_update(true, cx))
                            }))
                        },
                    )
                    .when(self.f_update.progress.is_some(), |el| {
                        el.child(update_action("取消下载", move |_, app| {
                            entity_cancel.update(app, |view, cx| {
                                if let Some(cancel) = view.f_update.cancel.take() {
                                    cancel.send_replace(true);
                                }
                                view.f_update.generation = view.f_update.generation.wrapping_add(1);
                                view.f_update.progress = None;
                                view.f_update.error = None;
                                cx.notify();
                            })
                        }))
                    })
                    .when(self.f_update.staged.is_some(), |el| {
                        el.child(update_action("安装已验证更新", move |_, app| {
                            entity_install.update(app, |view, cx| view.install_update(cx))
                        }))
                    })
                    .child(update_action("打开发布页", move |_, app| {
                        entity_release.update(app, |view, cx| {
                            view.handle_settings_action(SettingsAction::GoToReleases, cx);
                        })
                    })),
            )
            .into_any_element()
    }
}

fn update_action(
    label: &'static str,
    action: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(label)
        .px(px(10.0))
        .py(px(5.0))
        .rounded(Theme::RADIUS_SM)
        .bg(Theme::ACCENT_LIGHT)
        .text_color(Theme::TEXT_PRIMARY)
        .text_sm()
        .cursor_pointer()
        .hover(|s| s.bg(Theme::HOVER_BG_STRONG))
        .on_mouse_down(MouseButton::Left, move |_, window, app| action(window, app))
        .child(label)
}

fn release_client(config: &AppConfig) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let url = attempt.url();
            if attempt.previous().len() >= 10
                || !(url.scheme() == "https"
                    || (url.scheme() == "http"
                        && matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))))
            {
                attempt.error("拒绝不安全的重定向")
            } else {
                attempt.follow()
            }
        }))
        .user_agent(concat!("Yuumi-GPUI/", env!("CARGO_PKG_VERSION")));
    let addr = config.general.http_proxy_addr.trim();
    if config.general.enable_http_proxy && !addr.is_empty() {
        let url = if addr.contains("://") {
            addr.into()
        } else {
            format!("http://{addr}")
        };
        let proxy = reqwest::Proxy::all(url).map_err(|_| "HTTP 代理地址无效")?;
        builder = builder.proxy(proxy);
    }
    builder.build().map_err(|_| "无法创建版本服务客户端".into())
}

fn update_service(config: &AppConfig) -> Result<UpdateService, String> {
    let endpoint = option_env!("YUUMI_GPUI_UPDATE_ENDPOINT")
        .filter(|s| !s.is_empty())
        .ok_or("此 GPUI 构建未配置签名更新通道，请通过发布页获取兼容版本。")?;
    let key = option_env!("YUUMI_GPUI_UPDATE_PUBLIC_KEY")
        .filter(|s| !s.is_empty())
        .ok_or("此 GPUI 构建缺少可信更新公钥。")?;
    let target = format!(
        "windows-{}-gpui{}",
        std::env::consts::ARCH,
        if yuumi_core::runtime::is_portable() {
            "-portable"
        } else {
            ""
        }
    );
    Ok(UpdateService::new(
        release_client(config)?,
        endpoint.into(),
        env!("CARGO_PKG_VERSION").into(),
        target,
        key.into(),
    ))
}
