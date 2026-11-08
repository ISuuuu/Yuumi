//! 对局信息解析与实时构建（10 人段位、近期 KDA、宿命对局）
//!
//! 数据来源：
//! - 选人阶段：`/lol-champ-select/v1/session`（myTeam / theirTeam）
//! - 对局阶段：`/lol-gameflow/v1/session` 的 `gameData.teamOne/teamTwo`
//! - 段位：`/lol-ranked/v1/ranked-stats/{puuid}`
//! - 近期对局：`/lol-match-history/v1/products/lol/{puuid}/matches`
//! - 宿命对局详情：`/lol-match-history/v1/games/{gameId}`（仅在双方战绩存在交集时回捞）

use std::collections::{HashMap, HashSet};

use futures_util::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};

use crate::gameflow::match_stats::{compute_match_stats, StreakInfo, StreakType};
use crate::gameflow::player_identity::get_valid_tpid;
use crate::gameflow::team_builder::{
    resolve_team_side, MapSide, SnapshotParticipant, TeamBuildContext,
};
use crate::lcu::client::LcuClient;
use crate::lcu::game_data::GameDataAssets;
use crate::parsers::match_detail::{fetch_game_detail, GameDetail};
use crate::parsers::match_parser::{fetch_match_history, get_queue_info, MatchDisplay};

/// 每名玩家拉取的近期对局数量
pub const RECENT_MATCH_LIMIT: u32 = 20;
/// 玩家数据并发拉取上限
const PLAYER_FETCH_CONCURRENCY: usize = 5;
/// 宿命对局详情回捞上限（只回捞与本人战绩存在交集的对局）
const FATE_DETAIL_LIMIT: usize = 5;

// ─── 段位 ───

/// `/lol-ranked/v1/ranked-stats/{puuid}` 中单个队列的段位
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RankedQueueEntry {
    #[serde(default)]
    pub queue_type: String,
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub division: String,
    #[serde(default)]
    pub league_points: u32,
    #[serde(default)]
    pub wins: u32,
    #[serde(default)]
    pub losses: u32,
    /// 本赛季最高段位
    #[serde(default)]
    pub highest_tier: String,
    #[serde(default)]
    pub highest_division: String,
    /// 上赛季结算段位
    #[serde(default)]
    pub previous_season_end_tier: String,
    #[serde(default)]
    pub previous_season_end_division: String,
    /// 定级赛未完成时 `tier` 为空串
    #[serde(default)]
    pub is_provisional: bool,
}

impl RankedQueueEntry {
    /// 是否已完成定级（定级中 `tier` 为空）
    pub fn is_placed(&self) -> bool {
        !self.tier.trim().is_empty()
    }

    /// 该队列总场次
    pub fn total_games(&self) -> u32 {
        self.wins.saturating_add(self.losses)
    }

    /// 该队列胜率（0-100，无场次返回 0）
    pub fn win_rate(&self) -> f32 {
        let total = self.total_games();
        if total == 0 {
            return 0.0;
        }
        self.wins as f32 / total as f32 * 100.0
    }
}

/// `/lol-ranked/v1/ranked-stats/{puuid}` 响应
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RankedStats {
    #[serde(default)]
    pub queue_map: HashMap<String, RankedQueueEntry>,
}

impl RankedStats {
    /// 优先单双排，其次灵活组排，最后取任意已有段位的队列
    pub fn preferred_entry(&self) -> Option<&RankedQueueEntry> {
        ["RANKED_SOLO_5x5", "RANKED_FLEX_SR"]
            .iter()
            .filter_map(|key| self.queue_map.get(*key))
            .find(|entry| !entry.tier.trim().is_empty())
            .or_else(|| {
                self.queue_map
                    .values()
                    .find(|entry| !entry.tier.trim().is_empty())
            })
    }
}

/// 段位英文 → 中文
pub fn tier_display(tier: &str) -> &'static str {
    match tier.trim().to_ascii_uppercase().as_str() {
        "IRON" => "黑铁",
        "BRONZE" => "青铜",
        "SILVER" => "白银",
        "GOLD" => "黄金",
        "PLATINUM" => "铂金",
        "EMERALD" => "翡翠",
        "DIAMOND" => "钻石",
        "MASTER" => "大师",
        "GRANDMASTER" => "宗师",
        "CHALLENGER" => "王者",
        _ => "未定级",
    }
}

/// 段位简称，例如 "钻石 I"、"大师"；无段位返回 "未定级"
pub fn rank_tier_label(tier: &str, division: &str) -> String {
    let tier_zh = tier_display(tier);
    if tier_zh == "未定级" {
        return tier_zh.to_string();
    }
    let division = division.trim();
    if division.is_empty() || division.eq_ignore_ascii_case("NA") {
        return tier_zh.to_string();
    }
    if matches!(tier_zh, "大师" | "宗师" | "王者") {
        return tier_zh.to_string();
    }
    format!("{} {}", tier_zh, division)
}

/// 段位展示串，例如 "钻石 I · 45 LP"；无段位返回 "未定级"
pub fn rank_display(tier: &str, division: &str, league_points: u32) -> String {
    let label = rank_tier_label(tier, division);
    if label == "未定级" {
        return label;
    }
    format!("{} · {} LP", label, league_points)
}

// ─── 会话参与者 ───

