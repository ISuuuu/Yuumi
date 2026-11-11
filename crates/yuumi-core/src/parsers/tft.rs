//! 云顶之弈（TFT）数据解析：段位、近期战绩、资源字典、海克斯强化与热门阵容。
//!
//! 数据来源与 Vue 原版一致：
//! - 段位（`/lol-ranked/v1/ranked-stats/{puuid}`）与战绩
//!   （`/lol-match-history/v1/products/tft/{puuid}/matches`）走 LCU 本地接口；
//! - 英雄 / 羁绊 / 装备 / 海克斯强化走 CDragon 镜像
//!   （`cdragon/tft/zh_cn.json`，一次下载后落盘缓存，供三个页签共用）；
//! - 热门阵容走 OP.GG MCP（`mcp-api.op.gg/mcp` 的 `tft_list_meta_decks`）。
//!
//! 解析失败一律返回带定位信息的 [`TftError`]，不静默返回空列表。

use crate::lcu::client::LcuClient;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// CDragon 中文 TFT 数据字典（英雄 / 羁绊 / 装备 / 强化）
const CDRAGON_TFT_URL: &str = "https://raw.communitydragon.org/latest/cdragon/tft/zh_cn.json";
/// OP.GG MCP 阵容查询端点
const OPGG_MCP_URL: &str = "https://mcp-api.op.gg/mcp";
/// 资源字典落盘缓存文件名
const CATALOG_CACHE_FILE: &str = "tft_catalog.json";
/// CDragon 首次下载超时（整包数 MB，比常规接口宽松）
const CDRAGON_TIMEOUT: Duration = Duration::from_secs(30);
/// 战绩与段位接口超时
const LCU_TIMEOUT_NOTE: &str = "LCU 本地接口";

/// TFT 数据层错误
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TftError {
    /// LCU 请求失败（含未连接、超时、非 2xx）
    Lcu(String),
    /// 远程接口（CDragon / OP.GG MCP）请求失败
    Remote(String),
    /// 响应结构与预期不符
    Parse(String),
}

impl fmt::Display for TftError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TftError::Lcu(message) => write!(f, "{LCU_TIMEOUT_NOTE}请求失败: {message}"),
            TftError::Remote(message) => write!(f, "远程数据请求失败: {message}"),
            TftError::Parse(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for TftError {}

// ─── 数据结构 ───

/// 云顶段位（单排 / 狂暴 / 双人作战）
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TftRankStats {
    pub solo_tier: String,
    pub solo_division: String,
    pub solo_lp: i32,
    pub solo_wins: i32,
    pub solo_losses: i32,
    pub turbo_tier: String,
    pub turbo_rating: i32,
    pub turbo_wins: i32,
    pub double_tier: String,
    pub double_division: String,
    pub double_lp: i32,
    pub double_wins: i32,
    pub double_losses: i32,
}

impl TftRankStats {
    fn unranked() -> Self {
        Self {
            solo_tier: "UNRANKED".to_string(),
            solo_division: "NA".to_string(),
            turbo_tier: "NONE".to_string(),
            double_tier: "UNRANKED".to_string(),
            double_division: "NA".to_string(),
            ..Default::default()
        }
    }
}

/// 单个出战棋子
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TftUnit {
    pub character_id: String,
    pub name: String,
    pub icon_url: String,
    pub rarity: i32,
    /// 星级 1–3
    pub tier: i32,
    pub item_names: Vec<String>,
    pub item_icon_urls: Vec<String>,
}

/// 单条激活羁绊
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TftTrait {
    pub name: String,
    pub num_units: i32,
    pub tier_current: i32,
    pub icon_url: String,
}

/// 单局中单个玩家的数据
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TftParticipant {
    pub puuid: String,
    pub summoner_name: String,
    pub is_self: bool,
    pub placement: i32,
    pub level: i32,
    pub gold_left: i32,
    pub total_damage_to_players: i32,
    pub companion_icon_url: String,
    pub traits: Vec<TftTrait>,
    pub units: Vec<TftUnit>,
    pub augments: Vec<String>,
}

/// 单局战绩
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TftMatch {
    pub game_id: u64,
    pub queue_id: i32,
    pub queue_name: String,
    pub game_creation: u64,
    pub game_duration: u64,
    pub time_str: String,
    pub duration_str: String,
    pub placement: i32,
    pub level: i32,
    pub gold_left: i32,
    pub total_damage_to_players: i32,
    pub companion_icon_url: String,
    pub traits: Vec<TftTrait>,
    pub units: Vec<TftUnit>,
    pub augments: Vec<String>,
    /// 八人对局结算榜（按名次升序）
    pub participants: Vec<TftParticipant>,
}

/// 战绩汇总
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TftMatchSummary {
    pub total_games: usize,
    pub win_count: usize,
    pub top4_count: usize,
    pub top4_rate: f64,
    pub win_rate: f64,
    pub avg_placement: f64,
    pub matches: Vec<TftMatch>,
}

/// 海克斯强化条目
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TftAugmentInfo {
    pub api_name: String,
    pub name: String,
    pub desc: String,
    /// 归一化后的 `yuumi-asset://` 源路径（`/lol-game-data/assets/...`）
    pub icon_path: String,
    /// 1 银色 / 2 金色 / 3 棱彩
    pub tier: i32,
}

/// CDragon 资源字典（英雄 / 羁绊 / 装备 / 强化），键同时登记原始与全小写形式
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TftCatalog {
    pub champions: HashMap<String, String>,
    pub champion_icons: HashMap<String, String>,
    pub traits: HashMap<String, String>,
    pub trait_icons: HashMap<String, String>,
    pub item_names: HashMap<String, String>,
    pub item_icons: HashMap<String, String>,
    pub augments: Vec<TftAugmentInfo>,
}

fn lookup<'a>(map: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    if key.is_empty() {
        return None;
    }
    map.get(key)
        .or_else(|| map.get(&key.to_lowercase()))
        .map(String::as_str)
}

/// 装备键兼容：CDragon 里键可能是 `TFT_Item_X`，接口里也可能只给 `X`
fn lookup_item<'a>(map: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    if let Some(hit) = lookup(map, key) {
        return Some(hit);
    }
    let cleaned = key
        .strip_prefix("TFT_Item_")
        .or_else(|| key.strip_prefix("tft_item_"))
        .unwrap_or(key);
    lookup(map, cleaned).or_else(|| {
        let lowered = cleaned.to_lowercase();
        map.iter()
            .find(|(candidate, _)| candidate.to_lowercase().ends_with(&lowered))
            .map(|(_, value)| value.as_str())
    })
}

impl TftCatalog {
    pub fn champion_name(&self, character_id: &str) -> Option<&str> {
        lookup(&self.champions, character_id)
    }

