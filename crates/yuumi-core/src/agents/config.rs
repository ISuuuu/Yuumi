//! 自动化功能配置模型与持久化

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 极地大乱斗队列 ID
pub const ARAM_QUEUE_ID: i32 = 450;

/// 拥有共享板凳席的大乱斗家族队列（极地大乱斗 / 海克斯大乱斗 / 经典海斗）
pub const ARAM_BENCH_QUEUE_IDS: &[i32] = &[450, 2400, 2450];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoMatchConfig {
    /// 自动接受匹配
    pub auto_accept: bool,
    /// 自动接受延迟（秒）
    pub auto_accept_delay: u64,
    /// 自动重连
    pub auto_reconnect: bool,
    /// 延时再来一局
    pub auto_play_again: bool,
    /// 再来一局延迟（秒）
    ///
    /// 旧版按结算阶段区分固定延迟（WaitingForStats 10s / PreEndOfGame 3.25s /
    /// EndOfGame 1.575s），本字段仅作为其他结算阶段的兜底值保留。
    pub auto_play_again_delay: u64,
    /// 自动点赞队友
    pub auto_honor: bool,
    /// 自动接受好友组队邀请
    pub auto_handle_invite: bool,
    /// 空闲阶段自动创建预设房间
    pub auto_create_lobby: bool,
    /// 自动创建房间使用的队列 ID
    pub default_game_mode: u32,
    /// 选人阶段播报我方阵营（大乱斗报边）
    pub aram_team_side: bool,
    /// 阵营播报是否对队友可见（false 时使用仅本地可见的 celebration 消息）
    pub aram_team_side_visible: bool,
    /// 选人阶段对带标记玩家发送聊天提醒
    pub tag_reminder: bool,
}

