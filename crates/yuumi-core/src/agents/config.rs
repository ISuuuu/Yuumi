//! 自动化功能配置模型

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub auto_play_again_delay: u64,
    /// 自动点赞队友
    pub auto_honor: bool,
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
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoBpConfig {
    /// 自动选英雄开关
    pub auto_pick: bool,
    /// 自动选人意向优先
    pub auto_pick_intent: bool,
    /// 默认选用英雄 ID 列表
    pub pick_champion_ids: Vec<i32>,
    /// 自动禁英雄开关
    pub auto_ban: bool,
    /// 默认禁用英雄 ID 列表
    pub ban_champion_ids: Vec<i32>,
    /// 自动配置召唤师技能
    pub auto_spell: bool,
    /// 默认技能 1
    pub spell1_id: i32,
    /// 默认技能 2
    pub spell2_id: i32,
}

impl Default for AutoBpConfig {
    fn default() -> Self {
        Self {
            auto_pick: false,
            auto_pick_intent: true,
            pick_champion_ids: Vec::new(),
            auto_ban: false,
            ban_champion_ids: Vec::new(),
            auto_spell: false,
            spell1_id: 4,  // 闪现
            spell2_id: 14, // 点燃
        }
    }
}
