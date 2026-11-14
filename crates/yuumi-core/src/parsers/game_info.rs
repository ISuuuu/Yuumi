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

use crate::gameflow::match_stats::{
    compute_match_stats, AggregatedPositions, StreakInfo, StreakType,
};
use crate::gameflow::player_identity::get_valid_tpid;
use crate::gameflow::team_builder::{
    resolve_team_side, MapSide, SnapshotParticipant, TeamBuildContext,
};
use crate::lcu::client::LcuClient;
use crate::lcu::game_data::GameDataAssets;
use crate::parsers::match_detail::{fetch_game_detail, GameDetail};
use crate::parsers::match_parser::{
    fetch_match_history_smart, get_queue_info, is_laned_mode, MatchDisplay,
};

/// 每名玩家拉取的近期对局数量
pub const RECENT_MATCH_LIMIT: u32 = 50;
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
        // 未定级 / NONE / UNRANKED 不算已定级，界面上不再显示「未定级」
        is_ranked_tier(&self.tier)
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

/// 是否已定级（未定级 / NONE / UNRANKED / 空串 / "--" 均返回 false）
///
/// 界面按「无段位就不画段位文案」处理，避免到处出现「未定级」。
/// 同时接受英文 tier（LCU 原始值）与中文 tier（GameInfoPlayer.rank_tier 存的是中文名）。
pub fn is_ranked_tier(tier: &str) -> bool {
    let trimmed = tier.trim();
    let normalized = trimmed.to_ascii_uppercase();
    !normalized.is_empty()
        && normalized != "NONE"
        && normalized != "UNRANKED"
        && normalized != "NA"
        && normalized != "--"
        && trimmed != "未定级"
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

/// 宽容反序列化：兼容 LCU 在 `teamParticipantId` / `partyId` 上返回字符串、数字或 null
fn deserialize_opt_string_or_number<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct OptStringOrNumberVisitor;

    impl<'de> serde::de::Visitor<'de> for OptStringOrNumberVisitor {
        type Value = Option<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string, integer, or null")
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(None)
        }

        fn visit_some<D2>(self, deserializer: D2) -> Result<Self::Value, D2::Error>
        where
            D2: serde::Deserializer<'de>,
        {
            deserializer.deserialize_any(self)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(None)
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            if v.is_empty() {
                Ok(None)
            } else {
                Ok(Some(v.to_string()))
            }
        }

        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            if v.is_empty() {
                Ok(None)
            } else {
                Ok(Some(v))
            }
        }

        fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Some(v.to_string()))
        }

        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Some(v.to_string()))
        }

        fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Some((v as i64).to_string()))
        }
    }

    deserializer.deserialize_option(OptStringOrNumberVisitor)
}

/// 宽容反序列化：将 null 或缺失字段均转为默认值（避免 LCU 返回 `"teamTwo": null` 时整包失败）
fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    T: Default + serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    let opt = Option::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