impl Default for AutoMatchConfig {
    fn default() -> Self {
        Self {
            auto_accept: true,
            auto_accept_delay: 2,
            auto_reconnect: true,
            auto_play_again: false,
            auto_play_again_delay: 5,
            auto_honor: true,
            auto_handle_invite: false,
            auto_create_lobby: false,
            default_game_mode: 2400,
            aram_team_side: false,
            aram_team_side_visible: false,
            tag_reminder: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoBpConfig {
    /// 自动选英雄开关（在选人回合锁定）
    pub auto_pick: bool,
    /// 自动选人意向优先（只亮英雄，不锁定）
    pub auto_pick_intent: bool,
    /// 默认选用英雄 ID 列表
    pub pick_champion_ids: Vec<i32>,
    /// 分路选用候选（上 / 野 / 中 / 下 / 辅）
    pub pick_champion_ids_top: Vec<i32>,
    pub pick_champion_ids_jug: Vec<i32>,
    pub pick_champion_ids_mid: Vec<i32>,
    pub pick_champion_ids_bot: Vec<i32>,
    pub pick_champion_ids_sup: Vec<i32>,
    /// 自动禁英雄开关
    pub auto_ban: bool,
    /// 默认禁用英雄 ID 列表
    pub ban_champion_ids: Vec<i32>,
    /// 分路禁用候选（上 / 野 / 中 / 下 / 辅）
    pub ban_champion_ids_top: Vec<i32>,
    pub ban_champion_ids_jug: Vec<i32>,
    pub ban_champion_ids_mid: Vec<i32>,
    pub ban_champion_ids_bot: Vec<i32>,
    pub ban_champion_ids_sup: Vec<i32>,
    /// 禁用延迟（毫秒），旧版配置为 0~15 秒、步长 0.5 秒
    pub ban_delay_ms: u32,
    /// 假装禁用：先排除队友意向英雄再执行
    pub pretend_ban: bool,
    /// 自动配置召唤师技能
    pub auto_spell: bool,
    /// 默认技能 1
    pub spell1_id: i32,
    /// 默认技能 2
    pub spell2_id: i32,
    /// 分路技能候选（上 / 野 / 中 / 下 / 辅），不足两枚时回退默认技能
    pub spell_ids_top: Vec<i32>,
    pub spell_ids_jug: Vec<i32>,
    pub spell_ids_mid: Vec<i32>,
    pub spell_ids_bot: Vec<i32>,
    pub spell_ids_sup: Vec<i32>,
    /// 选人倒计时结束前自动确认锁定
    pub confirm_on_timeout: bool,
    /// 自动同意楼层交换
    pub auto_pick_order_swap: bool,
    /// 自动同意英雄交换
    pub auto_champ_trade: bool,
}

impl Default for AutoBpConfig {
    fn default() -> Self {
        Self {
            auto_pick: false,
            auto_pick_intent: true,
            pick_champion_ids: Vec::new(),
            pick_champion_ids_top: Vec::new(),
            pick_champion_ids_jug: Vec::new(),
            pick_champion_ids_mid: Vec::new(),
            pick_champion_ids_bot: Vec::new(),
            pick_champion_ids_sup: Vec::new(),
            auto_ban: false,
            ban_champion_ids: Vec::new(),
            ban_champion_ids_top: Vec::new(),
            ban_champion_ids_jug: Vec::new(),
            ban_champion_ids_mid: Vec::new(),
            ban_champion_ids_bot: Vec::new(),
            ban_champion_ids_sup: Vec::new(),
            ban_delay_ms: 0,
            pretend_ban: false,
            auto_spell: false,
            spell1_id: 4,  // 闪现
            spell2_id: 14, // 点燃
            spell_ids_top: Vec::new(),
            spell_ids_jug: Vec::new(),
            spell_ids_mid: Vec::new(),
            spell_ids_bot: Vec::new(),
            spell_ids_sup: Vec::new(),
            confirm_on_timeout: true,
            auto_pick_order_swap: false,
            auto_champ_trade: false,
        }
    }
}

impl AutoBpConfig {
    /// 按分路取选用候选；分路为空时回退默认候选
    pub fn pick_candidates(&self, position: &str) -> Vec<i32> {
        let lane = match position {
            "top" => &self.pick_champion_ids_top,
            "jungle" => &self.pick_champion_ids_jug,
            "middle" => &self.pick_champion_ids_mid,
            "bottom" => &self.pick_champion_ids_bot,
            "utility" => &self.pick_champion_ids_sup,
            _ => &self.pick_champion_ids,
        };
        let mut list = lane.clone();
        if !self.pick_champion_ids.is_empty() && lane != &self.pick_champion_ids {
            list.extend(self.pick_champion_ids.iter().copied());
        }
        list
    }

    /// 按分路取禁用候选；分路为空时回退默认候选
    pub fn ban_candidates(&self, position: &str) -> Vec<i32> {
        let lane = match position {
            "top" => &self.ban_champion_ids_top,
            "jungle" => &self.ban_champion_ids_jug,
            "middle" => &self.ban_champion_ids_mid,
            "bottom" => &self.ban_champion_ids_bot,
            "utility" => &self.ban_champion_ids_sup,
            _ => &self.ban_champion_ids,
        };
        let mut list = lane.clone();
        if !self.ban_champion_ids.is_empty() && lane != &self.ban_champion_ids {
            list.extend(self.ban_champion_ids.iter().copied());
        }
        list
    }

    /// 按分路取召唤师技能；分路不足两枚或分路未知时回退默认（与选用/禁用候选一致）
    pub fn spell_pair(&self, position: &str) -> Vec<i32> {
        let lane = match position {
            "top" => &self.spell_ids_top,
            "jungle" => &self.spell_ids_jug,
            "middle" => &self.spell_ids_mid,
            "bottom" => &self.spell_ids_bot,
            "utility" => &self.spell_ids_sup,
            _ => return vec![self.spell1_id, self.spell2_id],
        };
        if lane.len() >= 2 {
            vec![lane[0], lane[1]]
        } else {
            vec![self.spell1_id, self.spell2_id]
        }
    }
}

/// 工具箱全部自动化开关的持久化载体。
///
/// 视图层（`ToolsView`）只做展示，开关状态的权威来源是这份结构体：
/// 启动时从磁盘反序列化，任一开关变更后立即回写，并同步进 `LiveLcuService` 的 watch 通道。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AutomationSettings {
    /// 对局流程自动化（自动接受 / 重连 / 再来一局 / 点赞）
    pub auto_match: AutoMatchConfig,
    /// 选人阶段自动化（秒选 / 秒禁 / 召唤师技能）
    pub auto_bp: AutoBpConfig,
    /// 大乱斗板凳席悬浮窗
    pub aram_bench_overlay: bool,
    /// 自动同意队友换人 / 换楼
    pub auto_trade: bool,
}

impl Default for AutomationSettings {
    fn default() -> Self {
        Self {
            auto_match: AutoMatchConfig::default(),
            auto_bp: AutoBpConfig::default(),
            aram_bench_overlay: true,
            auto_trade: true,
        }
    }
}

impl AutomationSettings {
    /// 默认配置文件路径：`<配置目录>/Yuumi/automation.json`
    pub fn default_path() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join("Yuumi").join("automation.json"))
    }

