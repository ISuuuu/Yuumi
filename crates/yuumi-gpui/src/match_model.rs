//! 对接 yuumi-core 核心对局数据模型的视图适配层

pub use yuumi_core::gameflow::match_stats::{compute_match_stats, MatchLike};
pub use yuumi_core::parsers::match_detail::{GameDetail, GameDetailPlayer, GameDetailTeam};
pub use yuumi_core::parsers::match_parser::MatchDisplay;

#[cfg(test)]
use yuumi_core::lcu::game_data::GameDataAssets;
#[cfg(test)]
use yuumi_core::parsers::match_parser::{LcuMatchGame, LcuMatchHistoryResponse};

/// 界面展示使用的战绩卡片数据（包裹 MatchDisplay 并增加 UI 交互状态）
#[derive(Debug, Clone)]
pub struct MatchItem {
    pub display: MatchDisplay,
    pub expanded: bool,
}

impl MatchItem {
    pub fn new(display: MatchDisplay) -> Self {
        Self {
            display,
            expanded: false,
        }
    }
}

/// 对局回放状态枚举（对齐 Vue ReplayButton.vue）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReplayState {
    #[default]
    Idle,
    Checking,
    Downloading,
    Watch,
    Unsupported,
    Error,
}

impl ReplayState {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "回放",
            Self::Checking => "检查中...",
            Self::Downloading => "下载中...",
            Self::Watch => "观看回放",
            Self::Unsupported => "不可用",
            Self::Error => "重试",
        }
    }
}

// 转发 MatchLike 实现
impl MatchLike for MatchItem {
    fn is_remake(&self) -> bool {
        self.display.remake
    }

    fn is_win(&self) -> bool {
        self.display.win
    }

    fn kills(&self) -> u32 {
        self.display.kills.max(0) as u32
    }

    fn deaths(&self) -> u32 {
        self.display.deaths.max(0) as u32
    }

    fn assists(&self) -> u32 {
        self.display.assists.max(0) as u32
    }

    fn position(&self) -> Option<&str> {
        self.display.position.as_deref()
    }

    fn champion_id(&self) -> i64 {
        self.display.champion_id as i64
    }
}

pub fn position_to_zh(pos: Option<&str>) -> &'static str {
    match pos {
        Some("TOP") => "上单",
        Some("JUNGLE") => "打野",
        Some("MIDDLE") => "中单",
        Some("BOTTOM") => "下路",
        Some("UTILITY") => "辅助",
        _ => "全能",
    }
}

/// 常玩位置聚合（位置中文名, 场次），按场次降序；无分路信息的对局不计入
pub fn top_positions(matches: &[MatchItem], limit: usize) -> Vec<(String, usize)> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for m in matches {
        let label = position_to_zh(m.display.position.as_deref());
        if label == "全能" {
            continue;
        }
        match counts.iter_mut().find(|(name, _)| name == label) {
            Some((_, n)) => *n += 1,
            None => counts.push((label.to_string(), 1)),
        }
    }
    counts.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    counts.truncate(limit);
    counts
}

/// 近期英雄聚合（英雄名, 头像 URL, 场次），按场次降序
pub fn top_champions(matches: &[MatchItem], limit: usize) -> Vec<(String, String, usize)> {
    let mut counts: Vec<(String, String, usize)> = Vec::new();
    for m in matches {
        let name = m.display.name.trim();
        if name.is_empty() {
            continue;
        }
        match counts.iter_mut().find(|(n, _, _)| n == name) {
            Some((_, _, count)) => *count += 1,
            None => counts.push((name.to_string(), m.display.champion_icon_url.clone(), 1)),
        }
    }
    counts.sort_by_key(|(_, _, count)| std::cmp::Reverse(*count));
    counts.truncate(limit);
    counts
}

/// 解析单局 LCU 对局为 MatchItem
#[cfg(test)]
pub fn parse_fixture_game(game: &LcuMatchGame, assets: &GameDataAssets) -> Option<MatchItem> {
    let display = game.to_display(assets)?;
    Some(MatchItem::new(display))
}

/// 从内联 real fixtures 加载样本（仅供解析层回归测试，运行时一律使用真实战绩）
#[cfg(test)]
pub fn load_mock_matches() -> Vec<MatchItem> {
    let raw_fixtures = [
        include_str!("../../../src-tauri/testdata/match/real/top-solo.json"),
        include_str!("../../../src-tauri/testdata/match/real/mid-solo.json"),
        include_str!("../../../src-tauri/testdata/match/real/jungle-none.json"),
        include_str!("../../../src-tauri/testdata/match/real/bottom-carry.json"),
        include_str!("../../../src-tauri/testdata/match/real/bottom-support.json"),
        include_str!("../../../src-tauri/testdata/match/real/none-duo.json"),
        include_str!("../../../src-tauri/testdata/match/real/remake.json"),
        include_str!("../../../src-tauri/testdata/match/real/custom-3100.json"),
    ];

    let mut matches = Vec::new();
    let assets = GameDataAssets::default();

    for raw in raw_fixtures {
        if let Ok(resp) = serde_json::from_str::<LcuMatchHistoryResponse>(raw) {
            for g in resp.games.games {
                if let Some(item) = parse_fixture_game(&g, &assets) {
                    matches.push(item);
                }
            }
        }
    }

    // 复制构造 20 局滚动列表
    let base_len = matches.len();
    if base_len > 0 && base_len < 20 {
        let needed = 20 - base_len;
        for i in 0..needed {
            let mut clone = matches[i % base_len].clone();
            clone.display.game_id += (i as u64 + 1) * 100_000;
            matches.push(clone);
        }
    }

    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_mock_matches_via_yuumi_core() {
        let matches = load_mock_matches();
        assert_eq!(matches.len(), 20);
        let first = &matches[0];
        assert!(first.display.game_id > 0);
        assert!(!first.display.duration.is_empty());
    }

    #[test]
    fn test_compute_stats_with_yuumi_core() {
        let matches = load_mock_matches();
        let stats = compute_match_stats(&matches);
        assert_eq!(stats.total_matches, 20);
        assert!(stats.win_rate > 0);
    }
}