/// 选人 / 对局会话中的单名参与者（LCU 原始字段）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPlayer {
    #[serde(default)]
    pub cell_id: Option<i64>,
    #[serde(default)]
    pub champion_id: Option<i32>,
    #[serde(default)]
    pub champion_pick_intent: Option<i32>,
    #[serde(default)]
    pub assigned_position: Option<String>,
    #[serde(default)]
    pub summoner_id: Option<i64>,
    #[serde(default)]
    pub puuid: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub game_name: Option<String>,
    #[serde(default)]
    pub tag_line: Option<String>,
    #[serde(default)]
    pub summoner_name: Option<String>,
    #[serde(default)]
    pub team_participant_id: Option<String>,
    #[serde(default)]
    pub party_id: Option<String>,
    #[serde(default)]
    pub bot: Option<bool>,
    #[serde(default)]
    pub is_bot: Option<bool>,
}

impl SessionPlayer {
    /// 真实 PUUID（空串视为缺失）
    pub fn resolved_puuid(&self) -> Option<&str> {
        self.puuid
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }

    /// 真实召唤师 ID（0 视为缺失，选人阶段可能以 cellId 兜底）
    pub fn resolved_summoner_id(&self) -> Option<i64> {
        self.summoner_id.filter(|&id| id != 0)
    }

    /// 当前英雄 ID（未锁定时回退到预选英雄）
    pub fn resolved_champion_id(&self) -> i32 {
        let picked = self.champion_id.unwrap_or(0);
        if picked > 0 {
            picked
        } else {
            self.champion_pick_intent.unwrap_or(0)
        }
    }

    /// 名称与标签号拆分（"名字#TAG"）
    pub fn name_parts(&self) -> (String, String) {
        if let (Some(game_name), Some(tag_line)) = (&self.game_name, &self.tag_line) {
            let name = game_name.trim();
            let tag = tag_line.trim();
            if !name.is_empty() && !tag.is_empty() {
                return (name.to_string(), tag.to_string());
            }
        }

        let raw = self
            .display_name
            .as_deref()
            .or(self.summoner_name.as_deref())
            .or(self.game_name.as_deref())
            .unwrap_or("")
            .trim();

        match raw.split_once('#') {
            Some((name, tag)) => (name.trim().to_string(), tag.trim().to_string()),
            None => (raw.to_string(), String::new()),
        }
    }

    pub fn is_bot(&self) -> bool {
        self.bot.unwrap_or(false) || self.is_bot.unwrap_or(false)
    }

    fn team_participant_key(&self) -> Option<String> {
        get_valid_tpid(
            self.team_participant_id.as_deref(),
            self.party_id.as_deref(),
        )
    }
}

