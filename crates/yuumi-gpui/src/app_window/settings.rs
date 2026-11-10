// 设置路由：SettingsAction 分发、主题色与外观持久化。
use super::*;

impl MainWindow {
    /// 处理设置页交互动作
    pub(super) fn handle_settings_action(
        &mut self,
        action: SettingsAction,
        cx: &mut Context<Self>,
    ) {
        match action {
            SettingsAction::ToggleCollapse(name) => {
                self.settings_view.toggle_section(name);
                cx.notify();
            }
            SettingsAction::SetApiConcurrency(val) => {
                self.app_config.functions.api_concurrency_number = val;
                self.persist_config(cx);
            }
            SettingsAction::SetCareerGames(val) => {
                self.app_config.functions.career_games_number = val;
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
                self.app_config.functions.enable_opgg_on_top =
                    !self.app_config.functions.enable_opgg_on_top;
                self.persist_config(cx);
            }
            SettingsAction::DetectLolPath => {
                if let Some(path) = services::detect_lol_path() {
                    if !self.app_config.general.lol_path.contains(&path) {
                        self.app_config.general.lol_path.push(path.clone());
                        self.persist_config(cx);
                    }
                    self.settings_view
                        .set_feedback(format!("已检测到客户端: {}", path), false);
                } else {
                    self.settings_view
                        .set_feedback("未检测到运行中的英雄联盟客户端", true);
                }
                cx.notify();
            }
            SettingsAction::SelectLolFolder => {
                if let Some(path) = services::select_folder() {
                    if !self.app_config.general.lol_path.contains(&path) {
                        self.app_config.general.lol_path.push(path.clone());
                        self.persist_config(cx);
                    }
                    self.settings_view
                        .set_feedback(format!("已添加目录: {}", path), false);
                }
                cx.notify();
            }
            SettingsAction::AddWegamePath => {
                let wegame_marker = yuumi_core::config::WEGAME_MARKER.to_string();
                if !self.app_config.general.lol_path.contains(&wegame_marker) {
                    self.app_config.general.lol_path.push(wegame_marker);
                    self.persist_config(cx);
                }
                self.settings_view
                    .set_feedback("已添加 WeGame 启动项", false);
                cx.notify();
            }
            SettingsAction::RemoveLolPath(idx) => {
                if idx < self.app_config.general.lol_path.len() {
                    self.app_config.general.lol_path.remove(idx);
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
                std::thread::spawn(move || {
                    let _ = services::clear_game_cache(client_path.as_deref());
                });
                self.settings_view.set_feedback("缓存清理完成", false);
                cx.notify();
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
                        || value.strip_prefix("http://").or_else(|| value.strip_prefix("https://"))
                            .and_then(|rest| rest.split('/').next())
                            .is_none_or(|host| host.is_empty()))
                {
                    self.settings_view
                        .set_feedback("请输入有效的 HTTP 或 HTTPS 上传地址", true);
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
                    self.settings_view
                        .set_feedback("请输入有效的代理地址（例如 127.0.0.1:7897）", true);
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
            SettingsAction::SetSignalrServer(s) => {
                self.app_config.general.signalr_server_url = s;
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
                if let Some(folder) = services::select_folder() {
                    self.settings_view.f_inputs[3] = folder.clone();
                    self.app_config.functions.screenshot_save_path = folder;
                    self.persist_config(cx);
                }
            }
            SettingsAction::OpenScreenshotFolder => {
                let path = self.app_config.functions.screenshot_save_path.clone();
                std::thread::spawn(move || match services::screenshot::screenshot_dir(&path) {
                    Ok(p) => {
                    let _ = services::open_folder(p);
                    }
                    Err(e) => log::error!("打开截图目录失败: {e}"),
                });
            }
            SettingsAction::SetLogLevel(lvl) => {
                self.settings_view.open_log_level_dropdown = false;
                self.app_config.general.log_level = lvl;
                services::logging::set_level(lvl);
                self.persist_config(cx);
            }
            SettingsAction::OpenLogFolder => {
                std::thread::spawn(|| {
                    let _ = services::open_folder(services::logging::log_dir());
                });
            }
            SettingsAction::ToggleMica => {
                self.app_config.personalization.mica_enabled =
                    !self.app_config.personalization.mica_enabled;
                self.persist_config(cx);
            }
            SettingsAction::SetThemeMode(mode) => {
                self.settings_view.open_theme_mode_dropdown = false;
                self.app_config.personalization.theme_mode = mode;
                self.persist_config(cx);
            }
            SettingsAction::SetThemeColor(color) => {
                self.settings_view.open_theme_color_picker = false;
                self.app_config.personalization.theme_color = color;
                self.persist_config(cx);
            }
            SettingsAction::SetWinCardColor(color) => {
                self.settings_view.open_win_color_picker = false;
                self.app_config.personalization.win_card_color = color;
                self.persist_config(cx);
            }
            SettingsAction::SetLoseCardColor(color) => {
                self.settings_view.open_lose_color_picker = false;
                self.app_config.personalization.lose_card_color = color;
                self.persist_config(cx);
            }
            SettingsAction::SetRemakeCardColor(color) => {
                self.settings_view.open_remake_color_picker = false;
                self.app_config.personalization.remake_card_color = color;
                self.persist_config(cx);
            }
            SettingsAction::SetLightDeathsColor(color) => {
                self.settings_view.open_light_death_color_picker = false;
                self.app_config.personalization.light_deaths_number_color = color;
                self.persist_config(cx);
            }
            SettingsAction::SetDarkDeathsColor(color) => {
                self.settings_view.open_dark_death_color_picker = false;
                self.app_config.personalization.dark_deaths_number_color = color;
                self.persist_config(cx);
            }
            SettingsAction::ResetThemeColor => {
                self.settings_view.open_theme_color_picker = false;
                self.app_config.personalization.theme_color = "#009faa".to_string();
                self.persist_config(cx);
            }
            SettingsAction::ResetCardColors => {
                self.settings_view.open_win_color_picker = false;
                self.settings_view.open_lose_color_picker = false;
                self.settings_view.open_remake_color_picker = false;
                self.app_config.personalization.win_card_color = "#3339B01B".to_string();
                self.app_config.personalization.lose_card_color = "#33D3190C".to_string();
                self.app_config.personalization.remake_card_color = "#33A2A2A2".to_string();
                self.persist_config(cx);
            }
            SettingsAction::ResetDeathsColors => {
                self.settings_view.open_light_death_color_picker = false;
                self.settings_view.open_dark_death_color_picker = false;
                self.app_config.personalization.light_deaths_number_color = "#ffb60000".to_string();
                self.app_config.personalization.dark_deaths_number_color = "#ffff8d8d".to_string();
                self.persist_config(cx);
            }
            SettingsAction::SetDpiScale(s) => {
                self.settings_view.open_scale_dropdown = false;
                self.app_config.personalization.dpi_scale = s;
                self.persist_config(cx);
            }
            SettingsAction::SetLanguage(l) => {
                self.settings_view.open_lang_dropdown = false;
                self.app_config.personalization.language = l;
                self.persist_config(cx);
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
            }
            SettingsAction::SetHttpProxyAddr(addr) => {
                self.app_config.general.http_proxy_addr = addr;
                self.persist_config(cx);
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
                        self.settings_view.open_theme_color_picker =
                            !self.settings_view.open_theme_color_picker;
                    }
                    "win_color" => {
                        self.settings_view.open_win_color_picker =
                            !self.settings_view.open_win_color_picker;
                    }
                    "lose_color" => {
                        self.settings_view.open_lose_color_picker =
                            !self.settings_view.open_lose_color_picker;
                    }
                    "remake_color" => {
                        self.settings_view.open_remake_color_picker =
                            !self.settings_view.open_remake_color_picker;
                    }
                    "light_death" => {
                        self.settings_view.open_light_death_color_picker =
                            !self.settings_view.open_light_death_color_picker;
                    }
                    "dark_death" => {
                        self.settings_view.open_dark_death_color_picker =
                            !self.settings_view.open_dark_death_color_picker;
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
        let on_settings_action = move |action, _win: &mut Window, app: &mut App| {
            view.update(app, |this, cx| {
                this.handle_settings_action(action, cx);
            });
        };
        el.child(
            self.settings_view
                .render_panel(&self.app_config, window, on_settings_action),
        )
    }
}
