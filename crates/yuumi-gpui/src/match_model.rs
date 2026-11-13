//! 对接 yuumi-core 核心对局数据模型的视图适配层

pub use yuumi_core::gameflow::match_stats::{
    compute_match_stats, normalize_position, MatchLike, PositionKey,
};
pub use yuumi_core::parsers::match_detail::{GameDetail, GameDetailPlayer, GameDetailTeam};
pub use yuumi_core::parsers::match_parser::MatchDisplay;

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

    pub fn rehydrate_icons(&mut self, assets: &GameDataAssets) {
        self.display.rehydrate_icons(assets);
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

/// 近期常玩位置占比（对齐 Vue `computeAggregatedPositions`）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionShare {
    pub position: PositionKey,
    /// 该位置的场次（仅统计非重开、有分路信息的对局）
    pub count: usize,
    /// 占比，分母为有分路信息的对局数
    pub percentage: u32,
}

/// 近期常用英雄聚合（对齐 Vue `computeAggregatedChampions`）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChampionShare {
    pub champion_id: i32,
    pub icon_url: String,
    pub games: usize,
    pub wins: usize,
    pub losses: usize,
    pub win_rate: u32,
}

/// 位置排序基准：与 Vue `POSITION_KEYS` 顺序一致，用于场次相同时的稳定排序
fn position_rank(pos: PositionKey) -> u8 {
    match pos {
        PositionKey::Top => 0,
        PositionKey::Jungle => 1,
        PositionKey::Middle => 2,
        PositionKey::Bottom => 3,
        PositionKey::Utility => 4,
    }
}

/// 常玩位置聚合（按场次降序）；重开局与无分路信息的对局不计入
pub fn top_positions(matches: &[MatchItem], limit: usize) -> Vec<PositionShare> {
    let mut counts: Vec<(PositionKey, usize)> = Vec::new();
    let mut total_valid = 0usize;
    for m in matches {
        // 重开局不足 3 分钟，分路任务道具不会出现，位置不可靠
        if m.display.remake {
            continue;
        }
        let Some(pos) = normalize_position(m.display.position.as_deref()) else {
            continue;
        };
        total_valid += 1;
        match counts.iter_mut().find(|(p, _)| *p == pos) {
            Some((_, n)) => *n += 1,
            None => counts.push((pos, 1)),
        }
    }
    if total_valid == 0 {
        return Vec::new();
    }
    counts.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| position_rank(a.0).cmp(&position_rank(b.0)))
    });
    counts.truncate(limit);
    counts
        .into_iter()
        .map(|(position, count)| PositionShare {
            position,
            count,
            percentage: ((count as f64 / total_valid as f64) * 100.0).round() as u32,
        })
        .collect()
}