/// 宽容反序列化：对局 ID 允许 null、0 或负数并归一化为 `Option<u64>`
fn deserialize_opt_u64_lenient<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<i64>::deserialize(deserializer)?;
    Ok(opt.and_then(|v| if v > 0 { Some(v as u64) } else { None }))
}

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
    #[serde(default, deserialize_with = "deserialize_opt_string_or_number")]
    pub team_participant_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_or_number")]
    pub party_id: Option<String>,
    #[serde(default)]
    pub bot: Option<bool>,
    #[serde(default)]
    pub is_bot: Option<bool>,
    #[serde(default)]
    pub bot_champion_id: Option<i32>,
    #[serde(default)]
    pub bot_name: Option<String>,
    #[serde(default)]
    pub profile_icon_id: Option<i32>,
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

    /// 当前英雄 ID（未锁定时回退到人机英雄或预选英雄）
    pub fn resolved_champion_id(&self) -> i32 {
        let picked = self.champion_id.unwrap_or(0);
        if picked > 0 {
            picked
        } else if let Some(bot_cid) = self.bot_champion_id.filter(|&c| c > 0) {
            bot_cid
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
            .or(self.bot_name.as_deref())
            .unwrap_or("")
            .trim();

        match raw.split_once('#') {
            Some((name, tag)) => (name.trim().to_string(), tag.trim().to_string()),
            None => (raw.to_string(), String::new()),
        }
    }

    pub fn is_bot(&self) -> bool {
        self.bot.unwrap_or(false)
            || self.is_bot.unwrap_or(false)
            || self.bot_champion_id.unwrap_or(0) > 0
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
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub my_team: Vec<SessionPlayer>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
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
    #[serde(default)]
    pub is_custom: Option<bool>,
}

/// gameflow `gameData.playerChampionSelections` 中的单条英雄选择记录
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerChampionSelection {
    #[serde(default)]
    pub summoner_internal_name: Option<String>,
    #[serde(default)]
    pub puuid: Option<String>,
    #[serde(default)]
    pub summoner_id: Option<i64>,
    #[serde(default)]
    pub champion_id: Option<i32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameflowGameData {
    #[serde(default, deserialize_with = "deserialize_opt_u64_lenient")]
    pub game_id: Option<u64>,
    #[serde(default)]
    pub queue: Option<GameflowQueueInfo>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub team_one: Vec<SessionPlayer>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub team_two: Vec<SessionPlayer>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub player_champion_selections: Vec<PlayerChampionSelection>,
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
    pub detected_side: Option<MapSide>,
}

impl GameInfoSource {
    /// 从选人会话构建（依据 pin-drop 阵营或 cellId 区分蓝方/红方，不再硬编码 myTeam 为蓝方）
    pub fn from_champ_select(
        snapshot: &ChampSelectSnapshot,
        pin_drop_side: Option<MapSide>,
    ) -> Option<Self> {
        if snapshot.my_team.is_empty() && snapshot.their_team.is_empty() {
            return None;
        }
        let user_side = pin_drop_side.unwrap_or_else(|| {
            if let Some(cell_id) = snapshot.local_player_cell_id {
                if cell_id >= 5 {
                    return MapSide::Red;
                } else {
                    return MapSide::Blue;
                }
            }
            let has_red = snapshot
                .my_team
                .iter()
                .any(|p| p.cell_id.unwrap_or(-1) >= 5);
            let has_blue = snapshot.my_team.iter().any(|p| {
                let cid = p.cell_id.unwrap_or(-1);
                (0..5).contains(&cid)
            });
            if has_red && !has_blue {
                MapSide::Red
            } else {
                MapSide::Blue
            }
        });

        let (blue, red) = if user_side == MapSide::Red {
            (snapshot.their_team.clone(), snapshot.my_team.clone())
        } else {
            (snapshot.my_team.clone(), snapshot.their_team.clone())
        };

        Some(Self {
            phase: "ChampSelect".to_string(),
            queue_id: snapshot.queue_id.unwrap_or(0),
            game_id: None,
            blue,
            red,
            local_player_cell_id: snapshot.local_player_cell_id,
            detected_side: Some(user_side),
        })
    }

    /// 从 gameflow 会话构建（teamOne = 蓝方 100，teamTwo = 红方 200）
    pub fn from_gameflow(phase: &str, snapshot: &GameflowSessionSnapshot) -> Option<Self> {
        let game_data = snapshot.game_data.as_ref()?;
        if game_data.team_one.is_empty() && game_data.team_two.is_empty() {
            return None;
        }
        let mut blue = game_data.team_one.clone();
        let mut red = game_data.team_two.clone();
        // 原因 C 回填：teamOne/teamTwo 成员缺 championId 时，从 playerChampionSelections
        // 按 puuid 或 summonerId 补全英雄 ID
        backfill_champion_from_selections(&mut blue, &game_data.player_champion_selections);
        backfill_champion_from_selections(&mut red, &game_data.player_champion_selections);
        Some(Self {
            phase: phase.to_string(),
            queue_id: game_data.queue.as_ref().and_then(|q| q.id).unwrap_or(0),
            game_id: game_data.game_id,
            blue,
            red,
            local_player_cell_id: None,
            detected_side: None,
        })
    }

    /// 去重键：阶段 / 对局号 / 双方槽位英雄与身份，避免选人高频道次重复拉取
    pub fn refresh_key(&self) -> String {
        let mut parts = vec![
            self.phase.clone(),
            self.game_id.unwrap_or(0).to_string(),
            self.queue_id.to_string(),
            self.detected_side
                .map(|s| s.as_str())
                .unwrap_or("none")
                .to_string(),
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

/// 原因 C：gameflow 成员缺英雄 ID（<=0，含 botChampionId / pickIntent 兜底后仍无值）时，
/// 从 `playerChampionSelections` 按 puuid 优先、summonerId 次之回填 `championId`。
pub fn backfill_champion_from_selections(
    team: &mut [SessionPlayer],
    selections: &[PlayerChampionSelection],
) {
    if selections.is_empty() {
        return;
    }
    let valid_cid = |sel: &PlayerChampionSelection| sel.champion_id.filter(|&c| c > 0);

    for player in team.iter_mut() {
        if player.resolved_champion_id() > 0 {
            continue;
        }
        let by_puuid = player.resolved_puuid().and_then(|puuid| {
            selections
                .iter()
                .find(|sel| sel.puuid.as_deref().map(str::trim) == Some(puuid))
                .and_then(&valid_cid)
        });
        let by_summoner_id = player.resolved_summoner_id().and_then(|sid| {
            selections
                .iter()
                .find(|sel| sel.summoner_id == Some(sid))
                .and_then(&valid_cid)
        });
        if let Some(cid) = by_puuid.or(by_summoner_id) {
            player.champion_id = Some(cid);
        }
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
    /// 上面对应的英雄名（展示用，构建时填充）
    #[serde(default)]
    pub champion_name: String,
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

/// 单名玩家近期常用英雄统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopChampion {
    pub champion_id: i32,
    pub champion_name: String,
    pub count: usize,
    pub win_rate: u32,
}

/// 单条英雄熟练度（仅保留判定所需字段）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MasteryEntry {
    pub champion_id: i32,
    pub level: u32,
    pub points: u32,
}

/// 当前所选英雄的熟练度摘要（招牌 / 练英雄判定，对齐 Vue getPlayerMasteryDetail）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MasteryBadge {
    pub level: u32,
    pub points: u32,
    /// 招牌：熟练度前 3 且点数 >= 1w，或总点数 >= 10w
    pub is_top_champion: bool,
    /// 练英雄：非招牌且等级 <= 1 或点数 < 5000
    pub is_practicing: bool,
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
    /// 近期召唤师峡谷对局胜场（不含重开与娱乐模式）
    #[serde(default)]
    pub recent_wins: u32,
    /// 近期召唤师峡谷对局负场
    #[serde(default)]
    pub recent_losses: u32,
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
    /// 召唤师等级
    #[serde(default)]
    pub summoner_level: u32,
    /// 头像图标 ID
    #[serde(default)]
    pub profile_icon_id: i32,
    /// 近期对局历史列表
    #[serde(default)]
    pub matches: Vec<MatchDisplay>,
    /// 近期常用英雄统计（最多 3 个）
    #[serde(default)]
    pub top_champions: Vec<TopChampion>,
    /// 路人集自定义标签 (如 "大腿", "避坑")
    #[serde(default)]
    pub saved_tag: Option<String>,
    /// 历史相遇总次数
    #[serde(default)]
    pub saved_encounter_count: u32,
    /// 近期对局位置分布（主玩 / 副玩占比）
    #[serde(default)]
    pub positions: Option<AggregatedPositions>,
    /// 当前英雄熟练度摘要（招牌 / 练英雄）
    #[serde(default)]
    pub mastery: Option<MasteryBadge>,
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
    /// 是否为预览/演示数据
    #[serde(default)]
    pub is_demo: bool,
}

impl LiveGameInfo {
    pub fn players(&self) -> impl Iterator<Item = &GameInfoPlayer> {
        self.blue_team.iter().chain(self.red_team.iter())
    }

    /// 尝试从本地快照文件（YuumiX 或 Yuumi 的 gameinfo_snapshot.json）加载真实历史对局，
    /// 若无快照或解析异常，则回退到内置的演示数据生成器。
    pub fn load_snapshot_or_demo() -> Self {
        if let Some(info) = Self::try_load_snapshot() {
            return info;
        }
        Self::create_demo()
    }

    /// 尝试读取本地的对局快照
    pub fn try_load_snapshot() -> Option<Self> {
        let base_dirs = [
            dirs::config_dir().map(|p| p.join("YuumiX").join("gameinfo_snapshot.json")),
            dirs::config_dir().map(|p| p.join("Yuumi").join("gameinfo_snapshot.json")),
        ];

        for path_opt in base_dirs {
            let Some(path) = path_opt else { continue };
            if !path.exists() {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(info) = parse_snapshot_json(&content) {
                    return Some(info);
                }
            }
        }
        None
    }

    /// 创建高保真演示对局数据（在未运行游戏或未在对局时预览）
    pub fn create_demo() -> Self {
        make_demo_instance()
    }

    /// 落盘快照文件（保留对局信息 / 断线重连后直接复用，对齐旧版 Vue 的 localStorage 快照）
    pub fn cache_file_path() -> std::path::PathBuf {
        crate::runtime::app_data_dir().join("game_info_cache.json")
    }

    /// 保存对局快照：只存真实对局，且同一局内不允许被更瘦的快照覆盖
    pub fn save_cache(&self) {
        if self.is_demo || self.game_id.unwrap_or(0) == 0 {
            return;
        }
        if self.blue_team.len() + self.red_team.len() == 0 {
            return;
        }
        if !should_overwrite_cache(Self::load_cache().as_ref(), self) {
            log::debug!("[GameInfo] 跳过写入更瘦的对局快照");
            return;
        }
        let path = Self::cache_file_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match serde_json::to_string(self) {
            Ok(json) => {
                if let Err(error) = std::fs::write(&path, json) {
                    log::warn!("[GameInfo] 写入对局快照失败: {error}");
                }
            }
            Err(error) => log::warn!("[GameInfo] 序列化对局快照失败: {error}"),
        }
    }

    /// 读取落盘快照（演示数据不入缓存）
    pub fn load_cache() -> Option<Self> {
        let content = std::fs::read_to_string(Self::cache_file_path()).ok()?;
        serde_json::from_str::<Self>(&content)
            .ok()
            .filter(|info| !info.is_demo && info.game_id.unwrap_or(0) > 0)
    }

    /// 新一轮选人开始时清空落盘快照（与玩家数据缓存同步失效）
    pub fn clear_cache() {
        let _ = std::fs::remove_file(Self::cache_file_path());
    }
}

/// 是否允许用新快照覆盖已落盘快照：
/// 同一局内队伍人数变少说明是 InProgress 阶段的精简 session，禁止覆盖完整快照。
fn should_overwrite_cache(prev: Option<&LiveGameInfo>, next: &LiveGameInfo) -> bool {
    let Some(prev) = prev else {
        return true;
    };
    if prev.game_id != next.game_id {
        return true;
    }
    next.blue_team.len() + next.red_team.len() >= prev.blue_team.len() + prev.red_team.len()
}

/// 两份快照是否属于同一局：gameId 相等，或上一份是刚结束的选人快照
/// （gameId 尚未产生，选人必然紧邻对局开始）。演示数据与跨局快照一律不算同局。
fn is_same_match_snapshot(prev: &LiveGameInfo, next: &LiveGameInfo) -> bool {
    if prev.is_demo || next.game_id.is_none() {
        return false;
    }
    prev.game_id == next.game_id || (prev.game_id.is_none() && prev.phase == "ChampSelect")
}

/// 两名玩家是否同一人：puuid 或 summonerId 任一强身份相等
fn same_strong_identity(a: &GameInfoPlayer, b: &GameInfoPlayer) -> bool {
    (!a.puuid.is_empty() && a.puuid == b.puuid)
        || (a.summoner_id != 0 && a.summoner_id == b.summoner_id)
}

/// 队伍残缺补齐（对齐旧版 Vue「新队伍更短时保留已有更全的，不让整列消失」）：
/// 隐藏战绩玩家会被 LCU 从 gameflow 的 teamOne/teamTwo 剔除（2999 与
/// playerChampionSelections 里仍可见），导致选人阶段还在的玩家到加载阶段整列消失。
/// 从上一份同局快照按强身份把缺失玩家补回，返回 (蓝方补回数, 红方补回数)。
pub fn merge_missing_players_from_prev(
    next: &mut LiveGameInfo,
    prev: &LiveGameInfo,
) -> (usize, usize) {
    if !is_same_match_snapshot(prev, next) {
        return (0, 0);
    }
    let merge_side = |next_team: &mut Vec<GameInfoPlayer>, prev_team: &[GameInfoPlayer]| {
        let target = prev_team.len().min(5);
        // 整边恢复（session 单边被清空）时保留上一份快照的组队编号；
        // 部分补人时旧编号在新队伍里已失效，置空防止与现有分组误配同色
        let full_restore = next_team.is_empty();
        let mut added = 0;
        for player in prev_team {
            if next_team.len() >= target {
                break;
            }
            if next_team.iter().any(|p| same_strong_identity(p, player)) {
                continue;
            }
            let mut player = player.clone();
            if !full_restore {
                player.premade_group_id = None;
            }
            next_team.push(player);
            added += 1;
        }
        added
    };
    let blue = merge_side(&mut next.blue_team, &prev.blue_team);
    let red = merge_side(&mut next.red_team, &prev.red_team);
    (blue, red)
}
#[allow(clippy::too_many_arguments)]
fn make_mock_match(
    queue_id: i32,
    name: &str,
    win: bool,
    champion_id: i32,
    kills: i32,
    deaths: i32,
    assists: i32,
    short_time: &str,
) -> MatchDisplay {
    let kda = if deaths == 0 {
        "Perfect".to_string()
    } else {
        format!("{:.1}", (kills + assists) as f64 / deaths as f64)
    };
    MatchDisplay {
        queue_id,
        game_id: 100_000_000 + champion_id as u64,
        time: format!("2026-11-08 {}", short_time),
        short_time: short_time.to_string(),
        name: name.to_string(),
        map: if queue_id == 450 || queue_id == 2400 {
            "嚎哭深渊".to_string()
        } else {
            "召唤师峡谷".to_string()
        },
        duration: "25:30".to_string(),
        remake: false,
        win,
        placement: None,
        champion_id,
        spell1_id: 4,
        spell2_id: 14,
        champ_level: 15,
        kills,
        deaths,
        assists,
        kda,
        item_ids: vec![3078, 3053, 3111, 3075, 3065, 3742, 3340],
        rune_id: 8000,
        cs: 180,
        gold: 12500,
        time_stamp: 1794000000000,
        total_damage: 26000,
        total_heal: 3200,
        champion_icon_url: format!(
            "/lol-game-data/assets/v1/champion-icons/{}.png",
            champion_id
        ),
        spell1_icon_url: "/lol-game-data/assets/v1/summoner-spells/4.png".to_string(),
        spell2_icon_url: "/lol-game-data/assets/v1/summoner-spells/14.png".to_string(),
        rune_icon_url: String::new(),
        item_icon_urls: vec![
            "/lol-game-data/assets/v1/items/3078.png".to_string(),
            "/lol-game-data/assets/v1/items/3053.png".to_string(),
            "/lol-game-data/assets/v1/items/3111.png".to_string(),
        ],
        augment_ids: vec![],
        augment_icon_urls: vec![],
        augment_names: vec![],
        position: Some("MIDDLE".to_string()),
        role_bound_item: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn make_mock_player(
    puuid: &str,
    summoner_id: i64,
    cell_id: i64,
    summoner_name: &str,
    tag_line: &str,
    champion_id: i32,
    champion_name: &str,
    position: &str,
    premade_group_id: Option<usize>,
    rank_tier: &str,
    rank_division: &str,
    rank_lp: u32,
    ranked_wins: u32,
    ranked_losses: u32,
    win_rate: u32,
    avg_kda: f64,
    kills: f32,
    deaths: f32,
    assists: f32,
    streak_text: Option<&str>,
    streak_is_win: bool,
    fate: Option<FateInfo>,
    is_local: bool,
    summoner_level: u32,
    profile_icon_id: i32,
) -> GameInfoPlayer {
    let matches = vec![
        make_mock_match(420, "排位单双", true, champion_id, 10, 2, 8, "11-08 14:20"),
        make_mock_match(420, "排位单双", false, champion_id, 4, 6, 5, "11-08 13:30"),
        make_mock_match(2400, "海克斯大乱斗", true, 30, 12, 5, 20, "11-07 22:15"),
        make_mock_match(420, "排位单双", true, champion_id, 8, 3, 11, "11-07 20:00"),
        make_mock_match(450, "极地大乱斗", false, 21, 6, 8, 14, "11-06 18:40"),
    ];
    let top_champions = vec![
        TopChampion {
            champion_id,
            champion_name: champion_name.to_string(),
            count: 3,
            win_rate: 67,
        },
        TopChampion {
            champion_id: 30,
            champion_name: "卡尔萨斯".to_string(),
            count: 1,
            win_rate: 100,
        },
        TopChampion {
            champion_id: 21,
            champion_name: "厄运小姐".to_string(),
            count: 1,
            win_rate: 0,
        },
    ];

    GameInfoPlayer {
        puuid: puuid.to_string(),
        summoner_id,
        cell_id,
        summoner_name: summoner_name.to_string(),
        tag_line: tag_line.to_string(),
        champion_id,
        champion_name: champion_name.to_string(),
        position: position.to_string(),
        team_participant_id: Some(format!("{}", cell_id)),
        premade_group_id,
        rank_tier: rank_tier.to_string(),
        rank_division: rank_division.to_string(),
        rank_lp,
        rank_display: if rank_tier == "大师" || rank_tier == "宗师" || rank_tier == "王者" {
            format!("{} · {} LP", rank_tier, rank_lp)
        } else if rank_tier == "未定级" || rank_tier == "--" {
            "未定级".to_string()
        } else {
            format!("{} {} · {} LP", rank_tier, rank_division, rank_lp)
        },
        ranked_wins,
        ranked_losses,
        recent_match_count: matches.len(),
        recent_wins: matches
            .iter()
            .filter(|m| !m.remake && is_laned_mode(m.queue_id) && m.win)
            .count() as u32,
        recent_losses: matches
            .iter()
            .filter(|m| !m.remake && is_laned_mode(m.queue_id) && !m.win)
            .count() as u32,
        win_rate,
        avg_kda,
        avg_kills: kills,
        avg_deaths: deaths,
        avg_assists: assists,
        streak_text: streak_text.map(|s| s.to_string()),
        streak_is_win,
        fate,
        is_local_player: is_local,
        is_bot: false,
        summoner_level,
        profile_icon_id,
        matches,
        top_champions,
        saved_tag: None,
        saved_encounter_count: 0,
        positions: None,
        mastery: None,
        data_available: true,
    }
}

fn make_demo_instance() -> LiveGameInfo {
    LiveGameInfo {
        phase: "InProgress".to_string(),
        phase_label: "对局进行中".to_string(),
        queue_id: 420,
        queue_name: "单双排位".to_string(),
        game_id: Some(9_999_999_999),
        user_side: Some(MapSide::Blue),
        side_matched: true,
        snapshot_at: chrono::Utc::now().timestamp_millis(),
        is_demo: true,
        blue_team: vec![
            make_mock_player(
                "demo-p0",
                1000,
                0,
                "疾风剑豪",
                "国服",
                157,
                "疾风剑豪",
                "中单",
                Some(1),
                "大师",
                "I",
                188,
                35,
                15,
                70,
                3.8,
                8.5,
                3.2,
                6.1,
                Some("3连胜"),
                true,
                None,
                true,
                125,
                1116,
            ),
            make_mock_player(
                "demo-p1",
                1001,
                1,
                "瞎子王",
                "001",
                64,
                "盲僧",
                "打野",
                Some(1),
                "钻石",
                "I",
                65,
                30,
                20,
                60,
                3.2,
                6.0,
                4.0,
                7.0,
                None,
                false,
                None,
                false,
                88,
                7025,
            ),
            make_mock_player(
                "demo-p2",
                1002,
                2,
                "九尾天狐",
                "阿狸",
                103,
                "九尾妖狐",
                "中单",
                None,
                "钻石",
                "II",
                40,
                28,
                22,
                56,
                3.5,
                7.2,
                3.0,
                6.5,
                None,
                false,
                Some(FateInfo {
                    ally: true,
                    is_last_game: true,
                    game_id: 9999999001,
                    game_creation: 1794000000000,
                    champion_id: 103,
                    champion_name: "九尾妖狐".to_string(),
                    ally_count: 2,
                    enemy_count: 0,
                }),
                false,
                210,
                7037,
            ),
            make_mock_player(
                "demo-p3",
                1003,
                3,
                "暴走小萝莉",
                "ADC",
                222,
                "暴走萝莉",
                "下路",
                None,
                "翡翠",
                "I",
                88,
                25,
                25,
                50,
                2.8,
                8.0,
                5.0,
                6.0,
                None,
                false,
                None,
                false,
                65,
                1439,
            ),
            make_mock_player(
                "demo-p4",
                1004,
                4,
                "魂锁典狱长",
                "辅助",
                412,
                "魂锁典狱长",
                "辅助",
                None,
                "翡翠",
                "II",
                15,
                20,
                30,
                40,
                2.5,
                1.5,
                4.0,
                11.0,
                None,
                false,
                None,
                false,
                150,
                7210,
            ),
        ],
        red_team: vec![
            make_mock_player(
                "demo-p5",
                2000,
                5,
                "熔岩巨兽",
                "上单",
                54,
                "熔岩巨兽",
                "上单",
                Some(2),
                "钻石",
                "I",
                92,
                32,
                18,
                64,
                3.1,
                4.5,
                3.5,
                8.0,
                None,
                false,
                None,
                false,
                95,
                7089,
            ),
            make_mock_player(
                "demo-p6",
                2001,
                6,
                "德玛西亚皇子",
                "野王",
                59,
                "德玛西亚皇子",
                "打野",
                Some(2),
                "钻石",
                "III",
                12,
                26,
                24,
                52,
                2.9,
                5.0,
                4.5,
                7.5,
                None,
                false,
                None,
                false,
                140,
                3158,
            ),
            make_mock_player(
                "demo-p7",
                2002,
                7,
                "影流之主",
                "火影",
                238,
                "影流之主",
                "中单",
                Some(2),
                "大师",
                "I",
                240,
                40,
                10,
                80,
                4.2,
                11.0,
                3.0,
                4.0,
                Some("4连胜"),
                true,
                Some(FateInfo {
                    ally: false,
                    is_last_game: true,
                    game_id: 9999999002,
                    game_creation: 1794000000000,
                    champion_id: 238,
                    champion_name: "影流之镰".to_string(),
                    ally_count: 0,
                    enemy_count: 3,
                }),
                false,
                320,
                25,
            ),
            make_mock_player(
                "demo-p8",
                2003,
                8,
                "虚空之女",
                "卡莎",
                145,
                "虚空之女",
                "下路",
                None,
                "钻石",
                "IV",
                55,
                24,
                26,
                48,
                2.7,
                7.5,
                5.5,
                5.0,
                None,
                false,
                None,
                false,
                78,
                4209,
            ),
            make_mock_player(
                "demo-p9",
                2004,
                9,
                "深海泰坦",
                "诺提",
                111,
                "深海泰坦",
                "辅助",
                None,
                "翡翠",
                "I",
                33,
                25,
                25,
                50,
                2.4,
                2.0,
                5.0,
                10.0,
                None,
                false,
                None,
                false,
                112,
                1116,
            ),
        ],
    }
}

/// 解析本地 gameinfo_snapshot.json 文件内容并映射为 LiveGameInfo
fn parse_snapshot_json(content: &str) -> Result<LiveGameInfo, Box<dyn std::error::Error>> {
    let root: serde_json::Value = serde_json::from_str(content)?;
    let snapshot = root.get("Snapshot").unwrap_or(&root);

    let my_team = snapshot.get("MyTeam").and_then(|v| v.as_array());
    let their_team = snapshot.get("TheirTeam").and_then(|v| v.as_array());
    let player_data = snapshot.get("PlayerData").and_then(|v| v.as_object());

    if my_team.is_none() && their_team.is_none() {
        return Err("快照中未包含队伍成员数据".into());
    }

    let mut premade_counts: HashMap<String, usize> = HashMap::new();
    let mut count_parties = |team: &Option<&Vec<serde_json::Value>>| {
        if let Some(arr) = team {
            for slot in *arr {
                if let Some(pid) = slot.get("PartyId").and_then(|v| v.as_str()) {
                    if !pid.is_empty() {
                        *premade_counts.entry(pid.to_string()).or_default() += 1;
                    }
                }
            }
        }
    };
    count_parties(&my_team);
    count_parties(&their_team);

    let mut party_group_id_map: HashMap<String, usize> = HashMap::new();
    let mut next_group_id = 1;
    for (pid, count) in premade_counts.iter() {
        if *count >= 2 {
            party_group_id_map.insert(pid.clone(), next_group_id);
            next_group_id += 1;
        }
    }

    let map_team = |team_opt: Option<&Vec<serde_json::Value>>| -> Vec<GameInfoPlayer> {
        let mut players = Vec::new();
        let Some(team_arr) = team_opt else {
            return players;
        };

        for (idx, slot) in team_arr.iter().enumerate() {
            let cell_id = slot
                .get("CellId")
                .and_then(|v| v.as_i64())
                .unwrap_or(idx as i64);
            let puuid = slot
                .get("Puuid")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let champion_id = slot.get("ChampionId").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let party_id = slot.get("PartyId").and_then(|v| v.as_str()).unwrap_or("");
            let premade_group_id = party_group_id_map.get(party_id).copied();

            let pd = player_data.and_then(|m| m.get(&cell_id.to_string()));

            let (summoner_name, tag_line, summoner_level, profile_icon_id) =
                if let Some(info) = pd.and_then(|d| d.get("Info")) {
                    let s_name = info
                        .get("GameName")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let tag = info
                        .get("TagLine")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let lvl = info
                        .get("SummonerLevel")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(30) as u32;
                    let icon = info
                        .get("ProfileIconId")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(
                            slot.get("ProfileIconId")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(29),
                        ) as i32;
                    (s_name, tag, lvl, icon)
                } else {
                    let disp = slot
                        .get("DisplayName")
                        .and_then(|v| v.as_str())
                        .unwrap_or("召唤师")
                        .to_string();
                    let icon = slot
                        .get("ProfileIconId")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(29) as i32;
                    (disp, "".to_string(), 30, icon)
                };

            let name = if summoner_name.is_empty() {
                slot.get("DisplayName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("召唤师")
                    .to_string()
            } else {
                summoner_name
            };

            let (rank_tier, rank_division, rank_lp, ranked_wins, ranked_losses) =
                if let Some(solo) = pd.and_then(|d| d.get("Solo")) {
                    let tier = solo
                        .get("TierText")
                        .and_then(|v| v.as_str())
                        .unwrap_or("未定级")
                        .to_string();
                    let div = solo
                        .get("Division")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let lp = solo
                        .get("LeaguePoints")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;
                    let wins = solo.get("Wins").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    let losses = solo.get("Losses").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    (tier, div, lp, wins, losses)
                } else {
                    ("未定级".to_string(), "".to_string(), 0, 0, 0)
                };

            let win_rate = pd
                .and_then(|d| d.get("WinRate").and_then(|v| v.as_u64()))
                .unwrap_or(50) as u32;
            let avg_kda = pd
                .and_then(|d| d.get("AvgKda").and_then(|v| v.as_f64()))
                .unwrap_or(2.5);

            let mut matches = Vec::new();
            if let Some(m_arr) = pd.and_then(|d| d.get("Matches").and_then(|v| v.as_array())) {
                for m_val in m_arr {
                    let m_champ_id = m_val
                        .get("ChampionId")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0) as i32;
                    let m_win = m_val.get("Win").and_then(|v| v.as_bool()).unwrap_or(false);
                    let m_q_name = m_val
                        .get("QueueName")
                        .and_then(|v| v.as_str())
                        .unwrap_or("匹配模式")
                        .to_string();
                    let m_map = m_val
                        .get("MapName")
                        .and_then(|v| v.as_str())
                        .unwrap_or("召唤师峡谷")
                        .to_string();
                    let m_kda = m_val
                        .get("KdaText")
                        .and_then(|v| v.as_str())
                        .unwrap_or("0.0")
                        .to_string();
                    let m_time = m_val
                        .get("TimeText")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let m_short_time = m_val
                        .get("ShortTimeText")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let m_duration = m_val
                        .get("DurationText")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let m_kills = m_val.get("Kills").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    let m_deaths = m_val.get("Deaths").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    let m_assists =
                        m_val.get("Assists").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    let m_lvl = m_val
                        .get("ChampionLevel")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(18) as i32;
                    let m_spell1 =
                        m_val.get("Spell1Id").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    let m_spell2 =
                        m_val.get("Spell2Id").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    let m_qid = m_val.get("QueueId").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                    let m_gid = m_val.get("GameId").and_then(|v| v.as_u64()).unwrap_or(0);
                    let m_remake = m_val
                        .get("Remake")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    let item_ids: Vec<i32> = m_val
                        .get("ItemIds")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|x| x.as_i64().map(|n| n as i32))
                                .collect()
                        })
                        .unwrap_or_default();
                    let augment_ids: Vec<i32> = m_val
                        .get("AugmentIds")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|x| x.as_i64().map(|n| n as i32))
                                .collect()
                        })
                        .unwrap_or_default();

                    matches.push(MatchDisplay {
                        queue_id: m_qid,
                        game_id: m_gid,
                        time: m_time,
                        short_time: m_short_time,
                        name: m_q_name,
                        map: m_map,
                        duration: m_duration,
                        remake: m_remake,
                        win: m_win,
                        placement: None,
                        champion_id: m_champ_id,
                        spell1_id: m_spell1,
                        spell2_id: m_spell2,
                        champ_level: m_lvl,
                        kills: m_kills,
                        deaths: m_deaths,
                        assists: m_assists,
                        kda: m_kda,
                        item_ids: item_ids.clone(),
                        rune_id: 0,
                        cs: m_val
                            .get("CreepScore")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0) as i32,
                        gold: m_val.get("Gold").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                        time_stamp: m_val
                            .get("GameCreation")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0),
                        total_damage: m_val
                            .get("DamageToChampions")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0) as i32,
                        total_heal: m_val.get("TotalHeal").and_then(|v| v.as_i64()).unwrap_or(0)
                            as i32,
                        champion_icon_url: format!(
                            "/lol-game-data/assets/v1/champion-icons/{}.png",
                            m_champ_id
                        ),
                        spell1_icon_url: format!(
                            "/lol-game-data/assets/v1/summoner-spells/{}.png",
                            m_spell1
                        ),
                        spell2_icon_url: format!(
                            "/lol-game-data/assets/v1/summoner-spells/{}.png",
                            m_spell2
                        ),
                        rune_icon_url: String::new(),
                        item_icon_urls: item_ids
                            .iter()
                            .map(|id| format!("/lol-game-data/assets/v1/items/{}.png", id))
                            .collect(),
                        augment_ids: augment_ids.clone(),
                        augment_icon_urls: augment_ids
                            .iter()
                            .map(|id| {
                                format!("/lol-game-data/assets/v1/cherry-augments/{}.png", id)
                            })
                            .collect(),
                        augment_names: vec![],
                        position: None,
                        role_bound_item: None,
                    });
                }
            }

            let mut champ_counts: HashMap<i32, (usize, usize)> = HashMap::new();
            for m in &matches {
                let entry = champ_counts.entry(m.champion_id).or_insert((0, 0));
                entry.0 += 1;
                if m.win {
                    entry.1 += 1;
                }
            }
            let mut top_champions: Vec<TopChampion> = champ_counts
                .into_iter()
                .map(|(cid, (count, wins))| TopChampion {
                    champion_id: cid,
                    champion_name: format!("{}", cid),
                    count,
                    win_rate: if count > 0 {
                        ((wins as f64 / count as f64) * 100.0).round() as u32
                    } else {
                        0
                    },
                })
                .collect();
            top_champions.sort_by_key(|c| std::cmp::Reverse(c.count));
            top_champions.truncate(3);

            let rank_display = if rank_tier == "未定级" || rank_tier == "--" || rank_tier.is_empty()
            {
                "未定级".to_string()
            } else if rank_tier == "大师" || rank_tier == "宗师" || rank_tier == "王者" {
                format!("{} · {} LP", rank_tier, rank_lp)
            } else {
                format!("{} {} · {} LP", rank_tier, rank_division, rank_lp)
            };

            players.push(GameInfoPlayer {
                puuid,
                summoner_id: slot.get("SummonerId").and_then(|v| v.as_i64()).unwrap_or(0),
                cell_id,
                summoner_name: name,
                tag_line,
                champion_id,
                champion_name: format!("{}", champion_id),
                position: "".to_string(),
                team_participant_id: Some(format!("{}", cell_id)),
                premade_group_id,
                rank_tier,
                rank_division,
                rank_lp,
                rank_display,
                ranked_wins,
                ranked_losses,
                recent_match_count: matches.len(),
                recent_wins: matches
                    .iter()
                    .filter(|m| !m.remake && is_laned_mode(m.queue_id) && m.win)
                    .count() as u32,
                recent_losses: matches
                    .iter()
                    .filter(|m| !m.remake && is_laned_mode(m.queue_id) && !m.win)
                    .count() as u32,
                win_rate,
                avg_kda,
                avg_kills: 0.0,
                avg_deaths: 0.0,
                avg_assists: 0.0,
                streak_text: None,
                streak_is_win: false,
                fate: None,
                is_local_player: idx == 0,
                is_bot: false,
                summoner_level,
                profile_icon_id,
                matches,
                top_champions,
                saved_tag: None,
                saved_encounter_count: 0,
                positions: None,
                mastery: None,
                data_available: true,
            });
        }
        players
    };

    let blue_team = map_team(my_team);
    let red_team = map_team(their_team);

    Ok(LiveGameInfo {
        phase: "InProgress".to_string(),
        phase_label: "历史对局预览".to_string(),
        queue_id: 2400,
        queue_name: "海克斯大乱斗".to_string(),
        game_id: None,
        blue_team,
        red_team,
        user_side: Some(MapSide::Blue),
        side_matched: true,
        snapshot_at: chrono::Utc::now().timestamp_millis(),
        is_demo: true,
    })
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
    /// 会话内名字为空时的兜底身份（对局阶段 gameflow teamOne/teamTwo 的 summonerName 常为空串）
    pub identity: Option<SummonerIdentity>,
    /// 英雄熟练度列表（按点数降序），用于招牌 / 练英雄判定
    pub masteries: Vec<MasteryEntry>,
}

