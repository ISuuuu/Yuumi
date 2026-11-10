//! 全局配置数据模型与持久化管理
//!
//! 与磁盘文件 `%APPDATA%/Yuumi/config.json` 保持 100% 结构兼容（PascalCase 字段映射）。
//! 支持加载、回写以及与自动化运行时代理（`AutomationSettings`）的双向同步。

use crate::agents::config::AutomationSettings;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const CONFIG_VERSION: u32 = 3;
pub const WEGAME_MARKER: &str = "WeGame";

fn default_true() -> bool {
    true
}

fn default_theme_color() -> String {
    "#009faa".into()
}

fn default_screenshot_levels() -> Vec<u32> {
    vec![3, 4, 5, 8]
}

// ─── 通用设置 ───

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[serde(default)]
pub struct GeneralConfig {
    pub lol_path: Vec<String>,
    pub wegame_path: Option<String>,
    pub enable_start_lol_with_app: bool,
    pub enable_close_to_tray: Option<bool>,
    pub enable_game_start_minimize: bool,
    pub enable_check_update: bool,
    pub log_level: u32,
    #[serde(alias = "EnableGithubProxy")]
    pub enable_http_proxy: bool,
    #[serde(alias = "GithubProxyAddr")]
    pub http_proxy_addr: String,

    pub enable_signalr_hub: bool,
    pub signalr_server_url: String,
    pub signalr_user_id: String,

    pub upload_api_url: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            lol_path: Vec::new(),
            wegame_path: None,
            enable_start_lol_with_app: false,
            enable_close_to_tray: Some(true),
            enable_game_start_minimize: false,
            enable_check_update: true,
            log_level: 1,
            enable_http_proxy: false,
            http_proxy_addr: "127.0.0.1:7897".into(),
            enable_signalr_hub: false,
            signalr_server_url: String::new(),
            signalr_user_id: String::new(),
            upload_api_url: String::new(),
        }
    }
}

// ─── 个性化设置 ───

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[serde(default)]
pub struct PersonalizationConfig {
    pub mica_enabled: bool,
    pub dpi_scale: String,
    pub language: String,
    pub theme_mode: String,
    pub win_card_color: String,
    pub lose_card_color: String,
    pub remake_card_color: String,
    pub light_deaths_number_color: String,
    pub dark_deaths_number_color: String,
    #[serde(default = "default_theme_color")]
    pub theme_color: String,
}

impl Default for PersonalizationConfig {
    fn default() -> Self {
        Self {
            mica_enabled: true,
            dpi_scale: "Auto".into(),
            language: "Auto".into(),
            theme_mode: "Auto".into(),
            win_card_color: "#3339B01B".into(),
            lose_card_color: "#33D3190C".into(),
            remake_card_color: "#33A2A2A2".into(),
            light_deaths_number_color: "#ffb60000".into(),
            dark_deaths_number_color: "#ffff8d8d".into(),
            theme_color: "#009faa".into(),
        }
    }
}

// ─── 功能设置 ───

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[serde(default)]
pub struct FunctionsConfig {
    pub career_games_number: u32,
    pub api_concurrency_number: u32,
    pub game_info_filter: bool,
    pub show_tier_in_game_info: bool,
    pub auto_show_opgg: bool,
    pub enable_opgg_on_top: bool,
    pub enable_auto_accept_matching: bool,
    pub enable_auto_reconnect: bool,
    pub enable_auto_create_lobby: bool,
    pub default_game_mode: u32,
    pub auto_accept_matching_delay: u32,
    pub enable_random_skin: bool,

    // 自动选人（通用 + 分路）
    pub enable_auto_select_champion: bool,
    pub auto_select_champion: Vec<i32>,
    pub auto_select_champion_top: Vec<i32>,
    pub auto_select_champion_jug: Vec<i32>,
    pub auto_select_champion_mid: Vec<i32>,
    pub auto_select_champion_bot: Vec<i32>,
    pub auto_select_champion_sup: Vec<i32>,