    pub fn champion_icon(&self, character_id: &str) -> Option<&str> {
        lookup(&self.champion_icons, character_id)
    }

    pub fn trait_display_name(&self, trait_id: &str) -> Option<&str> {
        lookup(&self.traits, trait_id)
            .or_else(|| lookup(&self.traits, normalize_trait_key(trait_id)))
    }

    pub fn trait_icon(&self, trait_id: &str) -> Option<&str> {
        lookup(&self.trait_icons, trait_id)
            .or_else(|| lookup(&self.trait_icons, normalize_trait_key(trait_id)))
    }

    pub fn item_display_name(&self, item_key: &str) -> Option<&str> {
        lookup_item(&self.item_names, item_key)
    }

    pub fn item_icon(&self, item_key: &str) -> Option<&str> {
        lookup_item(&self.item_icons, item_key)
    }

    pub fn is_empty(&self) -> bool {
        self.champions.is_empty() && self.traits.is_empty() && self.augments.is_empty()
    }
}

/// 羁绊键归一化：`TFT17_DarkStar_Trait` → `DarkStar`
pub fn normalize_trait_key(key: &str) -> &str {
    let mut last = key.rsplit('_').next().unwrap_or(key);
    if last == "Trait" {
        let mut parts = key.rsplit('_');
        let _ = parts.next();
        last = parts.next().unwrap_or(key);
    }
    if last.len() > 5 && last.ends_with("Trait") {
        last = &last[..last.len() - 5];
    }
    last
}

/// 资源路径归一化：`.tex` → `.png`、补 `/lol-game-data/assets` 前缀、统一小写
pub fn normalize_asset_path(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut path = trimmed.to_lowercase().replace(".tex", ".png");
    if path.starts_with("/lol-game-data/") {
        // 已是完整路径
    } else if let Some(rest) = path.strip_prefix('/') {
        path = format!("/lol-game-data/assets/{rest}");
    } else {
        path = format!("/lol-game-data/assets/{path}");
    }
    path.replace("//", "/")
}

// ─── 段位解析 ───

/// 解析 `/lol-ranked/v1/ranked-stats/{puuid}` 响应
pub fn parse_ranked_stats(root: &Value) -> TftRankStats {
    let mut stats = TftRankStats::unranked();
    let Some(queues) = root.get("queues").and_then(Value::as_array) else {
        return stats;
    };

    for queue in queues {
        let queue_type = queue
            .get("queueType")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let division = queue
            .get("division")
            .or_else(|| queue.get("rank"))
            .and_then(Value::as_str)
            .unwrap_or("NA")
            .to_string();
        let lp = queue
            .get("leaguePoints")
            .and_then(Value::as_i64)
            .unwrap_or(0) as i32;
        let wins = queue.get("wins").and_then(Value::as_i64).unwrap_or(0) as i32;
        let losses = queue.get("losses").and_then(Value::as_i64).unwrap_or(0) as i32;

        match queue_type {
            "RANKED_TFT" => {
                stats.solo_tier = queue
                    .get("tier")
                    .and_then(Value::as_str)
                    .unwrap_or("UNRANKED")
                    .to_string();
                stats.solo_division = division;
                stats.solo_lp = lp;
                stats.solo_wins = wins;
                stats.solo_losses = losses;
            }
            "RANKED_TFT_TURBO" => {
                stats.turbo_tier = queue
                    .get("ratedTier")
                    .and_then(Value::as_str)
                    .unwrap_or("NONE")
                    .to_string();
                stats.turbo_rating = queue
                    .get("ratedRating")
                    .and_then(Value::as_i64)
                    .unwrap_or(0) as i32;
                stats.turbo_wins = wins;
            }
            "RANKED_TFT_DOUBLE_UP" => {
                stats.double_tier = queue
                    .get("tier")
                    .and_then(Value::as_str)
                    .unwrap_or("UNRANKED")
                    .to_string();
                stats.double_division = division;
                stats.double_lp = lp;
                stats.double_wins = wins;
                stats.double_losses = losses;
            }
            _ => {}
        }
    }
    stats
}

/// 查询云顶段位
pub async fn fetch_tft_ranked_stats(
    client: &LcuClient,
    puuid: &str,
) -> Result<TftRankStats, TftError> {
    let path = format!("/lol-ranked/v1/ranked-stats/{puuid}");
    let value = client
        .get(&path)
        .await
        .map_err(|err| TftError::Lcu(format!("云顶段位 GET {path} 失败: {err}")))?;
    let stats = parse_ranked_stats(&value);
    log::info!(
        "[TFT] 段位就绪 solo={} {} turbo={} double={}",
        stats.solo_tier,
        stats.solo_division,
        stats.turbo_tier,
        stats.double_tier
    );
    Ok(stats)
}

// ─── 战绩解析 ───

fn queue_name_of(queue_id: i32) -> String {
    match queue_id {
        1090 => "云顶之弈(匹配)",
        1100 => "云顶之弈(排位)",
        1130 => "狂暴模式",
        1160 => "双人作战",
        _ => "云顶模式",
    }
    .to_string()
}

/// 解包 `json` 字段（LCU 详情与 SGP 都可能把整局塞成字符串）与 `stats` 包裹
fn unwrap_game(raw: &Value) -> Value {
    match raw.get("json") {
        Some(Value::String(text)) => {
            serde_json::from_str::<Value>(text).unwrap_or_else(|_| raw.clone())
        }
        Some(Value::Object(_)) => raw.get("json").cloned().unwrap_or_else(|| raw.clone()),
        _ => raw.clone(),
    }
}

fn str_field<'a>(value: &'a Value, keys: &[&str]) -> &'a str {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .unwrap_or_default()
}

fn int_field(value: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_i64))
}