/// 玩家真实 Riot ID
#[derive(Debug, Clone, Default)]
pub struct SummonerIdentity {
    pub game_name: String,
    pub tag_line: String,
    pub level: u32,
}

/// 会话内可用于召唤师身份反查的线索
#[derive(Debug, Clone, Default)]
pub struct IdentityHint {
    pub summoner_id: i64,
    /// 会话里已有的名字（脱敏/占位也算），用于 puuid 与 summonerId 都查不到时按名反查
    pub fallback_name: Option<String>,
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
        champion_name: String::new(),
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

/// 阵营绑定：对局阶段优先按真实 teamOne/teamTwo 匹配；选人或脱敏阶段采用 detected_side 兜底
fn bind_team_side(
    source: &GameInfoSource,
    self_puuid: &str,
    self_summoner_id: Option<i64>,
) -> (MapSide, bool) {
    let is_champ_select = source.phase == "ChampSelect";
    let blue_snapshots: Vec<SnapshotParticipant> = source.blue.iter().map(snapshot_of).collect();
    let red_snapshots: Vec<SnapshotParticipant> = source.red.iter().map(snapshot_of).collect();

    let ctx = TeamBuildContext {
        current_summoner_id: self_summoner_id,
        current_summoner_puuid: Some(self_puuid),
        map_side: source.detected_side,
        champ_select_team_snapshot: &[],
    };

    let (side, matched) = resolve_team_side(&blue_snapshots, &red_snapshots, &ctx);
    let resolved = source.detected_side.or(side).unwrap_or(MapSide::Blue);
    (resolved, matched && !is_champ_select)
}

/// 构建单名玩家的对局信息（共享上下文集中传入，避免参数过长）
struct PlayerBuildContext<'a> {
    my_games: &'a [MyRecentGame],
    latest_my_game_id: Option<u64>,
    self_puuid: &'a str,
    self_summoner_id: Option<i64>,
    assets: &'a GameDataAssets,
    saved_markers: HashMap<String, (Option<String>, u32)>,
}