/// 近期常用英雄聚合（按英雄 ID 去重，跳过重开局与无效英雄）。
/// 排序对齐 Vue：场次 → 胜率 → KDA（重开不计入场次）
pub fn top_champions(matches: &[MatchItem], limit: usize) -> Vec<ChampionShare> {
    struct Acc {
        icon_url: String,
        games: usize,
        wins: usize,
        losses: usize,
        kills: i64,
        deaths: i64,
        assists: i64,
    }

    let mut entries: Vec<(i32, Acc)> = Vec::new();
    for m in matches {
        let d = &m.display;
        if d.remake || d.champion_id <= 0 {
            continue;
        }
        let idx = match entries.iter().position(|(id, _)| *id == d.champion_id) {
            Some(idx) => idx,
            None => {
                entries.push((
                    d.champion_id,
                    Acc {
                        icon_url: d.champion_icon_url.clone(),
                        games: 0,
                        wins: 0,
                        losses: 0,
                        kills: 0,
                        deaths: 0,
                        assists: 0,
                    },
                ));
                entries.len() - 1
            }
        };
        let acc = &mut entries[idx].1;
        acc.games += 1;
        if d.win {
            acc.wins += 1;
        } else {
            acc.losses += 1;
        }
        acc.kills += d.kills as i64;
        acc.deaths += d.deaths as i64;
        acc.assists += d.assists as i64;
    }

    let kda_ratio = |acc: &Acc| -> f64 {
        if acc.deaths == 0 {
            // 对齐 Vue：无死亡视为最高，排序时按 999 处理
            999.0
        } else {
            (acc.kills + acc.assists) as f64 / acc.deaths as f64
        }
    };

    entries.sort_by(|a, b| {
        b.1.games
            .cmp(&a.1.games)
            .then_with(|| {
                let wr_a = (a.1.wins as f64 / a.1.games as f64 * 100.0).round() as u32;
                let wr_b = (b.1.wins as f64 / b.1.games as f64 * 100.0).round() as u32;
                wr_b.cmp(&wr_a)
            })
            .then_with(|| {
                kda_ratio(&b.1)
                    .partial_cmp(&kda_ratio(&a.1))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    entries.truncate(limit);
    entries
        .into_iter()
        .map(|(champion_id, acc)| ChampionShare {
            champion_id,
            icon_url: acc.icon_url,
            games: acc.games,
            wins: acc.wins,
            losses: acc.losses,
            win_rate: (acc.wins as f64 / acc.games as f64 * 100.0).round() as u32,
        })
        .collect()
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
        include_str!("../../yuumi-core/testdata/match/real/top-solo.json"),
        include_str!("../../yuumi-core/testdata/match/real/mid-solo.json"),
        include_str!("../../yuumi-core/testdata/match/real/jungle-none.json"),
        include_str!("../../yuumi-core/testdata/match/real/bottom-carry.json"),
        include_str!("../../yuumi-core/testdata/match/real/bottom-support.json"),
        include_str!("../../yuumi-core/testdata/match/real/none-duo.json"),
        include_str!("../../yuumi-core/testdata/match/real/remake.json"),
        include_str!("../../yuumi-core/testdata/match/real/custom-3100.json"),
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

    #[test]
    fn top_champions_aggregates_by_champion_id() {
        let mut matches = load_mock_matches();
        // 不同队列名但同一英雄，必须合并成一条（此前按对局名聚合会错分成多条）
        for m in matches.iter_mut() {
            m.display.champion_id = 103;
            m.display.champion_icon_url = "/icon/103.png".into();
            m.display.remake = false;
            m.display.win = true;
        }
        let champs = top_champions(&matches, 6);
        assert_eq!(champs.len(), 1);
        assert_eq!(champs[0].champion_id, 103);
        assert_eq!(champs[0].games, matches.len());
        assert_eq!(champs[0].win_rate, 100);
    }

    #[test]
    fn top_champions_skips_remake_and_sorts_by_win_rate() {
        let mut matches = load_mock_matches();
        for m in matches.iter_mut() {
            m.display.remake = false;
            m.display.champion_id = 0;
        }
        // 英雄 1 三场一胜，英雄 2 三场全胜：场次相同按胜率降序
        for (i, m) in matches.iter_mut().take(6).enumerate() {
            m.display.champion_id = if i < 3 { 1 } else { 2 };
            m.display.win = if i < 3 { i == 0 } else { true };
        }
        // 夹一场重开，不计入场次
        matches[6].display.champion_id = 3;
        matches[6].display.remake = true;

        let champs = top_champions(&matches, 6);
        assert_eq!(champs.len(), 2);
        assert_eq!(champs[0].champion_id, 2);
        assert_eq!(champs[0].win_rate, 100);
        assert_eq!(champs[1].champion_id, 1);
        assert_eq!(champs[1].win_rate, 33);
    }

    #[test]
    fn top_positions_skips_remake_and_rounds_percentage() {
        let mut matches = load_mock_matches();
        for m in matches.iter_mut() {
            m.display.position = None;
            m.display.remake = false;
        }
        matches[0].display.position = Some("MIDDLE".into());
        matches[1].display.position = Some("MIDDLE".into());
        matches[2].display.position = Some("UTILITY".into());
        // 重开即使带分路也不能进入统计
        matches[3].display.position = Some("UTILITY".into());
        matches[3].display.remake = true;

        let positions = top_positions(&matches, 2);
        assert_eq!(positions.len(), 2);
        assert_eq!(positions[0].position, PositionKey::Middle);
        assert_eq!(positions[0].count, 2);
        assert_eq!(positions[0].percentage, 67);
        assert_eq!(positions[1].position, PositionKey::Utility);
        assert_eq!(positions[1].percentage, 33);
    }

    #[test]
    fn top_positions_empty_when_no_laned_games() {
        let mut matches = load_mock_matches();
        for m in matches.iter_mut() {
            m.display.position = None;
        }
        assert!(top_positions(&matches, 2).is_empty());
    }
}