fn parse_units(raw: Option<&Value>, catalog: &TftCatalog) -> Vec<TftUnit> {
    let Some(list) = raw.and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut units = Vec::with_capacity(list.len());
    for unit in list {
        let character_id = str_field(unit, &["character_id", "characterId", "name"]).to_string();
        let name = catalog
            .champion_name(&character_id)
            .map(str::to_string)
            .or_else(|| {
                unit.get("name")
                    .and_then(Value::as_str)
                    .filter(|text| !text.is_empty())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| character_id.clone());

        let mut item_names = Vec::new();
        let mut item_icon_urls = Vec::new();
        if let Some(items) = unit
            .get("itemNames")
            .or_else(|| unit.get("items"))
            .and_then(Value::as_array)
        {
            for item in items {
                let key = match item {
                    Value::String(text) if !text.is_empty() => text.clone(),
                    Value::Number(number) => number.to_string(),
                    _ => continue,
                };
                item_names.push(
                    catalog
                        .item_display_name(&key)
                        .map(str::to_string)
                        .unwrap_or_else(|| key.clone()),
                );
                if let Some(icon) = catalog.item_icon(&key) {
                    item_icon_urls.push(normalize_asset_path(icon));
                }
            }
        }

        let icon_url = catalog
            .champion_icon(&character_id)
            .map(normalize_asset_path)
            .or_else(|| {
                unit.get("icon")
                    .or_else(|| unit.get("squareIcon"))
                    .or_else(|| unit.get("iconPath"))
                    .and_then(Value::as_str)
                    .map(normalize_asset_path)
            })
            .unwrap_or_default();

        units.push(TftUnit {
            character_id,
            name,
            icon_url,
            rarity: int_field(unit, &["rarity"]).unwrap_or(0) as i32,
            tier: int_field(unit, &["tier"]).unwrap_or(1) as i32,
            item_names,
            item_icon_urls,
        });
    }
    units
}

fn parse_traits(raw: Option<&Value>, catalog: &TftCatalog) -> Vec<TftTrait> {
    let Some(list) = raw.and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut traits = Vec::new();
    for trait_value in list {
        let trait_id = str_field(trait_value, &["name", "trait_name"]).to_string();
        let num_units = int_field(trait_value, &["num_units", "numUnits"]).unwrap_or(0) as i32;
        let tier_current =
            int_field(trait_value, &["tier_current", "tierCurrent", "style"]).unwrap_or(0) as i32;
        if tier_current <= 0 && num_units <= 0 {
            continue;
        }
        let name = catalog
            .trait_display_name(&trait_id)
            .map(str::to_string)
            .unwrap_or_else(|| normalize_trait_key(&trait_id).to_string());
        let icon_url = catalog
            .trait_icon(&trait_id)
            .map(normalize_asset_path)
            .or_else(|| {
                trait_value
                    .get("icon")
                    .or_else(|| trait_value.get("iconPath"))
                    .and_then(Value::as_str)
                    .map(normalize_asset_path)
            })
            .unwrap_or_default();
        traits.push(TftTrait {
            name,
            num_units,
            tier_current,
            icon_url,
        });
    }
    traits
}

fn parse_augment_keys(raw: Option<&Value>) -> Vec<String> {
    raw.and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|item| match item {
                    Value::String(text) if !text.is_empty() => Some(text.clone()),
                    Value::Number(number) => Some(number.to_string()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// 解析单局中的单个玩家
fn parse_participant(
    raw: &Value,
    identity: Option<(&str, &str)>,
    current_puuid: &str,
    catalog: &TftCatalog,
) -> TftParticipant {
    let stats = raw.get("stats").filter(|value| value.is_object());
    let scoped = stats.unwrap_or(raw);

    let mut puuid = identity
        .map(|(puuid, _)| puuid.to_string())
        .unwrap_or_default();
    if puuid.is_empty() {
        let direct = str_field(raw, &["puuid"]);
        puuid = if direct.is_empty() {
            str_field(scoped, &["puuid"]).to_string()
        } else {
            direct.to_string()
        };
    }

    let mut summoner_name = identity
        .map(|(_, name)| name.to_string())
        .unwrap_or_default();
    if summoner_name.is_empty() {
        let game_name = str_field(raw, &["riotIdGameName"]);
        let tag_line = str_field(raw, &["riotIdTagline"]);
        if !game_name.is_empty() {
            summoner_name = if tag_line.is_empty() {
                game_name.to_string()
            } else {
                format!("{game_name}#{tag_line}")
            };
        }
    }
    if summoner_name.is_empty() {
        summoner_name = str_field(scoped, &["summonerName", "gameName"]).to_string();
    }
    if summoner_name.is_empty() {
        summoner_name = str_field(raw, &["summonerName", "gameName"]).to_string();
    }
    if summoner_name.is_empty() {
        let pid = int_field(raw, &["participantId"]).unwrap_or(0);
        summoner_name = if pid > 0 {
            format!("玩家 {pid}")
        } else {
            "召唤师".to_string()
        };
    }

    let companion_icon_url = raw
        .get("companion")
        .or_else(|| scoped.get("companion"))
        .and_then(|companion| {
            int_field(companion, &["skin_ID", "item_ID", "skinId"])
                .map(|id| format!("/lol-game-data/assets/v1/profile-icons/{id}.jpg"))
        })
        .unwrap_or_default();

    TftParticipant {
        puuid: puuid.clone(),
        summoner_name,
        is_self: !current_puuid.is_empty() && puuid == current_puuid,
        placement: int_field(scoped, &["placement", "rank"])
            .or_else(|| int_field(raw, &["placement", "rank"]))
            .unwrap_or(8) as i32,
        level: int_field(scoped, &["level", "champLevel"]).unwrap_or(1) as i32,
        gold_left: int_field(scoped, &["goldLeft", "gold_left"]).unwrap_or(0) as i32,
        total_damage_to_players: int_field(
            scoped,
            &["totalDamageToPlayers", "total_damage_to_players"],
        )
        .unwrap_or(0) as i32,
        companion_icon_url,
        traits: parse_traits(scoped.get("traits").or_else(|| raw.get("traits")), catalog),
        units: parse_units(scoped.get("units").or_else(|| raw.get("units")), catalog),
        augments: parse_augment_keys(scoped.get("augments").or_else(|| raw.get("augments"))),
    }
}

fn format_time(game_creation_ms: u64) -> String {
    let secs = (game_creation_ms / 1000) as i64;
    let offset = chrono::FixedOffset::east_opt(8 * 3600);
    offset
        .and_then(|offset| {
            chrono::DateTime::from_timestamp(secs, 0)
                .map(|utc| utc.with_timezone(&offset).format("%m-%d %H:%M").to_string())
        })
        .unwrap_or_else(|| "01-01 00:00".to_string())
}

fn format_duration(seconds: u64) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

/// 解析 TFT 战绩响应（`games.games` 或直接 `games` 数组均可）
pub fn parse_match_summary(
    root: &Value,
    current_puuid: &str,
    catalog: &TftCatalog,
) -> Result<TftMatchSummary, TftError> {
    let games = root
        .get("games")
        .and_then(|games| games.get("games").or(Some(games)))
        .and_then(Value::as_array)
        .ok_or_else(|| TftError::Parse("TFT 战绩响应缺少 games 数组".to_string()))?;

    let mut matches = Vec::with_capacity(games.len());
    let mut win_count = 0usize;
    let mut top4_count = 0usize;
    let mut placement_sum = 0i64;

    for raw_game in games {
        let game = unwrap_game(raw_game);
        let game_id = int_field(&game, &["gameId"]).unwrap_or(0) as u64;
        let queue_id = int_field(&game, &["queueId"]).unwrap_or(0) as i32;
        let game_creation = int_field(&game, &["gameCreation"]).unwrap_or(0) as u64;
        let game_duration =
            int_field(&game, &["game_length", "gameDuration", "gameLength"]).unwrap_or(0) as u64;

        // participantId → (puuid, 名字)，兼容 SR 风格的 participantIdentities
        let mut identities: HashMap<i64, (String, String)> = HashMap::new();
        if let Some(list) = game.get("participantIdentities").and_then(Value::as_array) {
            for item in list {
                let pid = int_field(item, &["participantId"]).unwrap_or(0);
                let Some(player) = item.get("player") else {
                    continue;
                };
                let puuid = str_field(player, &["puuid", "currentPuuid"]).to_string();
                let game_name = str_field(player, &["gameName", "game_name"]);
                let tag_line = str_field(player, &["tagLine", "tag_line"]);
                let name = if !game_name.is_empty() {
                    if tag_line.is_empty() {
                        game_name.to_string()
                    } else {
                        format!("{game_name}#{tag_line}")
                    }
                } else {
                    str_field(player, &["summonerName", "displayName", "name"]).to_string()
                };
                if pid > 0 {
                    identities.insert(pid, (puuid, name));
                }
            }
        }

        let mut participants: Vec<TftParticipant> = game
            .get("participants")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .map(|raw| {
                        let pid = int_field(raw, &["participantId"]).unwrap_or(0);
                        let identity = identities
                            .get(&pid)
                            .map(|(puuid, name)| (puuid.as_str(), name.as_str()));
                        parse_participant(raw, identity, current_puuid, catalog)
                    })
                    .collect()
            })
            .unwrap_or_default();
        participants.sort_by_key(|participant| participant.placement);

        let mine = participants
            .iter()
            .find(|participant| participant.is_self)
            .cloned()
            .or_else(|| participants.first().cloned())
            .unwrap_or_default();

        if mine.placement == 1 {
            win_count += 1;
        }
        if mine.placement <= 4 {
            top4_count += 1;
        }
        placement_sum += mine.placement as i64;

        matches.push(TftMatch {
            game_id,
            queue_id,
            queue_name: queue_name_of(queue_id),
            game_creation,
            game_duration,
            time_str: format_time(game_creation),
            duration_str: format_duration(game_duration),
            placement: mine.placement,
            level: mine.level,
            gold_left: mine.gold_left,
            total_damage_to_players: mine.total_damage_to_players,
            companion_icon_url: mine.companion_icon_url,
            traits: mine.traits,
            units: mine.units,
            augments: mine.augments,
            participants,
        });
    }

    let total_games = matches.len();
    let (top4_rate, win_rate, avg_placement) = if total_games > 0 {
        (
            top4_count as f64 / total_games as f64 * 100.0,
            win_count as f64 / total_games as f64 * 100.0,
            placement_sum as f64 / total_games as f64,
        )
    } else {
        (0.0, 0.0, 0.0)
    };

    Ok(TftMatchSummary {
        total_games,
        win_count,
        top4_count,
        top4_rate,
        win_rate,
        avg_placement,
        matches,
    })
}

/// 查询云顶战绩：LCU 本地接口，`begIndex` / `beginIndex` 两种拼写依次尝试
pub async fn fetch_tft_match_history(
    client: &LcuClient,
    puuid: &str,
    beg_index: u32,
    end_index: u32,
    catalog: &TftCatalog,
) -> Result<TftMatchSummary, TftError> {
    let base = format!("/lol-match-history/v1/products/tft/{puuid}/matches");
    let candidates = [
        format!("{base}?begIndex={beg_index}&endIndex={end_index}"),
        format!("{base}?beginIndex={beg_index}&endIndex={end_index}"),
        base.clone(),
    ];

    let mut last_error = String::new();
    for path in candidates {
        match client.get(&path).await {
            Ok(value) => {
                let summary = parse_match_summary(&value, puuid, catalog)?;
                log::info!(
                    "[TFT] 战绩就绪 {} 局（胜 {} / 前四 {}）",
                    summary.total_games,
                    summary.win_count,
                    summary.top4_count
                );
                return Ok(summary);
            }
            Err(err) => {
                log::warn!("[TFT] 战绩接口失败 GET {path}: {err}");
                last_error = err;
            }
        }
    }
    Err(TftError::Lcu(format!(
        "云顶战绩 GET {base} 全部尝试失败: {last_error}"
    )))
}

// ─── CDragon 资源字典与海克斯 ───

fn strip_html(raw: &str) -> String {
    let with_breaks = raw
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("</p>", "\n")
        .replace("</div>", "\n");
    let mut out = String::with_capacity(with_breaks.len());
    let mut in_tag = false;
    for ch in with_breaks.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.trim().to_string()
}

/// 解析 `@Key@` 描述占位符，取值来自 `effects`；缺失时保留原样
fn resolve_desc_tokens(desc: &str, effects: Option<&serde_json::Map<String, Value>>) -> String {
    let Some(effects) = effects else {
        return desc.to_string();
    };
    let mut out = String::with_capacity(desc.len());
    let mut rest = desc;
    while let Some(start) = rest.find('@') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('@') else {
            out.push_str(after);
            return out;
        };
        let token = &after[..end];
        match effects.get(token) {
            Some(Value::Number(number)) => out.push_str(&number.to_string()),
            Some(Value::String(text)) => out.push_str(text),
            _ => {
                out.push('@');
                out.push_str(token);
                out.push('@');
            }
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

fn augment_tier(api_name: &str, icon: &str, name: &str) -> i32 {
    let api = api_name.to_lowercase();
    let icon = icon.to_lowercase();
    let prismatic =
        |text: &str| text.contains("prismatic") || text.contains("tier3") || text.contains("hr_t3");
    let gold =
        |text: &str| text.contains("gold") || text.contains("tier2") || text.contains("hr_t2");
    let silver =
        |text: &str| text.contains("silver") || text.contains("tier1") || text.contains("hr_t1");

    if prismatic(&api) || prismatic(&icon) {
        return 3;
    }
    if gold(&api) || gold(&icon) {
        return 2;
    }
    if silver(&api) || silver(&icon) {
        return 1;
    }
    if name.contains(" III") {
        return 3;
    }
    if name.contains(" II") {
        return 2;
    }
    if name.contains(" I") {
        return 1;
    }
    2
}

fn extract_augments(root: &Value) -> Vec<TftAugmentInfo> {
    let mut result = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let Some(list) = root.get("augments").and_then(Value::as_array) else {
        return result;
    };
    for item in list {
        let name = str_field(item, &["name", "displayName", "title"]);
        if name.is_empty() {
            continue;
        }
        let api_name = {
            let raw = str_field(item, &["apiName", "api_name", "hexId"]);
            if raw.is_empty() {
                name.to_string()
            } else {
                raw.to_string()
            }
        };
        if seen.iter().any(|existing| existing == &api_name) {
            continue;
        }
        let icon = str_field(item, &["icon", "squareIcon", "imgUrl"]);
        let lower_api = api_name.to_lowercase();
        let lower_icon = icon.to_lowercase();
        let is_augment = lower_api.contains("augment")
            || lower_icon.contains("augment")
            || (lower_api.starts_with("tft")
                && (lower_api.contains("aug")
                    || lower_api.contains("superrune")
                    || lower_api.contains("hextech")));
        if !is_augment {
            continue;
        }
        seen.push(api_name.clone());

        let raw_desc = str_field(item, &["desc", "description", "descClean", "tooltip"]);
        let desc = resolve_desc_tokens(
            &strip_html(raw_desc),
            item.get("effects").and_then(Value::as_object),
        );
        result.push(TftAugmentInfo {
            tier: augment_tier(&api_name, icon, name),
            api_name,
            name: name.to_string(),
            desc,
            icon_path: normalize_asset_path(icon),
        });
    }
    result
}

fn insert_name_and_icon(
    names: &mut HashMap<String, String>,
    icons: &mut HashMap<String, String>,
    api_name: &str,
    display: &str,
    icon: &str,
) {
    let lower = api_name.to_lowercase();
    if !display.is_empty() {
        names.insert(api_name.to_string(), display.to_string());
        names.insert(lower.clone(), display.to_string());
    }
    if !icon.is_empty() {
        let normalized = normalize_asset_path(icon);
        icons.insert(api_name.to_string(), normalized.clone());
        icons.insert(lower, normalized);
    }
}

/// 解析 CDragon `cdragon/tft/zh_cn.json`
pub fn parse_catalog(root: &Value) -> TftCatalog {
    let mut catalog = TftCatalog::default();

    let mut consume_set = |set_value: &Value| {
        if let Some(champions) = set_value.get("champions").and_then(Value::as_array) {
            for champion in champions {
                let api_name = str_field(champion, &["apiName"]);
                if api_name.is_empty() {
                    continue;
                }
                insert_name_and_icon(
                    &mut catalog.champions,
                    &mut catalog.champion_icons,
                    api_name,
                    str_field(champion, &["name"]),
                    str_field(champion, &["squareIcon", "icon"]),
                );
            }
        }
        if let Some(traits) = set_value.get("traits").and_then(Value::as_array) {
            for trait_value in traits {
                let api_name = str_field(trait_value, &["apiName"]);
                if api_name.is_empty() {
                    continue;
                }
                insert_name_and_icon(
                    &mut catalog.traits,
                    &mut catalog.trait_icons,
                    api_name,
                    str_field(trait_value, &["name"]),
                    str_field(trait_value, &["icon"]),
                );
            }
        }
    };

    if let Some(sets) = root.get("setData").and_then(Value::as_array) {
        for set_value in sets {
            consume_set(set_value);
        }
    }
    if let Some(sets) = root.get("sets").and_then(Value::as_object) {
        for set_value in sets.values() {
            consume_set(set_value);
        }
    }

    for key in ["items", "augments"] {
        if let Some(items) = root.get(key).and_then(Value::as_array) {
            for item in items {
                let api_name = str_field(item, &["apiName"]);
                if api_name.is_empty() {
                    continue;
                }
                insert_name_and_icon(
                    &mut catalog.item_names,
                    &mut catalog.item_icons,
                    api_name,
                    str_field(item, &["name"]),
                    str_field(item, &["icon"]),
                );
            }
        }
    }

    catalog.augments = extract_augments(root);
    catalog
}

fn cache_path() -> PathBuf {
    crate::runtime::app_data_dir()
        .join("cache")
        .join(CATALOG_CACHE_FILE)
}

fn read_disk_catalog() -> Option<TftCatalog> {
    let path = cache_path();
    let text = std::fs::read_to_string(&path).ok()?;
    match serde_json::from_str::<TftCatalog>(&text) {
        Ok(catalog) if !catalog.is_empty() => Some(catalog),
        Ok(_) => {
            log::warn!("[TFT] 磁盘缓存为空字典，忽略: {}", path.display());
            None
        }
        Err(err) => {
            log::warn!("[TFT] 磁盘缓存解析失败 {err}: {}", path.display());
            None
        }
    }
}

/// 磁盘读取放到阻塞线程池，避免大字典（数 MB）解析卡住 UI 线程
async fn read_disk_catalog_async() -> Option<TftCatalog> {
    tokio::task::spawn_blocking(read_disk_catalog)
        .await
        .ok()
        .flatten()
}

/// 缓存写回独立线程：序列化 + 落盘都不占用调用方线程
fn write_disk_catalog_async(catalog: Arc<TftCatalog>) {
    std::thread::spawn(move || {
        let path = cache_path();
        if let Some(parent) = path.parent() {
            if let Err(err) = std::fs::create_dir_all(parent) {
                log::warn!("[TFT] 创建缓存目录失败: {err}");
                return;
            }
        }
        match serde_json::to_string(catalog.as_ref()) {
            Ok(text) => {
                if let Err(err) = std::fs::write(&path, text) {
                    log::warn!("[TFT] 写入 {} 失败: {err}", path.display());
                }
            }
            Err(err) => log::warn!("[TFT] 序列化资源字典失败: {err}"),
        }
    });
}

fn remote_client(proxy: Option<&str>) -> reqwest::Client {
    let mut builder = reqwest::Client::builder()
        .timeout(CDRAGON_TIMEOUT)
        .user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        );
    if let Some(addr) = proxy.filter(|addr| !addr.trim().is_empty()) {
        let trimmed = addr.trim();
        let url = if trimmed.contains("://") {
            trimmed.to_string()
        } else {
            format!("http://{trimmed}")
        };
        match reqwest::Proxy::all(&url) {
            Ok(configured) => {
                log::info!("[TFT] 远程请求已配置代理: {url}");
                builder = builder.proxy(configured);
            }
            Err(err) => log::warn!("[TFT] 代理地址无效 {trimmed}: {err}"),
        }
    }
    builder.build().unwrap_or_default()
}

/// 进程内缓存的资源字典
fn cached_catalog() -> &'static Mutex<Option<Arc<TftCatalog>>> {
    static CATALOG: OnceLock<Mutex<Option<Arc<TftCatalog>>>> = OnceLock::new();
    CATALOG.get_or_init(|| Mutex::new(None))
}

fn memory_catalog() -> Option<Arc<TftCatalog>> {
    let guard = cached_catalog().lock().ok()?;
    guard.as_ref().cloned()
}

/// 写入内存缓存并返回共享句柄
fn store_catalog(catalog: TftCatalog) -> Arc<TftCatalog> {
    let catalog = Arc::new(catalog);
    if let Ok(mut guard) = cached_catalog().lock() {
        *guard = Some(catalog.clone());
    }
    write_disk_catalog_async(catalog.clone());
    catalog
}

/// 从 CDragon 下载并解析资源字典
async fn download_catalog(proxy: Option<&str>) -> Result<Arc<TftCatalog>, TftError> {
    log::info!("[TFT] 下载 CDragon 资源字典: {CDRAGON_TFT_URL}");
    let response = remote_client(proxy)
        .get(CDRAGON_TFT_URL)
        .send()
        .await
        .map_err(|err| TftError::Remote(format!("CDragon 请求失败: {err}")))?;
    let status = response.status();
    if !status.is_success() {
        return Err(TftError::Remote(format!("CDragon 返回 HTTP {status}")));
    }
    let root: Value = response
        .json()
        .await
        .map_err(|err| TftError::Parse(format!("CDragon 响应解析失败: {err}")))?;
    let catalog = parse_catalog(&root);
    if catalog.is_empty() {
        return Err(TftError::Parse(
            "CDragon 响应中未找到英雄/羁绊/强化数据".to_string(),
        ));
    }
    log::info!(
        "[TFT] 资源字典就绪 英雄={} 羁绊={} 装备={} 强化={}",
        catalog.champions.len() / 2,
        catalog.traits.len() / 2,
        catalog.item_names.len() / 2,
        catalog.augments.len()
    );
    Ok(store_catalog(catalog))
}

/// 拉取（或复用）TFT 资源字典：内存 → 磁盘 → CDragon。
///
/// 强制刷新失败时回滚到已有缓存（对照旧版「网络失败不清空界面数据」）。
pub async fn load_tft_catalog(
    force_refresh: bool,
    proxy: Option<&str>,
) -> Result<Arc<TftCatalog>, TftError> {
    if !force_refresh {
        if let Some(catalog) = memory_catalog() {
            return Ok(catalog);
        }
        if let Some(catalog) = read_disk_catalog_async().await {
            log::info!("[TFT] 资源字典命中磁盘缓存");
            return Ok(store_catalog(catalog));
        }
    }

    match download_catalog(proxy).await {
        Ok(catalog) => Ok(catalog),
        Err(err) => {
            if let Some(catalog) = memory_catalog() {
                log::warn!("[TFT] 资源字典刷新失败，回滚内存缓存: {err}");
                return Ok(catalog);
            }
            if let Some(catalog) = read_disk_catalog_async().await {
                log::warn!("[TFT] 资源字典刷新失败，回滚磁盘缓存: {err}");
                return Ok(store_catalog(catalog));
            }
            Err(err)
        }
    }
}

// ─── OP.GG 热门阵容 ───

fn stat_number(stat: &Value, key: &str) -> Option<f64> {
    stat.get(key).and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_str()?.parse::<f64>().ok())
    })
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MetaDeckUnit {
    pub character_id: String,
    pub name: String,
    pub icon_url: String,
    pub cell: Option<(i32, i32)>,
    pub items: Vec<String>,
    pub item_icons: Vec<String>,
    pub tier: i32,
    pub is_core: bool,
    pub priority: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MetaDeckTrait {
    pub name: String,
    pub num_units: i32,
    pub style: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MetaDeckPhase {
    pub level: String,
    pub units: Vec<String>,
}

/// 一套热门阵容
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaDeck {
    pub id: String,
    pub name: String,
    pub cost: i32,
    pub team_code: String,
    /// S / A / B / C / D（按胜率+选取率排序后分组）
    pub tier: String,
    pub badges: Vec<String>,
    pub win_rate: Option<f64>,
    pub pick_rate: Option<f64>,
    pub top4_rate: Option<f64>,
    pub avg_placement: Option<f64>,
    pub traits: Vec<MetaDeckTrait>,
    pub units: Vec<MetaDeckUnit>,
    pub early: Option<MetaDeckPhase>,
    pub middle: Option<MetaDeckPhase>,
}

/// 阵容查询结果（含数据版本信息）
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TftMetaDecks {
    pub decks: Vec<MetaDeck>,
    pub game_stat_counts: Option<i64>,
    pub game_stat_time: Option<String>,
}

fn deck_display_name(name: &Value) -> String {
    if let Some(text) = name.as_str() {
        return text.to_string();
    }
    name.get("zh_CN")
        .or_else(|| name.get("en_US"))
        .and_then(Value::as_str)
        .or_else(|| {
            name.as_object()
                .and_then(|map| map.values().find_map(|value| value.as_str()))
        })
        .unwrap_or_default()
        .to_string()
}

fn deck_badges(raw: &Value) -> Vec<String> {
    let Some(list) = raw.as_array() else {
        return Vec::new();
    };
    let mut badges = Vec::new();
    for badge in list {
        let key = str_field(badge, &["key"]);
        let value = badge.get("value");
        match key {
            "reroll" => {
                if let Some(level) = value.and_then(Value::as_i64) {
                    badges.push(format!("{level}级D牌"));
                }
            }
            "difficulty" => match value.and_then(Value::as_i64) {
                Some(1) => badges.push("难度: 简单".to_string()),
                Some(2) => badges.push("难度: 普通".to_string()),
                Some(3) => badges.push("难度: 困难".to_string()),
                _ => {}
            },
            "honey" => {
                if value.and_then(Value::as_bool).unwrap_or(false) {
                    badges.push("黑马上分".to_string());
                }
            }
            _ => {}
        }
    }
    badges
}

fn parse_deck_units(raw: &Value, catalog: &TftCatalog) -> Vec<MetaDeckUnit> {
    let Some(list) = raw.as_array() else {
        return Vec::new();
    };
    list.iter()
        .map(|unit| {
            let character_id = {
                let raw_id = str_field(unit, &["characterId", "character_id", "key"]);
                raw_id.to_string()
            };
            let cell = unit.get("cell").and_then(|cell| {
                let x = cell.get("x").and_then(Value::as_i64)? as i32;
                let y = cell.get("y").and_then(Value::as_i64)? as i32;
                Some((x, y))
            });
            let mut items = Vec::new();
            let mut item_icons = Vec::new();
            if let Some(list) = unit.get("items").and_then(Value::as_array) {
                for item in list {
                    let Some(key) = item.as_str().filter(|key| !key.is_empty()) else {
                        continue;
                    };
                    item_icons.push(
                        catalog
                            .item_icon(key)
                            .map(normalize_asset_path)
                            .unwrap_or_default(),
                    );
                    items.push(
                        catalog
                            .item_display_name(key)
                            .map(str::to_string)
                            .unwrap_or_else(|| key.to_string()),
                    );
                }
            }
            MetaDeckUnit {
                name: catalog
                    .champion_name(&character_id)
                    .map(str::to_string)
                    .unwrap_or_else(|| {
                        character_id
                            .rsplit('_')
                            .next()
                            .unwrap_or(&character_id)
                            .to_string()
                    }),
                icon_url: catalog
                    .champion_icon(&character_id)
                    .map(normalize_asset_path)
                    .unwrap_or_default(),
                character_id,
                cell,
                items,
                item_icons,
                tier: int_field(unit, &["tier"]).unwrap_or(0) as i32,
                is_core: unit.get("isCore").and_then(Value::as_bool).unwrap_or(false),
                priority: int_field(unit, &["priority"]).unwrap_or(0) as i32,
            }
        })
        .collect()
}

fn parse_deck_phase(raw: Option<&Value>, catalog: &TftCatalog) -> Option<MetaDeckPhase> {
    let phase = raw?;
    let level = match phase.get("level") {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        _ => String::new(),
    };
    let units = phase
        .get("units")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .map(|unit| {
                    let character_id = str_field(unit, &["characterId", "character_id", "key"]);
                    catalog
                        .champion_name(character_id)
                        .map(str::to_string)
                        .unwrap_or_else(|| {
                            character_id
                                .rsplit('_')
                                .next()
                                .unwrap_or(character_id)
                                .to_string()
                        })
                })
                .collect()
        })
        .unwrap_or_default();
    Some(MetaDeckPhase { level, units })
}