    /// 从指定路径读取；文件缺失或内容损坏时退回默认配置，不向上抛错。
    pub fn load_from_path(path: &Path) -> Self {
        let Ok(raw) = std::fs::read_to_string(path) else {
            log::info!("[Config] 未找到自动化配置 {}，使用默认值", path.display());
            return Self::default();
        };
        match serde_json::from_str::<Self>(&raw) {
            Ok(cfg) => cfg,
            Err(e) => {
                log::warn!(
                    "[Config] 自动化配置解析失败({})，已回退默认值: {}",
                    path.display(),
                    e
                );
                Self::default()
            }
        }
    }

    /// 从默认路径读取
    pub fn load() -> Self {
        match Self::default_path() {
            Some(path) => Self::load_from_path(&path),
            None => Self::default(),
        }
    }

    /// 写入指定路径（自动创建父目录）
    pub fn save_to_path(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建配置目录失败 {}: {}", parent.display(), e))?;
        }
        let raw = serde_json::to_string_pretty(self)
            .map_err(|e| format!("序列化自动化配置失败: {}", e))?;
        std::fs::write(path, raw)
            .map_err(|e| format!("写入自动化配置失败 {}: {}", path.display(), e))
    }

    /// 写入默认路径
    pub fn save(&self) -> Result<(), String> {
        match Self::default_path() {
            Some(path) => self.save_to_path(&path),
            None => Err("当前系统无可用配置目录".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "yuumi-automation-{}-{}.json",
            name,
            std::process::id()
        ))
    }

    #[test]
    fn settings_roundtrip_through_disk() {
        let path = temp_path("roundtrip");
        let _ = std::fs::remove_file(&path);

        let mut settings = AutomationSettings::default();
        settings.auto_match.auto_accept = false;
        settings.auto_match.auto_accept_delay = 7;
        settings.auto_bp.auto_pick = true;
        settings.auto_bp.pick_champion_ids = vec![157, 238];
        settings.auto_bp.spell2_id = 12;
        settings.aram_bench_overlay = false;
        settings.auto_trade = false;

        settings.save_to_path(&path).expect("配置写入应当成功");
        let loaded = AutomationSettings::load_from_path(&path);
        assert_eq!(loaded, settings);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_file_falls_back_to_default() {
        let path = temp_path("missing");
        let _ = std::fs::remove_file(&path);
        assert_eq!(
            AutomationSettings::load_from_path(&path),
            AutomationSettings::default()
        );
    }

    #[test]
    fn corrupted_file_falls_back_to_default() {
        let path = temp_path("corrupted");
        std::fs::write(&path, "{ this is not json").expect("写入脏数据");
        assert_eq!(
            AutomationSettings::load_from_path(&path),
            AutomationSettings::default()
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn partial_file_keeps_defaults_for_missing_fields() {
        let path = temp_path("partial");
        std::fs::write(
            &path,
            r#"{"aram_bench_overlay": false, "auto_bp": {"auto_pick": true}}"#,
        )
        .expect("写入部分配置");
        let loaded = AutomationSettings::load_from_path(&path);
        assert!(!loaded.aram_bench_overlay);
        assert!(loaded.auto_bp.auto_pick);
        // 未出现的字段保留默认值
        assert!(loaded.auto_bp.auto_pick_intent);
        assert_eq!(loaded.auto_bp.spell1_id, 4);
        assert!(loaded.auto_match.auto_accept);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let path = temp_path("unknown");
        std::fs::write(&path, r#"{"future_field": 1, "auto_trade": false}"#).expect("写入未知字段");
        let loaded = AutomationSettings::load_from_path(&path);
        assert!(!loaded.auto_trade);
        let _ = std::fs::remove_file(&path);
    }
}