/// `/lol-champ-select/v1/session` 关注字段
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectSnapshot {
    #[serde(default)]
    pub my_team: Vec<SessionPlayer>,
    #[serde(default)]
    pub their_team: Vec<SessionPlayer>,
    #[serde(default)]
    pub local_player_cell_id: Option<i64>,
    #[serde(default)]
    pub queue_id: Option<i32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameflowQueueInfo {
    #[serde(default)]
    pub id: Option<i32>,
    #[serde(default)]
    pub game_mode: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameflowGameData {
    #[serde(default)]
    pub game_id: Option<u64>,
    #[serde(default)]
    pub queue: Option<GameflowQueueInfo>,
    #[serde(default)]
    pub team_one: Vec<SessionPlayer>,
    #[serde(default)]
    pub team_two: Vec<SessionPlayer>,
}

/// `/lol-gameflow/v1/session` 关注字段
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameflowSessionSnapshot {
    #[serde(default)]
    pub game_data: Option<GameflowGameData>,
}

// ─── 对局信息数据源 ───

/// 当前对局的 10 人原始来源（选人或对局会话）
#[derive(Debug, Clone)]
pub struct GameInfoSource {
    pub phase: String,
    pub queue_id: i32,
    pub game_id: Option<u64>,
    pub blue: Vec<SessionPlayer>,
    pub red: Vec<SessionPlayer>,
    pub local_player_cell_id: Option<i64>,
}

impl GameInfoSource {
    /// 从选人会话构建（myTeam 作为我方一侧，theirTeam 作为敌方一侧）
    pub fn from_champ_select(snapshot: &ChampSelectSnapshot) -> Option<Self> {
        if snapshot.my_team.is_empty() && snapshot.their_team.is_empty() {
            return None;
        }
        Some(Self {
            phase: "ChampSelect".to_string(),
            queue_id: snapshot.queue_id.unwrap_or(0),
            game_id: None,
            blue: snapshot.my_team.clone(),
            red: snapshot.their_team.clone(),
            local_player_cell_id: snapshot.local_player_cell_id,
        })
    }

    /// 从 gameflow 会话构建（teamOne = 蓝方 100，teamTwo = 红方 200）
    pub fn from_gameflow(phase: &str, snapshot: &GameflowSessionSnapshot) -> Option<Self> {
        let game_data = snapshot.game_data.as_ref()?;
        if game_data.team_one.is_empty() && game_data.team_two.is_empty() {
            return None;
        }
        Some(Self {
            phase: phase.to_string(),
            queue_id: game_data.queue.as_ref().and_then(|q| q.id).unwrap_or(0),
            game_id: game_data.game_id,
            blue: game_data.team_one.clone(),
            red: game_data.team_two.clone(),
            local_player_cell_id: None,
        })
    }

    /// 去重键：阶段 / 对局号 / 双方槽位英雄与身份，避免选人高频道次重复拉取
    pub fn refresh_key(&self) -> String {
        let mut parts = vec![
            self.phase.clone(),
            self.game_id.unwrap_or(0).to_string(),
            self.queue_id.to_string(),
        ];
        for group in [&self.blue, &self.red] {
            let slots: Vec<String> = group
                .iter()
                .map(|p| {
                    format!(
                        "{}:{}:{}",
                        p.cell_id.unwrap_or(-1),
                        p.resolved_champion_id(),
                        p.resolved_puuid().unwrap_or("-")
                    )
                })
                .collect();
            parts.push(slots.join(","));
        }
        parts.join("|")
    }

    /// 去重后的真实 PUUID 列表
    pub fn puuids(&self) -> Vec<String> {
        let mut seen = HashSet::new();
        self.blue
            .iter()
            .chain(self.red.iter())
            .filter_map(|p| p.resolved_puuid().map(|s| s.to_string()))
            .filter(|p| seen.insert(p.clone()))
            .collect()
    }
}

// ─── 视图模型 ───

/// 宿命对局（曾同局交手）信息
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FateInfo {
    /// 最近一次交手是否为队友
    pub ally: bool,
    /// 最近一次交手是否为双方最近一场
    pub is_last_game: bool,
    pub game_id: u64,
    pub game_creation: u64,
    /// 该玩家在最近一次交手中使用的英雄
    pub champion_id: i32,
    pub ally_count: u32,
    pub enemy_count: u32,
}

impl FateInfo {
    /// 徽标文案
    pub fn badge_text(&self) -> String {
        let base = match (self.ally, self.is_last_game) {
            (true, true) => "上局队友",
            (false, true) => "上局对手",
            (true, false) => "近期队友",
            (false, false) => "近期对手",
        };
        let total = self.ally_count + self.enemy_count;
        if total >= 2 && self.ally_count > 0 && self.enemy_count > 0 {
            format!("{} · 交手{}次", base, total)
        } else {
            base.to_string()
        }
    }
}

/// 单名玩家的对局信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameInfoPlayer {
    pub puuid: String,
    pub summoner_id: i64,
    pub cell_id: i64,
    pub summoner_name: String,
    pub tag_line: String,
    pub champion_id: i32,
    pub champion_name: String,
    pub position: String,
    pub team_participant_id: Option<String>,
    pub premade_group_id: Option<usize>,
    pub rank_tier: String,
    pub rank_division: String,
    pub rank_lp: u32,
    pub rank_display: String,
    pub ranked_wins: u32,
    pub ranked_losses: u32,
    pub recent_match_count: usize,
    pub win_rate: u32,
    pub avg_kda: f64,
    pub avg_kills: f32,
    pub avg_deaths: f32,
    pub avg_assists: f32,
    pub streak_text: Option<String>,
    pub streak_is_win: bool,
    pub fate: Option<FateInfo>,
    pub is_local_player: bool,
    pub is_bot: bool,
    /// 是否成功拉到段位或战绩（否则前端展示占位）
    pub data_available: bool,
}

/// 10 人对局信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveGameInfo {
    pub phase: String,
    pub phase_label: String,
    pub queue_id: i32,
    pub queue_name: String,
    pub game_id: Option<u64>,
    pub blue_team: Vec<GameInfoPlayer>,
    pub red_team: Vec<GameInfoPlayer>,
    pub user_side: Option<MapSide>,
    pub side_matched: bool,
    /// 生成快照的时间戳（毫秒）
    pub snapshot_at: i64,
}

impl LiveGameInfo {
    pub fn players(&self) -> impl Iterator<Item = &GameInfoPlayer> {
        self.blue_team.iter().chain(self.red_team.iter())
    }
}

/// 阶段中文标签
pub fn phase_label(phase: &str) -> &'static str {
    match phase {
        "ChampSelect" => "英雄选择中",
        "GameStart" => "正在进入对局",
        "InProgress" => "对局进行中",
        "Reconnect" => "正在重连对局",
        "PreEndOfGame" | "WaitingForStats" | "EndOfGame" => "对局结算中",
        "WatchInProgress" => "观战进行中",
        _ => "等待进入对局",
    }
}

/// 分路中文名
pub fn position_display(assigned_position: Option<&str>) -> String {
    match assigned_position
        .unwrap_or("")
        .trim()
        .to_ascii_uppercase()
        .as_str()
    {
        "TOP" => "上单".to_string(),
        "JUNGLE" => "打野".to_string(),
        "MIDDLE" | "MID" => "中单".to_string(),
        "BOTTOM" | "BOT" | "ADC" => "下路".to_string(),
        "UTILITY" | "SUPPORT" => "辅助".to_string(),
        _ => "补位".to_string(),
    }
}

/// 单名玩家拉取到的数据
#[derive(Debug, Clone, Default)]
pub struct PlayerFetchData {
    pub ranked: Option<RankedQueueEntry>,
    pub recent: Vec<MatchDisplay>,
}

/// 本人近期对局（用于宿命判定，仅回捞存在交集的对局详情）
#[derive(Debug, Clone)]
pub struct MyRecentGame {
    pub game_id: u64,
    pub game_creation: u64,
    ally_puuids: HashSet<String>,
    enemy_puuids: HashSet<String>,
    champions: HashMap<String, i32>,
}