fn deck_score(deck: &MetaDeck) -> f64 {
    deck.win_rate.unwrap_or(0.0) * 100.0 + deck.pick_rate.unwrap_or(0.0) * 5.0
}

/// 解析 OP.GG MCP `tft_list_meta_decks` 的 payload（`result.content[0].text` 内的 JSON）
pub fn parse_meta_decks(text: &str, catalog: &TftCatalog) -> Result<TftMetaDecks, TftError> {
    let root: Value = serde_json::from_str(text)
        .map_err(|err| TftError::Parse(format!("OP.GG 阵容 JSON 解析失败: {err}")))?;
    let list = root
        .get("data")
        .or_else(|| root.get("decks"))
        .and_then(Value::as_array)
        .ok_or_else(|| TftError::Parse("OP.GG 阵容响应缺少 data 数组".to_string()))?;

    let mut decks: Vec<MetaDeck> = list
        .iter()
        .map(|deck| {
            let stat = deck.get("stat").cloned().unwrap_or(Value::Null);
            let traits = deck
                .get("traits")
                .and_then(Value::as_array)
                .map(|list| {
                    list.iter()
                        .map(|trait_value| {
                            let key = str_field(trait_value, &["key"]);
                            MetaDeckTrait {
                                name: catalog
                                    .trait_display_name(key)
                                    .map(str::to_string)
                                    .unwrap_or_else(|| normalize_trait_key(key).to_string()),
                                num_units: int_field(trait_value, &["numUnits", "num_units"])
                                    .unwrap_or(0) as i32,
                                style: int_field(trait_value, &["style"]).unwrap_or(0) as i32,
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();

            MetaDeck {
                id: str_field(deck, &["id"]).to_string(),
                name: deck_display_name(deck.get("name").unwrap_or(&Value::Null)),
                cost: int_field(deck, &["cost"]).unwrap_or(0) as i32,
                team_code: str_field(deck, &["teamCode", "team_code"]).to_string(),
                tier: String::new(),
                badges: deck_badges(deck.get("badge").unwrap_or(&Value::Null)),
                win_rate: stat_number(&stat, "win_rate"),
                pick_rate: stat_number(&stat, "pick_rate"),
                top4_rate: stat_number(&stat, "top4_rate"),
                avg_placement: stat_number(&stat, "avg_placement"),
                traits,
                units: parse_deck_units(deck.get("units").unwrap_or(&Value::Null), catalog),
                early: parse_deck_phase(deck.get("early"), catalog),
                middle: parse_deck_phase(deck.get("middle"), catalog),
            }
        })
        .collect();

    // 与 Vue 一致：胜率 + 选取率加权排序后按分位标记 S/A/B/C/D
    decks.sort_by(|left, right| {
        deck_score(right)
            .partial_cmp(&deck_score(left))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let total = decks.len();
    for (index, deck) in decks.iter_mut().enumerate() {
        let ratio = if total == 0 {
            0.0
        } else {
            index as f64 / total as f64
        };
        deck.tier = if ratio < 0.2 {
            "S"
        } else if ratio < 0.4 {
            "A"
        } else if ratio < 0.6 {
            "B"
        } else if ratio < 0.8 {
            "C"
        } else {
            "D"
        }
        .to_string();
    }

    let metadata = root.get("metadata").cloned().unwrap_or(Value::Null);
    Ok(TftMetaDecks {
        decks,
        game_stat_counts: metadata.get("gameStatCounts").and_then(Value::as_i64),
        game_stat_time: metadata
            .get("gameStatDateTime")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

/// 查询 OP.GG 热门阵容（JSON-RPC `tft_list_meta_decks`）
pub async fn fetch_tft_meta_decks(
    proxy: Option<&str>,
    catalog: &TftCatalog,
) -> Result<TftMetaDecks, TftError> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "tft_list_meta_decks",
            "arguments": {
                "desired_output_fields": [
                    "id", "name", "cost", "teamCode", "badge", "stat",
                    "traits", "units", "early", "middle"
                ]
            }
        }
    });

    let response = remote_client(proxy)
        .post(OPGG_MCP_URL)
        .json(&body)
        .send()
        .await
        .map_err(|err| TftError::Remote(format!("OP.GG 阵容请求失败: {err}")))?;
    let status = response.status();
    if !status.is_success() {
        return Err(TftError::Remote(format!("OP.GG 阵容返回 HTTP {status}")));
    }
    let root: Value = response
        .json()
        .await
        .map_err(|err| TftError::Parse(format!("OP.GG 阵容响应解析失败: {err}")))?;
    if let Some(error) = root.get("error") {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("未知 MCP 错误");
        return Err(TftError::Remote(format!("OP.GG MCP 返回错误: {message}")));
    }
    let text = root
        .get("result")
        .and_then(|result| result.get("content"))
        .and_then(Value::as_array)
        .and_then(|content| content.first())
        .and_then(|first| first.get("text"))
        .and_then(Value::as_str)
        .ok_or_else(|| TftError::Parse("OP.GG 阵容响应缺少 result.content[0].text".to_string()))?;

    let decks = parse_meta_decks(text, catalog)?;
    log::info!("[TFT] 阵容就绪 {} 套", decks.decks.len());
    Ok(decks)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> TftCatalog {
        let raw: Value =
            serde_json::from_str(include_str!("../../testdata/tft/synthetic/catalog.json"))
                .expect("资源字典样本应可解析");
        parse_catalog(&raw)
    }

    #[test]
    fn parse_catalog_maps_names_icons_and_augments() {
        let catalog = catalog();
        assert_eq!(catalog.champion_name("TFT17_Aatrox"), Some("亚托克斯"));
        assert_eq!(catalog.champion_name("tft17_aatrox"), Some("亚托克斯"));
        assert_eq!(
            catalog.champion_icon("TFT17_Jinx"),
            Some("/lol-game-data/assets/assets/characters/tft17_jinx/skins/base/images/tft17_jinx_square.png")
        );
        assert_eq!(catalog.item_display_name("InfinityEdge"), Some("无尽之刃"));
        assert_eq!(
            catalog.trait_display_name("TFT17_DarkStar_Trait"),
            Some("暗星")
        );
        assert_eq!(catalog.augments.len(), 3);
    }

    #[test]
    fn augments_strip_html_resolve_tokens_and_tag_tiers() {
        let catalog = catalog();
        let gold = catalog
            .augments
            .iter()
            .find(|augment| augment.api_name == "TFT17_Augment_Two")
            .expect("应包含金色强化");
        assert_eq!(gold.tier, 2);
        assert_eq!(gold.desc, "获得 12 金币。");

        let prismatic = catalog
            .augments
            .iter()
            .find(|augment| augment.api_name == "TFT17_Augment_Three")
            .expect("应包含棱彩强化");
        assert_eq!(prismatic.tier, 3);
    }

    #[test]
    fn normalize_trait_key_strips_set_prefix_and_suffix() {
        assert_eq!(normalize_trait_key("TFT17_DarkStar_Trait"), "DarkStar");
        assert_eq!(normalize_trait_key("RhaastUniqueTrait"), "RhaastUnique");
        assert_eq!(normalize_trait_key("TFT17_Sniper_Trait"), "Sniper");
    }

    #[test]
    fn parse_ranked_stats_reads_three_queues() {
        let raw: Value = serde_json::from_str(include_str!(
            "../../testdata/tft/synthetic/ranked-stats.json"
        ))
        .expect("段位样本应可解析");
        let stats = parse_ranked_stats(&raw);
        assert_eq!(stats.solo_tier, "DIAMOND");
        assert_eq!(stats.solo_division, "II");
        assert_eq!(stats.solo_lp, 45);
        assert_eq!(stats.solo_wins, 62);
        assert_eq!(stats.turbo_tier, "ORANGE");
        assert_eq!(stats.turbo_rating, 1234);
        assert_eq!(stats.double_tier, "PLATINUM");
        assert_eq!(stats.double_losses, 5);
    }

    #[test]
    fn parse_match_summary_computes_stats_and_sorts_participants() {
        let raw: Value = serde_json::from_str(include_str!(
            "../../testdata/tft/synthetic/match-history.json"
        ))
        .expect("战绩样本应可解析");
        let summary = parse_match_summary(&raw, "self-puuid", &catalog()).expect("战绩应可解析");

        assert_eq!(summary.total_games, 2);
        assert_eq!(summary.win_count, 1);
        assert_eq!(summary.top4_count, 1);
        assert!((summary.top4_rate - 50.0).abs() < f64::EPSILON);
        assert!((summary.avg_placement - 3.5).abs() < f64::EPSILON);

        let first = &summary.matches[0];
        assert_eq!(first.game_id, 7_000_000_001);
        assert_eq!(first.queue_name, "云顶之弈(排位)");
        assert_eq!(first.placement, 1);
        assert_eq!(first.units.len(), 2);
        assert_eq!(first.units[0].name, "亚托克斯");
        // 第二件装备用空值占位，解析时必须跳过而不是产生空名字
        assert_eq!(first.units[0].item_names.len(), 2);
        assert_eq!(first.traits.len(), 2, "0 层的羁绊应被过滤");
        assert_eq!(first.augments.len(), 3);
        assert_eq!(first.participants.len(), 2);
        assert_eq!(first.participants[0].placement, 1);
        assert_eq!(first.participants[0].summoner_name, "Yuumi#0001");
        assert!(first.participants[0].is_self);

        let second = &summary.matches[1];
        assert_eq!(second.queue_name, "云顶之弈(匹配)");
        assert_eq!(second.placement, 6);
    }

    #[test]
    fn parse_match_summary_rejects_missing_games() {
        let raw: Value = serde_json::from_str("{\"unexpected\": true}").expect("样本应可解析");
        let result = parse_match_summary(&raw, "self-puuid", &catalog());
        assert!(
            matches!(&result, Err(TftError::Parse(message)) if message.contains("games")),
            "缺 games 时应返回解析错误，实际 {result:?}"
        );
    }

    #[test]
    fn parse_meta_decks_sorts_and_marks_tiers() {
        let text = include_str!("../../testdata/tft/synthetic/meta-decks.json");
        let decks = parse_meta_decks(text, &catalog()).expect("阵容样本应可解析");

        assert_eq!(decks.decks.len(), 2);
        assert_eq!(decks.game_stat_counts, Some(1_284_093));
        let top = &decks.decks[0];
        assert_eq!(top.name, "暗星金克丝");
        assert_eq!(top.tier, "S");
        assert_eq!(top.badges, vec!["6级D牌", "难度: 普通", "黑马上分"]);
        assert_eq!(top.traits[0].name, "暗星");
        assert_eq!(top.units.len(), 2);
        assert_eq!(top.units[0].name, "亚托克斯");
        assert_eq!(top.units[0].cell, Some((4, 3)));
        // 第三个装备位是 null，应被跳过
        assert_eq!(top.units[0].items.len(), 2);
        assert!(top.units[0].is_core);
        assert_eq!(
            top.early.as_ref().map(|phase| phase.level.as_str()),
            Some("4")
        );
        assert_eq!(
            top.early.as_ref().map(|phase| phase.units.clone()),
            Some(vec!["金克丝".to_string()])
        );
        assert_eq!(decks.decks[1].tier, "B");
    }

    #[test]
    fn parse_meta_decks_rejects_missing_data() {
        let error = parse_meta_decks("{\"metadata\":{}}", &catalog());
        assert!(matches!(error, Err(TftError::Parse(_))));
    }

    #[test]
    fn normalize_asset_path_handles_prefixes_and_tex() {
        assert_eq!(
            normalize_asset_path("ASSETS/Items/TFT17_Item_InfinityEdge.tex"),
            "/lol-game-data/assets/assets/items/tft17_item_infinityedge.png"
        );
        assert_eq!(
            normalize_asset_path("/lol-game-data/assets/v1/x.png"),
            "/lol-game-data/assets/v1/x.png"
        );
        assert_eq!(normalize_asset_path(""), "");
    }
}