fn build_player(
    session: &SessionPlayer,
    premade_group_id: Option<usize>,
    fetch: Option<&PlayerFetchData>,
    ctx: &PlayerBuildContext<'_>,
) -> GameInfoPlayer {
    let (mut summoner_name, mut tag_line) = session.name_parts();
    // 名字以召唤师接口返回为准（对齐 Vue 版），会话字段仅在接口不可用时兜底
    if let Some(identity) = fetch.and_then(|f| f.identity.as_ref()) {
        summoner_name = identity.game_name.clone();
        if !identity.tag_line.is_empty() {
            tag_line = identity.tag_line.clone();
        }
    }
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

    let is_local_player = !puuid.is_empty()
        && (puuid == ctx.self_puuid
            || (summoner_id != 0 && Some(summoner_id) == ctx.self_summoner_id));

    let player_game_ids: Vec<u64> = recent.iter().map(|m| m.game_id).collect();
    // 本人不计算宿命：自己的对局列表必然与「本人近期对局」全部命中，会错误地显示「友」标记
    let fate = if puuid.is_empty() || is_local_player {
        None
    } else {
        detect_fate(
            &puuid,
            &player_game_ids,
            ctx.my_games,
            ctx.latest_my_game_id,
        )
        .map(|mut fate| {
            fate.champion_name = ctx
                .assets
                .champions
                .get(&fate.champion_id)
                .cloned()
                .unwrap_or_default();
            fate
        })
    };

    let mut champ_counts: HashMap<i32, (usize, usize)> = HashMap::new();
    for m in recent {
        let entry = champ_counts.entry(m.champion_id).or_insert((0, 0));
        entry.0 += 1;
        if m.win {
            entry.1 += 1;
        }
    }
    let mut top_champions: Vec<TopChampion> = champ_counts
        .into_iter()
        .map(|(cid, (total, wins))| {
            let win_rate = wins.saturating_mul(100).checked_div(total).unwrap_or(0) as u32;
            let cname = ctx.assets.champions.get(&cid).cloned().unwrap_or_else(|| {
                if cid > 0 {
                    format!("英雄 {}", cid)
                } else {
                    "未知".to_string()
                }
            });
            TopChampion {
                champion_id: cid,
                champion_name: cname,
                count: total,
                win_rate,
            }
        })
        .collect();
    top_champions.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| b.win_rate.cmp(&a.win_rate))
    });
    top_champions.truncate(3);

    // 近期胜负只统计召唤师峡谷对局（排除重开与娱乐模式），对齐旧版「近 50 局统计胜负」
    let (recent_wins, recent_losses) = recent
        .iter()
        .filter(|m| !m.remake && is_laned_mode(m.queue_id))
        .fold((0u32, 0u32), |(wins, losses), m| {
            if m.win {
                (wins + 1, losses)
            } else {
                (wins, losses + 1)
            }
        });

    let (saved_tag, saved_encounter_count) = if !puuid.is_empty() {
        ctx.saved_markers
            .get(&puuid)
            .map(|(t, c)| (t.clone(), *c))
            .unwrap_or((None, 0))
    } else {
        (None, 0)
    };

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
        recent_wins,
        recent_losses,
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
        summoner_level: fetch
            .and_then(|f| f.identity.as_ref())
            .map(|identity| identity.level)
            .unwrap_or(0),
        profile_icon_id: session.profile_icon_id.unwrap_or(0),
        matches: recent.to_vec(),
        top_champions,
        saved_tag,
        saved_encounter_count,
        positions: (stats.positions.total_valid > 0).then_some(stats.positions),
        mastery: fetch.and_then(|f| mastery_badge(&f.masteries, champion_id)),
        data_available: ranked.is_some() || recent_count > 0,
    }
}

/// 复用路人集存储句柄：对局信息每次重建都打开 SQLite 建表开销过大，进程内只打开一次
fn saved_players_storage() -> Option<&'static crate::storage::SavedPlayersStorage> {
    static STORAGE: std::sync::OnceLock<Option<crate::storage::SavedPlayersStorage>> =
        std::sync::OnceLock::new();
    STORAGE
        .get_or_init(|| crate::storage::SavedPlayersStorage::open_default().ok())
        .as_ref()
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
    let saved_markers = saved_players_storage()
        .map(|s| s.get_saved_markers(self_puuid))
        .unwrap_or_default();

    let build_ctx = PlayerBuildContext {
        my_games: &my_recent.games,
        latest_my_game_id: my_recent.latest_game_id,
        self_puuid,
        self_summoner_id,
        assets,
        saved_markers,
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
        is_demo: false,
    }
}

// ─── 实时拉取 ───

/// 拉取指定召唤师的完整排位数据（含单排 / 灵活各队列）
pub async fn fetch_ranked_stats(client: &LcuClient, puuid: &str) -> Result<RankedStats, String> {
    let path = format!("/lol-ranked/v1/ranked-stats/{}", puuid);
    let value = client.get(&path).await?;
    serde_json::from_value(value).map_err(|e| format!("解析排位数据失败: {}", e))
}

/// 批量并发拉取多名玩家的段位（并发 3，对齐旧版战绩查询限流），失败或未定级者跳过。
/// 返回 puuid -> 该玩家最优先队列的段位项。
pub async fn fetch_ranked_stats_batch(
    client: &LcuClient,
    puuids: Vec<String>,
) -> HashMap<String, RankedQueueEntry> {
    stream::iter(puuids)
        .map(|puuid| async move {
            let ranked = client
                .get(&format!("/lol-ranked/v1/ranked-stats/{}", puuid))
                .await
                .ok()?;
            let stats = serde_json::from_value::<RankedStats>(ranked).ok()?;
            Some((puuid, stats))
        })
        .buffer_unordered(3)
        .filter_map(|result| async move {
            result.and_then(|(puuid, stats)| placed_ranked_entry(&stats).map(|e| (puuid, e)))
        })
        .collect::<HashMap<_, _>>()
        .await
}

/// 从段位统计中取最优先队列的段位项；未定级（NONE/UNRANKED）视为无段位。
pub fn placed_ranked_entry(stats: &RankedStats) -> Option<RankedQueueEntry> {
    let entry = stats.preferred_entry()?;
    let tier = entry.tier.trim().to_ascii_uppercase();
    if tier.is_empty() || tier == "NONE" || tier == "UNRANKED" {
        return None;
    }
    Some(entry.clone())
}

/// 拉取单个玩家的数据分两段执行：
/// - `full = false`：只取身份与段位（2 个请求），用于首帧快速出骨架；
/// - `full = true`：再补近期对局与熟练度，已有骨架缓存时直接复用。
async fn fetch_player_data(
    client: &LcuClient,
    puuid: &str,
    assets: &GameDataAssets,
    hint: Option<&IdentityHint>,
    is_self: bool,
    full: bool,
    game_id: u64,
) -> PlayerFetchData {
    if let Some((data, stage)) = cached_player_data(puuid, game_id) {
        if !full || stage == PLAYER_DATA_STAGE_FULL {
            return data;
        }
    }

    let ranked_path = format!("/lol-ranked/v1/ranked-stats/{}", puuid);
    let name_resolved = |identity: &Option<SummonerIdentity>| {
        identity.is_some() || hint.is_some_and(|h| h.fallback_name.is_some())
    };

    if !full {
        let (ranked_resp, identity) = tokio::join!(
            client.get(&ranked_path),
            fetch_summoner_identity(client, puuid, hint)
        );
        let data = PlayerFetchData {
            ranked: parse_ranked_entry(ranked_resp),
            recent: Vec::new(),
            identity,
            masteries: Vec::new(),
        };
        // 名字尚未查到时不写缓存，下次刷新重试
        if name_resolved(&data.identity) {
            store_player_data(puuid, &data, PLAYER_DATA_STAGE_QUICK, game_id);
        }
        return data;
    }

    // 完整拉取：已有骨架缓存时只补战绩与熟练度
    if let Some((base, _)) = cached_player_data(puuid, game_id) {
        let (history_resp, masteries) = tokio::join!(
            fetch_match_history_smart(client, puuid, 0, RECENT_MATCH_LIMIT - 1, assets, false),
            fetch_player_masteries(client, puuid, hint, is_self)
        );
        let mut recent = history_resp.unwrap_or_default();
        recent.truncate(RECENT_MATCH_LIMIT as usize);
        let data = PlayerFetchData {
            recent,
            masteries,
            ..base
        };
        if name_resolved(&data.identity) {
            store_player_data(puuid, &data, PLAYER_DATA_STAGE_FULL, game_id);
        }
        return data;
    }

    let (ranked_resp, history_resp, identity, masteries) = tokio::join!(
        client.get(&ranked_path),
        fetch_match_history_smart(client, puuid, 0, RECENT_MATCH_LIMIT - 1, assets, false),
        fetch_summoner_identity(client, puuid, hint),
        fetch_player_masteries(client, puuid, hint, is_self)
    );

    let mut recent = history_resp.unwrap_or_default();
    // LCU 产品接口实际会比 endIndex 多返回一场，统一裁剪为 20 场
    recent.truncate(RECENT_MATCH_LIMIT as usize);

    let data = PlayerFetchData {
        ranked: parse_ranked_entry(ranked_resp),
        recent,
        identity,
        masteries,
    };
    if name_resolved(&data.identity) {
        store_player_data(puuid, &data, PLAYER_DATA_STAGE_FULL, game_id);
    }
    data
}

/// 解析段位统计响应，取最优先队列的段位项
fn parse_ranked_entry(ranked_resp: Result<serde_json::Value, String>) -> Option<RankedQueueEntry> {
    ranked_resp.ok().and_then(|value| {
        serde_json::from_value::<RankedStats>(value)
            .ok()
            .and_then(|stats| stats.preferred_entry().cloned())
    })
}

/// 缓存中的单名玩家数据（带拉取时刻、所属对局与完成阶段）
struct CachedPlayerData {
    data: PlayerFetchData,
    /// 1 = 仅身份+段位（骨架），2 = 含战绩与熟练度（完整）
    stage: u8,
    fetched_at: std::time::Instant,
    /// 数据所属的真实对局 id（选人阶段拉取时为 0，进入对局后绑定）
    game_id: u64,
}

/// 骨架阶段：身份与段位
const PLAYER_DATA_STAGE_QUICK: u8 = 1;
/// 完整阶段：额外包含近期对局与熟练度
const PLAYER_DATA_STAGE_FULL: u8 = 2;

/// 玩家数据进程内缓存：选人/对局阶段反复刷新时直接复用已拉到的段位与战绩，
/// 只对首次出现的 puuid 发请求（对齐 Vue 版 playerData 复用）
fn cached_player_data_map() -> &'static std::sync::Mutex<HashMap<String, CachedPlayerData>> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<HashMap<String, CachedPlayerData>>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

/// 缓存有效期：仅对「尚未绑定对局」的条目生效；绑定到某一局后一局内一直复用
const PLAYER_DATA_TTL: std::time::Duration = std::time::Duration::from_secs(600);
/// 缓存条目上限，超出后整体清空
const PLAYER_DATA_CACHE_LIMIT: usize = 64;
/// 单名玩家保留的熟练度条目上限（按点数降序裁剪）
const MASTERY_LIST_LIMIT: usize = 200;

/// 清空玩家数据缓存：新一轮对局开始时调用，避免复用上一局拉到的战绩与段位
pub fn clear_player_data_cache() {
    if let Ok(mut cache) = cached_player_data_map().lock() {
        cache.clear();
    }
}

/// 读取缓存条目：返回数据与完成阶段
///
/// 对齐旧版 Vue：同一局（gameId 相同）内的数据全程复用，不按时间过期；
/// 跨局（gameId 变化）立即失效，避免把上一局的战绩带进新局。
fn cached_player_data(puuid: &str, game_id: u64) -> Option<(PlayerFetchData, u8)> {
    let mut cache = cached_player_data_map().lock().ok()?;
    let (same_game, idle_expired, data) = {
        let entry = cache.get(puuid)?;
        // 选人阶段（game_id = 0）拉到的条目，进入真实对局后沿用
        let same_game = entry.game_id == game_id || entry.game_id == 0;
        let idle_expired = entry.game_id == 0 && entry.fetched_at.elapsed() > PLAYER_DATA_TTL;
        (same_game, idle_expired, (entry.data.clone(), entry.stage))
    };
    if !same_game || idle_expired {
        cache.remove(puuid);
        return None;
    }
    // 选人阶段拉到的条目在进入真实对局后绑定该局，之后一局内不再受 TTL 影响
    if game_id != 0 {
        if let Some(entry) = cache.get_mut(puuid) {
            entry.game_id = game_id;
        }
    }
    Some(data)
}

