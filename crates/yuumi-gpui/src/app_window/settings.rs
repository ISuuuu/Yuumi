// 设置路由：SettingsAction 分发、主题色与外观持久化。
use super::*;

impl MainWindow {
    /// 按当前配置重建运行时主题色板（主题模式/主题色/胜负卡色/死亡数字色）
    pub(crate) fn apply_theme(&self) {
        let system_dark = platform::system_prefers_dark();
        Theme::apply(&self.app_config.personalization, system_dark);
    }

    /// 按当前配置初始化全局界面语言（对照 Vue `setLocale`；与 apply_theme 同样的接线模式）
    pub(crate) fn apply_language(&self) {
        crate::i18n::set_language(crate::i18n::Lang::from_config(
            &self.app_config.personalization.language,
        ));
    }

    pub(crate) fn initialize_window_appearance(&mut self, window: &Window) {
        self.apply_theme();
        self.apply_language();
        let Ok(hwnd) = platform::native_window_handle(window) else {
            log::warn!("[Platform] 无法获取主窗口句柄");
            return;
        };
        platform::set_window_rounded_corners(hwnd);
        if self.app_config.personalization.mica_enabled {
            match platform::set_mica(hwnd, true) {
                Ok(()) => self.mica_active = true,
                Err(error) => log::warn!("[Platform] 启用云母效果失败: {error}"),
            }
        }
    }