impl MyRecentGame {
    /// 从对局详情构建（self_puuid 不在对局中时返回 None）
    pub fn from_detail(detail: &GameDetail, self_puuid: &str) -> Option<Self> {
        let self_team_id = detail
            .blue
            .players
            .iter()
            .chain(detail.red.players.iter())
            .find(|p| p.puuid == self_puuid)
            .map(|p| p.team_id)?;

        let mut ally_puuids = HashSet::new();
        let mut enemy_puuids = HashSet::new();
        let mut champions = HashMap::new();

        for player in detail.blue.players.iter().chain(detail.red.players.iter()) {
            if player.puuid.trim().is_empty() {
                continue;
            }
            champions.insert(player.puuid.clone(), player.champion_id);
            if player.team_id == self_team_id {
                ally_puuids.insert(player.puuid.clone());
            } else {
                enemy_puuids.insert(player.puuid.clone());
            }
        }

        Some(Self {
            game_id: detail.game_id,
            game_creation: detail.game_creation,
            ally_puuids,
            enemy_puuids,
            champions,
        })
    }

    fn fate_for(&self, player_puuid: &str) -> Option<bool> {
        if self.ally_puuids.contains(player_puuid) {
            Some(true)
        } else if self.enemy_puuids.contains(player_puuid) {
            Some(false)
        } else {
            None
        }
    }
}

/// 本人近期对局的宿命判定上下文
///
/// `latest_game_id` 取本人真实战绩列表的第一场，`games` 仅包含双方战绩存在交集、
/// 且已回捞详情的对局；两者分离才能正确区分「上局」与「近期」。
#[derive(Debug, Clone, Default)]
pub struct MyRecentContext {
    pub latest_game_id: Option<u64>,
    pub games: Vec<MyRecentGame>,
}

/// 判定宿命对局：按本人对局时间从新到旧取最近一次交手
pub fn detect_fate(
    player_puuid: &str,
    player_game_ids: &[u64],
    my_games: &[MyRecentGame],
    latest_my_game_id: Option<u64>,
) -> Option<FateInfo> {
    if player_game_ids.is_empty() {
        return None;
    }

    let mut latest: Option<(&MyRecentGame, bool)> = None;
    let mut ally_count = 0u32;
    let mut enemy_count = 0u32;

    for game in my_games {
        if !player_game_ids.contains(&game.game_id) {
            continue;
        }
        let Some(ally) = game.fate_for(player_puuid) else {
            continue;
        };
        if ally {
            ally_count += 1;
        } else {
            enemy_count += 1;
        }
        if latest.is_none() {
            latest = Some((game, ally));
        }
    }

    let (game, ally) = latest?;
    Some(FateInfo {
        ally,
        is_last_game: Some(game.game_id) == latest_my_game_id,
        game_id: game.game_id,
        game_creation: game.game_creation,
        champion_id: game.champions.get(player_puuid).copied().unwrap_or(0),
        ally_count,
        enemy_count,
    })
}

/// 组队标识：同一 teamParticipantId 出现 2 人及以上才计为一组
fn resolve_premade_groups(players: &[SessionPlayer]) -> Vec<Option<usize>> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for player in players {
        if let Some(key) = player.team_participant_key() {
            *counts.entry(key).or_insert(0) += 1;
        }
    }

    let mut group_index: HashMap<String, usize> = HashMap::new();
    let mut next_index = 0usize;

    players
        .iter()
        .map(|player| {
            let key = player.team_participant_key()?;
            if counts.get(&key).copied().unwrap_or(0) < 2 {
                return None;
            }
            let index = *group_index.entry(key).or_insert_with(|| {
                let index = next_index;
                next_index += 1;
                index
            });
            Some(index)
        })
        .collect()
}

fn streak_text(streak: Option<&StreakInfo>) -> Option<String> {
    streak.map(|s| {
        let label = match s.streak_type {
            StreakType::Win => "连胜",
            StreakType::Loss => "连败",
        };
        format!("{}{}", s.count, label)
    })
}

fn snapshot_of(player: &SessionPlayer) -> SnapshotParticipant<'_> {
    SnapshotParticipant {
        puuid: player.resolved_puuid(),
        summoner_id: player.resolved_summoner_id(),
        summoner_name: player.display_name.as_deref(),
        game_name: player.game_name.as_deref(),
        display_name: player.display_name.as_deref(),
        cell_id: player.cell_id,
    }
}

/// 阵营绑定：对局阶段按真实 teamOne/teamTwo 判定，选人阶段按 myTeam 归为我方
fn bind_team_side(
    source: &GameInfoSource,
    self_puuid: &str,
    self_summoner_id: Option<i64>,
) -> (MapSide, bool) {
    let blue_snapshots: Vec<SnapshotParticipant> = source.blue.iter().map(snapshot_of).collect();
    let red_snapshots: Vec<SnapshotParticipant> = source.red.iter().map(snapshot_of).collect();

    // 选人阶段 myTeam 固定占据 0..4 槽位，尚无真实阵营信息
    let is_champ_select = source.phase == "ChampSelect";
    let champ_select_snapshot: &[SnapshotParticipant] = if is_champ_select {
        &blue_snapshots
    } else {
        &[]
    };

    let ctx = TeamBuildContext {
        current_summoner_id: self_summoner_id,
        current_summoner_puuid: Some(self_puuid),
        map_side: None,
        champ_select_team_snapshot: champ_select_snapshot,
    };

    let (side, matched) = resolve_team_side(&blue_snapshots, &red_snapshots, &ctx);
    (side.unwrap_or(MapSide::Blue), matched && !is_champ_select)
}