fn store_player_data(puuid: &str, data: &PlayerFetchData, stage: u8, game_id: u64) {
    let Ok(mut cache) = cached_player_data_map().lock() else {
        return;
    };
    // 同局条目常驻；未绑定对局的条目按 TTL 清理
    cache.retain(|_, entry| {
        entry.game_id == game_id || entry.fetched_at.elapsed() <= PLAYER_DATA_TTL
    });
    if cache.len() >= PLAYER_DATA_CACHE_LIMIT {
        cache.clear();
    }
    cache.insert(
        puuid.to_string(),
        CachedPlayerData {
            data: data.clone(),
            stage,
            fetched_at: std::time::Instant::now(),
            game_id,
        },
    );
}

/// 查询真实 Riot ID：puuid → summonerId → 会话名字 三级兜底（对齐 Vue 版 fetchRealPlayerData）。
/// 对局中 gameflow session 的 summonerName/displayName 全为空串，而 /lol-summoner 接口仍返回 gameName/tagLine
async fn fetch_summoner_identity(
    client: &LcuClient,
    puuid: &str,
    hint: Option<&IdentityHint>,
) -> Option<SummonerIdentity> {
    let by_puuid = format!("/lol-summoner/v2/summoners/puuid/{}", puuid);
    if let Ok(value) = client.get(&by_puuid).await {
        if let Some(identity) = summoner_identity_from(&value) {
            return Some(identity);
        }
    }

    let summoner_id = hint.map(|h| h.summoner_id).filter(|&id| id > 0);
    if let Some(summoner_id) = summoner_id {
        let by_id = format!("/lol-summoner/v1/summoners/{}", summoner_id);
        if let Ok(value) = client.get(&by_id).await {
            if let Some(identity) = summoner_identity_from(&value) {
                return Some(identity);
            }
        }
    }

    let name = hint.and_then(|h| h.fallback_name.as_deref())?;
    let encoded = percent_encoding::utf8_percent_encode(name, percent_encoding::NON_ALPHANUMERIC);
    let by_name = format!("/lol-summoner/v1/summoners?name={}", encoded);
    let value = client.get(&by_name).await.ok()?;
    summoner_identity_from(&value)
}

/// 拉取英雄熟练度：puuid → 自己走 local-player → summonerId 三级兜底（对齐 Vue fetchPlayerMastery），
/// 结果按点数降序裁剪，供招牌 / 练英雄判定
async fn fetch_player_masteries(
    client: &LcuClient,
    puuid: &str,
    hint: Option<&IdentityHint>,
    is_self: bool,
) -> Vec<MasteryEntry> {
    let by_puuid = format!("/lol-champion-mastery/v1/{}/champion-mastery", puuid);
    if let Some(list) = fetch_mastery_list(client, &by_puuid).await {
        return list;
    }

    if is_self {
        if let Some(list) = fetch_mastery_list(
            client,
            "/lol-champion-mastery/v1/local-player/champion-mastery",
        )
        .await
        {
            return list;
        }
    }

    let summoner_id = hint.map(|h| h.summoner_id).filter(|&id| id > 0);
    if let Some(summoner_id) = summoner_id {
        let by_id = format!(
            "/lol-champion-mastery/v1/summoners/{}/champion-mastery",
            summoner_id
        );
        if let Some(list) = fetch_mastery_list(client, &by_id).await {
            return list;
        }
    }

    Vec::new()
}