    /// 处理设置页交互动作
    pub(super) fn handle_settings_action(
        &mut self,
        action: SettingsAction,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            SettingsAction::ToggleCollapse(name) => {
                self.settings_view.toggle_section(name);
                cx.notify();
            }
            SettingsAction::SetApiConcurrency(val) => {
                let clamped = val.clamp(1, 10);
                self.app_config.functions.api_concurrency_number = clamped;
                yuumi_core::lcu::client::set_api_concurrency(clamped as usize);
                self.persist_config(cx);
            }
            SettingsAction::SetCareerGames(val) => {
                let clamped = val.clamp(5, 50);
                self.app_config.functions.career_games_number = clamped;
                self.persist_config(cx);
            }
            SettingsAction::ToggleGameInfoFilter => {
                self.app_config.functions.game_info_filter =
                    !self.app_config.functions.game_info_filter;
                self.persist_config(cx);
            }
            SettingsAction::ToggleReserveGameInfo => {
                self.app_config.functions.enable_reserve_gameinfo =
                    !self.app_config.functions.enable_reserve_gameinfo;
                self.persist_config(cx);
            }
            SettingsAction::ToggleShowTierInGameInfo => {
                self.app_config.functions.show_tier_in_game_info =
                    !self.app_config.functions.show_tier_in_game_info;
                self.persist_config(cx);
            }
            SettingsAction::ToggleAutoShowOpgg => {
                self.app_config.functions.auto_show_opgg =
                    !self.app_config.functions.auto_show_opgg;
                self.persist_config(cx);
            }
            SettingsAction::TogglePinOpgg => {
                let enabled = !self.app_config.functions.enable_opgg_on_top;
                self.app_config.functions.enable_opgg_on_top = enabled;
                if let Some(handle) = self.opgg_window {
                    cx.defer(move |cx| {
                        let _ = handle.update(cx, |_, window, _| {
                            if let Ok(hwnd) = platform::native_window_handle(window) {
                                platform::set_window_topmost(hwnd, enabled);
                            }
                        });
                    });
                }
                self.persist_config(cx);
            }
            SettingsAction::DetectLolPath => {
                // 对照 Vue B01/B02/B03
                if let Some(path) = services::detect_lol_path() {
                    if self.app_config.general.lol_path.contains(&path) {
                        self.toast(ToastKind::Info, crate::i18n::t("settings.pathExists"), cx);
                    } else {
                        self.app_config.general.lol_path.push(path.clone());
                        self.sync_home_paths();
                        self.persist_config_silent(cx);
                        self.toast(
                            ToastKind::Success,
                            crate::i18n::t("settings.pathAdded").replace("{path}", &path),
                            cx,
                        );
                    }
                } else {
                    self.toast(ToastKind::Error, crate::i18n::t("settings.lolClientNotFound"), cx);
                }
                cx.notify();
            }
            SettingsAction::SelectLolFolder => {
                // 对照 Vue B05/B06：取消选择不提示
                if let Some(path) = services::select_folder() {
                    if self.app_config.general.lol_path.contains(&path) {
                        self.toast(ToastKind::Info, crate::i18n::t("settings.pathExists"), cx);
                    } else {
                        self.app_config.general.lol_path.push(path.clone());
                        self.sync_home_paths();
                        self.persist_config_silent(cx);
                        self.toast(
                            ToastKind::Success,
                            crate::i18n::t("settings.pathAdded").replace("{path}", &path),
                            cx,
                        );
                    }
                }
                cx.notify();
            }
            SettingsAction::AddWegamePath => {
                // 对照 Vue B08/B09
                let wegame_marker = yuumi_core::config::WEGAME_MARKER.to_string();
                if self.app_config.general.lol_path.contains(&wegame_marker) {
                    self.toast(ToastKind::Info, crate::i18n::t("settings.wegameExists"), cx);
                } else {
                    self.app_config.general.lol_path.push(wegame_marker);
                    self.sync_home_paths();
                    self.persist_config_silent(cx);
                    self.toast(ToastKind::Success, crate::i18n::t("settings.wegameAdded"), cx);
                }
                cx.notify();
            }
            SettingsAction::RemoveLolPath(idx) => {
                if idx < self.app_config.general.lol_path.len() {
                    self.app_config.general.lol_path.remove(idx);
                    self.sync_home_paths();
                    self.persist_config(cx);
                }
            }
            SettingsAction::ToggleStartLolWithApp => {
                self.app_config.general.enable_start_lol_with_app =
                    !self.app_config.general.enable_start_lol_with_app;
                self.persist_config(cx);
            }
            SettingsAction::ClearCache => {
                let client_path = self.app_config.general.lol_path.first().cloned();
                self.toast(ToastKind::Info, crate::i18n::t("settings.clearingCache"), cx);
                cx.notify();
                cx.spawn(async move |entity, cx| {
                    let res = cx
                        .background_executor()
                        .spawn(async move { services::clear_game_cache(client_path.as_deref()) })
                        .await;
                    let _ = entity.update(cx, |this, cx| {
                        match res {
                            // 对照 Vue B23：结果走 Toast，"正在清理"仍是局部进行中提示
                            Ok(msg) => this.toast(ToastKind::Success, msg, cx),
                            Err(e) => this.toast(
                                ToastKind::Error,
                                crate::i18n::t("settings.clearCacheFailed").replace("{error}", &e),
                                cx,
                            ),
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
            SettingsAction::ToggleCloseToTray => {
                let cur = self.app_config.general.enable_close_to_tray.unwrap_or(true);
                self.app_config.general.enable_close_to_tray = Some(!cur);
                self.persist_config(cx);
            }
            SettingsAction::ToggleMinimizeOnGameStart => {
                self.app_config.general.enable_game_start_minimize =
                    !self.app_config.general.enable_game_start_minimize;
                self.persist_config(cx);
            }
            SettingsAction::EditFInput(key, value) => {
                let index = match key {
                    "f-upload" => 0,
                    "f-user" => 1,
                    "f-proxy" => 2,
                    "f-screenshot" => 3,
                    _ => return,
                };
                self.settings_view.f_inputs[index] = value;
                cx.notify();
            }
            SettingsAction::SaveFInput(key) => {
                let index = match key {
                    "f-upload" => 0,
                    "f-user" => 1,
                    "f-proxy" => 2,
                    "f-screenshot" => 3,
                    _ => return,
                };
                let value = self.settings_view.f_inputs[index].trim().to_string();
                if index == 0
                    && !value.is_empty()
                    && (value.chars().any(char::is_whitespace)
                        || value
                            .strip_prefix("http://")
                            .or_else(|| value.strip_prefix("https://"))
                            .and_then(|rest| rest.split('/').next())
                            .is_none_or(|host| host.is_empty()))
                {
                    self.toast(ToastKind::Error, crate::i18n::t("settings.uploadUrlInvalid"), cx);
                    cx.notify();
                    return;
                }
                if index == 2
                    && !value.is_empty()
                    && (value.chars().any(char::is_whitespace)
                        || value.rsplit_once(':').is_none_or(|(host, port)| {
                            host.is_empty() || port.parse::<u16>().is_err() || port == "0"
                        }))
                {
                    self.toast(
                        ToastKind::Error,
                        crate::i18n::t("settings.proxyAddrInvalid"),
                        cx,
                    );
                    cx.notify();
                    return;
                }
                self.settings_view.f_inputs[index] = value.clone();
                match index {
                    0 => self.app_config.general.upload_api_url = value,
                    1 => self.app_config.general.signalr_user_id = value,
                    2 => self.app_config.general.http_proxy_addr = value,
                    3 => self.app_config.functions.screenshot_save_path = value,
                    _ => unreachable!(),
                }
                self.persist_config(cx);
                if index == 2 {
                    self.apply_opgg_proxy(cx);
                }
            }
            SettingsAction::SetUploadApiUrl(url) => {
                self.app_config.general.upload_api_url = url;
                self.persist_config(cx);
            }
            SettingsAction::ToggleLcuRealtime => {
                self.app_config.functions.lcu_realtime_enabled =
                    !self.app_config.functions.lcu_realtime_enabled;
                self.persist_config(cx);
            }
            SettingsAction::SetLcuUserId(id) => {
                self.app_config.functions.lcu_user_id = id;
                self.persist_config(cx);
            }
            SettingsAction::ToggleSignalrHub => {
                self.app_config.general.enable_signalr_hub =
                    !self.app_config.general.enable_signalr_hub;
                self.persist_config(cx);
            }
            SettingsAction::SetSignalrUser(u) => {
                self.app_config.general.signalr_user_id = u;
                self.persist_config(cx);
            }
            SettingsAction::ToggleHideTft => {
                self.app_config.functions.hide_tft = !self.app_config.functions.hide_tft;
                self.persist_config(cx);
            }
            SettingsAction::ToggleHideSavedPlayers => {
                self.app_config.functions.hide_saved_players =
                    !self.app_config.functions.hide_saved_players;
                self.persist_config(cx);
            }
            SettingsAction::ToggleScreenshotOnMultikill => {
                self.app_config.functions.enable_screenshot_on_multikill =
                    !self.app_config.functions.enable_screenshot_on_multikill;
                self.persist_config(cx);
            }
            SettingsAction::ToggleScreenshotLevel(lvl) => {
                if let Some(pos) = self
                    .app_config
                    .functions
                    .screenshot_on_multikill_levels
                    .iter()
                    .position(|l| *l == lvl)
                {
                    self.app_config
                        .functions
                        .screenshot_on_multikill_levels
                        .remove(pos);
                } else {
                    self.app_config
                        .functions
                        .screenshot_on_multikill_levels
                        .push(lvl);
                }
                self.persist_config(cx);
            }
            SettingsAction::SelectScreenshotFolder => {
                // 对照 Vue B14：取消选择不提示
                if let Some(folder) = services::select_folder() {
                    self.settings_view.f_inputs[3] = folder.clone();
                    self.app_config.functions.screenshot_save_path = folder;
                    self.persist_config_silent(cx);
                    self.toast(ToastKind::Success, crate::i18n::t("settings.screenshotDirUpdated"), cx);
                }
            }
            SettingsAction::OpenScreenshotFolder => {
                // 对照 Vue B16：失败以 Toast 呈现（原实现仅写日志）
                let path = self.app_config.functions.screenshot_save_path.clone();
                cx.spawn(async move |entity, cx| {
                    let res = cx
                        .background_executor()
                        .spawn(async move {
                            services::screenshot::screenshot_dir(&path)
                                .and_then(|dir| services::open_folder(dir))
                        })
                        .await;
                    if let Err(e) = res {
                        let _ = entity.update(cx, |this, cx| {
                            this.toast(
                                ToastKind::Error,
                                crate::i18n::t("settings.screenshotFolderOpenFailed").replace("{error}", &e),
                                cx,
                            );
                        });
                    }
                })
                .detach();
            }
            SettingsAction::SetLogLevel(lvl) => {
                self.settings_view.open_log_level_dropdown = false;
                self.app_config.general.log_level = lvl;
                services::logging::set_level(lvl);
                self.persist_config(cx);
            }
            SettingsAction::OpenLogFolder => {
                // 对照 Vue B24：失败以 Toast 呈现（原实现完全丢弃结果）
                cx.spawn(async move |entity, cx| {
                    let res = cx
                        .background_executor()
                        .spawn(async move { services::open_folder(services::logging::log_dir()) })
                        .await;
                    if let Err(e) = res {
                        let _ = entity.update(cx, |this, cx| {
                            this.toast(
                                ToastKind::Error,
                                crate::i18n::t("settings.logFolderOpenFailed").replace("{error}", &e),
                                cx,
                            );
                        });
                    }
                })
                .detach();
            }
            SettingsAction::ToggleMica => {
                let new_val = !self.app_config.personalization.mica_enabled;
                if new_val != self.mica_active {
                    if let Err(error) = platform::native_window_handle(window)
                        .and_then(|hwnd| platform::set_mica(hwnd, new_val))
                    {
                        log::warn!("[Platform] 切换云母效果失败: {error}");
                        self.toast(ToastKind::Error, crate::i18n::t("settings.micaError"), cx);
                        return;
                    }
                }
                self.mica_active = new_val;
                self.app_config.personalization.mica_enabled = new_val;
                self.persist_config(cx);
            }
            SettingsAction::SetThemeMode(mode) => {
                self.settings_view.open_theme_mode_dropdown = false;
                self.app_config.personalization.theme_mode = mode;
                self.persist_config(cx);
            }
            SettingsAction::SetThemeColor(color) => {
                self.settings_view.color_hex_input = color.clone();
                self.app_config.personalization.theme_color = color;
                self.persist_config(cx);
            }
            SettingsAction::SetWinCardColor(color) => {
                self.settings_view.color_hex_input = color.clone();
                self.app_config.personalization.win_card_color = color;
                self.persist_config(cx);
            }
            SettingsAction::SetLoseCardColor(color) => {
                self.settings_view.color_hex_input = color.clone();
                self.app_config.personalization.lose_card_color = color;
                self.persist_config(cx);
            }
            SettingsAction::SetRemakeCardColor(color) => {
                self.settings_view.color_hex_input = color.clone();
                self.app_config.personalization.remake_card_color = color;
                self.persist_config(cx);
            }
            SettingsAction::SetLightDeathsColor(color) => {
                self.settings_view.color_hex_input = color.clone();
                self.app_config.personalization.light_deaths_number_color = color;
                self.persist_config(cx);
            }
            SettingsAction::SetDarkDeathsColor(color) => {
                self.settings_view.color_hex_input = color.clone();
                self.app_config.personalization.dark_deaths_number_color = color;
                self.persist_config(cx);
            }
            SettingsAction::ResetThemeColor => {
                self.settings_view.close_all_color_pickers();
                self.app_config.personalization.theme_color = "#009faa".to_string();
                self.settings_view.color_hex_input = "#009faa".to_string();
                self.persist_config(cx);
            }
            SettingsAction::ResetCardColors => {
                self.settings_view.close_all_color_pickers();
                self.app_config.personalization.win_card_color = "#3339B01B".to_string();
                self.app_config.personalization.lose_card_color = "#33D3190C".to_string();
                self.app_config.personalization.remake_card_color = "#33A2A2A2".to_string();
                self.persist_config(cx);
            }
            SettingsAction::ResetDeathsColors => {
                self.settings_view.close_all_color_pickers();
                self.app_config.personalization.light_deaths_number_color = "#ffb60000".to_string();
                self.app_config.personalization.dark_deaths_number_color = "#ffff8d8d".to_string();
                self.persist_config(cx);
            }
            SettingsAction::SampleSv(key, s, v) => {
                self.settings_view.color_picker_s = s.clamp(0.0, 1.0);
                self.settings_view.color_picker_v = v.clamp(0.0, 1.0);
                let h = self.settings_view.color_picker_h;
                let a = self.settings_view.color_picker_a;
                let supports_alpha = matches!(key, "win_color" | "lose_color" | "remake_color");
                let hex = crate::views::settings::hsva_to_hex(
                    h,
                    self.settings_view.color_picker_s,
                    self.settings_view.color_picker_v,
                    a,
                    supports_alpha,
                );
                self.settings_view.color_hex_input = hex.clone();
                match key {
                    "theme_color" => self.app_config.personalization.theme_color = hex,
                    "win_color" => self.app_config.personalization.win_card_color = hex,
                    "lose_color" => self.app_config.personalization.lose_card_color = hex,
                    "remake_color" => self.app_config.personalization.remake_card_color = hex,
                    "light_death" => {
                        self.app_config.personalization.light_deaths_number_color = hex
                    }
                    "dark_death" => {
                        self.app_config.personalization.dark_deaths_number_color = hex
                    }
                    _ => return,
                }
                self.persist_config_silent(cx);
            }
            SettingsAction::SampleHue(key, h) => {
                self.settings_view.color_picker_h = h.rem_euclid(360.0);
                let s = self.settings_view.color_picker_s;
                let v = self.settings_view.color_picker_v;
                let a = self.settings_view.color_picker_a;
                let supports_alpha = matches!(key, "win_color" | "lose_color" | "remake_color");
                let hex = crate::views::settings::hsva_to_hex(
                    self.settings_view.color_picker_h,
                    s,
                    v,
                    a,
                    supports_alpha,
                );
                self.settings_view.color_hex_input = hex.clone();
                match key {
                    "theme_color" => self.app_config.personalization.theme_color = hex,
                    "win_color" => self.app_config.personalization.win_card_color = hex,
                    "lose_color" => self.app_config.personalization.lose_card_color = hex,
                    "remake_color" => self.app_config.personalization.remake_card_color = hex,
                    "light_death" => {
                        self.app_config.personalization.light_deaths_number_color = hex
                    }
                    "dark_death" => {
                        self.app_config.personalization.dark_deaths_number_color = hex
                    }
                    _ => return,
                }
                self.persist_config_silent(cx);
            }
            SettingsAction::SampleAlpha(key, a) => {
                self.settings_view.color_picker_a = a.clamp(0.0, 1.0);
                let h = self.settings_view.color_picker_h;
                let s = self.settings_view.color_picker_s;
                let v = self.settings_view.color_picker_v;
                let hex = crate::views::settings::hsva_to_hex(
                    h,
                    s,
                    v,
                    self.settings_view.color_picker_a,
                    true,
                );
                self.settings_view.color_hex_input = hex.clone();
                match key {
                    "win_color" => self.app_config.personalization.win_card_color = hex,
                    "lose_color" => self.app_config.personalization.lose_card_color = hex,
                    "remake_color" => self.app_config.personalization.remake_card_color = hex,
                    _ => return,
                }
                self.persist_config_silent(cx);
            }
            SettingsAction::SelectColorPreset(key, hex) => {
                let (h, s, v, a) = crate::views::settings::hex_to_hsva(&hex);
                self.settings_view.color_picker_key = key;
                self.settings_view.color_picker_h = h;
                self.settings_view.color_picker_s = s;
                self.settings_view.color_picker_v = v;
                self.settings_view.color_picker_a = a;
                self.settings_view.color_hex_input = hex.clone();
                match key {
                    "theme_color" => self.app_config.personalization.theme_color = hex,
                    "win_color" => self.app_config.personalization.win_card_color = hex,
                    "lose_color" => self.app_config.personalization.lose_card_color = hex,
                    "remake_color" => self.app_config.personalization.remake_card_color = hex,
                    "light_death" => {
                        self.app_config.personalization.light_deaths_number_color = hex
                    }
                    "dark_death" => {
                        self.app_config.personalization.dark_deaths_number_color = hex
                    }
                    _ => return,
                }
                self.persist_config(cx);
            }
            SettingsAction::EditColorPickerHex(val) => {
                self.settings_view.color_hex_input = val;
                cx.notify();
            }
            SettingsAction::ApplyColorPickerHex(key) => {
                let input = self.settings_view.color_hex_input.trim();
                let supports_alpha = matches!(key, "win_color" | "lose_color" | "remake_color");
                let cur_alpha = (self.settings_view.color_picker_a * 100.0).round() as u32;
                if let Some(valid) =
                    crate::views::settings::normalize_hex_input(input, supports_alpha, cur_alpha)
                {
                    let (h, s, v, a) = crate::views::settings::hex_to_hsva(&valid);
                    self.settings_view.color_picker_h = h;
                    self.settings_view.color_picker_s = s;
                    self.settings_view.color_picker_v = v;
                    self.settings_view.color_picker_a = a;
                    self.settings_view.color_hex_input = valid.clone();
                    match key {
                        "theme_color" => {
                            self.app_config.personalization.theme_color = valid
                        }
                        "win_color" => {
                            self.app_config.personalization.win_card_color = valid
                        }
                        "lose_color" => {
                            self.app_config.personalization.lose_card_color = valid
                        }
                        "remake_color" => {
                            self.app_config.personalization.remake_card_color = valid
                        }
                        "light_death" => {
                            self.app_config.personalization.light_deaths_number_color = valid
                        }
                        "dark_death" => {
                            self.app_config.personalization.dark_deaths_number_color = valid
                        }
                        _ => return,
                    }
                    self.persist_config(cx);
                } else {
                    self.toast(
                        ToastKind::Error,
                        "请输入合法的十六进制颜色代码（如 #1890FF 或 #3339B01B）",
                        cx,
                    );
                    cx.notify();
                }
            }
            SettingsAction::CloseColorPicker => {
                self.settings_view.close_all_color_pickers();
                cx.notify();
            }
            SettingsAction::SetDpiScale(s) => {
                self.settings_view.open_scale_dropdown = false;
                let scale_override = scale_factor_override_for(&s);
                self.app_config.personalization.dpi_scale = s;
                self.persist_config_silent(cx);
                // 运行时立即生效：Auto=恢复跟随系统 DPI，100/125/150=固定缩放
                window.set_scale_factor_override(scale_override);
                // 对照 Vue B12：DPI 专用成功提示
                self.toast(ToastKind::Success, crate::i18n::t("settings.dpiScaleApplied"), cx);
            }
            SettingsAction::SetLanguage(l) => {
                self.settings_view.open_lang_dropdown = false;
                self.app_config.personalization.language = l;
                self.persist_config_silent(cx);
                // 对照 Vue B13：语言专用成功提示
                self.toast(ToastKind::Success, crate::i18n::t("settings.langSaved"), cx);
            }
            SettingsAction::CheckUpdate => {
                self.check_updates(true, cx);
            }
            SettingsAction::ToggleCheckUpdate => {
                self.app_config.general.enable_check_update =
                    !self.app_config.general.enable_check_update;
                self.persist_config(cx);
            }
            SettingsAction::GoToReleases => {
                #[cfg(target_os = "windows")]
                let _ = std::process::Command::new("cmd")
                    .args([
                        "/c",
                        "start",
                        "",
                        "https://github.com/ISuuuu/Yuumi/releases/latest",
                    ])
                    .spawn();
            }
            SettingsAction::ToggleHttpProxy => {
                self.app_config.general.enable_http_proxy =
                    !self.app_config.general.enable_http_proxy;
                self.persist_config(cx);
                self.apply_opgg_proxy(cx);
            }
            SettingsAction::ToggleChangelog => {
                self.settings_view.show_changelog = !self.settings_view.show_changelog;
                if self.settings_view.show_changelog {
                    self.load_release_notes(false, cx);
                }
                cx.notify();
            }
            SettingsAction::RefreshChangelog => self.load_release_notes(true, cx),
            SettingsAction::OpenRepo => {
                #[cfg(target_os = "windows")]
                let _ = std::process::Command::new("cmd")
                    .args(["/c", "start", "", "https://github.com/ISuuuu/Yuumi"])
                    .spawn();
            }
            SettingsAction::ToggleDropdown(name) => {
                match name {
                    "log_level" => {
                        self.settings_view.open_log_level_dropdown =
                            !self.settings_view.open_log_level_dropdown;
                    }
                    "theme_mode" => {
                        self.settings_view.open_theme_mode_dropdown =
                            !self.settings_view.open_theme_mode_dropdown;
                    }
                    "dpi_scale" => {
                        self.settings_view.open_scale_dropdown =
                            !self.settings_view.open_scale_dropdown;
                    }
                    "language" => {
                        self.settings_view.open_lang_dropdown =
                            !self.settings_view.open_lang_dropdown;
                    }
                    "theme_color" => {
                        let next = !self.settings_view.open_theme_color_picker;
                        self.settings_view.close_all_color_pickers();
                        self.settings_view.open_theme_color_picker = next;
                        if next {
                            let (h, s, v, a) = crate::views::settings::hex_to_hsva(
                                &self.app_config.personalization.theme_color,
                            );
                            self.settings_view.color_picker_key = "theme_color";
                            self.settings_view.color_picker_h = h;
                            self.settings_view.color_picker_s = s;
                            self.settings_view.color_picker_v = v;
                            self.settings_view.color_picker_a = a;
                            self.settings_view.color_hex_input =
                                self.app_config.personalization.theme_color.clone();
                        }
                    }
                    "win_color" => {
                        let next = !self.settings_view.open_win_color_picker;
                        self.settings_view.close_all_color_pickers();
                        self.settings_view.open_win_color_picker = next;
                        if next {
                            let (h, s, v, a) = crate::views::settings::hex_to_hsva(
                                &self.app_config.personalization.win_card_color,
                            );
                            self.settings_view.color_picker_key = "win_color";
                            self.settings_view.color_picker_h = h;
                            self.settings_view.color_picker_s = s;
                            self.settings_view.color_picker_v = v;
                            self.settings_view.color_picker_a = a;
                            self.settings_view.color_hex_input =
                                self.app_config.personalization.win_card_color.clone();
                        }
                    }
                    "lose_color" => {
                        let next = !self.settings_view.open_lose_color_picker;
                        self.settings_view.close_all_color_pickers();
                        self.settings_view.open_lose_color_picker = next;
                        if next {
                            let (h, s, v, a) = crate::views::settings::hex_to_hsva(
                                &self.app_config.personalization.lose_card_color,
                            );
                            self.settings_view.color_picker_key = "lose_color";
                            self.settings_view.color_picker_h = h;
                            self.settings_view.color_picker_s = s;
                            self.settings_view.color_picker_v = v;
                            self.settings_view.color_picker_a = a;
                            self.settings_view.color_hex_input =
                                self.app_config.personalization.lose_card_color.clone();
                        }
                    }
                    "remake_color" => {
                        let next = !self.settings_view.open_remake_color_picker;
                        self.settings_view.close_all_color_pickers();
                        self.settings_view.open_remake_color_picker = next;
                        if next {
                            let (h, s, v, a) = crate::views::settings::hex_to_hsva(
                                &self.app_config.personalization.remake_card_color,
                            );
                            self.settings_view.color_picker_key = "remake_color";
                            self.settings_view.color_picker_h = h;
                            self.settings_view.color_picker_s = s;
                            self.settings_view.color_picker_v = v;
                            self.settings_view.color_picker_a = a;
                            self.settings_view.color_hex_input =
                                self.app_config.personalization.remake_card_color.clone();
                        }
                    }
                    "light_death" => {
                        let next = !self.settings_view.open_light_death_color_picker;
                        self.settings_view.close_all_color_pickers();
                        self.settings_view.open_light_death_color_picker = next;
                        if next {
                            let (h, s, v, a) = crate::views::settings::hex_to_hsva(
                                &self.app_config.personalization.light_deaths_number_color,
                            );
                            self.settings_view.color_picker_key = "light_death";
                            self.settings_view.color_picker_h = h;
                            self.settings_view.color_picker_s = s;
                            self.settings_view.color_picker_v = v;
                            self.settings_view.color_picker_a = a;
                            self.settings_view.color_hex_input =
                                self.app_config.personalization.light_deaths_number_color.clone();
                        }
                    }
                    "dark_death" => {
                        let next = !self.settings_view.open_dark_death_color_picker;
                        self.settings_view.close_all_color_pickers();
                        self.settings_view.open_dark_death_color_picker = next;
                        if next {
                            let (h, s, v, a) = crate::views::settings::hex_to_hsva(
                                &self.app_config.personalization.dark_deaths_number_color,
                            );
                            self.settings_view.color_picker_key = "dark_death";
                            self.settings_view.color_picker_h = h;
                            self.settings_view.color_picker_s = s;
                            self.settings_view.color_picker_v = v;
                            self.settings_view.color_picker_a = a;
                            self.settings_view.color_hex_input =
                                self.app_config.personalization.dark_deaths_number_color.clone();
                        }
                    }
                    _ => {}
                }
                cx.notify();
            }
        }
    }

    /// 按预设索引选择主题色（基线已有的动作，当前渲染走 SettingAction 路径）
    #[allow(dead_code)]
    pub(super) fn select_theme_index(&mut self, idx: usize, cx: &mut Context<Self>) {
        if let Some((color, _)) = crate::views::settings::THEME_COLOR_PRESETS.get(idx) {
            self.app_config.personalization.theme_color = color.to_string();
            self.persist_config(cx);
        }
    }

    /// 设置路由渲染与 SettingsAction 回调。
    pub(super) fn render_settings_route(
        &mut self,
        el: Stateful<Div>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let view_entity = cx.entity().clone();
        let view = view_entity.clone();
        let on_settings_action = move |action, win: &mut Window, app: &mut App| {
            view.update(app, |this, cx| {
                this.handle_settings_action(action, win, cx);
            });
        };
        el.child(
            self.settings_view
                .render_panel(&self.app_config, window, on_settings_action),
        )
    }
}