    // 自动禁人（通用 + 分路）
    pub enable_auto_ban_champion: bool,
    pub auto_ban_champion: Vec<i32>,
    pub auto_ban_champion_top: Vec<i32>,
    pub auto_ban_champion_jug: Vec<i32>,
    pub auto_ban_champion_mid: Vec<i32>,
    pub auto_ban_champion_bot: Vec<i32>,
    pub auto_ban_champion_sup: Vec<i32>,
    pub auto_ban_delay: f64,
    pub pretend_ban: bool,

    // 自动交换
    pub auto_accept_ceil_swap: bool,
    pub auto_accept_champ_trade: bool,

    // 自动召唤师技能（通用 + 分路）
    pub enable_auto_set_spells: bool,
    pub auto_set_summoner_spell: Vec<i32>,
    pub auto_set_summoner_spell_top: Vec<i32>,
    pub auto_set_summoner_spell_jug: Vec<i32>,
    pub auto_set_summoner_spell_mid: Vec<i32>,
    pub auto_set_summoner_spell_bot: Vec<i32>,
    pub auto_set_summoner_spell_sup: Vec<i32>,

    // 对局信息保留 & LCU 实时查询
    pub enable_reserve_gameinfo: bool,
    pub lcu_realtime_enabled: bool,
    pub lcu_user_id: String,
    pub enable_auto_hover_champion: bool,
    pub auto_select_confirm_on_timeout: bool,

    #[serde(default = "default_true")]
    pub upload_enabled: bool,
    #[serde(default = "default_true")]
    pub hide_tft: bool,
    pub hide_saved_players: bool,
    #[serde(default = "default_true")]
    pub enable_bench_overlay: bool,

    pub enable_screenshot_on_multikill: bool,
    #[serde(default = "default_screenshot_levels")]
    pub screenshot_on_multikill_levels: Vec<u32>,
    pub screenshot_save_path: String,

    // 自动游戏流程
    pub enable_auto_handle_invite: bool,
    pub enable_auto_honor: bool,
    pub enable_auto_play_again: bool,
    pub enable_auto_aram_team_side: bool,
    pub aram_team_side_visible_to_team: bool,
    pub enable_auto_tag_reminder: bool,
    pub watched_friend_puuids: Vec<String>,
    pub enable_friend_radar: bool,
    pub enable_dodge_in_game_info: bool,
    /// 再来一局兜底延迟（秒）：与 `AutoMatchConfig.auto_play_again_delay` 双向映射 // [D05-a]
    pub auto_play_again_delay: u32,
    /// 自动同意队友换人 / 换楼：`AutomationSettings.auto_trade` 的 AppConfig 等价字段 // [D05-a]
    pub auto_trade: bool,
}

impl Default for FunctionsConfig {
    fn default() -> Self {
        Self {
            career_games_number: 20,
            api_concurrency_number: 5,
            game_info_filter: false,
            show_tier_in_game_info: false,
            auto_show_opgg: false,
            enable_opgg_on_top: false,
            enable_auto_accept_matching: false,
            enable_auto_reconnect: false,
            enable_auto_create_lobby: false,
            default_game_mode: 2400,
            auto_accept_matching_delay: 0,
            enable_random_skin: false,
            enable_auto_select_champion: false,
            auto_select_champion: Vec::new(),
            auto_select_champion_top: Vec::new(),
            auto_select_champion_jug: Vec::new(),
            auto_select_champion_mid: Vec::new(),
            auto_select_champion_bot: Vec::new(),
            auto_select_champion_sup: Vec::new(),
            enable_auto_ban_champion: false,
            auto_ban_champion: Vec::new(),
            auto_ban_champion_top: Vec::new(),
            auto_ban_champion_jug: Vec::new(),
            auto_ban_champion_mid: Vec::new(),
            auto_ban_champion_bot: Vec::new(),
            auto_ban_champion_sup: Vec::new(),
            auto_ban_delay: 0.0,
            pretend_ban: false,
            auto_accept_ceil_swap: false,
            auto_accept_champ_trade: false,
            enable_auto_set_spells: false,
            auto_set_summoner_spell: vec![4, 14],
            auto_set_summoner_spell_top: Vec::new(),
            auto_set_summoner_spell_jug: Vec::new(),
            auto_set_summoner_spell_mid: Vec::new(),
            auto_set_summoner_spell_bot: Vec::new(),
            auto_set_summoner_spell_sup: Vec::new(),
            enable_reserve_gameinfo: false,
            lcu_realtime_enabled: false,
            lcu_user_id: String::new(),
            enable_auto_hover_champion: false,
            auto_select_confirm_on_timeout: true,
            upload_enabled: true,
            hide_tft: true,
            hide_saved_players: false,
            enable_bench_overlay: true,
            enable_screenshot_on_multikill: false,
            screenshot_on_multikill_levels: default_screenshot_levels(),
            screenshot_save_path: String::new(),
            enable_auto_handle_invite: false,
            enable_auto_honor: false,
            enable_auto_play_again: false,
            enable_auto_aram_team_side: false,
            aram_team_side_visible_to_team: false,
            enable_auto_tag_reminder: false,
            watched_friend_puuids: Vec::new(),
            enable_friend_radar: false,
            enable_dodge_in_game_info: false,
            auto_play_again_delay: 5,
            auto_trade: false,
        }
    }
}