/// 请求并解析单个熟练度接口；返回 None 表示接口不可用（用于触发下一级兜底）
async fn fetch_mastery_list(client: &LcuClient, path: &str) -> Option<Vec<MasteryEntry>> {
    let value = client.get(path).await.ok()?;
    let items = value.as_array()?;
    let mut list: Vec<MasteryEntry> = items
        .iter()
        .filter_map(|item| {
            let champion_id = item
                .get("championId")
                .or_else(|| item.get("champion_id"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;
            if champion_id <= 0 {
                return None;
            }
            Some(MasteryEntry {
                champion_id,
                level: item
                    .get("championLevel")
                    .or_else(|| item.get("masteryLevel"))
                    .or_else(|| item.get("level"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
                points: item
                    .get("championPoints")
                    .or_else(|| item.get("points"))
                    .or_else(|| item.get("score"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
            })
        })
        .collect();
    list.sort_by_key(|entry| std::cmp::Reverse(entry.points));
    list.truncate(MASTERY_LIST_LIMIT);
    Some(list)
}

/// 当前所选英雄的熟练度摘要；未查到熟练度数据时返回 None
fn mastery_badge(masteries: &[MasteryEntry], champion_id: i32) -> Option<MasteryBadge> {
    if masteries.is_empty() || champion_id <= 0 {
        return None;
    }
    let current = masteries.iter().find(|m| m.champion_id == champion_id);
    let level = current.map(|m| m.level).unwrap_or(0);
    let points = current.map(|m| m.points).unwrap_or(0);
    let in_top3 = masteries
        .iter()
        .take(3)
        .any(|m| m.champion_id == champion_id);
    let is_top_champion = (in_top3 && points >= 10_000) || points >= 100_000;
    Some(MasteryBadge {
        level,
        points,
        is_top_champion,
        is_practicing: !is_top_champion && (level <= 1 || points < 5_000),
    })
}

/// 从召唤师信息载荷取真实 Riot ID（腾讯客户端 displayName 常为空，优先 gameName）
fn summoner_identity_from(value: &serde_json::Value) -> Option<SummonerIdentity> {
    let pick = |key: &str| {
        value
            .get(key)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let game_name = pick("gameName").or_else(|| pick("displayName"))?;
    Some(SummonerIdentity {
        game_name,
        tag_line: pick("tagLine").unwrap_or_default(),
        level: value
            .get("summonerLevel")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
    })
}

/// 实时构建当前对局信息：并发拉取 10 人完整数据 + 宿命对局回捞
pub async fn fetch_live_game_info(
    client: &LcuClient,
    self_puuid: &str,
    self_summoner_id: Option<i64>,
    source: GameInfoSource,
    recent_matches: &[MatchDisplay],
    assets: &GameDataAssets,
) -> LiveGameInfo {
    fetch_live_game_info_inner(
        client,
        self_puuid,
        self_summoner_id,
        source,
        recent_matches,
        assets,
        true,
    )
    .await
}

/// 快速构建：只拉身份与段位，用于首帧骨架（战绩/熟练度/宿命随后由完整拉取补齐）
pub async fn fetch_live_game_info_quick(
    client: &LcuClient,
    self_puuid: &str,
    self_summoner_id: Option<i64>,
    source: GameInfoSource,
    recent_matches: &[MatchDisplay],
    assets: &GameDataAssets,
) -> LiveGameInfo {
    fetch_live_game_info_inner(
        client,
        self_puuid,
        self_summoner_id,
        source,
        recent_matches,
        assets,
        false,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn fetch_live_game_info_inner(
    client: &LcuClient,
    self_puuid: &str,
    self_summoner_id: Option<i64>,
    source: GameInfoSource,
    recent_matches: &[MatchDisplay],
    assets: &GameDataAssets,
    full: bool,
) -> LiveGameInfo {
    let puuids = source.puuids();

    // 会话内的身份线索：对局阶段 gameflow session 的 summonerName/displayName 常为空串，
    // 名字统一以 /lol-summoner 接口返回为准（对齐 Vue 版 puuid → summonerId → 名字 三级兜底）
    let hints: HashMap<String, IdentityHint> = source
        .blue
        .iter()
        .chain(source.red.iter())
        .filter_map(|p| {
            let puuid = p.resolved_puuid()?;
            let (name, _) = p.name_parts();
            Some((
                puuid.to_string(),
                IdentityHint {
                    summoner_id: p.resolved_summoner_id().unwrap_or(0),
                    fallback_name: (!name.is_empty()).then_some(name),
                },
            ))
        })
        .collect();

    let targets: Vec<(String, Option<IdentityHint>)> = puuids
        .into_iter()
        .map(|puuid| {
            let hint = hints.get(&puuid).cloned();
            (puuid, hint)
        })
        .collect();

    // 本局真实 gameId：用于把玩家数据缓存绑定到这一局（同一局内全程复用，跨局失效）
    let game_id = source.game_id.unwrap_or(0);
    let fetched: Vec<(String, PlayerFetchData)> = stream::iter(targets)
        .map(|(puuid, hint)| async move {
            let is_self = puuid == self_puuid;
            let data = fetch_player_data(
                client,
                &puuid,
                assets,
                hint.as_ref(),
                is_self,
                full,
                game_id,
            )
            .await;
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

// ─── 对局中预缓存（对局结束记录相遇玩家） ───

/// 对局中缓存的本局玩家（对齐 Vue 版 `CurrentGameCache`）。
///
/// 结算阶段 LCU 载荷里的 `summonerName` 常为空，若等到结算才现拉数据，
/// 名字只能写成占位符；因此在 InProgress 阶段就把 10 人身份解析成真实 Riot ID 缓存，
/// 结算时只消费一次这份缓存（取走即失效），避免把历史对局补记进来。
#[derive(Debug, Clone)]
pub struct CurrentGameCache {
    pub game_id: u64,
    pub queue_id: i32,
    pub players: Vec<CachedGamePlayer>,
}

/// 预缓存中的单名玩家
#[derive(Debug, Clone)]
pub struct CachedGamePlayer {
    pub puuid: String,
    pub summoner_name: String,
    pub tag_line: Option<String>,
    pub profile_icon_id: i32,
    pub champion_id: i32,
}

/// 拉取当前对局 10 人并解析真实召唤师名。
///
/// 返回 `None` 表示会话不可用、无对局号，或属于不录入路人集的云顶队列。
pub async fn fetch_current_game_cache(client: &LcuClient) -> Option<CurrentGameCache> {
    let value = client.get("/lol-gameflow/v1/session").await.ok()?;
    let snapshot = serde_json::from_value::<GameflowSessionSnapshot>(value).ok()?;
    let game_data = snapshot.game_data.as_ref()?;
    let game_id = game_data.game_id?;
    let queue_id = game_data.queue.as_ref().and_then(|q| q.id).unwrap_or(0);
    // 云顶对局不录入路人集（与 Vue 版一致）
    if matches!(queue_id, 1090 | 1100 | 1130 | 1160) {
        return None;
    }

    // (summonerId, puuid, 会话内名称兜底, 英雄 ID)
    let mut targets: Vec<(i64, String, String, i32)> = Vec::new();
    for player in game_data.team_one.iter().chain(game_data.team_two.iter()) {
        let Some(summoner_id) = player.resolved_summoner_id() else {
            continue;
        };
        targets.push((
            summoner_id,
            player.resolved_puuid().unwrap_or_default().to_string(),
            player.summoner_name.clone().unwrap_or_default(),
            player.resolved_champion_id(),
        ));
    }

    let players: Vec<CachedGamePlayer> = stream::iter(targets)
        .map(|(summoner_id, session_puuid, fallback_name, champion_id)| {
            let client = client.clone();
            async move {
                let info = fetch_cache_summoner(&client, summoner_id, &session_puuid).await;
                let field = |key: &str| {
                    info.as_ref()
                        .and_then(|v| v.get(key))
                        .and_then(|v| v.as_str())
                };
                let puuid = field("puuid")
                    .filter(|s| !s.trim().is_empty())
                    .map(str::to_string)
                    .unwrap_or(session_puuid);
                CachedGamePlayer {
                    puuid,
                    // 国服 Riot ID 体系下 displayName 常为空串，先过滤再取 gameName
                    summoner_name: preferred_summoner_name(
                        field("displayName"),
                        field("gameName"),
                        &fallback_name,
                    ),
                    tag_line: field("tagLine")
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string),
                    profile_icon_id: info
                        .as_ref()
                        .and_then(|v| v.get("profileIconId"))
                        .and_then(|v| v.as_i64())
                        .filter(|n| *n > 0)
                        .unwrap_or(0) as i32,
                    champion_id,
                }
            }
        })
        .buffer_unordered(10)
        .collect()
        .await;

    if players.is_empty() {
        return None;
    }
    Some(CurrentGameCache {
        game_id,
        queue_id,
        players,
    })
}

/// 按 displayName → gameName → 会话内名称 的顺序取第一个非空名字
fn preferred_summoner_name(display: Option<&str>, game: Option<&str>, fallback: &str) -> String {
    for name in [display, game].into_iter().flatten() {
        let name = name.trim();
        if !name.is_empty() {
            return name.to_string();
        }
    }
    fallback.trim().to_string()
}

/// 优先按 summonerId 查询完整召唤师信息，失败再按 puuid 兜底
async fn fetch_cache_summoner(
    client: &LcuClient,
    summoner_id: i64,
    puuid: &str,
) -> Option<serde_json::Value> {
    if summoner_id > 0 {
        let path = format!("/lol-summoner/v1/summoners/{}", summoner_id);
        if let Ok(value) = client.get(&path).await {
            if value.is_object() {
                return Some(value);
            }
        }
    }
    if !puuid.is_empty() {
        let path = format!("/lol-summoner/v2/summoners/puuid/{}", puuid);
        if let Ok(value) = client.get(&path).await {
            if value.is_object() {
                return Some(value);
            }
        }
    }
    None
}

// ─── 2999 端口 LiveClientData 兜底 ───

/// 2999 `liveclientdata/playerlist` 中的单个玩家（宽容反序列化，逐条跳过异常条目）
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveClientPlayer {
    #[serde(default)]
    pub champion_id: Option<i32>,
    #[serde(default)]
    pub champion_name: Option<String>,
    #[serde(default)]
    pub raw_champion_name: Option<String>,
    #[serde(default)]
    pub summoner_name: Option<String>,
    #[serde(default)]
    pub riot_id_game_name: Option<String>,
    #[serde(default)]
    pub riot_id_tag_line: Option<String>,
    #[serde(default)]
    pub riot_id: Option<String>,
    #[serde(default)]
    pub puuid: Option<String>,
    #[serde(default)]
    pub summoner_id: Option<i64>,
    #[serde(default)]
    pub bot: Option<bool>,
    #[serde(default)]
    pub is_bot: Option<bool>,
    #[serde(default)]
    pub team: Option<String>,
}

/// 一次 2999 快照：playerlist + 本机当前玩家名（阵营判定与队伍补全共用同一次拉取）
#[derive(Debug, Clone, Default)]
pub struct LiveClientSnapshot {
    pub players: Vec<LiveClientPlayer>,
    pub active_name: Option<String>,
}

/// 2999 LiveClientData 专用 HTTP 客户端（忽略自签证书，进程内复用）
fn liveclient_http() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .no_proxy()
            .timeout(std::time::Duration::from_millis(1500))
            .build()
            .unwrap_or_default()
    })
}

/// 拉取一次 2999 LiveClientData 快照（playerlist 失败即整体失败；activeplayername 失败仅缺阵营判定输入）
pub async fn fetch_liveclient_snapshot() -> Option<LiveClientSnapshot> {
    let client = liveclient_http();
    let active_name = match client
        .get("https://127.0.0.1:2999/liveclientdata/activeplayername")
        .send()
        .await
    {
        Ok(resp) => resp.json::<String>().await.ok(),
        Err(_) => None,
    };
    let raw_list = client
        .get("https://127.0.0.1:2999/liveclientdata/playerlist")
        .send()
        .await
        .ok()?
        .json::<Vec<serde_json::Value>>()
        .await
        .ok()?;

    let players = raw_list
        .iter()
        .filter_map(|entry| serde_json::from_value::<LiveClientPlayer>(entry.clone()).ok())
        .collect();

    Some(LiveClientSnapshot {
        players,
        active_name: active_name
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty()),
    })
}

/// LiveClient 队伍标识 → 阵营（ORDER -> 蓝方, CHAOS -> 红方）
fn map_side_from_liveclient_team(team: &str) -> Option<MapSide> {
    if team.eq_ignore_ascii_case("ORDER") {
        Some(MapSide::Blue)
    } else if team.eq_ignore_ascii_case("CHAOS") {
        Some(MapSide::Red)
    } else {
        None
    }
}

/// LiveClient 玩家可用于与 activeplayername 匹配的三个名字字段
fn liveclient_player_names(p: &LiveClientPlayer) -> [Option<&str>; 3] {
    [
        p.riot_id_game_name.as_deref(),
        p.summoner_name.as_deref(),
        p.riot_id.as_deref(),
    ]
}

/// 从 2999 快照判定本机玩家阵营（ORDER -> 蓝方, CHAOS -> 红方）
pub fn liveclient_map_side(snapshot: &LiveClientSnapshot) -> Option<MapSide> {
    let active = snapshot
        .active_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())?;
    let matched_teams: Vec<&str> = snapshot
        .players
        .iter()
        .filter(|p| {
            liveclient_player_names(p)
                .into_iter()
                .flatten()
                .any(|n| n == active)
        })
        .filter_map(|p| p.team.as_deref())
        .collect();
    let first = *matched_teams.first()?;
    if matched_teams.iter().all(|&t| t == first) {
        map_side_from_liveclient_team(first)
    } else {
        None
    }
}

/// 解析 LiveClient 玩家的英雄 ID：championId 优先，其次按本地化英雄名反查资源字典
fn liveclient_champion_id(p: &LiveClientPlayer, assets: &GameDataAssets) -> i32 {
    if let Some(cid) = p.champion_id.filter(|&c| c > 0) {
        return cid;
    }
    if let Some(name) = p
        .champion_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let norm = normalize_lookup_name(name);
        if let Some((cid, _)) = assets
            .champions
            .iter()
            .find(|(_, cn)| normalize_lookup_name(cn) == norm)
        {
            return *cid;
        }
    }
    // rawChampionName（game_character_displayname_Xxx）为英文内部键，资源字典无对应项，放弃匹配
    0
}

/// 把 2999 玩家列表按 ORDER/CHAOS 映射为 (蓝方, 红方) 会话参与者；阵营未知条目丢弃以防串队
pub fn liveclient_teams(
    snapshot: &LiveClientSnapshot,
    assets: &GameDataAssets,
) -> (Vec<SessionPlayer>, Vec<SessionPlayer>) {
    let mut blue = Vec::new();
    let mut red = Vec::new();
    for p in &snapshot.players {
        let champion_id = liveclient_champion_id(p, assets);
        let session = SessionPlayer {
            champion_id: Some(champion_id),
            puuid: p.puuid.clone(),
            summoner_id: p.summoner_id,
            display_name: p
                .riot_id_game_name
                .clone()
                .or_else(|| p.summoner_name.clone()),
            summoner_name: p.summoner_name.clone(),
            game_name: p.riot_id_game_name.clone(),
            tag_line: p.riot_id_tag_line.clone(),
            bot: p.bot,
            is_bot: p.is_bot,
            ..Default::default()
        };
        if session.resolved_puuid().is_none()
            && session.resolved_summoner_id().is_none()
            && champion_id <= 0
            && session.resolved_name_candidates().is_empty()
        {
            continue;
        }
        match p.team.as_deref().and_then(map_side_from_liveclient_team) {
            Some(MapSide::Blue) => blue.push(session),
            Some(MapSide::Red) => red.push(session),
            None => {}
        }
    }
    (blue, red)
}

/// 归一化反查名：去 #tag、剔除空白并转小写（对齐 Vue normalizeLookupName）
pub fn normalize_lookup_name(name: &str) -> String {
    name.split('#')
        .next()
        .unwrap_or(name)
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// 占位名检测：空白、"未知"、"召唤师"、"玩家 N"
fn is_placeholder_name(name: &str) -> bool {
    let trimmed = name.trim();
    trimmed.is_empty() || trimmed == "未知" || trimmed == "召唤师" || trimmed.starts_with("玩家")
}

/// 是否为真实召唤师名：非占位，且不是任何英雄显示名（防脱敏英雄名误撞，对齐 Vue isRealSummonerName）
fn is_real_summoner_name(name: &str, assets: &GameDataAssets) -> bool {
    let trimmed = name.trim();
    if is_placeholder_name(trimmed) {
        return false;
    }
    let norm = normalize_lookup_name(trimmed);
    if norm.is_empty() {
        return false;
    }
    !assets
        .champions
        .values()
        .any(|cn| normalize_lookup_name(cn) == norm)
}

impl SessionPlayer {
    /// 可用于实名匹配的候选名字（display_name / summoner_name / game_name）
    fn resolved_name_candidates(&self) -> Vec<&str> {
        [
            self.display_name.as_deref(),
            self.summoner_name.as_deref(),
            self.game_name.as_deref(),
        ]
        .into_iter()
        .flatten()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
    }
}

/// 强身份匹配：puuid 相等，或真实 summonerId 相等（summonerId 充当 cellId 兜底时不参与比对）
fn strong_identity_matches(p: &SessionPlayer, lp: &SessionPlayer) -> bool {
    if let (Some(pp), Some(lp_p)) = (p.resolved_puuid(), lp.resolved_puuid()) {
        if pp == lp_p {
            return true;
        }
    }
    let sid = p
        .resolved_summoner_id()
        .filter(|&sid| p.cell_id.is_none_or(|cell| sid != cell));
    let lp_sid = lp
        .resolved_summoner_id()
        .filter(|&sid| lp.cell_id.is_none_or(|cell| sid != cell));
    sid.is_some() && sid == lp_sid
}

/// 实名归一化名（仅当候选名中存在真实召唤师名时返回）
fn real_normalized_name(p: &SessionPlayer, assets: &GameDataAssets) -> Option<String> {
    p.resolved_name_candidates()
        .into_iter()
        .find(|name| is_real_summoner_name(name, assets))
        .map(normalize_lookup_name)
}

/// 无实名身份（puuid 为空且 summonerId 缺失或充当 cellId 兜底）
fn is_anonymous_player(p: &SessionPlayer) -> bool {
    p.resolved_puuid().is_none()
        && p.resolved_summoner_id()
            .is_none_or(|sid| p.cell_id.is_some_and(|cell| sid == cell))
}

/// 把 LiveClient 玩家数据回填进既有槽位（仅在缺失侧回填，实名与身份严禁被占位数据覆写）
fn absorb_live_player(
    player: &mut SessionPlayer,
    lp: &SessionPlayer,
    assets: &GameDataAssets,
) -> bool {
    let mut changed = false;
    if player.resolved_champion_id() <= 0 && lp.resolved_champion_id() > 0 {
        player.champion_id = lp.champion_id;
        changed = true;
    }
    if player.resolved_puuid().is_none() && lp.resolved_puuid().is_some() {
        player.puuid = lp.puuid.clone();
        changed = true;
    }
    if player.resolved_summoner_id().is_none() && lp.resolved_summoner_id().is_some() {
        player.summoner_id = lp.summoner_id;
        changed = true;
    }
    if player.tag_line.is_none() {
        player.tag_line = lp.tag_line.clone();
    }
    // 名字仅在「本地为占位 + LiveClient 名字真实」时回填，脱敏英雄名/占位名严禁覆写实名
    let lp_real_name = lp
        .resolved_name_candidates()
        .into_iter()
        .find(|name| is_real_summoner_name(name, assets))
        .map(str::to_string);
    if let Some(real_name) = lp_real_name {
        let current = player
            .display_name
            .as_deref()
            .or(player.summoner_name.as_deref())
            .unwrap_or("");
        if is_placeholder_name(current) {
            player.display_name = Some(real_name.clone());
            if player.game_name.is_none() {
                player.game_name = Some(real_name);
            }
            changed = true;
        }
    }
    changed
}

/// 单支队伍的 2999 合并：
/// 1) 强身份匹配（puuid → summonerId）；2) 双实名名字匹配；3) 匿名占位槽按剩余顺序回填；
/// 4) 人数不足 5 时追加剩余 LiveClient 玩家（整边缺失的敌方队伍由此补齐）
fn merge_liveclient_team(
    team: &mut Vec<SessionPlayer>,
    live: Vec<SessionPlayer>,
    assets: &GameDataAssets,
) -> bool {
    let mut changed = false;
    let mut pool: Vec<Option<SessionPlayer>> = live.into_iter().map(Some).collect();

    // 1) 强身份匹配
    for player in team.iter_mut() {
        let Some(match_idx) = pool.iter().position(|slot| {
            slot.as_ref()
                .is_some_and(|lp| strong_identity_matches(player, lp))
        }) else {
            continue;
        };
        let Some(lp) = pool[match_idx].take() else {
            continue;
        };
        changed |= absorb_live_player(player, &lp, assets);
    }

    // 2) 真名匹配（双方都必须是真实召唤师名，脱敏英雄名/占位名严禁参与）
    for player in team.iter_mut() {
        let Some(p_name) = real_normalized_name(player, assets) else {
            continue;
        };
        let Some(match_idx) = pool.iter().position(|slot| {
            slot.as_ref()
                .is_some_and(|lp| real_normalized_name(lp, assets).is_some_and(|n| n == p_name))
        }) else {
            continue;
        };
        let Some(lp) = pool[match_idx].take() else {
            continue;
        };
        changed |= absorb_live_player(player, &lp, assets);
    }

    // 3) 匿名占位槽按剩余顺序回填（对齐 Vue 敌方有序回填策略，匿名槽严禁误撞实名数据）
    for player in team.iter_mut() {
        if !is_anonymous_player(player) {
            continue;
        }
        let Some(match_idx) = pool.iter().position(|slot| slot.is_some()) else {
            break;
        };
        let Some(lp) = pool[match_idx].take() else {
            break;
        };
        changed |= absorb_live_player(player, &lp, assets);
    }

    // 4) 追加未匹配到的 LiveClient 玩家（整边缺失 / 人数不足 5）
    for lp in pool.into_iter().flatten() {
        if team.len() >= 5 {
            break;
        }
        team.push(lp);
        changed = true;
    }

    changed
}

/// 原因 D：把 2999 LiveClientData 玩家列表合并进 gameflow 双方队伍，
/// 补全缺失的玩家（整边为空）与 `champion_id <= 0` 的英雄 ID。返回是否发生修改。
pub fn merge_liveclient_teams(
    blue: &mut Vec<SessionPlayer>,
    red: &mut Vec<SessionPlayer>,
    live_blue: Vec<SessionPlayer>,
    live_red: Vec<SessionPlayer>,
    assets: &GameDataAssets,
) -> bool {
    let mut changed = merge_liveclient_team(blue, live_blue, assets);
    changed |= merge_liveclient_team(red, live_red, assets);
    changed
}

/// 尝试从游戏引擎 LiveClientData (2999 端口) 探测当前玩家阵营（ORDER -> 蓝方, CHAOS -> 红方）
pub async fn detect_liveclient_map_side() -> Option<MapSide> {
    let snapshot = fetch_liveclient_snapshot().await?;
    liveclient_map_side(&snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameflow::match_stats::PositionKey;

    const CHAMP_SELECT_FIXTURE: &str =
        include_str!("../../testdata/session/champ-select-ranked-420.json");
    const GAMEFLOW_FIXTURE: &str =
        include_str!("../../testdata/session/gameflow-in-progress-440.json");
    const RANKED_FIXTURE: &str = include_str!("../../testdata/session/ranked-stats-solo.json");

    const SELF_PUUID: &str = "puuid-self-top";
    const SELF_SUMMONER_ID: i64 = 1001;

    fn champ_select_source() -> GameInfoSource {
        let snapshot: ChampSelectSnapshot =
            serde_json::from_str(CHAMP_SELECT_FIXTURE).expect("选人快照应能反序列化");
        GameInfoSource::from_champ_select(&snapshot, None).expect("选人快照应包含双方玩家")
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
    fn shrunken_gameflow_team_recovers_hidden_player_from_champ_select() {
        // 选人阶段：我方含一名隐藏战绩玩家（无 puuid / summonerId，仅所选英雄可见）
        let mut champ_src = champ_select_source();
        let hidden = champ_src.blue.last_mut().expect("我方应有玩家");
        hidden.puuid = None;
        hidden.summoner_id = None;

        let champ_info = build_live_game_info(
            &champ_src,
            SELF_PUUID,
            Some(SELF_SUMMONER_ID),
            &MyRecentContext::default(),
            &HashMap::new(),
            &GameDataAssets::default(),
        );
        assert_eq!(champ_info.blue_team.len(), 5);
        let hidden_cid = champ_info.blue_team[4].champion_id;
        assert!(champ_info.blue_team[4].puuid.is_empty());

        // 进入对局：LCU 把隐藏战绩玩家从队伍名单剔除，其余 4 人身份齐全
        let mut game_src = champ_src;
        game_src.phase = "InProgress".to_string();
        game_src.game_id = Some(7012345678);
        game_src.blue.pop();

        let mut game_info = build_live_game_info(
            &game_src,
            SELF_PUUID,
            Some(SELF_SUMMONER_ID),
            &MyRecentContext::default(),
            &HashMap::new(),
            &GameDataAssets::default(),
        );
        assert_eq!(game_info.blue_team.len(), 4);

        // 从选人快照补回该玩家，保留所选英雄
        let added = merge_missing_players_from_prev(&mut game_info, &champ_info);
        assert_eq!(added, (1, 0));
        assert_eq!(game_info.blue_team.len(), 5);
        let recovered = &game_info.blue_team[4];
        assert!(recovered.puuid.is_empty());
        assert_eq!(recovered.champion_id, hidden_cid);
        // 部分补人时旧组队编号已失效，不应与新队伍分组误配
        assert_eq!(recovered.premade_group_id, None);
    }

    #[test]
    fn mid_game_shrunken_session_recovers_full_side() {
        // 同局中途 session 残缺（单边被清空）时整边恢复，组队编号保留
        let full_src = gameflow_source();
        let prev = build_live_game_info(
            &full_src,
            SELF_PUUID,
            Some(SELF_SUMMONER_ID),
            &MyRecentContext::default(),
            &HashMap::new(),
            &GameDataAssets::default(),
        );

        let mut shrunk_src = full_src.clone();
        shrunk_src.blue.clear();
        let mut next = build_live_game_info(
            &shrunk_src,
            SELF_PUUID,
            Some(SELF_SUMMONER_ID),
            &MyRecentContext::default(),
            &HashMap::new(),
            &GameDataAssets::default(),
        );
        let premade_before = prev.blue_team.iter().find_map(|p| p.premade_group_id);
        let added = merge_missing_players_from_prev(&mut next, &prev);
        assert_eq!(added, (5, 0));
        assert_eq!(next.blue_team.len(), 5);
        assert_eq!(
            next.blue_team.iter().find_map(|p| p.premade_group_id),
            premade_before
        );
    }

    #[test]
    fn merge_skips_across_different_games_and_demo() {
        let prev = build_live_game_info(
            &gameflow_source(),
            SELF_PUUID,
            Some(SELF_SUMMONER_ID),
            &MyRecentContext::default(),
            &HashMap::new(),
            &GameDataAssets::default(),
        );

        // 跨局：gameId 不同不补
        let mut next_src = gameflow_source();
        next_src.game_id = Some(999);
        next_src.blue.pop();
        let mut next = build_live_game_info(
            &next_src,
            SELF_PUUID,
            Some(SELF_SUMMONER_ID),
            &MyRecentContext::default(),
            &HashMap::new(),
            &GameDataAssets::default(),
        );
        assert_eq!(merge_missing_players_from_prev(&mut next, &prev), (0, 0));
        assert_eq!(next.blue_team.len(), 4);

        // 演示数据：即使 gameId 碰巧相同也不得混入真实对局
        let demo = LiveGameInfo::create_demo();
        next.game_id = demo.game_id;
        assert_eq!(merge_missing_players_from_prev(&mut next, &demo), (0, 0));
        assert_eq!(next.blue_team.len(), 4);
    }

    #[test]
    fn empty_session_names_fall_back_to_fetched_identity() {
        // 对局阶段 gameflow session 的 summonerName/displayName 全为空串（腾讯客户端实测），
        // 名字必须由 /lol-summoner/v2/summoners/puuid 补查结果兜底
        let mut source = gameflow_source();
        for player in source.blue.iter_mut().chain(source.red.iter_mut()) {
            player.display_name = None;
            player.summoner_name = None;
            player.game_name = None;
            player.tag_line = None;
        }

        let mut fetch = HashMap::new();
        for player in source.blue.iter().chain(source.red.iter()) {
            let Some(puuid) = player.resolved_puuid() else {
                continue;
            };
            fetch.insert(
                puuid.to_string(),
                PlayerFetchData {
                    identity: Some(SummonerIdentity {
                        game_name: format!("name-{puuid}"),
                        tag_line: "1234".to_string(),
                        level: 0,
                    }),
                    ..Default::default()
                },
            );
        }

        let info = build_live_game_info(
            &source,
            SELF_PUUID,
            Some(SELF_SUMMONER_ID),
            &MyRecentContext::default(),
            &fetch,
            &GameDataAssets::default(),
        );

        for player in info.blue_team.iter().chain(info.red_team.iter()) {
            if player.puuid.is_empty() {
                assert_eq!(player.summoner_name, "未知玩家");
            } else {
                assert_eq!(player.summoner_name, format!("name-{}", player.puuid));
                assert_eq!(player.tag_line, "1234");
            }
        }
    }

    #[test]
    fn fetched_identity_overrides_session_name() {
        // 会话字段可能被脱敏或只给了旧名，名字统一以召唤师接口返回为准（Vue 版同名逻辑）
        let source = gameflow_source();
        let target = source
            .blue
            .iter()
            .chain(source.red.iter())
            .find_map(|p| p.resolved_puuid().map(str::to_string))
            .expect("fixture 应至少有一名真实玩家");

        let mut fetch = HashMap::new();
        fetch.insert(
            target.clone(),
            PlayerFetchData {
                identity: Some(SummonerIdentity {
                    game_name: "接口名".to_string(),
                    tag_line: "9999".to_string(),
                    level: 0,
                }),
                ..Default::default()
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
            .players()
            .find(|p| p.puuid == target)
            .expect("应能找到目标玩家");
        assert_eq!(player.summoner_name, "接口名");
        assert_eq!(player.tag_line, "9999");
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
    fn cache_overwrite_guard_keeps_fuller_snapshot() {
        let mut full = LiveGameInfo::create_demo();
        full.is_demo = false;
        full.game_id = Some(123456);

        // 没有旧快照：允许写入
        assert!(should_overwrite_cache(None, &full));

        // 同局更瘦的快照（InProgress 精简 session）：禁止覆盖
        let mut thin = full.clone();
        thin.blue_team.truncate(1);
        thin.red_team.clear();
        assert!(!should_overwrite_cache(Some(&full), &thin));

        // 同局同样完整或更全：允许覆盖
        assert!(should_overwrite_cache(Some(&full), &full));

        // 换了一局：允许覆盖
        let mut other = thin.clone();
        other.game_id = Some(999999);
        assert!(should_overwrite_cache(Some(&full), &other));
    }

    #[test]
    fn rank_display_handles_unranked_and_apex_tiers() {
        assert_eq!(rank_display("", "", 0), "未定级");
        assert_eq!(rank_display("UNRANKED", "", 0), "未定级");
        assert_eq!(rank_display("MASTER", "I", 320), "大师 · 320 LP");
        assert_eq!(rank_display("CHALLENGER", "I", 800), "王者 · 800 LP");

        // 界面据 is_ranked_tier 决定是否绘制段位文案：未定级一律不画
        assert!(!is_ranked_tier(""));
        assert!(!is_ranked_tier("NONE"));
        assert!(!is_ranked_tier("UNRANKED"));
        assert!(!is_ranked_tier("--"));
        assert!(!is_ranked_tier("未定级"));
        assert!(is_ranked_tier("DIAMOND"));
        // GameInfoPlayer.rank_tier 存中文名，必须同样判定为已定级
        assert!(is_ranked_tier("钻石"));
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
                identity: None,
                masteries: Vec::new(),
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
    fn built_player_exposes_position_distribution() {
        let source = champ_select_source();
        let mut fetch = HashMap::new();
        fetch.insert(
            "puuid-ally-mid".to_string(),
            PlayerFetchData {
                recent: vec![
                    sample_match(1, true, 5, 2, 6),
                    sample_match(2, false, 3, 5, 4),
                    sample_match(3, true, 8, 1, 9),
                ],
                ..Default::default()
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
        let positions = player
            .positions
            .as_ref()
            .expect("三场带分路的对局应产出位置分布");
        assert_eq!(positions.total_valid, 3);
        assert_eq!(positions.primary_position, Some(PositionKey::Top));
        assert_eq!(positions.primary_percentage, 100);
    }

    #[test]
    fn mastery_badge_marks_top_and_practicing_champions() {
        let masteries = vec![
            MasteryEntry {
                champion_id: 266,
                level: 12,
                points: 320_000,
            },
            MasteryEntry {
                champion_id: 103,
                level: 7,
                points: 42_000,
            },
            MasteryEntry {
                champion_id: 22,
                level: 5,
                points: 12_000,
            },
        ];

        let top = mastery_badge(&masteries, 266).expect("应有熟练度摘要");
        assert!(top.is_top_champion);
        assert!(!top.is_practicing);
        assert_eq!(top.points, 320_000);

        // 未出现在熟练度列表中的英雄 → 0 级 0 点，判定为练英雄
        let practicing = mastery_badge(&masteries, 5).expect("应有熟练度摘要");
        assert!(!practicing.is_top_champion);
        assert!(practicing.is_practicing);
        assert_eq!(practicing.level, 0);

        assert!(mastery_badge(&[], 266).is_none());
        assert!(mastery_badge(&masteries, 0).is_none());
    }

    #[test]
    fn local_player_has_no_fate_badge() {
        let source = champ_select_source();
        let shared_game = 9001u64;
        let mut fetch = HashMap::new();
        for puuid in source.puuids() {
            fetch.insert(
                puuid,
                PlayerFetchData {
                    ranked: None,
                    recent: vec![sample_match(shared_game, true, 1, 2, 3)],
                    identity: None,
                    masteries: Vec::new(),
                },
            );
        }
        // 本人自身也在那局游戏的队友名单里：修复前会命中「友」宿命
        let my_games = vec![MyRecentGame {
            game_id: shared_game,
            game_creation: 0,
            ally_puuids: HashSet::from([SELF_PUUID.to_string()]),
            enemy_puuids: HashSet::new(),
            champions: HashMap::new(),
        }];
        let info = build_live_game_info(
            &source,
            SELF_PUUID,
            Some(SELF_SUMMONER_ID),
            &MyRecentContext {
                latest_game_id: Some(shared_game),
                games: my_games,
            },
            &fetch,
            &GameDataAssets::default(),
        );

        let me = info
            .players()
            .find(|p| p.is_local_player)
            .expect("应识别出本人");
        assert!(me.fate.is_none(), "本人不应带宿命标记（否则会显示「友」）");
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

    #[test]
    fn create_demo_creates_valid_state() {
        let demo = LiveGameInfo::create_demo();
        assert!(demo.is_demo);
        assert_eq!(demo.blue_team.len(), 5);
        assert_eq!(demo.red_team.len(), 5);
        assert_eq!(demo.queue_id, 420);
        assert_eq!(demo.queue_name, "单双排位");
        assert_eq!(demo.phase_label, "对局进行中");

        let local = demo.players().find(|p| p.is_local_player);
        assert!(local.is_some());
        let local_player = local.unwrap();
        assert_eq!(local_player.summoner_name, "疾风剑豪");
        assert_eq!(local_player.champion_name, "疾风剑豪");
        assert_eq!(local_player.streak_text.as_deref(), Some("3连胜"));

        let fate_players: Vec<_> = demo.players().filter(|p| p.fate.is_some()).collect();
        assert_eq!(fate_players.len(), 2, "应包含一名队友宿命和一名对手宿命");
    }

    #[test]
    fn placed_ranked_entry_prefers_solo_and_rejects_unranked() {
        let placed: RankedStats =
            serde_json::from_str(RANKED_FIXTURE).expect("段位 fixture 应能反序列化");
        let entry = placed_ranked_entry(&placed).expect("单双排钻石应有效");
        assert_eq!(entry.tier, "DIAMOND");
        assert_eq!(entry.division, "I");
        assert_eq!(rank_tier_label(&entry.tier, &entry.division), "钻石 I");

        // 仅灵活组排时回落到 flex
        let flex_only: RankedStats = serde_json::from_value(serde_json::json!({
            "queueMap": { "RANKED_FLEX_SR": { "queueType": "RANKED_FLEX_SR", "tier": "GOLD", "division": "II" } }
        }))
        .unwrap();
        assert_eq!(placed_ranked_entry(&flex_only).unwrap().tier, "GOLD");

        // NONE / UNRANKED / 空均视为未定级
        for tier in ["NONE", "UNRANKED", ""] {
            let unranked: RankedStats = serde_json::from_value(serde_json::json!({
                "queueMap": { "RANKED_SOLO_5x5": { "queueType": "RANKED_SOLO_5x5", "tier": tier, "division": "I" } }
            }))
            .unwrap();
            assert!(
                placed_ranked_entry(&unranked).is_none(),
                "tier={:?} 应视为未定级",
                tier
            );
        }
        assert!(placed_ranked_entry(&RankedStats::default()).is_none());
    }

    #[test]
    fn champ_select_red_side_via_pin_drop_or_cell_id() {
        let mut snapshot: ChampSelectSnapshot =
            serde_json::from_str(CHAMP_SELECT_FIXTURE).expect("选人快照应能反序列化");
        // ARAM 等场景下 myTeam cellId 可能为 0..4，但 pin-drop 告知为红色方
        let source_pin = GameInfoSource::from_champ_select(&snapshot, Some(MapSide::Red))
            .expect("选人快照应有效");
        let info_pin = build_live_game_info(
            &source_pin,
            SELF_PUUID,
            Some(SELF_SUMMONER_ID),
            &MyRecentContext::default(),
            &HashMap::new(),
            &GameDataAssets::default(),
        );
        assert_eq!(info_pin.user_side, Some(MapSide::Red));
        assert!(info_pin.red_team.iter().any(|p| p.puuid == SELF_PUUID));
        assert!(info_pin
            .blue_team
            .iter()
            .any(|p| p.puuid == "puuid-enemy-top"));

        // 无 pin-drop 时依据 localPlayerCellId >= 5 判定为红方
        snapshot.local_player_cell_id = Some(6);
        let source_cell =
            GameInfoSource::from_champ_select(&snapshot, None).expect("选人快照应有效");
        let info_cell = build_live_game_info(
            &source_cell,
            SELF_PUUID,
            Some(SELF_SUMMONER_ID),
            &MyRecentContext::default(),
            &HashMap::new(),
            &GameDataAssets::default(),
        );
        assert_eq!(info_cell.user_side, Some(MapSide::Red));
        assert!(info_cell.red_team.iter().any(|p| p.puuid == SELF_PUUID));
    }

    #[test]
    fn gameflow_snapshot_accepts_numeric_tpid_and_null_teams() {
        let payload = serde_json::json!({
            "gameData": {
                "gameId": 123456789,
                "queue": { "id": 450, "gameMode": "ARAM", "isCustom": false },
                "teamOne": [
                    {
                        "cellId": 0,
                        "championId": 1,
                        "summonerId": 1001,
                        "puuid": "puuid-self-top",
                        "displayName": "悠米本米#GPUI",
                        "teamParticipantId": 0,
                        "partyId": 987654
                    }
                ],
                "teamTwo": null
            }
        });
        let snapshot: GameflowSessionSnapshot = serde_json::from_value(payload)
            .expect("数字 teamParticipantId/partyId 与 null teamTwo 应能反序列化");
        let gd = snapshot.game_data.expect("gameData 应存在");
        assert_eq!(gd.team_one.len(), 1);
        assert_eq!(gd.team_one[0].team_participant_id.as_deref(), Some("0"));
        assert_eq!(gd.team_one[0].party_id.as_deref(), Some("987654"));
        assert!(gd.team_two.is_empty());
    }

    const GAMEFLOW_MISSING_CHAMPS_FIXTURE: &str =
        include_str!("../../testdata/session/gameflow-in-progress-450-missing-champions.json");

    #[test]
    fn gameflow_fixture_backfills_champion_from_selections() {
        let snapshot: GameflowSessionSnapshot =
            serde_json::from_str(GAMEFLOW_MISSING_CHAMPS_FIXTURE).expect("fixture 应能反序列化");
        let source =
            GameInfoSource::from_gameflow("InProgress", &snapshot).expect("fixture 应有队伍");
        // 原因 C：championId <= 0 的成员按 puuid / summonerId 从 playerChampionSelections 回填
        assert_eq!(
            source.blue[0].champion_id,
            Some(266),
            "应按 puuid 回填本人英雄"
        );
        assert_eq!(
            source.blue[1].champion_id,
            Some(64),
            "puuid 为空时应按 summonerId 回填"
        );
        assert_eq!(source.blue[2].champion_id, Some(0), "无选择记录时保持原值");
        assert_eq!(source.red[0].champion_id, Some(498));
        assert_eq!(source.red[1].champion_id, Some(0), "无选择记录时保持原值");
    }

    fn session_player(
        puuid: &str,
        summoner_id: i64,
        champion_id: i32,
        display_name: &str,
    ) -> SessionPlayer {
        SessionPlayer {
            puuid: (!puuid.is_empty()).then(|| puuid.to_string()),
            summoner_id: (summoner_id > 0).then_some(summoner_id),
            champion_id: (champion_id > 0).then_some(champion_id),
            display_name: (!display_name.is_empty()).then(|| display_name.to_string()),
            ..Default::default()
        }
    }

    fn merge_assets() -> GameDataAssets {
        GameDataAssets {
            champions: HashMap::from([(54, "亚托克斯".to_string()), (103, "九尾妖狐".to_string())]),
            ..Default::default()
        }
    }

    #[test]
    fn merge_liveclient_teams_appends_missing_enemy_side() {
        let mut blue: Vec<SessionPlayer> = (0..5)
            .map(|i| {
                session_player(
                    &format!("puuid-b{i}"),
                    100 + i as i64,
                    266 + i,
                    &format!("友方{i}"),
                )
            })
            .collect();
        let mut red: Vec<SessionPlayer> = Vec::new();
        let live_blue: Vec<SessionPlayer> = (0..5)
            .map(|i| {
                session_player(
                    &format!("puuid-b{i}"),
                    100 + i as i64,
                    266 + i,
                    &format!("友方{i}"),
                )
            })
            .collect();
        let live_red: Vec<SessionPlayer> = (0..5)
            .map(|i| {
                session_player(
                    &format!("puuid-r{i}"),
                    200 + i as i64,
                    54 + i,
                    &format!("敌方{i}"),
                )
            })
            .collect();

        let changed =
            merge_liveclient_teams(&mut blue, &mut red, live_blue, live_red, &merge_assets());
        assert!(changed, "补全敌方整队后应报告变更");
        assert_eq!(red.len(), 5, "敌方空队应从 CHAOS 侧补齐");
        assert_eq!(red[0].puuid.as_deref(), Some("puuid-r0"));
        assert_eq!(red[0].champion_id, Some(54));
        // 蓝方已完整：不得被改写，也不得追加重复玩家
        assert_eq!(blue.len(), 5);
        assert_eq!(blue[0].champion_id, Some(266));
    }

    #[test]
    fn merge_liveclient_teams_backfills_placeholder_slots() {
        let mut team = vec![
            session_player("puuid-real", 1, 266, "真人"),
            session_player("", 0, 0, ""),
            session_player("", 0, 0, ""),
        ];
        let live = vec![
            session_player("puuid-other", 9, 999, "别人"),
            session_player("puuid-x", 10, 54, "亚托克斯"),
            session_player("puuid-y", 11, 103, "敌丙"),
        ];

        let changed = merge_liveclient_teams(
            &mut team,
            &mut Vec::new(),
            live,
            Vec::new(),
            &merge_assets(),
        );
        assert!(changed);
        // 实名玩家不得被任何 LiveClient 数据改写
        assert_eq!(team[0].puuid.as_deref(), Some("puuid-real"));
        assert_eq!(team[0].champion_id, Some(266));
        assert_eq!(team[0].display_name.as_deref(), Some("真人"));
        // 匿名占位 1：按序回填身份、英雄与实名
        assert_eq!(team[1].puuid.as_deref(), Some("puuid-other"));
        assert_eq!(team[1].champion_id, Some(999));
        assert_eq!(team[1].display_name.as_deref(), Some("别人"));
        // 匿名占位 2：LiveClient 名为脱敏英雄名时仅回填身份与英雄，严禁覆写名字
        assert_eq!(team[2].puuid.as_deref(), Some("puuid-x"));
        assert_eq!(team[2].champion_id, Some(54));
        assert_eq!(team[2].display_name, None);
        // 剩余 LiveClient 玩家追加进人数不足的队伍
        assert_eq!(team.len(), 4);
        assert_eq!(team[3].puuid.as_deref(), Some("puuid-y"));
    }

    #[test]
    fn merge_liveclient_teams_matches_by_real_name_only() {
        let mut team = vec![
            session_player("puuid-a", 1, 0, "真人甲"),
            session_player("", 0, 0, "玩家 3"),
        ];
        let live = vec![
            session_player("puuid-b", 2, 103, "真人甲"),
            session_player("puuid-c", 3, 54, "亚托克斯"),
        ];

        let changed = merge_liveclient_teams(
            &mut team,
            &mut Vec::new(),
            live,
            Vec::new(),
            &merge_assets(),
        );
        assert!(changed);
        // 真名匹配：同名实名玩家获得英雄与身份回填（原有 puuid 保留）
        assert_eq!(team[0].puuid.as_deref(), Some("puuid-a"));
        assert_eq!(team[0].champion_id, Some(103));
        // "玩家 3" 为占位名：亚托克斯是英雄名不可作实名匹配，但匿名槽允许按序回填
        assert_eq!(team[1].puuid.as_deref(), Some("puuid-c"));
        assert_eq!(team[1].champion_id, Some(54));
    }

    #[test]
    fn liveclient_snapshot_maps_side_and_teams() {
        let raw = serde_json::json!([
            {
                "championId": 266,
                "riotIdGameName": "悠米本米",
                "riotId": "悠米本米#GPUI",
                "summonerName": "悠米本米",
                "riotIdTagLine": "GPUI",
                "puuid": "puuid-self",
                "summonerId": 1001,
                "team": "ORDER"
            },
            {
                "championId": 0,
                "championName": "亚托克斯",
                "riotIdGameName": "敌甲",
                "puuid": "puuid-e1",
                "summonerId": 2001,
                "team": "CHAOS"
            },
            { "championId": 103, "riotIdGameName": "歧义名", "puuid": "puuid-x", "team": "ORDER" },
            { "championId": 64, "riotIdGameName": "歧义名", "puuid": "puuid-y", "team": "CHAOS" },
            { "riotIdGameName": "无阵营", "puuid": "puuid-z" }
        ]);
        let players = raw
            .as_array()
            .expect("应为数组")
            .iter()
            .filter_map(|entry| serde_json::from_value::<LiveClientPlayer>(entry.clone()).ok())
            .collect::<Vec<_>>();
        assert_eq!(players.len(), 5, "全部条目应可宽容反序列化");

        let assets = merge_assets();
        let snapshot = LiveClientSnapshot {
            players,
            active_name: Some("悠米本米".to_string()),
        };

        assert_eq!(liveclient_map_side(&snapshot), Some(MapSide::Blue));

        let (blue, red) = liveclient_teams(&snapshot, &assets);
        assert_eq!(blue.len(), 2, "ORDER 玩家进入蓝方，无阵营条目丢弃");
        assert_eq!(red.len(), 2, "CHAOS 玩家进入红方");
        assert_eq!(blue[0].puuid.as_deref(), Some("puuid-self"));
        assert_eq!(blue[0].tag_line.as_deref(), Some("GPUI"));
        assert_eq!(
            red[0].champion_id,
            Some(54),
            "championName 亚托克斯应反查字典补 ID"
        );
        assert_eq!(red[1].champion_id, Some(64));

        // 本机名缺失或跨阵营歧义时不得推断阵营
        let mut no_active = snapshot.clone();
        no_active.active_name = None;
        assert_eq!(liveclient_map_side(&no_active), None);
        let mut ambiguous = snapshot.clone();
        ambiguous.active_name = Some("歧义名".to_string());
        assert_eq!(liveclient_map_side(&ambiguous), None);
    }

    #[test]
    fn is_real_summoner_name_rejects_champion_and_placeholder_names() {
        let assets = merge_assets();
        assert!(is_real_summoner_name("悠米本米", &assets));
        assert!(is_real_summoner_name("悠米本米#GPUI", &assets));
        assert!(
            !is_real_summoner_name("亚托克斯", &assets),
            "英雄名严禁参与实名匹配"
        );
        assert!(
            !is_real_summoner_name("玩家 3", &assets),
            "占位名严禁参与实名匹配"
        );
        assert!(!is_real_summoner_name("未知", &assets));
        assert!(!is_real_summoner_name("", &assets));
    }

    #[test]
    fn preferred_summoner_name_priority() {
        assert_eq!(
            preferred_summoner_name(Some("显示名"), Some("游戏名"), "兜底"),
            "显示名"
        );
        assert_eq!(
            preferred_summoner_name(Some("  "), Some("游戏名"), "兜底"),
            "游戏名"
        );
        assert_eq!(preferred_summoner_name(None, None, " 兜底 "), "兜底");
    }
}