/// 构建单名玩家的对局信息（共享上下文集中传入，避免参数过长）
struct PlayerBuildContext<'a> {
    my_games: &'a [MyRecentGame],
    latest_my_game_id: Option<u64>,
    self_puuid: &'a str,
    self_summoner_id: Option<i64>,
    assets: &'a GameDataAssets,
}

fn build_player(
    session: &SessionPlayer,
    premade_group_id: Option<usize>,
    fetch: Option<&PlayerFetchData>,
    ctx: &PlayerBuildContext<'_>,
) -> GameInfoPlayer {
    let (summoner_name, tag_line) = session.name_parts();
    let puuid = session.resolved_puuid().unwrap_or("").to_string();
    let summoner_id = session.resolved_summoner_id().unwrap_or(0);
    let champion_id = session.resolved_champion_id();

    let ranked = fetch.and_then(|f| f.ranked.as_ref());
    let recent = fetch.map(|f| f.recent.as_slice()).unwrap_or(&[]);
    let stats = compute_match_stats(recent);
    let recent_count = recent.len();

    let average = |values: Vec<f32>| {
        if values.is_empty() {
            0.0
        } else {
            values.iter().sum::<f32>() / values.len() as f32
        }
    };
    let avg_kills = average(recent.iter().map(|m| m.kills.max(0) as f32).collect());
    let avg_deaths = average(recent.iter().map(|m| m.deaths.max(0) as f32).collect());
    let avg_assists = average(recent.iter().map(|m| m.assists.max(0) as f32).collect());

    let player_game_ids: Vec<u64> = recent.iter().map(|m| m.game_id).collect();
    let fate = if puuid.is_empty() {
        None
    } else {
        detect_fate(
            &puuid,
            &player_game_ids,
            ctx.my_games,
            ctx.latest_my_game_id,
        )
    };

    let is_local_player = !puuid.is_empty()
        && (puuid == ctx.self_puuid
            || (summoner_id != 0 && Some(summoner_id) == ctx.self_summoner_id));

    GameInfoPlayer {
        puuid,
        summoner_id,
        cell_id: session.cell_id.unwrap_or(-1),
        summoner_name: if summoner_name.is_empty() {
            "未知玩家".to_string()
        } else {
            summoner_name
        },
        tag_line,
        champion_id,
        champion_name: ctx
            .assets
            .champions
            .get(&champion_id)
            .cloned()
            .unwrap_or_else(|| {
                if champion_id > 0 {
                    format!("英雄 {}", champion_id)
                } else {
                    "未选择".to_string()
                }
            }),
        position: position_display(session.assigned_position.as_deref()),
        team_participant_id: session.team_participant_key(),
        premade_group_id,
        rank_tier: tier_display(ranked.map(|r| r.tier.as_str()).unwrap_or("")).to_string(),
        rank_division: ranked.map(|r| r.division.clone()).unwrap_or_default(),
        rank_lp: ranked.map(|r| r.league_points).unwrap_or(0),
        rank_display: ranked
            .map(|r| rank_display(&r.tier, &r.division, r.league_points))
            .unwrap_or_else(|| "未定级".to_string()),
        ranked_wins: ranked.map(|r| r.wins).unwrap_or(0),
        ranked_losses: ranked.map(|r| r.losses).unwrap_or(0),
        recent_match_count: recent_count,
        win_rate: stats.win_rate,
        avg_kda: stats.avg_kda,
        avg_kills,
        avg_deaths,
        avg_assists,
        streak_text: streak_text(stats.streak.as_ref()),
        streak_is_win: stats
            .streak
            .as_ref()
            .is_some_and(|s| s.streak_type == StreakType::Win),
        fate,
        is_local_player,
        is_bot: session.is_bot(),
        data_available: ranked.is_some() || recent_count > 0,
    }
}

/// 由会话来源 + 各玩家拉取结果构建 10 人对局信息（纯函数，便于测试）
pub fn build_live_game_info(
    source: &GameInfoSource,
    self_puuid: &str,
    self_summoner_id: Option<i64>,
    my_recent: &MyRecentContext,
    fetch: &HashMap<String, PlayerFetchData>,
    assets: &GameDataAssets,
) -> LiveGameInfo {
    let (side, side_matched) = bind_team_side(source, self_puuid, self_summoner_id);
    let build_ctx = PlayerBuildContext {
        my_games: &my_recent.games,
        latest_my_game_id: my_recent.latest_game_id,
        self_puuid,
        self_summoner_id,
        assets,
    };

    let blue_premade = resolve_premade_groups(&source.blue);
    let red_premade = resolve_premade_groups(&source.red);

    let build_team = |players: &[SessionPlayer], groups: &[Option<usize>]| {
        players
            .iter()
            .zip(groups.iter())
            .map(|(player, group)| {
                let data = player.resolved_puuid().and_then(|puuid| fetch.get(puuid));
                build_player(player, *group, data, &build_ctx)
            })
            .collect::<Vec<_>>()
    };

    let queue_info = get_queue_info(source.queue_id);

    LiveGameInfo {
        phase: source.phase.clone(),
        phase_label: phase_label(&source.phase).to_string(),
        queue_id: source.queue_id,
        queue_name: if source.queue_id > 0 {
            queue_info.name.to_string()
        } else {
            "对局模式未知".to_string()
        },
        game_id: source.game_id,
        blue_team: build_team(&source.blue, &blue_premade),
        red_team: build_team(&source.red, &red_premade),
        user_side: Some(side),
        side_matched,
        snapshot_at: chrono::Utc::now().timestamp_millis(),
    }
}