// ─── 全局总配置结构 ───

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AppConfig {
    #[serde(rename = "Version", default = "default_config_version")]
    pub version: u32,
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub personalization: PersonalizationConfig,
    #[serde(default)]
    pub functions: FunctionsConfig,
    #[serde(flatten)]
    pub other: std::collections::HashMap<String, serde_json::Value>,
}

fn default_config_version() -> u32 {
    CONFIG_VERSION
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            general: GeneralConfig::default(),
            personalization: PersonalizationConfig::default(),
            functions: FunctionsConfig::default(),
            other: std::collections::HashMap::new(),
        }
    }
}

impl AppConfig {
    /// 配置文件默认路径：%APPDATA%/Yuumi/config.json
    pub fn default_path() -> PathBuf {
        crate::runtime::app_data_dir().join("config.json")
    }

    /// 从指定路径加载配置；文件缺失或损坏时平滑回退默认配置
    pub fn load_from_path(path: &Path) -> Self {
        if !path.exists() {
            log::info!("[Config] 配置文件不存在: {}，使用默认配置", path.display());
            return Self::default();
        }

        match std::fs::read_to_string(path) {
            Ok(content) => match serde_json::from_str::<Self>(&content) {
                Ok(cfg) => {
                    log::info!("[Config] 成功从 {} 加载配置", path.display());
                    cfg
                }
                Err(e) => {
                    log::warn!(
                        "[Config] 配置文件解析失败({}), 回退默认配置: {}",
                        path.display(),
                        e
                    );
                    Self::default()
                }
            },
            Err(e) => {
                log::warn!(
                    "[Config] 读取配置文件失败({}), 回退默认配置: {}",
                    path.display(),
                    e
                );
                Self::default()
            }
        }
    }

    /// 从默认路径加载全局配置
    pub fn load() -> Self {
        Self::load_from_path(&Self::default_path())
    }

    /// 从默认路径加载全局配置并返回异常信息（供 UI 首帧提示，对照 Vue A01）。
    /// 文件不存在视为首次启动，返回 `None`；读取或解析失败回退默认配置并携带原因。
    pub fn load_with_error() -> (Self, Option<String>) {
        let path = Self::default_path();
        if !path.exists() {
            return (Self::default(), None);
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str::<Self>(&content) {
                Ok(cfg) => (cfg, None),
                Err(e) => {
                    log::warn!(
                        "[Config] 配置文件解析失败({}), 回退默认配置: {}",
                        path.display(),
                        e
                    );
                    (Self::default(), Some(format!("解析失败: {}", e)))
                }
            },
            Err(e) => {
                log::warn!(
                    "[Config] 读取配置文件失败({}), 回退默认配置: {}",
                    path.display(),
                    e
                );
                (Self::default(), Some(format!("读取失败: {}", e)))
            }
        }
    }

