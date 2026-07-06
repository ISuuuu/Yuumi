use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 获取配置文件路径: %APPDATA%/Yuumi/config.json
fn config_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("Yuumi");
    path.push("config.json");
    path
}

// ─── 通用设置 ───

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct GeneralConfig {
    pub lol_path: Vec<String>,
    pub enable_start_lol_with_app: bool,
    pub enable_close_to_tray: Option<bool>,
    pub enable_game_start_minimize: bool,
    pub enable_check_update: bool,
    pub log_level: u32,
    pub enable_github_proxy: bool,
    pub github_proxy_addr: String,
    pub enable_opgg_proxy: bool,
    pub opgg_proxy_addr: String,

    // SignalR 远程反代
    pub enable_signalr_hub: bool,
    pub signalr_server_url: String,
    pub signalr_user_id: String,

    // 对局上传
    #[serde(default)]
    pub upload_api_url: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            lol_path: Vec::new(),
            enable_start_lol_with_app: false,
            enable_close_to_tray: None,
            enable_game_start_minimize: false,
            enable_check_update: true,
            log_level: 0,
            enable_github_proxy: false,
            github_proxy_addr: "127.0.0.1:7897".into(),
            enable_opgg_proxy: false,
            opgg_proxy_addr: "127.0.0.1:7897".into(),
            enable_signalr_hub: false,
            signalr_server_url: String::new(),
            signalr_user_id: String::new(),
            upload_api_url: String::new(),
        }
    }
}

// ─── 个性化设置 ───

fn default_true() -> bool {
    true
}

fn default_theme_color() -> String {
    "#009faa".into()
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
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
            win_card_color: "#2839b01b".into(),
            lose_card_color: "#28d3190c".into(),
            remake_card_color: "#28a2a2a2".into(),
            light_deaths_number_color: "#ffb60000".into(),
            dark_deaths_number_color: "#ffff8d8d".into(),
            theme_color: "#009faa".into(),
        }
    }
}

// ─── 功能设置 ───

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
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
    #[serde(default)]
    pub enable_reserve_gameinfo: bool,
    #[serde(default)]
    pub lcu_realtime_enabled: bool,
    #[serde(default)]
    pub lcu_user_id: String,
    // 自动亮起英雄（仅预选，不锁定）
    #[serde(default)]
    pub enable_auto_hover_champion: bool,
    // 倒计时结束时自动确认选择（配合 hover 模式）
    #[serde(default = "default_true")]
    pub auto_select_confirm_on_timeout: bool,

    #[serde(default = "default_true")]
    pub upload_enabled: bool,

    /// 隐藏云顶之弈模式（默认隐藏）
    #[serde(default = "default_true")]
    pub hide_tft: bool,

    /// 大乱斗板凳席悬浮窗（默认开启）
    #[serde(default = "default_true")]
    pub enable_bench_overlay: bool,
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
            auto_set_summoner_spell: Vec::new(),
            auto_set_summoner_spell_top: Vec::new(),
            auto_set_summoner_spell_jug: Vec::new(),
            auto_set_summoner_spell_mid: Vec::new(),
            auto_set_summoner_spell_bot: Vec::new(),
            auto_set_summoner_spell_sup: Vec::new(),
            enable_reserve_gameinfo: true,
            lcu_realtime_enabled: false,
            enable_auto_hover_champion: false,
            auto_select_confirm_on_timeout: true,
            lcu_user_id: String::new(),
            upload_enabled: true,
            hide_tft: true,
            enable_bench_overlay: true,
        }
    }
}

// ─── 其他设置 ───

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct OtherConfig {
    pub last_notice_sha: String,
    pub search_history: String,
}

impl Default for OtherConfig {
    fn default() -> Self {
        Self {
            last_notice_sha: String::new(),
            search_history: String::new(),
        }
    }
}

// ─── 顶层配置 ───

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AppConfig {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub personalization: PersonalizationConfig,
    #[serde(default)]
    pub functions: FunctionsConfig,
    #[serde(default)]
    pub other: OtherConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            personalization: PersonalizationConfig::default(),
            functions: FunctionsConfig::default(),
            other: OtherConfig::default(),
        }
    }
}

impl AppConfig {
    /// 从磁盘加载配置，文件不存在返回默认值并保存；
    /// 解析失败时备份损坏文件并写入错误信息，再返回默认值。
    pub fn load() -> Self {
        let path = config_path();
        if !path.exists() {
            let config = Self::default();
            config.save();
            return config;
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                log::error!("读取配置文件失败: {}, 使用默认配置", e);
                return Self::default();
            }
        };
        match serde_json::from_str::<Self>(&text) {
            Ok(cfg) => cfg,
            Err(e) => {
                // 备份损坏的配置文件
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let backup_path = path.with_extension(format!("json.backup.{}", ts));
                if let Err(be) = std::fs::copy(&path, &backup_path) {
                    log::warn!("备份损坏配置文件失败: {}", be);
                } else {
                    log::warn!("配置文件解析失败，已备份到: {}", backup_path.display());
                }
                // 写入错误提示文件，供前端读取
                let error_path = path.with_extension("json.error");
                let error_msg = format!(
                    "配置文件格式错误，已恢复为默认设置。\n错误详情: {}\n原文件已备份至: {}",
                    e,
                    backup_path.display()
                );
                let _ = std::fs::write(&error_path, &error_msg);
                log::error!("配置文件解析失败，使用默认值: {}", e);
                Self::default()
            }
        }
    }

    /// 读取配置加载错误信息（前端调用，读取后自动清除）
    pub fn take_load_error() -> Option<String> {
        let error_path = config_path().with_extension("json.error");
        if error_path.exists() {
            let msg = std::fs::read_to_string(&error_path).ok();
            let _ = std::fs::remove_file(&error_path);
            msg
        } else {
            None
        }
    }

    /// 将当前配置写回磁盘
    pub fn save(&self) {
        let path = config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match serde_json::to_string_pretty(self) {
            Ok(text) => {
                if let Err(e) = std::fs::write(&path, text) {
                    log::error!("写入配置文件失败: {}", e);
                }
            }
            Err(e) => {
                log::error!("序列化配置失败: {}", e);
            }
        }
    }
}