// ─── 实时拉取 ───

/// 拉取指定召唤师的完整排位数据（含单排 / 灵活各队列）
pub async fn fetch_ranked_stats(client: &LcuClient, puuid: &str) -> Result<RankedStats, String> {
    let path = format!("/lol-ranked/v1/ranked-stats/{}", puuid);
    let value = client.get(&path).await?;
    serde_json::from_value(value).map_err(|e| format!("解析排位数据失败: {}", e))
}

/// 拉取单个玩家的排位与近期对局（任一失败均降级为空）
async fn fetch_player_data(
    client: &LcuClient,
    puuid: &str,
    assets: &GameDataAssets,
) -> PlayerFetchData {
    let ranked_path = format!("/lol-ranked/v1/ranked-stats/{}", puuid);
    let (ranked_resp, history_resp) = tokio::join!(
        client.get(&ranked_path),
        fetch_match_history(client, puuid, Some(0), Some(RECENT_MATCH_LIMIT - 1), assets)
    );

    let ranked = ranked_resp.ok().and_then(|value| {
        serde_json::from_value::<RankedStats>(value)
            .ok()
            .and_then(|stats| stats.preferred_entry().cloned())
    });

    let mut recent = history_resp.unwrap_or_default();
    // LCU 产品接口实际会比 endIndex 多返回一场，统一裁剪为 20 场
    recent.truncate(RECENT_MATCH_LIMIT as usize);

    PlayerFetchData { ranked, recent }
}

