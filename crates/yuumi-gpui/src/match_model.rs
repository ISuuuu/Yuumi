//! 对接 yuumi-core 核心对局数据模型的视图适配层

use std::collections::HashMap;
use std::path::Path;

pub use yuumi_core::gameflow::match_stats::{
    compute_match_stats, MatchLike,
};
pub use yuumi_core::lcu::game_data::GameDataAssets;
pub use yuumi_core::parsers::match_detail::{
    GameDetail, GameDetailTeam, RawGameDetail,
};
pub use yuumi_core::parsers::match_parser::{
    LcuMatchGame, LcuMatchHistoryResponse, MatchDisplay,
};

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

/// 解析单局 LCU 对局为 MatchItem
pub fn parse_fixture_game(game: &LcuMatchGame, assets: &GameDataAssets) -> Option<MatchItem> {
    let display = game.to_display(assets)?;
    Some(MatchItem::new(display))
}

/// 从 real fixtures 文件或目录加载样本
pub fn load_real_fixtures(match_data_dir: &Path) -> Vec<MatchItem> {
    let real_dir = match_data_dir.join("real");
    let mut matches = Vec::new();
    let assets = GameDataAssets::default();

    if let Ok(entries) = std::fs::read_dir(&real_dir) {
        let mut paths: Vec<_> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
            .collect();
        paths.sort();

        for p in paths {
            if let Ok(content) = std::fs::read_to_string(&p) {
                if let Ok(resp) = serde_json::from_str::<LcuMatchHistoryResponse>(&content) {
                    for g in resp.games.games {
                        if let Some(item) = parse_fixture_game(&g, &assets) {
                            matches.push(item);
                        }
                    }
                }
            }
        }
    }

    if matches.is_empty() {
        return load_mock_matches();
    }

    // 复制构造 20 局平滑滚动列表
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

/// 从内联 real fixtures 加载测试样本（20 场保底）
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

/// 从 fixtures 加载真实或保底对局详情映射 (game_id -> GameDetail)
pub fn load_fixture_game_details() -> HashMap<u64, GameDetail> {
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

    let mut map = HashMap::new();
    let assets = GameDataAssets::default();

    for raw in raw_fixtures {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(raw) {
            if let Some(games) = val["games"]["games"].as_array() {
                for g in games {
                    if let Ok(raw_detail) = serde_json::from_value::<RawGameDetail>(g.clone()) {
                        let detail = raw_detail.to_game_detail(&assets);
                        map.insert(detail.game_id, detail);
                    }
                }
            }
        }
    }

    map
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

    #[test]
    fn test_game_info_mock_data() {
        let info = crate::game_info_view::load_mock_game_info();
        assert_eq!(info.blue_team.len(), 5);
        assert_eq!(info.red_team.len(), 5);
        assert_eq!(info.user_side, Some(yuumi_core::gameflow::team_builder::MapSide::Blue));
        assert!(info.side_matched);

        let local_player = info
            .blue_team
            .iter()
            .find(|p| p.is_local_player)
            .expect("必须包含本机玩家");
        assert_eq!(local_player.summoner_name, "悠米本米");
        assert_eq!(local_player.win_rate, 80);

        let blue_premade: Vec<_> = info
            .blue_team
            .iter()
            .filter(|p| p.premade_group_id.is_some())
            .collect();
        assert_eq!(blue_premade.len(), 2);

        let red_premade: Vec<_> = info
            .red_team
            .iter()
            .filter(|p| p.premade_group_id.is_some())
            .collect();
        assert_eq!(red_premade.len(), 3);
    }
}