    /// 写入指定路径（自动创建目录）
    pub fn save_to_path(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建配置目录失败 {}: {}", parent.display(), e))?;
        }
        let raw = serde_json::to_string_pretty(self)
            .map_err(|e| format!("序列化配置失败: {}", e))?;
        std::fs::write(path, raw)
            .map_err(|e| format!("写入配置文件失败 {}: {}", path.display(), e))
    }

    /// 保存配置到默认路径
    pub fn save(&self) -> Result<(), String> {
        Self::save_to_path(self, &Self::default_path())
    }

    /// 将 AppConfig 中相关的自动化配置导出为运行时 AutomationSettings
    pub fn to_automation_settings(&self) -> AutomationSettings {
        let f = &self.functions;
        AutomationSettings {
            auto_match: crate::agents::config::AutoMatchConfig {
                auto_accept: f.enable_auto_accept_matching,
                auto_accept_delay: f.auto_accept_matching_delay as u64,
                auto_reconnect: f.enable_auto_reconnect,
                auto_play_again: f.enable_auto_play_again,
                auto_play_again_delay: f.auto_play_again_delay as u64,
                auto_honor: f.enable_auto_honor,
                auto_handle_invite: f.enable_auto_handle_invite,
                auto_create_lobby: f.enable_auto_create_lobby,
                default_game_mode: f.default_game_mode,
                aram_team_side: f.enable_auto_aram_team_side,
                aram_team_side_visible: f.aram_team_side_visible_to_team,
                tag_reminder: f.enable_auto_tag_reminder,
            },
            auto_bp: crate::agents::config::AutoBpConfig {
                auto_pick: f.enable_auto_select_champion,
                auto_pick_intent: f.enable_auto_hover_champion,
                pick_champion_ids: f.auto_select_champion.clone(),
                pick_champion_ids_top: f.auto_select_champion_top.clone(),
                pick_champion_ids_jug: f.auto_select_champion_jug.clone(),
                pick_champion_ids_mid: f.auto_select_champion_mid.clone(),
                pick_champion_ids_bot: f.auto_select_champion_bot.clone(),
                pick_champion_ids_sup: f.auto_select_champion_sup.clone(),
                auto_ban: f.enable_auto_ban_champion,
                ban_champion_ids: f.auto_ban_champion.clone(),
                ban_champion_ids_top: f.auto_ban_champion_top.clone(),
                ban_champion_ids_jug: f.auto_ban_champion_jug.clone(),
                ban_champion_ids_mid: f.auto_ban_champion_mid.clone(),
                ban_champion_ids_bot: f.auto_ban_champion_bot.clone(),
                ban_champion_ids_sup: f.auto_ban_champion_sup.clone(),
                ban_delay_ms: (f.auto_ban_delay.max(0.0) * 1000.0).round() as u32,
                pretend_ban: f.pretend_ban,
                auto_spell: f.enable_auto_set_spells,
                spell1_id: f.auto_set_summoner_spell.first().copied().unwrap_or(4),
                spell2_id: f.auto_set_summoner_spell.get(1).copied().unwrap_or(14),
                spell_ids_top: f.auto_set_summoner_spell_top.clone(),
                spell_ids_jug: f.auto_set_summoner_spell_jug.clone(),
                spell_ids_mid: f.auto_set_summoner_spell_mid.clone(),
                spell_ids_bot: f.auto_set_summoner_spell_bot.clone(),
                spell_ids_sup: f.auto_set_summoner_spell_sup.clone(),
                confirm_on_timeout: f.auto_select_confirm_on_timeout,
                auto_pick_order_swap: f.auto_accept_ceil_swap,
                auto_champ_trade: f.auto_accept_champ_trade,
            },
            aram_bench_overlay: f.enable_bench_overlay,
            auto_trade: f.auto_trade,
        }
    }

    /// 根据运行时 AutomationSettings 同步更新 AppConfig 中的自动化键值
    pub fn sync_automation_settings(&mut self, s: &AutomationSettings) {
        let f = &mut self.functions;
        f.enable_auto_accept_matching = s.auto_match.auto_accept;
        f.auto_accept_matching_delay = s.auto_match.auto_accept_delay as u32;
        f.enable_auto_reconnect = s.auto_match.auto_reconnect;
        f.enable_auto_play_again = s.auto_match.auto_play_again;
        f.auto_play_again_delay = s
            .auto_match
            .auto_play_again_delay
            .min(u32::MAX as u64) as u32;
        f.enable_auto_honor = s.auto_match.auto_honor;
        f.enable_auto_handle_invite = s.auto_match.auto_handle_invite;
        f.enable_auto_create_lobby = s.auto_match.auto_create_lobby;
        f.default_game_mode = s.auto_match.default_game_mode;
        f.enable_auto_aram_team_side = s.auto_match.aram_team_side;
        f.aram_team_side_visible_to_team = s.auto_match.aram_team_side_visible;
        f.enable_auto_tag_reminder = s.auto_match.tag_reminder;

        f.enable_auto_select_champion = s.auto_bp.auto_pick;
        f.enable_auto_hover_champion = s.auto_bp.auto_pick_intent;
        f.auto_select_champion = s.auto_bp.pick_champion_ids.clone();
        f.auto_select_champion_top = s.auto_bp.pick_champion_ids_top.clone();
        f.auto_select_champion_jug = s.auto_bp.pick_champion_ids_jug.clone();
        f.auto_select_champion_mid = s.auto_bp.pick_champion_ids_mid.clone();
        f.auto_select_champion_bot = s.auto_bp.pick_champion_ids_bot.clone();
        f.auto_select_champion_sup = s.auto_bp.pick_champion_ids_sup.clone();

        f.enable_auto_ban_champion = s.auto_bp.auto_ban;
        f.auto_ban_champion = s.auto_bp.ban_champion_ids.clone();
        f.auto_ban_champion_top = s.auto_bp.ban_champion_ids_top.clone();
        f.auto_ban_champion_jug = s.auto_bp.ban_champion_ids_jug.clone();
        f.auto_ban_champion_mid = s.auto_bp.ban_champion_ids_mid.clone();
        f.auto_ban_champion_bot = s.auto_bp.ban_champion_ids_bot.clone();
        f.auto_ban_champion_sup = s.auto_bp.ban_champion_ids_sup.clone();
        f.auto_ban_delay = s.auto_bp.ban_delay_ms as f64 / 1000.0;
        f.pretend_ban = s.auto_bp.pretend_ban;

        f.enable_auto_set_spells = s.auto_bp.auto_spell;
        f.auto_set_summoner_spell = vec![s.auto_bp.spell1_id, s.auto_bp.spell2_id];
        f.auto_set_summoner_spell_top = s.auto_bp.spell_ids_top.clone();
        f.auto_set_summoner_spell_jug = s.auto_bp.spell_ids_jug.clone();
        f.auto_set_summoner_spell_mid = s.auto_bp.spell_ids_mid.clone();
        f.auto_set_summoner_spell_bot = s.auto_bp.spell_ids_bot.clone();
        f.auto_set_summoner_spell_sup = s.auto_bp.spell_ids_sup.clone();
        f.auto_select_confirm_on_timeout = s.auto_bp.confirm_on_timeout;

        f.enable_bench_overlay = s.aram_bench_overlay;
        f.auto_trade = s.auto_trade;
        f.auto_accept_ceil_swap = s.auto_bp.auto_pick_order_swap;
        f.auto_accept_champ_trade = s.auto_bp.auto_champ_trade;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [D05-a] to/sync 对新增字段的双向映射必须自洽，避免半截映射
    #[test]
    fn automation_settings_roundtrip_keeps_new_fields() {
        let mut cfg = AppConfig::default();
        cfg.functions.auto_play_again_delay = 9;
        cfg.functions.auto_trade = true;

        let settings = cfg.to_automation_settings();
        assert_eq!(settings.auto_match.auto_play_again_delay, 9);
        assert!(settings.auto_trade);

        let mut restored = AppConfig::default();
        restored.sync_automation_settings(&settings);
        assert_eq!(restored.functions.auto_play_again_delay, 9);
        assert!(restored.functions.auto_trade);
    }
}