/// 实时构建当前对局信息：并发拉取 10 人数据 + 宿命对局回捞
pub async fn fetch_live_game_info(
    client: &LcuClient,
    self_puuid: &str,
    self_summoner_id: Option<i64>,
    source: GameInfoSource,
    recent_matches: &[MatchDisplay],
    assets: &GameDataAssets,
) -> LiveGameInfo {
    let puuids = source.puuids();

    let fetched: Vec<(String, PlayerFetchData)> = stream::iter(puuids)
        .map(|puuid| async move {
            let data = fetch_player_data(client, &puuid, assets).await;
            (puuid, data)
        })
        .buffer_unordered(PLAYER_FETCH_CONCURRENCY)
        .collect()
        .await;
    let fetch_map: HashMap<String, PlayerFetchData> = fetched.into_iter().collect();

    // 本人战绩交集 → 仅回捞少量详情用于宿命判定
    let my_game_ids: Vec<u64> = recent_matches.iter().map(|m| m.game_id).collect();
    let mut overlap_ids: Vec<u64> = Vec::new();
    for data in fetch_map.values() {
        for game_id in data.recent.iter().map(|m| m.game_id) {
            if my_game_ids.contains(&game_id) && !overlap_ids.contains(&game_id) {
                overlap_ids.push(game_id);
            }
        }
    }
    overlap_ids.sort_by_key(|game_id| {
        my_game_ids
            .iter()
            .position(|id| id == game_id)
            .unwrap_or(usize::MAX)
    });
    overlap_ids.truncate(FATE_DETAIL_LIMIT);

    let details: Vec<GameDetail> = stream::iter(overlap_ids)
        .map(|game_id| async move { fetch_game_detail(client, game_id, assets).await.ok() })
        .buffer_unordered(3)
        .filter_map(|detail| async move { detail })
        .collect()
        .await;

    let mut my_games: Vec<MyRecentGame> = details
        .iter()
        .filter_map(|detail| MyRecentGame::from_detail(detail, self_puuid))
        .collect();
    my_games.sort_by_key(|game| {
        my_game_ids
            .iter()
            .position(|id| id == &game.game_id)
            .unwrap_or(usize::MAX)
    });

    build_live_game_info(
        &source,
        self_puuid,
        self_summoner_id,
        &MyRecentContext {
            latest_game_id: my_game_ids.first().copied(),
            games: my_games,
        },
        &fetch_map,
        assets,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHAMP_SELECT_FIXTURE: &str =
        include_str!("../../../../src-tauri/testdata/session/champ-select-ranked-420.json");
    const GAMEFLOW_FIXTURE: &str =
        include_str!("../../../../src-tauri/testdata/session/gameflow-in-progress-440.json");
    const RANKED_FIXTURE: &str =
        include_str!("../../../../src-tauri/testdata/session/ranked-stats-solo.json");

    const SELF_PUUID: &str = "puuid-self-top";
    const SELF_SUMMONER_ID: i64 = 1001;

    fn champ_select_source() -> GameInfoSource {
        let snapshot: ChampSelectSnapshot =
            serde_json::from_str(CHAMP_SELECT_FIXTURE).expect("选人快照应能反序列化");
        GameInfoSource::from_champ_select(&snapshot).expect("选人快照应包含双方玩家")
    }

    fn gameflow_source() -> GameInfoSource {
        let snapshot: GameflowSessionSnapshot =
            serde_json::from_str(GAMEFLOW_FIXTURE).expect("对局快照应能反序列化");
        GameInfoSource::from_gameflow("InProgress", &snapshot).expect("对局快照应包含双方玩家")
    }

    fn sample_match(
        game_id: u64,
        win: bool,
        kills: i32,
        deaths: i32,
        assists: i32,
    ) -> MatchDisplay {
        MatchDisplay {
            queue_id: 420,
            game_id,
            time: "2026-11-08 12:00".to_string(),
            short_time: "11-08 12:00".to_string(),
            name: "排位单双排".to_string(),
            map: "召唤师峡谷".to_string(),
            duration: "30:00".to_string(),
            remake: false,
            win,
            placement: None,
            champion_id: 266,
            spell1_id: 4,
            spell2_id: 12,
            champ_level: 18,
            kills,
            deaths,
            assists,
            kda: "3.00".to_string(),
            item_ids: vec![0; 7],
            rune_id: 8000,
            cs: 200,
            gold: 12000,
            time_stamp: 1_760_000_000_000,
            total_damage: 20000,
            total_heal: 1000,
            champion_icon_url: String::new(),
            spell1_icon_url: String::new(),
            spell2_icon_url: String::new(),
            rune_icon_url: String::new(),
            item_icon_urls: Vec::new(),
            augment_ids: Vec::new(),
            augment_icon_urls: Vec::new(),
            augment_names: Vec::new(),
            position: Some("TOP".to_string()),
            role_bound_item: None,
        }
    }

    fn my_recent_game(game_id: u64, ally: &[&str], enemy: &[&str]) -> MyRecentGame {
        let mut champions = HashMap::new();
        for puuid in ally.iter().chain(enemy.iter()) {
            champions.insert((*puuid).to_string(), 103);
        }
        MyRecentGame {
            game_id,
            game_creation: 1_760_000_000_000,
            ally_puuids: ally.iter().map(|p| (*p).to_string()).collect(),
            enemy_puuids: enemy.iter().map(|p| (*p).to_string()).collect(),
            champions,
        }
    }

    #[test]
    fn champ_select_fixture_builds_ten_players_and_premade_groups() {
        let source = champ_select_source();
        assert_eq!(source.blue.len(), 5);
        assert_eq!(source.red.len(), 5);
        assert_eq!(source.queue_id, 420);
        assert_eq!(source.local_player_cell_id, Some(2));
        assert_eq!(source.puuids().len(), 8, "两名敌方玩家身份缺失应被跳过");

        let info = build_live_game_info(
            &source,
            SELF_PUUID,
            Some(SELF_SUMMONER_ID),
            &MyRecentContext::default(),
            &HashMap::new(),
            &GameDataAssets::default(),
        );

        assert_eq!(info.blue_team.len(), 5);
        assert_eq!(info.red_team.len(), 5);
        assert_eq!(info.queue_name, "排位单双排");
        assert_eq!(info.phase_label, "英雄选择中");
        assert_eq!(info.user_side, Some(MapSide::Blue));
        assert!(!info.side_matched, "选人阶段不应声称精准阵营");

        let local = info
            .players()
            .find(|p| p.is_local_player)
            .expect("必须识别本机玩家");
        assert_eq!(local.summoner_name, "悠米本米");
        assert_eq!(local.tag_line, "GPUI");
        assert_eq!(local.champion_id, 266);

        // 我方打野与下路双人组
        let blue_duo: Vec<_> = info
            .blue_team
            .iter()
            .filter(|p| p.premade_group_id.is_some())
            .collect();
        assert_eq!(blue_duo.len(), 2);

        // 敌方三人开黑
        let red_trio: Vec<_> = info
            .red_team
            .iter()
            .filter(|p| p.premade_group_id.is_some())
            .collect();
        assert_eq!(red_trio.len(), 3);

        // 身份缺失玩家降级为占位
        let unknown: Vec<_> = info
            .red_team
            .iter()
            .filter(|p| p.puuid.is_empty())
            .collect();
        assert_eq!(unknown.len(), 2);
        assert!(unknown.iter().all(|p| p.summoner_name == "未知玩家"));
        assert!(unknown.iter().all(|p| !p.data_available));
    }

    #[test]
    fn gameflow_fixture_marks_local_side_and_game_id() {
        let source = gameflow_source();
        assert_eq!(source.game_id, Some(7012345678));
        assert_eq!(source.queue_id, 440);

        let info = build_live_game_info(
            &source,
            SELF_PUUID,
            Some(SELF_SUMMONER_ID),
            &MyRecentContext::default(),
            &HashMap::new(),
            &GameDataAssets::default(),
        );

        assert_eq!(info.queue_name, "排位灵活组排");
        assert_eq!(info.phase_label, "对局进行中");
        assert_eq!(info.user_side, Some(MapSide::Red));
        assert!(info.side_matched, "对局阶段应精准识别阵营");

        let local = info
            .players()
            .find(|p| p.is_local_player)
            .expect("必须识别本机玩家");
        assert!(info.red_team.iter().any(|p| p.puuid == local.puuid));
        assert_eq!(info.blue_team.len(), 5);
        assert_eq!(info.red_team.len(), 5);
    }

    #[test]
    fn ranked_fixture_prefers_solo_queue() {
        let stats: RankedStats =
            serde_json::from_str(RANKED_FIXTURE).expect("排位数据应能反序列化");
        let entry = stats.preferred_entry().expect("应取到单双排段位");
        assert_eq!(entry.tier, "DIAMOND");
        assert_eq!(entry.division, "I");
        assert_eq!(entry.league_points, 45);
        assert_eq!(
            rank_display(&entry.tier, &entry.division, entry.league_points),
            "钻石 I · 45 LP"
        );
    }

    #[test]
    fn rank_display_handles_unranked_and_apex_tiers() {
        assert_eq!(rank_display("", "", 0), "未定级");
        assert_eq!(rank_display("UNRANKED", "", 0), "未定级");
        assert_eq!(rank_display("MASTER", "I", 320), "大师 · 320 LP");
        assert_eq!(rank_display("CHALLENGER", "I", 800), "王者 · 800 LP");
    }

    #[test]
    fn build_uses_fetched_rank_and_recent_stats() {
        let source = champ_select_source();
        let recent = vec![
            sample_match(1, true, 8, 2, 6),
            sample_match(2, true, 4, 4, 10),
            sample_match(3, false, 2, 6, 3),
        ];
        let mut fetch = HashMap::new();
        fetch.insert(
            "puuid-ally-mid".to_string(),
            PlayerFetchData {
                ranked: Some(RankedQueueEntry {
                    queue_type: "RANKED_SOLO_5x5".to_string(),
                    tier: "MASTER".to_string(),
                    division: "I".to_string(),
                    league_points: 210,
                    wins: 66,
                    losses: 34,
                    ..Default::default()
                }),
                recent,
            },
        );

        let info = build_live_game_info(
            &source,
            SELF_PUUID,
            Some(SELF_SUMMONER_ID),
            &MyRecentContext::default(),
            &fetch,
            &GameDataAssets::default(),
        );

        let player = info
            .blue_team
            .iter()
            .find(|p| p.puuid == "puuid-ally-mid")
            .expect("应命中有数据的中单");
        assert_eq!(player.rank_display, "大师 · 210 LP");
        assert_eq!(player.recent_match_count, 3);
        assert_eq!(player.win_rate, 67);
        assert!((player.avg_kills - 14.0 / 3.0).abs() < 1e-4);
        assert!((player.avg_deaths - 4.0).abs() < 1e-4);
        assert_eq!(player.streak_text.as_deref(), Some("2连胜"));
        assert!(player.data_available);

        let no_data = info
            .blue_team
            .iter()
            .find(|p| p.puuid == "puuid-ally-jug")
            .expect("应命中无数据的打野");
        assert!(!no_data.data_available);
        assert_eq!(no_data.rank_display, "未定级");
        assert_eq!(no_data.win_rate, 0);
    }

    #[test]
    fn fate_uses_latest_encounter_and_counts_mixed_history() {
        let my_games = vec![
            my_recent_game(9001, &[SELF_PUUID, "puuid-enemy-mid"], &["puuid-enemy-top"]),
            my_recent_game(9002, &[SELF_PUUID], &["puuid-enemy-mid"]),
        ];

        let ally = detect_fate("puuid-enemy-mid", &[9001, 9002], &my_games, Some(9001))
            .expect("应识别宿命对局");
        assert!(ally.ally);
        assert!(ally.is_last_game);
        assert_eq!(ally.ally_count, 1);
        assert_eq!(ally.enemy_count, 1);
        assert_eq!(ally.champion_id, 103);
        assert_eq!(ally.badge_text(), "上局队友 · 交手2次");

        let enemy =
            detect_fate("puuid-enemy-top", &[9001], &my_games, Some(9001)).expect("应识别对手宿命");
        assert!(!enemy.ally);
        assert_eq!(enemy.badge_text(), "上局对手");

        assert!(detect_fate("puuid-stranger", &[7777], &my_games, Some(9001)).is_none());
        assert!(detect_fate("puuid-enemy-mid", &[], &my_games, None).is_none());
    }

    #[test]
    fn fate_marks_recent_when_encounter_is_not_latest_game() {
        let my_games = vec![my_recent_game(9001, &[SELF_PUUID, "puuid-enemy-mid"], &[])];
        let fate =
            detect_fate("puuid-enemy-mid", &[9001], &my_games, Some(9999)).expect("应识别宿命对局");
        assert!(fate.ally);
        assert!(!fate.is_last_game, "交集对局不是最近一场时应标记为近期");
        assert_eq!(fate.badge_text(), "近期队友");
    }

    #[test]
    fn refresh_key_changes_when_champion_locked() {
        let mut source = champ_select_source();
        let before = source.refresh_key();
        source.blue[2].champion_id = Some(103);
        assert_ne!(before, source.refresh_key());
    }

    #[test]
    fn session_player_name_parts_falls_back_to_display_name() {
        let player = SessionPlayer {
            display_name: Some("玩家甲#CN9".to_string()),
            ..Default::default()
        };
        assert_eq!(
            player.name_parts(),
            ("玩家甲".to_string(), "CN9".to_string())
        );

        let empty = SessionPlayer::default();
        assert_eq!(empty.name_parts(), (String::new(), String::new()));
    }
}
